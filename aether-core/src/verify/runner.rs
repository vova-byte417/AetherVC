//! 验证运行器
//!
//! 执行编译检查、Lint、单元测试等验证步骤。

use super::{
    CheckStatus, VerificationCheck, VerificationHistoryEntry, VerificationReport,
    VerificationStatus, VerifyMode,
};
use crate::config::types::VerifyConfig;
use crate::utils::Result;
use std::process::Command;
use std::time::Instant;
use tracing::{info, warn};

/// 验证运行器
pub struct VerificationRunner {
    config: VerifyConfig,
    /// 验证历史
    history: Vec<VerificationHistoryEntry>,
}

impl VerificationRunner {
    pub fn new(config: VerifyConfig) -> Self {
        Self {
            config,
            history: Vec::new(),
        }
    }

    /// 运行验证（针对指定 commit）
    pub async fn run(
        &mut self,
        commit_hash: &str,
        mode: VerifyMode,
        repo_path: &str,
    ) -> VerificationReport {
        let start = Instant::now();
        let mut report = VerificationReport::new(commit_hash);

        if !self.config.enabled {
            report.overall_status = VerificationStatus::Skipped;
            report.duration_ms = start.elapsed().as_millis() as u64;
            return report;
        }

        info!(
            "[Verify] 开始验证 commit={} mode={:?}",
            &commit_hash[..commit_hash.len().min(8)],
            mode
        );

        // 确定要执行的检查
        let checks_to_run = self.determine_checks(mode);

        for check_name in &checks_to_run {
            let check_start = Instant::now();
            match check_name.as_str() {
                "compile" => {
                    let check = self.run_compile_check(repo_path);
                    report.checks.push(check);
                }
                "lint" => {
                    let check = self.run_lint_check(repo_path);
                    report.checks.push(check);
                }
                "unit_tests" => {
                    let check = self.run_unit_tests(repo_path);
                    report.checks.push(check);
                }
                "format" => {
                    let check = self.run_format_check(repo_path);
                    report.checks.push(check);
                }
                _ => {
                    report.checks.push(VerificationCheck {
                        name: check_name.clone(),
                        status: CheckStatus::Skipped,
                        output: None,
                        duration_ms: 0,
                        details: None,
                    });
                }
            }
        }

        // 判定总体状态
        report.overall_status = self.compute_overall_status(&report.checks);
        report.duration_ms = start.elapsed().as_millis() as u64;

        // 记录历史
        self.history.push(VerificationHistoryEntry {
            commit_hash: commit_hash.to_string(),
            overall_status: report.overall_status.clone(),
            pass_rate: report.pass_rate(),
            duration_ms: report.duration_ms,
            timestamp: chrono::Utc::now(),
        });

        info!(
            "[Verify] 完成! status={:?} pass_rate={:.0}% duration={}ms",
            report.overall_status,
            report.pass_rate() * 100.0,
            report.duration_ms
        );

        report
    }

    /// 根据模式确定要执行的检查
    fn determine_checks(&self, mode: VerifyMode) -> Vec<String> {
        match mode {
            VerifyMode::Quick => {
                let mut checks = Vec::new();
                if self.config.basic.compile_check {
                    checks.push("compile".into());
                }
                if self.config.basic.lint_check {
                    checks.push("lint".into());
                }
                checks
            }
            VerifyMode::Smart => {
                let mut checks: Vec<String> = self.determine_checks(VerifyMode::Quick);
                if self.config.testing.run_unit_tests {
                    checks.push("unit_tests".into());
                }
                checks
            }
            VerifyMode::Full => {
                let mut checks: Vec<String> = self.determine_checks(VerifyMode::Smart);
                if self.config.basic.format_check {
                    checks.push("format".into());
                }
                checks
            }
        }
    }

    /// 运行编译检查
    fn run_compile_check(&self, repo_path: &str) -> VerificationCheck {
        let start = Instant::now();
        // 尝试 cargo check
        let result = Command::new("cargo")
            .args(["check", "--quiet"])
            .current_dir(repo_path)
            .output();

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if output.status.success() {
                    VerificationCheck::passed(
                        "compile",
                        format!("编译通过\n{}", stdout),
                        duration_ms,
                    )
                } else {
                    VerificationCheck::failed(
                        "compile",
                        format!("编译失败"),
                        format!("{}\n{}", stdout, stderr),
                        duration_ms,
                    )
                }
            }
            Err(e) => VerificationCheck::failed(
                "compile",
                format!("无法执行 cargo check: {}", e),
                String::new(),
                duration_ms,
            ),
        }
    }

    /// 运行 Lint 检查
    fn run_lint_check(&self, repo_path: &str) -> VerificationCheck {
        let start = Instant::now();
        let result = Command::new("cargo")
            .args(["clippy", "--quiet", "--", "-D", "warnings"])
            .current_dir(repo_path)
            .output();

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if output.status.success() {
                    VerificationCheck::passed(
                        "lint",
                        format!("Lint 通过\n{}", stdout),
                        duration_ms,
                    )
                } else {
                    VerificationCheck::failed(
                        "lint",
                        format!("Lint 发现问题"),
                        format!("{}\n{}", stdout, stderr),
                        duration_ms,
                    )
                }
            }
            Err(e) => {
                // clippy 可能未安装，降级处理
                warn!("cargo clippy 不可用: {}", e);
                VerificationCheck {
                    name: "lint".into(),
                    status: CheckStatus::Skipped,
                    output: Some(format!("cargo clippy 不可用: {}", e)),
                    duration_ms,
                    details: None,
                }
            }
        }
    }

    /// 运行单元测试
    fn run_unit_tests(&self, repo_path: &str) -> VerificationCheck {
        let start = Instant::now();
        let test_command = &self.config.testing.test_command;

        // 解析命令（简单的空格分割）
        let parts: Vec<&str> = test_command.split_whitespace().collect();
        let (cmd, args) = if parts.is_empty() {
            ("cargo", vec!["test"])
        } else {
            (parts[0], parts[1..].to_vec())
        };

        let result = Command::new(cmd)
            .args(&args)
            .current_dir(repo_path)
            .output();

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if output.status.success() {
                    VerificationCheck::passed(
                        "unit_tests",
                        format!("测试全部通过\n{}", stdout),
                        duration_ms,
                    )
                } else {
                    VerificationCheck::failed(
                        "unit_tests",
                        format!("部分测试失败"),
                        format!("{}\n{}", stdout, stderr),
                        duration_ms,
                    )
                }
            }
            Err(e) => VerificationCheck::failed(
                "unit_tests",
                format!("无法执行测试命令: {}", e),
                String::new(),
                duration_ms,
            ),
        }
    }

    /// 运行格式检查
    fn run_format_check(&self, repo_path: &str) -> VerificationCheck {
        let start = Instant::now();
        let result = Command::new("cargo")
            .args(["fmt", "--check"])
            .current_dir(repo_path)
            .output();

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(output) => {
                if output.status.success() {
                    VerificationCheck::passed(
                        "format",
                        String::from("代码格式正确"),
                        duration_ms,
                    )
                } else {
                    VerificationCheck::failed(
                        "format",
                        String::from("代码格式不符合规范，请运行 cargo fmt"),
                        String::from_utf8_lossy(&output.stdout).to_string(),
                        duration_ms,
                    )
                }
            }
            Err(e) => VerificationCheck {
                name: "format".into(),
                status: CheckStatus::Skipped,
                output: Some(format!("cargo fmt 不可用: {}", e)),
                duration_ms,
                details: None,
            },
        }
    }

    /// 计算总体状态
    fn compute_overall_status(&self, checks: &[VerificationCheck]) -> VerificationStatus {
        if checks.is_empty() {
            return VerificationStatus::Skipped;
        }

        let has_failed = checks.iter().any(|c| c.status == CheckStatus::Failed);
        let all_skipped = checks.iter().all(|c| c.status == CheckStatus::Skipped);

        if has_failed {
            VerificationStatus::Failed
        } else if all_skipped {
            VerificationStatus::Skipped
        } else {
            VerificationStatus::Passed
        }
    }

    /// 获取验证历史
    pub fn history(&self, limit: usize) -> &[VerificationHistoryEntry] {
        let len = self.history.len();
        let start = if len > limit { len - limit } else { 0 };
        &self.history[start..]
    }

    /// 历史通过率
    pub fn history_pass_rate(&self) -> f32 {
        let total = self.history.len() as f32;
        if total == 0.0 {
            return 1.0;
        }
        let passed = self
            .history
            .iter()
            .filter(|h| h.overall_status == VerificationStatus::Passed)
            .count() as f32;
        passed / total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_checks_quick() {
        let config = VerifyConfig::default();
        let runner = VerificationRunner::new(config);
        let checks = runner.determine_checks(VerifyMode::Quick);
        assert!(checks.contains(&"compile".to_string()));
        assert!(checks.contains(&"lint".to_string()));
        assert!(!checks.contains(&"unit_tests".to_string()));
    }

    #[test]
    fn test_determine_checks_full() {
        let config = VerifyConfig::default();
        let runner = VerificationRunner::new(config);
        let checks = runner.determine_checks(VerifyMode::Full);
        assert!(checks.contains(&"compile".to_string()));
        assert!(checks.contains(&"lint".to_string()));
        assert!(checks.contains(&"unit_tests".to_string()));
    }

    #[test]
    fn test_compute_overall_status_all_passed() {
        let config = VerifyConfig::default();
        let runner = VerificationRunner::new(config);
        let checks = vec![
            VerificationCheck::passed("compile", "ok", 100),
            VerificationCheck::passed("lint", "ok", 200),
        ];
        assert_eq!(runner.compute_overall_status(&checks), VerificationStatus::Passed);
    }

    #[test]
    fn test_compute_overall_status_one_failed() {
        let config = VerifyConfig::default();
        let runner = VerificationRunner::new(config);
        let checks = vec![
            VerificationCheck::passed("compile", "ok", 100),
            VerificationCheck::failed("lint", "error", "output", 200),
        ];
        assert_eq!(runner.compute_overall_status(&checks), VerificationStatus::Failed);
    }

    #[test]
    fn test_report_pass_rate() {
        let mut report = VerificationReport::new("abc123");
        report.checks = vec![
            VerificationCheck::passed("compile", "ok", 100),
            VerificationCheck::failed("lint", "error", "output", 200),
        ];
        assert_eq!(report.pass_rate(), 0.5);
    }

    #[test]
    fn test_history_pass_rate_empty() {
        let config = VerifyConfig::default();
        let runner = VerificationRunner::new(config);
        assert_eq!(runner.history_pass_rate(), 1.0);
    }
}
