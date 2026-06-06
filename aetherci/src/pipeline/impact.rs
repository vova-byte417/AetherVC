//! 影响分析模块
//!
//! 评估变更的影响范围和风险等级。
//! 基于依赖图查询和代码特征进行回归风险预测。

use aether_core::storage::graph_db::GraphStore;
use crate::domain::{
    ChangeIntent, ClassificationResult, DiffStatistics, ImpactAssessment, RiskItem, RiskSeverity,
};
use std::sync::Arc;
use tracing::{debug, info};

/// 影响分析器
pub struct ImpactAnalyzer {
    /// 图存储（用于查询依赖关系）
    graph_store: Option<Arc<dyn GraphStore>>,
}

impl ImpactAnalyzer {
    pub fn new(graph_store: Option<Arc<dyn GraphStore>>) -> Self {
        Self { graph_store }
    }

    /// 执行影响分析
    pub async fn analyze(
        &self,
        diff: &str,
        classification: &ClassificationResult,
        stats: &DiffStatistics,
        commit_hash: &str,
    ) -> ImpactAssessment {
        // 1. 提取受影响模块
        let affected_modules = self.extract_affected_modules(diff, stats);

        // 2. 从图存储查询受影响的依赖
        let graph_affected = self.query_graph_impact(commit_hash).await;

        // 3. 合并模块列表
        let mut all_modules = affected_modules.clone();
        for m in graph_affected {
            if !all_modules.contains(&m) {
                all_modules.push(m);
            }
        }

        // 4. 风险评估
        let risks = self.assess_risks(classification, stats);
        let is_breaking = self.detect_breaking_change(classification, diff, stats);

        // 5. 生成验证建议
        let validations = self.generate_validations(classification, &all_modules, &risks);

        ImpactAssessment {
            affected_modules: all_modules,
            risks,
            suggested_validations: validations,
            is_breaking_change: is_breaking,
        }
    }

    /// 从 diff 中提取受影响的模块
    fn extract_affected_modules(&self, diff: &str, _stats: &DiffStatistics) -> Vec<String> {
        let mut modules: Vec<String> = Vec::new();

        for line in diff.lines() {
            if line.starts_with("--- ") || line.starts_with("+++ ") {
                let path = line[4..].trim();
                let path = path.strip_prefix("a/").unwrap_or(path);
                let path = path.strip_prefix("b/").unwrap_or(path);

                if path == "/dev/null" {
                    continue;
                }

                // 提取模块名（第一级目录或文件名）
                let module = if let Some(first_slash) = path.find('/') {
                    path[..first_slash].to_string()
                } else {
                    // 去掉扩展名
                    if let Some(dot) = path.rfind('.') {
                        path[..dot].to_string()
                    } else {
                        path.to_string()
                    }
                };

                if module != "." && !modules.contains(&module) {
                    modules.push(module);
                }
            }
        }

        // 同时从文件路径提取
        for line in diff.lines() {
            if line.starts_with("diff --git ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                for part in parts {
                    let path = part.strip_prefix("a/").unwrap_or(part);
                    let path = path.strip_prefix("b/").unwrap_or(path);
                    if path != "a/" && path != "b/" && !path.is_empty() {
                        // 提取第一级目录源文件
                        if let Some(slash) = path.find('/') {
                            let m = path[..slash].to_string();
                            if m != "." && m != "src" && !modules.contains(&m) {
                                modules.push(m);
                            }
                        }
                    }
                }
            }
        }

        // 如果是从 src 下的子目录
        if modules.is_empty() {
            for line in diff.lines() {
                if line.starts_with("--- ") || line.starts_with("+++ ") {
                    let path = line[4..].trim();
                    let path = path.strip_prefix("a/").unwrap_or(path);
                    let path = path.strip_prefix("b/").unwrap_or(path);
                    if path.starts_with("src/") {
                        let rest = &path[4..];
                        let module = if let Some(slash) = rest.find('/') {
                            rest[..slash].to_string()
                        } else {
                            rest.to_string()
                        };
                        if !modules.contains(&module) {
                            modules.push(module);
                        }
                    }
                }
            }
        }

        modules
    }

    /// 查询图存储中的影响范围
    async fn query_graph_impact(&self, commit_hash: &str) -> Vec<String> {
        if let Some(ref graph) = self.graph_store {
            match graph.get_affected(commit_hash).await {
                Ok(affected) => {
                    debug!("图查询影响范围: {} 个下游依赖", affected.len());
                    affected
                }
                Err(e) => {
                    debug!("图查询失败: {}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        }
    }

    /// 评估风险
    fn assess_risks(
        &self,
        classification: &ClassificationResult,
        stats: &DiffStatistics,
    ) -> Vec<RiskItem> {
        let mut risks = Vec::new();

        // 风险1：大量文件变更
        if stats.files_changed > 10 {
            risks.push(RiskItem {
                description: format!(
                    "变更涉及 {} 个文件，范围较大，回归风险高",
                    stats.files_changed
                ),
                severity: RiskSeverity::High,
                mitigation: Some("建议分批次提交或增加集成测试覆盖".to_string()),
            });
        } else if stats.files_changed > 5 {
            risks.push(RiskItem {
                description: format!(
                    "变更涉及 {} 个文件，需关注模块间兼容性",
                    stats.files_changed
                ),
                severity: RiskSeverity::Medium,
                mitigation: Some("建议验证模块间接口调用是否正常".to_string()),
            });
        }

        // 风险2：大量删除
        if stats.deletions > 200 {
            risks.push(RiskItem {
                description: format!("删除了 {} 行代码，可能存在功能缺失", stats.deletions),
                severity: RiskSeverity::High,
                mitigation: Some("建议确认被删除的代码是否在其他地方有引用".to_string()),
            });
        } else if stats.deletions > 100 {
            risks.push(RiskItem {
                description: format!("删除了 {} 行代码", stats.deletions),
                severity: RiskSeverity::Medium,
                mitigation: Some("建议检查是否有遗留引用".to_string()),
            });
        }

        // 风险3：破坏性变更
        if classification.primary_intent == ChangeIntent::FeatureRemoval {
            risks.push(RiskItem {
                description: "此变更删除了功能，可能为破坏性变更".to_string(),
                severity: RiskSeverity::Critical,
                mitigation: Some("建议确认无下游依赖此功能后方可合入".to_string()),
            });
        }

        // 风险4：安全相关变更
        if classification.primary_intent == ChangeIntent::SecurityHardening {
            risks.push(RiskItem {
                description: "安全相关变更，需仔细审查".to_string(),
                severity: RiskSeverity::High,
                mitigation: Some("建议进行安全审查和渗透测试".to_string()),
            });
        }

        // 风险5：依赖更新
        if classification.primary_intent == ChangeIntent::DependencyUpdate {
            risks.push(RiskItem {
                description: "依赖版本变更可能引入兼容性问题".to_string(),
                severity: RiskSeverity::Medium,
                mitigation: Some("建议运行完整测试套件，检查 API 兼容性".to_string()),
            });
        }

        // 默认：低风险
        if risks.is_empty() {
            risks.push(RiskItem {
                description: "变更范围可控，风险较低".to_string(),
                severity: RiskSeverity::Low,
                mitigation: Some("常规 Code Review + 单元测试".to_string()),
            });
        }

        risks
    }

    /// 检测是否为破坏性变更
    fn detect_breaking_change(
        &self,
        classification: &ClassificationResult,
        diff: &str,
        _stats: &DiffStatistics,
    ) -> bool {
        // 功能删除
        if classification.primary_intent == ChangeIntent::FeatureRemoval {
            return true;
        }

        // 检测 diff 中的 breaking change 标记
        let diff_lower = diff.to_lowercase();
        if diff_lower.contains("breaking change")
            || diff_lower.contains("break:")
            || diff.contains("BREAKING CHANGE")
        {
            return true;
        }

        // 检测删除的 API（函数/类/接口）
        let deleted_entities = diff
            .lines()
            .filter(|l| l.starts_with('-') && !l.starts_with("--- "))
            .filter(|l| {
                let t = l.trim_start_matches('-').trim();
                (t.starts_with("def ")
                    || t.starts_with("class ")
                    || t.starts_with("function ")
                    || t.starts_with("interface ")
                    || t.starts_with("export ")
                    || t.starts_with("pub fn ")
                    || t.starts_with("pub struct "))
            })
            .count();

        if deleted_entities > 3 {
            return true;
        }

        false
    }

    /// 生成验证建议
    fn generate_validations(
        &self,
        classification: &ClassificationResult,
        modules: &[String],
        risks: &[RiskItem],
    ) -> Vec<String> {
        let mut validations = Vec::new();

        // 基于变更类型
        match classification.primary_intent {
            ChangeIntent::FeatureAddition => {
                validations.push("验证新功能的正向和异常路径".to_string());
                validations.push("添加新功能的单元测试".to_string());
            }
            ChangeIntent::Bugfix => {
                validations.push("验证缺陷复现路径已被修复".to_string());
                validations.push("确认修复未引入新问题（回归测试）".to_string());
            }
            ChangeIntent::Refactor => {
                validations.push("运行完整测试套件，确保行为未变".to_string());
                validations.push("对比重构前后的 API 输出".to_string());
            }
            ChangeIntent::Performance => {
                validations.push("运行性能基准测试".to_string());
                validations.push("监控生产环境资源消耗".to_string());
            }
            ChangeIntent::FeatureRemoval => {
                validations.push("确认删除的 API/功能无下游调用".to_string());
                validations.push("更新相关文档和变更日志".to_string());
            }
            _ => {
                validations.push("运行受影响模块的单元测试".to_string());
                validations.push("进行 Code Review".to_string());
            }
        }

        // 基于影响模块
        if !modules.is_empty() {
            let module_list = modules.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
            validations.push(format!("重点验证模块: {}", module_list));
        }

        // 基于风险
        for risk in risks {
            if let Some(ref mitigation) = risk.mitigation {
                if !validations.contains(mitigation) {
                    validations.push(mitigation.clone());
                }
            }
        }

        validations
    }
}

impl Default for ImpactAnalyzer {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ConfidenceScore;

    fn test_stats() -> DiffStatistics {
        DiffStatistics {
            files_changed: 3,
            additions: 50,
            deletions: 20,
            languages: vec!["typescript".to_string()],
            entities: vec![],
            intent_groups: vec![],
        }
    }

    #[tokio::test]
    async fn test_basic_impact_analysis() {
        let analyzer = ImpactAnalyzer::new(None);
        let classification = ClassificationResult {
            primary_intent: ChangeIntent::FeatureAddition,
            confidence: ConfidenceScore::new(0.8, "test"),
            secondary_intents: vec![],
        };

        let result = analyzer
            .analyze(
                "--- a/src/auth/login.ts\n+++ b/src/auth/login.ts\n+new line",
                &classification,
                &test_stats(),
                "abc123",
            )
            .await;

        assert!(!result.affected_modules.is_empty());
        assert!(!result.risks.is_empty());
        assert!(!result.suggested_validations.is_empty());
    }

    #[tokio::test]
    async fn test_breaking_change_detection() {
        let analyzer = ImpactAnalyzer::new(None);
        let classification = ClassificationResult {
            primary_intent: ChangeIntent::FeatureRemoval,
            confidence: ConfidenceScore::new(0.8, "test"),
            secondary_intents: vec![],
        };

        let result = analyzer
            .analyze("removed stuff", &classification, &test_stats(), "abc123")
            .await;

        assert!(result.is_breaking_change);
    }
}
