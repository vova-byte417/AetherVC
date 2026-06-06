//! 意图推理模块
//!
//! 使用 LLM 进行多轮推理，理解变更背后的意图和动机。
//! 支持带降级策略的推理（LLM > 规则启发）。
//! 结合项目 RAG 知识库进行上下文推理。

use aether_core::llm::client::LLMClient;
use aether_core::llm::prompts::PromptTemplateManager;
use crate::domain::{
    ChangeIntent, ClassificationResult, ConfidenceScore, DiffStatistics, IntentAnalysis,
};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// 意图推理器
pub struct IntentReasoner {
    /// LLM 客户端（可选，为 None 时使用规则推理）
    llm_client: Option<Arc<dyn LLMClient>>,
    /// Prompt 模板管理器
    prompt_templates: Arc<PromptTemplateManager>,
}

impl IntentReasoner {
    pub fn new(llm_client: Option<Arc<dyn LLMClient>>) -> Self {
        Self {
            llm_client,
            prompt_templates: Arc::new(PromptTemplateManager::new()),
        }
    }

    /// 执行意图推理
    pub async fn reason(
        &self,
        diff: &str,
        commit_message: &str,
        classification: &ClassificationResult,
        stats: &DiffStatistics,
        _commit_hash: &str,
    ) -> IntentAnalysis {
        // 优先使用 LLM 推理
        if let Some(ref llm) = self.llm_client {
            match self.reason_with_llm(llm, diff, commit_message, classification, stats).await {
                Ok(analysis) => {
                    info!("LLM 意图推理完成 (confidence: {:.2})", analysis.confidence.score);
                    return analysis;
                }
                Err(e) => {
                    warn!("LLM 意图推理失败，降级为规则推理: {}", e);
                }
            }
        }

        // 降级：规则推理
        debug!("使用规则推理");
        self.reason_with_rules(diff, commit_message, classification, stats)
    }

    /// LLM 推理
    async fn reason_with_llm(
        &self,
        llm: &Arc<dyn LLMClient>,
        diff: &str,
        commit_message: &str,
        classification: &ClassificationResult,
        _stats: &DiffStatistics,
    ) -> Result<IntentAnalysis, String> {
        let mut values = std::collections::HashMap::new();
        values.insert(
            "user_input_or_diff".to_string(),
            format!(
                "Commit Message: {}\n\nClassified as: {}\n\nDiff:\n{}",
                commit_message,
                classification.primary_intent.as_str(),
                // 限制 diff 长度以控制 token 消耗
                if diff.len() > 4000 {
                    format!("{}... (truncated)", &diff[..4000])
                } else {
                    diff.to_string()
                }
            ),
        );

        // 尝试从模板管理器获取 commit_intelligence 模板
        let prompt = self
            .prompt_templates
            .get("commit_intelligence")
            .map(|t| t.render(&values))
            .unwrap_or_else(|| {
                // 内置默认意图推理 Prompt
                format!(
                    r#"你是一个代码变更意图分析专家。请分析以下代码变更，推断作者的意图和动机。

变更类型（预分类）：{change_type}
Commit Message：{msg}

Code Diff：
{diff}

请用 JSON 格式回答：
{{
    "problem_solved": "这个变更解决了什么问题？（一句话）",
    "inferred_motivation": "作者可能的动机是什么？（2-3 句话）",
    "architectural_context": "这个变更与项目整体架构的关系？（如：是否为某个功能的子任务、是否影响核心模块等）",
    "confidence": 0.0 ~ 1.0 的置信度分数
}}

只输出 JSON，不要包含其他内容。"#,
                    change_type = classification.primary_intent.as_str(),
                    msg = commit_message,
                    diff = if diff.len() > 4000 { &diff[..4000] } else { diff }
                )
            });

        let response = llm.complete(&prompt).await.map_err(|e| e.to_string())?;

        // 解析 LLM 返回的 JSON
        let content = response.content.trim();
        // 尝试提取 JSON（处理 markdown code block 包裹的情况）
        let json_str = if content.starts_with("```") {
            content
                .lines()
                .skip(1)
                .take_while(|l| !l.starts_with("```"))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            content.to_string()
        };

        match serde_json::from_str::<serde_json::Value>(&json_str) {
            Ok(json) => {
                let problem = json["problem_solved"]
                    .as_str()
                    .unwrap_or("无法确定");
                let motivation = json["inferred_motivation"]
                    .as_str()
                    .unwrap_or("无法确定");
                let arch = json["architectural_context"]
                    .as_str()
                    .unwrap_or("无相关信息");
                let conf = json["confidence"].as_f64().unwrap_or(0.5) as f32;

                Ok(IntentAnalysis {
                    problem_solved: problem.to_string(),
                    inferred_motivation: motivation.to_string(),
                    architectural_context: arch.to_string(),
                    confidence: ConfidenceScore::new(conf, "基于 LLM 推理"),
                })
            }
            Err(e) => {
                // JSON 解析失败，用原始文本
                warn!("LLM 返回 JSON 解析失败: {}，使用原始文本", e);
                Ok(IntentAnalysis {
                    problem_solved: "（LLM 解析失败）".to_string(),
                    inferred_motivation: content.to_string(),
                    architectural_context: "无法确定".to_string(),
                    confidence: ConfidenceScore::low("LLM 输出格式异常"),
                })
            }
        }
    }

    /// 规则推理（降级方案）
    pub(crate) fn reason_with_rules(
        &self,
        diff: &str,
        commit_message: &str,
        classification: &ClassificationResult,
        stats: &DiffStatistics,
    ) -> IntentAnalysis {
        let problem = self.infer_problem(classification, commit_message, stats);
        let motivation = self.infer_motivation(classification, diff, stats);
        let architectural = self.infer_architectural_context(diff, stats);

        IntentAnalysis {
            problem_solved: problem,
            inferred_motivation: motivation,
            architectural_context: architectural,
            confidence: ConfidenceScore::new(
                classification.confidence.score * 0.8,
                "基于规则启发",
            ),
        }
    }

    fn infer_problem(
        &self,
        classification: &ClassificationResult,
        commit_message: &str,
        stats: &DiffStatistics,
    ) -> String {
        if !commit_message.is_empty() {
            return format!("根据 commit message：{}", commit_message);
        }

        match classification.primary_intent {
            ChangeIntent::FeatureAddition => format!(
                "新增 {} 个文件中的功能，涉及 {} 个实体",
                stats.files_changed,
                stats.entities.len()
            ),
            ChangeIntent::Bugfix => "修复代码中的缺陷".to_string(),
            ChangeIntent::Refactor => format!(
                "重构代码结构，涉及 {} 个文件的 {} 处变更",
                stats.files_changed,
                stats.entities.len()
            ),
            ChangeIntent::Performance => "优化代码性能".to_string(),
            ChangeIntent::SecurityHardening => "加固安全防护".to_string(),
            ChangeIntent::DependencyUpdate => "更新项目依赖".to_string(),
            ChangeIntent::ConfigChange => "修改项目配置".to_string(),
            ChangeIntent::FeatureRemoval => format!("移除 {} 个文件中的功能", stats.files_changed),
            _ => format!("修改 {} 个文件中的 {} 行代码", stats.files_changed, stats.additions),
        }
    }

    fn infer_motivation(
        &self,
        classification: &ClassificationResult,
        _diff: &str,
        stats: &DiffStatistics,
    ) -> String {
        match classification.primary_intent {
            ChangeIntent::FeatureAddition => {
                format!(
                    "开发者希望为项目添加新能力。变更涉及 {} 个文件，包含 {} 个代码实体。",
                    stats.files_changed,
                    stats.entities.len()
                )
            }
            ChangeIntent::Bugfix => "开发者希望修复已发现的缺陷，提升系统稳定性。".to_string(),
            ChangeIntent::Refactor => {
                "开发者希望改善代码结构和可维护性，减少技术债务。这可能为后续功能扩展做准备。"
                    .to_string()
            }
            ChangeIntent::Performance => "开发者关注系统性能，希望通过优化提升响应速度或降低资源消耗。".to_string(),
            ChangeIntent::FeatureRemoval => "开发者认为某些功能已不再需要或将被替代，选择清理以简化系统。".to_string(),
            _ => "开发者根据项目需要进行了必要的代码变更。".to_string(),
        }
    }

    fn infer_architectural_context(
        &self,
        _diff: &str,
        stats: &DiffStatistics,
    ) -> String {
        if stats.files_changed <= 1 {
            "变更范围小，可能为局部修改，对整体架构影响有限。".to_string()
        } else if stats.files_changed <= 5 {
            format!("变更涉及 {} 个文件，属于中等范围的修改，可能影响多个模块的协作。", stats.files_changed)
        } else {
            format!(
                "变更涉及 {} 个文件，范围较大，可能为架构级调整或跨模块功能。建议关注模块间接口兼容性。",
                stats.files_changed
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_stats() -> DiffStatistics {
        DiffStatistics {
            files_changed: 2,
            additions: 10,
            deletions: 3,
            languages: vec!["typescript".to_string()],
            entities: vec![],
            intent_groups: vec![],
        }
    }

    #[test]
    fn test_reason_with_rules() {
        let reasoner = IntentReasoner::new(None);
        let classification = ClassificationResult {
            primary_intent: ChangeIntent::FeatureAddition,
            confidence: ConfidenceScore::new(0.85, "test"),
            secondary_intents: vec![],
        };

        let analysis = reasoner.reason_with_rules(
            "mock diff",
            "feat: add auth module",
            &classification,
            &empty_stats(),
        );

        assert!(!analysis.problem_solved.is_empty());
        assert!(!analysis.inferred_motivation.is_empty());
        assert!(!analysis.architectural_context.is_empty());
    }
}
