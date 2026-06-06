use crate::domain::commit::{ChangeCategory, RiskLevel, SemanticInfo};

/// 语义分析器 trait
/// 负责将代码变更（diff）转换为结构化的语义信息
#[async_trait::async_trait]
pub trait SemanticAnalyzer: Send + Sync {
    /// 分析代码变更，返回语义信息
    async fn analyze(&self, diff: &str, commit_message: &str) -> crate::utils::Result<SemanticInfo>;
}

/// 简单的规则驱动的语义分析器
/// 基于关键词和模式匹配进行初步分析
pub struct RuleBasedAnalyzer;

impl RuleBasedAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// 基于 commit message 推断变更类型
    fn infer_change_type(message: &str) -> ChangeCategory {
        let msg_lower = message.to_lowercase();
        if msg_lower.starts_with("feat") || msg_lower.contains("add") {
            ChangeCategory::Feature
        } else if msg_lower.starts_with("fix") || msg_lower.contains("bug") {
            ChangeCategory::Bugfix
        } else if msg_lower.starts_with("refactor") || msg_lower.contains("refactor") {
            ChangeCategory::Refactor
        } else if msg_lower.starts_with("perf") || msg_lower.contains("optimize") {
            ChangeCategory::Performance
        } else if msg_lower.starts_with("break") || msg_lower.contains("breaking") {
            ChangeCategory::Breaking
        } else if msg_lower.starts_with("doc") || msg_lower.contains("document") {
            ChangeCategory::Documentation
        } else if msg_lower.starts_with("test") || msg_lower.contains("test") {
            ChangeCategory::Test
        } else {
            ChangeCategory::Feature
        }
    }

    /// 推断影响范围（从 diff 中提取文件名对应的模块）
    fn infer_modules(diff: &str) -> Vec<String> {
        let mut modules = Vec::new();
        for line in diff.lines() {
            if line.starts_with("--- ") || line.starts_with("+++ ") {
                if let Some(path) = line.split_whitespace().nth(1) {
                    let clean_path = path.trim_start_matches("a/").trim_start_matches("b/");
                    if let Some(module) = clean_path.split('/').next() {
                        if module != "." && !modules.contains(&module.to_string()) {
                            modules.push(module.to_string());
                        }
                    }
                }
            }
        }
        modules
    }

    /// 推断风险等级
    fn infer_risk(message: &str, diff_analysis: &DiffAnalysis) -> RiskLevel {
        if message.contains("breaking") || message.contains("critical") {
            RiskLevel::Critical
        } else if diff_analysis.deleted_lines > 100 || diff_analysis.files_changed > 10 {
            RiskLevel::High
        } else if diff_analysis.deleted_lines > 50 || diff_analysis.files_changed > 5 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }

    /// 提取 diff 的基本统计信息
    fn analyze_diff_stats(diff: &str) -> DiffAnalysis {
        let mut files_changed = 0u32;
        let mut added_lines = 0u32;
        let mut deleted_lines = 0u32;

        for line in diff.lines() {
            if line.starts_with("--- ") {
                files_changed += 1;
            } else if line.starts_with('+') && !line.starts_with("+++") {
                added_lines += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                deleted_lines += 1;
            }
        }

        DiffAnalysis {
            files_changed,
            added_lines,
            deleted_lines,
        }
    }
}

#[derive(Debug, Clone)]
struct DiffAnalysis {
    files_changed: u32,
    added_lines: u32,
    deleted_lines: u32,
}

#[async_trait::async_trait]
impl SemanticAnalyzer for RuleBasedAnalyzer {
    async fn analyze(
        &self,
        diff: &str,
        commit_message: &str,
    ) -> crate::utils::Result<SemanticInfo> {
        let change_type = Self::infer_change_type(commit_message);
        let modules = Self::infer_modules(diff);
        let stats = Self::analyze_diff_stats(diff);
        let risk = Self::infer_risk(commit_message, &stats);

        let intent = commit_message
            .split(':')
            .nth(1)
            .unwrap_or(commit_message)
            .trim()
            .to_string();

        let summary = format!(
            "变更 {} 个文件，+{} -{} 行，影响模块: {}",
            stats.files_changed,
            stats.added_lines,
            stats.deleted_lines,
            modules.join(", ")
        );

        Ok(SemanticInfo::new(intent, change_type, modules, summary, risk))
    }
}

impl Default for RuleBasedAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::commit::ChangeCategory;

    const TEST_DIFF: &str = r#"diff --git a/auth/login.rs b/auth/login.rs
index 1234567..abcdefg 100644
--- a/auth/login.rs
+++ b/auth/login.rs
@@ -1,5 +1,10 @@
+pub fn login() -> bool {
+    // new login logic
+    true
+}
-module old_func;
---
+++ b/auth/mod.rs
+pub mod login;"#;

    #[tokio::test]
    async fn test_rule_based_analyzer() {
        let analyzer = RuleBasedAnalyzer::new();
        let info = analyzer
            .analyze(TEST_DIFF, "feat: add login function to auth module")
            .await
            .unwrap();

        assert_eq!(info.change_type, ChangeCategory::Feature);
        assert!(info.affected_modules.contains(&"auth".to_string()));
    }

    #[tokio::test]
    async fn test_infer_bugfix() {
        let analyzer = RuleBasedAnalyzer::new();
        let info = analyzer
            .analyze("--- a/src/main.rs\n+++ b/src/main.rs", "fix: resolve null pointer bug")
            .await
            .unwrap();

        assert_eq!(info.change_type, ChangeCategory::Bugfix);
    }

    #[tokio::test]
    async fn test_infer_refactor() {
        let analyzer = RuleBasedAnalyzer::new();
        let info = analyzer
            .analyze("--- a/lib.rs\n+++ b/lib.rs", "refactor: clean up module structure")
            .await
            .unwrap();

        assert_eq!(info.change_type, ChangeCategory::Refactor);
    }
}
