//! 验证集成 (Verification Hook)
//!
//! 对 AI 产生的 commit 自动运行验证流水线，生成验证报告。

pub mod runner;

pub use runner::VerificationRunner;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 验证报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    /// 报告 ID
    pub id: String,
    /// commit hash
    pub commit_hash: String,
    /// 验证检查列表
    pub checks: Vec<VerificationCheck>,
    /// 总体状态
    pub overall_status: VerificationStatus,
    /// 总耗时（毫秒）
    pub duration_ms: u64,
    /// 生成时间
    pub generated_at: DateTime<Utc>,
}

impl VerificationReport {
    pub fn new(commit_hash: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            commit_hash: commit_hash.into(),
            checks: Vec::new(),
            overall_status: VerificationStatus::Pending,
            duration_ms: 0,
            generated_at: Utc::now(),
        }
    }

    /// 通过率
    pub fn pass_rate(&self) -> f32 {
        let total = self.checks.len() as f32;
        if total == 0.0 {
            return 1.0;
        }
        let passed = self
            .checks
            .iter()
            .filter(|c| c.status == CheckStatus::Passed)
            .count() as f32;
        passed / total
    }
}

/// 验证检查项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationCheck {
    /// 检查名称：compile / lint / unit_tests / static_analysis / security_scan
    pub name: String,
    /// 检查状态
    pub status: CheckStatus,
    /// 命令输出
    pub output: Option<String>,
    /// 耗时（毫秒）
    pub duration_ms: u64,
    /// 失败详情
    pub details: Option<String>,
}

impl VerificationCheck {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Pending,
            output: None,
            duration_ms: 0,
            details: None,
        }
    }

    pub fn passed(name: impl Into<String>, output: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Passed,
            output: Some(output.into()),
            duration_ms,
            details: None,
        }
    }

    pub fn failed(
        name: impl Into<String>,
        details: impl Into<String>,
        output: impl Into<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Failed,
            output: Some(output.into()),
            duration_ms,
            details: Some(details.into()),
        }
    }
}

/// 验证状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VerificationStatus {
    Pending,
    Passed,
    Failed,
    Skipped,
    Error,
}

impl VerificationStatus {
    pub fn as_str(&self) -> &str {
        match self {
            VerificationStatus::Pending => "pending",
            VerificationStatus::Passed => "passed",
            VerificationStatus::Failed => "failed",
            VerificationStatus::Skipped => "skipped",
            VerificationStatus::Error => "error",
        }
    }
}

/// 检查状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CheckStatus {
    Pending,
    Passed,
    Failed,
    Skipped,
    Error(String),
}

/// 验证模式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerifyMode {
    /// 快速模式：仅编译 + lint
    Quick,
    /// 智能模式：分析变更并只运行相关测试
    Smart,
    /// 全量模式：运行所有检查
    Full,
}

/// 验证历史条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationHistoryEntry {
    pub commit_hash: String,
    pub overall_status: VerificationStatus,
    pub pass_rate: f32,
    pub duration_ms: u64,
    pub timestamp: DateTime<Utc>,
}
