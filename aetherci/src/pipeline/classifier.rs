//! 变更分类器
//!
//! 基于预处理结果和 diff 内容，对变更进行细粒度分类。
//! 支持基于 commit message 启发规则和代码特征的分类。

use crate::domain::{ChangeIntent, ClassificationResult, ConfidenceScore, DiffStatistics};

/// 变更分类器
pub struct Classifier;

impl Classifier {
    pub fn new() -> Self {
        Self
    }

    /// 分类变更类型
    pub fn classify(&self, diff: &str, commit_message: &str, stats: &DiffStatistics) -> ClassificationResult {
        let msg_lower = commit_message.to_lowercase();

        // 1. 基于 commit message 前缀的规则分类
        let (primary_intent, score, rationale) = if msg_lower.starts_with("feat")
            || msg_lower.contains("add ")
            || msg_lower.contains("新增")
            || msg_lower.contains("添加")
        {
            (
                ChangeIntent::FeatureAddition,
                0.85,
                "commit message 以 feat/add 开头，表明功能新增".to_string(),
            )
        } else if msg_lower.starts_with("fix")
            || msg_lower.contains("bug")
            || msg_lower.contains("修复")
            || msg_lower.contains("修正")
        {
            (
                ChangeIntent::Bugfix,
                0.85,
                "commit message 以 fix/bug 开头，表明问题修复".to_string(),
            )
        } else if msg_lower.starts_with("refactor")
            || msg_lower.contains("重构")
            || msg_lower.contains("refactor")
        {
            (
                ChangeIntent::Refactor,
                0.80,
                "commit message 明确表示重构".to_string(),
            )
        } else if msg_lower.starts_with("perf")
            || msg_lower.contains("optimize")
            || msg_lower.contains("优化")
        {
            (
                ChangeIntent::Performance,
                0.80,
                "commit message 以 perf 开头，表明性能优化".to_string(),
            )
        } else if msg_lower.starts_with("chore(deps)")
            || msg_lower.contains("deps")
            || msg_lower.contains("upgrade")
            || msg_lower.contains("update")
            || msg_lower.contains("更新依赖")
        {
            (
                ChangeIntent::DependencyUpdate,
                0.75,
                "变更涉及依赖更新".to_string(),
            )
        } else if msg_lower.starts_with("chore(config)")
            || msg_lower.contains("config")
            || msg_lower.contains("配置")
        {
            (
                ChangeIntent::ConfigChange,
                0.75,
                "变更涉及配置修改".to_string(),
            )
        } else if msg_lower.starts_with("doc")
            || msg_lower.contains("document")
            || msg_lower.contains("文档")
        {
            (
                ChangeIntent::Documentation,
                0.90,
                "commit message 明确表示文档变更".to_string(),
            )
        } else if msg_lower.starts_with("test")
            || msg_lower.contains("test")
            || msg_lower.contains("测试")
        {
            (
                ChangeIntent::Test,
                0.85,
                "commit message 明确表示测试变更".to_string(),
            )
        } else if msg_lower.starts_with("security")
            || msg_lower.contains("secure")
            || msg_lower.contains("vuln")
            || msg_lower.contains("安全")
        {
            (
                ChangeIntent::SecurityHardening,
                0.85,
                "变更涉及安全加固".to_string(),
            )
        } else {
            // 2. 基于 diff 特征的推断
            self.infer_from_diff(diff, stats)
        };

        ClassificationResult {
            primary_intent,
            confidence: ConfidenceScore::new(score, rationale),
            secondary_intents: self.detect_secondary_intents(diff, stats),
        }
    }

    /// 从 diff 内容特征推断变更类型
    fn infer_from_diff(&self, diff: &str, stats: &DiffStatistics) -> (ChangeIntent, f32, String) {
        let diff_lower = diff.to_lowercase();

        // 检测是否有新增的文件（功能新增信号）
        let has_new_file = diff.contains("new file mode");
        let has_deleted_file = diff.contains("deleted file mode");

        // 检测依赖文件变更
        let has_dep_change = diff.contains("Cargo.toml")
            || diff.contains("package.json")
            || diff.contains("requirements.txt")
            || diff.contains("go.mod")
            || diff.contains("pom.xml");

        // 检测配置文件变更
        let has_config_change = diff.contains(".json")
            || diff.contains(".toml")
            || diff.contains(".yaml")
            || diff.contains(".yml")
            || diff.contains(".env")
            || diff.contains(".ini");

        // 检测测试文件
        let has_test_file = diff.contains("/test")
            || diff.contains("/tests")
            || diff.contains("test_")
            || diff.contains(".test.")
            || diff.contains(".spec.");

        // 纯删除 = 功能删除
        if has_deleted_file && !has_new_file && stats.additions < stats.deletions / 2 {
            return (
                ChangeIntent::FeatureRemoval,
                0.55,
                "检测到文件删除且新增行少，推断为功能删除".to_string(),
            );
        }

        // 大量删除 = 可能重构
        if stats.deletions > 50 && stats.additions > 20 {
            return (
                ChangeIntent::Refactor,
                0.50,
                "大量新增和删除行，推断为重构".to_string(),
            );
        }

        // 依赖文件变更
        if has_dep_change {
            return (
                ChangeIntent::DependencyUpdate,
                0.65,
                "检测到依赖文件变更".to_string(),
            );
        }

        // 配置文件变更
        if has_config_change {
            return (
                ChangeIntent::ConfigChange,
                0.60,
                "检测到配置文件变更".to_string(),
            );
        }

        // 测试文件变更
        if has_test_file {
            return (
                ChangeIntent::Test,
                0.60,
                "检测到测试文件变更".to_string(),
            );
        }

        // 新增文件为主 = 功能新增
        if has_new_file {
            return (
                ChangeIntent::FeatureAddition,
                0.50,
                "检测到新文件，推断为功能新增".to_string(),
            );
        }

        // 默认
        (
            ChangeIntent::FeatureModification,
            0.35,
            "未检测到明确特征，默认为功能修改".to_string(),
        )
    }

    /// 检测次要变更类型
    fn detect_secondary_intents(&self, _diff: &str, _stats: &DiffStatistics) -> Vec<ChangeIntent> {
        // MVP: 暂不检测次要类型
        let mut secondaries = Vec::new();

        // 如果同时包含测试文件和源文件变更，标记为测试
        // 暂简化处理
        secondaries
    }
}

impl Default for Classifier {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DiffStatistics;

    fn empty_stats() -> DiffStatistics {
        DiffStatistics {
            files_changed: 1,
            additions: 5,
            deletions: 2,
            languages: vec!["python".to_string()],
            entities: vec![],
            intent_groups: vec![],
        }
    }

    #[test]
    fn test_classify_feat() {
        let classifier = Classifier::new();
        let result = classifier.classify("mock diff", "feat: add user login", &empty_stats());
        assert_eq!(result.primary_intent, ChangeIntent::FeatureAddition);
        assert!(result.confidence.score > 0.8);
    }

    #[test]
    fn test_classify_fix() {
        let classifier = Classifier::new();
        let result = classifier.classify("mock diff", "fix: resolve auth bug", &empty_stats());
        assert_eq!(result.primary_intent, ChangeIntent::Bugfix);
    }

    #[test]
    fn test_classify_refactor() {
        let classifier = Classifier::new();
        let result = classifier.classify("mock diff", "refactor: extract service", &empty_stats());
        assert_eq!(result.primary_intent, ChangeIntent::Refactor);
    }

    #[test]
    fn test_classify_from_diff() {
        let classifier = Classifier::new();
        // 无 commit message，仅靠 diff 特征
        let result = classifier.classify(
            "new file mode 100644\n--- /dev/null\n+++ b/src/auth.ts",
            "",
            &empty_stats(),
        );
        assert_eq!(result.primary_intent, ChangeIntent::FeatureAddition);
    }
}
