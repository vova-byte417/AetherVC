//! 文档生成器
//!
//! 将流水线各阶段的结果合并，生成结构化 JSON 报告和人类可读的 Markdown 文档。
//! 输出格式遵循 PRD 定义的模板。

use crate::domain::{
    ChangeIntent, ClassificationResult, CommitAnalysisReport, CommitSummary, DetailedChanges,
    DiffStatistics, HistoricalContext, ImpactAssessment, IntentAnalysis, PipelineInput,
    Recommendations,
};
use chrono::Utc;

/// 报告生成器
pub struct ReportGenerator;

impl ReportGenerator {
    pub fn new() -> Self {
        Self
    }

    /// 生成完整报告
    pub fn generate(
        &self,
        input: &PipelineInput,
        stats: &DiffStatistics,
        classification: &ClassificationResult,
        intent: &IntentAnalysis,
        impact: &ImpactAssessment,
    ) -> CommitAnalysisReport {
        CommitAnalysisReport {
            commit_hash: input.commit_hash.clone(),
            author: input.author.clone(),
            timestamp: input.timestamp,
            summary: CommitSummary {
                one_liner: self.generate_one_liner(classification, &input.commit_message),
                change_type: classification.primary_intent.clone(),
                confidence: classification.confidence.clone(),
            },
            detailed_changes: DetailedChanges {
                entities: stats.entities.clone(),
                key_diffs: self.generate_key_diffs(stats, classification),
                stats: stats.clone(),
            },
            intent_analysis: intent.clone(),
            impact: impact.clone(),
            history: HistoricalContext {
                related_commits: Vec::new(),
                evolution_path: "MVP 阶段，暂不支持跨 commit 演化分析".to_string(),
            },
            recommendations: Recommendations {
                review_focus: self.generate_review_focus(classification, impact, stats),
                suggested_tests: self.generate_test_suggestions(classification, impact),
            },
        }
    }

    /// 生成 Markdown 文档
    pub fn render_markdown(&self, report: &CommitAnalysisReport) -> String {
        let mut md = String::new();

        // 标题
        md.push_str("# Commit 变更分析报告\n\n");
        md.push_str(&format!(
            "> 由 AetherCommit Intelligence 自动生成 | {}\n\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S")
        ));

        // 基本信息表格
        md.push_str("| 属性 | 内容 |\n");
        md.push_str("|------|------|\n");
        md.push_str(&format!("| **Commit Hash** | `{}` |\n", report.commit_hash));
        md.push_str(&format!("| **作者** | {} |\n", report.author));
        md.push_str(&format!(
            "| **时间** | {} |\n",
            report.timestamp.format("%Y-%m-%d %H:%M:%S")
        ));
        md.push_str(&format!(
            "| **变更类型** | {} |\n",
            report.summary.change_type
        ));
        md.push_str(&format!(
            "| **置信度** | {:.0}% |\n",
            report.summary.confidence.score * 100.0
        ));
        md.push('\n');

        // 1. 变更摘要
        md.push_str("## 1. 变更摘要\n\n");
        md.push_str(&format!("**一句话总结**：{}\n\n", report.summary.one_liner));
        md.push_str(&format!(
            "**分类依据**：{}\n\n",
            report.summary.confidence.rationale
        ));

        // 2. 详细变更内容
        md.push_str("## 2. 详细变更内容\n\n");

        // 统计概览
        md.push_str("### 2.1 统计概览\n\n");
        md.push_str("| 指标 | 数值 |\n");
        md.push_str("|------|------|\n");
        md.push_str(&format!(
            "| 变更文件数 | {} |\n",
            report.detailed_changes.stats.files_changed
        ));
        md.push_str(&format!(
            "| 新增行数 | +{} |\n",
            report.detailed_changes.stats.additions
        ));
        md.push_str(&format!(
            "| 删除行数 | -{} |\n",
            report.detailed_changes.stats.deletions
        ));
        if !report.detailed_changes.stats.languages.is_empty() {
            md.push_str(&format!(
                "| 涉及语言 | {} |\n",
                report.detailed_changes.stats.languages.join(", ")
            ));
        }
        md.push('\n');

        // 变更实体
        if !report.detailed_changes.entities.is_empty() {
            md.push_str("### 2.2 变更实体\n\n");
            md.push_str("| 操作 | 实体名 | 类型 | 文件 |\n");
            md.push_str("|------|--------|------|------|\n");
            for entity in &report.detailed_changes.entities {
                md.push_str(&format!(
                    "| {} | `{}` | {} | `{}` |\n",
                    entity.operation, entity.name, entity.entity_type, entity.file_path
                ));
            }
            md.push('\n');
        }

        // 意图分组
        if !report.detailed_changes.stats.intent_groups.is_empty() {
            md.push_str("### 2.3 变更分组\n\n");
            for group in &report.detailed_changes.stats.intent_groups {
                md.push_str(&format!("- **{}**：涉及文件 `{}`，实体：{}\n", group.label,
                    group.files.join(", "),
                    group.entities.iter().map(|e| format!("`{}`", e)).collect::<Vec<_>>().join(", ")
                ));
            }
            md.push('\n');
        }

        // 关键 diff
        if !report.detailed_changes.key_diffs.is_empty() {
            md.push_str("### 2.4 关键变更点\n\n");
            for diff_desc in &report.detailed_changes.key_diffs {
                md.push_str(&format!("- {}\n", diff_desc));
            }
            md.push('\n');
        }

        // 3. 变更意图与动机
        md.push_str("## 3. 变更意图与动机\n\n");
        md.push_str(&format!("**解决的问题**：{}\n\n", report.intent_analysis.problem_solved));
        md.push_str(&format!("**推断动机**：{}\n\n", report.intent_analysis.inferred_motivation));
        md.push_str(&format!(
            "**架构上下文**：{}\n\n",
            report.intent_analysis.architectural_context
        ));
        md.push_str(&format!(
            "**置信度**：{:.0}%（{}）\n\n",
            report.intent_analysis.confidence.score * 100.0,
            report.intent_analysis.confidence.rationale
        ));

        // 4. 影响范围与风险
        md.push_str("## 4. 影响范围与风险\n\n");

        if report.impact.is_breaking_change {
            md.push_str("> ⚠ **警告：此变更可能为破坏性变更！**\n\n");
        }

        md.push_str("### 4.1 受影响模块\n\n");
        if report.impact.affected_modules.is_empty() {
            md.push_str("- 无法确定受影响模块\n");
        } else {
            for module in &report.impact.affected_modules {
                md.push_str(&format!("- `{}`\n", module));
            }
        }
        md.push('\n');

        md.push_str("### 4.2 风险评估\n\n");
        md.push_str("| 风险等级 | 描述 | 缓解建议 |\n");
        md.push_str("|----------|------|----------|\n");
        for risk in &report.impact.risks {
            md.push_str(&format!(
                "| **{}** | {} | {} |\n",
                risk.severity.as_str(),
                risk.description,
                risk.mitigation.as_deref().unwrap_or("-")
            ));
        }
        md.push('\n');

        md.push_str("### 4.3 建议验证点\n\n");
        for (i, validation) in report.impact.suggested_validations.iter().enumerate() {
            md.push_str(&format!("{}. {}\n", i + 1, validation));
        }
        md.push('\n');

        // 5. 历史上下文
        md.push_str("## 5. 历史上下文\n\n");
        if report.history.related_commits.is_empty() {
            md.push_str("- 未找到相关历史 commit\n");
        } else {
            for commit in &report.history.related_commits {
                md.push_str(&format!("- `{}`\n", commit));
            }
        }
        md.push_str(&format!("\n**演化路径**：{}\n\n", report.history.evolution_path));

        // 6. 建议
        md.push_str("## 6. 建议\n\n");
        md.push_str("### Review 重点\n\n");
        for (i, focus) in report.recommendations.review_focus.iter().enumerate() {
            md.push_str(&format!("{}. {}\n", i + 1, focus));
        }
        md.push('\n');

        md.push_str("### 测试推荐\n\n");
        for (i, test) in report.recommendations.suggested_tests.iter().enumerate() {
            md.push_str(&format!("{}. {}\n", i + 1, test));
        }
        md.push('\n');

        md.push_str("---\n\n");
        md.push_str("*本报告由 AetherCommit Intelligence (AetherCI) v0.1 自动生成*\n");

        md
    }

    /// 生成 JSON 字符串
    pub fn render_json(&self, report: &CommitAnalysisReport) -> String {
        serde_json::to_string_pretty(report).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
    }

    // ─── 辅助方法 ───

    fn generate_one_liner(
        &self,
        classification: &ClassificationResult,
        commit_message: &str,
    ) -> String {
        if !commit_message.is_empty() {
            commit_message.to_string()
        } else {
            format!("{}变更", classification.primary_intent)
        }
    }

    fn generate_key_diffs(
        &self,
        stats: &DiffStatistics,
        _classification: &ClassificationResult,
    ) -> Vec<String> {
        let mut key_diffs = Vec::new();

        key_diffs.push(format!(
            "变更 {} 个文件，+{} 行，-{} 行",
            stats.files_changed, stats.additions, stats.deletions
        ));

        // 实体变更摘要
        if !stats.entities.is_empty() {
            let added: Vec<_> = stats
                .entities
                .iter()
                .filter(|e| e.operation == crate::domain::EntityOperation::Added)
                .take(5)
                .collect();
            let deleted: Vec<_> = stats
                .entities
                .iter()
                .filter(|e| e.operation == crate::domain::EntityOperation::Deleted)
                .take(5)
                .collect();
            let modified: Vec<_> = stats
                .entities
                .iter()
                .filter(|e| e.operation == crate::domain::EntityOperation::Modified)
                .take(5)
                .collect();

            if !added.is_empty() {
                key_diffs.push(format!(
                    "新增实体: {}",
                    added.iter().map(|e| format!("`{}`({})", e.name, e.entity_type)).collect::<Vec<_>>().join(", ")
                ));
            }
            if !modified.is_empty() {
                key_diffs.push(format!(
                    "修改实体: {}",
                    modified.iter().map(|e| format!("`{}`({})", e.name, e.entity_type)).collect::<Vec<_>>().join(", ")
                ));
            }
            if !deleted.is_empty() {
                key_diffs.push(format!(
                    "删除实体: {}",
                    deleted.iter().map(|e| format!("`{}`({})", e.name, e.entity_type)).collect::<Vec<_>>().join(", ")
                ));
            }
        }

        key_diffs
    }

    fn generate_review_focus(
        &self,
        classification: &ClassificationResult,
        _impact: &ImpactAssessment,
        stats: &DiffStatistics,
    ) -> Vec<String> {
        let mut focus = Vec::new();

        match classification.primary_intent {
            ChangeIntent::FeatureAddition => {
                focus.push("新功能的边界条件和异常处理".to_string());
                focus.push("API 设计的合理性和一致性".to_string());
            }
            ChangeIntent::Bugfix => {
                focus.push("修复方案的完整性和正确性".to_string());
                focus.push("是否可能引入新的边界问题".to_string());
            }
            ChangeIntent::Refactor => {
                focus.push("重构是否保持了原有行为".to_string());
                focus.push("是否还有进一步简化的空间".to_string());
            }
            ChangeIntent::FeatureRemoval => {
                focus.push("确认删除的功能确实无下游依赖".to_string());
                focus.push("相关的文档和配置是否需要同步清理".to_string());
            }
            ChangeIntent::SecurityHardening => {
                focus.push("安全修复的完整性和覆盖范围".to_string());
                focus.push("是否存在同类型的安全隐患".to_string());
            }
            ChangeIntent::DependencyUpdate => {
                focus.push("新版本的 Breaking Changes".to_string());
                focus.push("依赖的传递性影响".to_string());
            }
            _ => {
                focus.push("代码逻辑的正确性".to_string());
                focus.push("变更对现有测试的影响".to_string());
            }
        }

        if stats.files_changed > 5 {
            focus.push(format!(
                "跨 {} 个文件的影响一致性",
                stats.files_changed
            ));
        }

        focus
    }

    fn generate_test_suggestions(
        &self,
        classification: &ClassificationResult,
        impact: &ImpactAssessment,
    ) -> Vec<String> {
        let mut tests = Vec::new();

        match classification.primary_intent {
            ChangeIntent::FeatureAddition => {
                tests.push("为新功能编写完整的单元测试".to_string());
                tests.push("编写集成测试验证端到端流程".to_string());
            }
            ChangeIntent::Bugfix => {
                tests.push("编写回归测试覆盖缺陷场景".to_string());
                tests.push("运行现有测试套件确认无回归".to_string());
            }
            ChangeIntent::Refactor => {
                tests.push("运行完整的现有测试套件".to_string());
                tests.push("对比重构前后测试覆盖率".to_string());
            }
            ChangeIntent::Performance => {
                tests.push("运行性能基准测试".to_string());
                tests.push("压力测试验证性能改进".to_string());
            }
            _ => {
                tests.push("运行受影响模块的单元测试".to_string());
                tests.push("运行项目完整测试套件".to_string());
            }
        }

        // 针对影响的模块
        for module in impact.affected_modules.iter().take(3) {
            tests.push(format!("验证 `{}` 模块的接口兼容性", module));
        }

        tests
    }
}

impl Default for ReportGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ChangedEntity, ConfidenceScore, EntityOperation, IntentGroup, RiskItem, RiskSeverity,
    };
    use chrono::Utc;

    fn make_test_report() -> CommitAnalysisReport {
        CommitAnalysisReport {
            commit_hash: "abc1234".to_string(),
            author: "AI-Agent-X".to_string(),
            timestamp: Utc::now(),
            summary: CommitSummary {
                one_liner: "feat: add user authentication".to_string(),
                change_type: ChangeIntent::FeatureAddition,
                confidence: ConfidenceScore::new(0.85, "commit message"),
            },
            detailed_changes: DetailedChanges {
                entities: vec![
                    ChangedEntity {
                        name: "login".to_string(),
                        entity_type: "function".to_string(),
                        file_path: "src/auth/login.ts".to_string(),
                        operation: EntityOperation::Added,
                        previous_name: None,
                    },
                ],
                key_diffs: vec!["新增 login 函数".to_string()],
                stats: DiffStatistics {
                    files_changed: 2,
                    additions: 45,
                    deletions: 0,
                    languages: vec!["typescript".to_string()],
                    entities: vec![],
                    intent_groups: vec![IntentGroup {
                        label: "auth_login".to_string(),
                        files: vec!["src/auth/login.ts".to_string()],
                        entities: vec!["login".to_string()],
                    }],
                },
            },
            intent_analysis: IntentAnalysis {
                problem_solved: "系统缺少用户认证功能".to_string(),
                inferred_motivation: "需要为用户提供安全的登录机制".to_string(),
                architectural_context: "属于认证模块的核心功能，影响整个系统的安全架构".to_string(),
                confidence: ConfidenceScore::new(0.8, "LLM inference"),
            },
            impact: ImpactAssessment {
                affected_modules: vec!["auth".to_string(), "middleware".to_string()],
                risks: vec![RiskItem {
                    description: "新功能涉及安全认证".to_string(),
                    severity: RiskSeverity::Medium,
                    mitigation: Some("安全审查".to_string()),
                }],
                suggested_validations: vec!["测试登录流程".to_string(), "验证 JWT token".to_string()],
                is_breaking_change: false,
            },
            history: HistoricalContext {
                related_commits: vec![],
                evolution_path: "新功能首次引入".to_string(),
            },
            recommendations: Recommendations {
                review_focus: vec!["认证流程安全性".to_string()],
                suggested_tests: vec!["单元测试".to_string(), "集成测试".to_string()],
            },
        }
    }

    #[test]
    fn test_markdown_rendering() {
        let generator = ReportGenerator::new();
        let report = make_test_report();
        let md = generator.render_markdown(&report);

        assert!(md.contains("# Commit 变更分析报告"));
        assert!(md.contains("abc1234"));
        assert!(md.contains("AI-Agent-X"));
        assert!(md.contains("功能新增"));
        assert!(md.contains("## 1. 变更摘要"));
        assert!(md.contains("## 2. 详细变更内容"));
        assert!(md.contains("## 3. 变更意图与动机"));
        assert!(md.contains("## 4. 影响范围与风险"));
        assert!(md.contains("## 5. 历史上下文"));
        assert!(md.contains("## 6. 建议"));
    }

    #[test]
    fn test_json_rendering() {
        let generator = ReportGenerator::new();
        let report = make_test_report();
        let json = generator.render_json(&report);

        assert!(json.contains("abc1234"));
        assert!(json.contains("AI-Agent-X"));
        // 验证可解析
        let _parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    }
}
