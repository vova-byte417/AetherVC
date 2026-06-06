//! Pipeline 编排器
//!
//! 协调预处理器 → 分类器 → 意图推理器 → 影响分析器 → 报告生成器的完整流程。
//! 提供同步和异步执行接口。

use aether_core::llm::client::LLMClient;
use aether_core::storage::graph_db::GraphStore;
use std::sync::Arc;
use tracing::info;

use super::classifier::Classifier;
use super::impact::ImpactAnalyzer;
use super::intent::IntentReasoner;
use super::preprocessor::Preprocessor;
use super::report::ReportGenerator;
use crate::domain::{PipelineInput, PipelineOutput};

/// 语义 Diff 理解 Pipeline
///
/// 完整的五阶段流水线：
/// 1. 预处理 → 2. 分类 → 3. 意图推理 → 4. 影响分析 → 5. 文档生成
pub struct SemanticDiffPipeline {
    preprocessor: Preprocessor,
    classifier: Classifier,
    intent_reasoner: IntentReasoner,
    impact_analyzer: ImpactAnalyzer,
    report_generator: ReportGenerator,
}

impl SemanticDiffPipeline {
    /// 创建 Pipeline
    pub fn new(
        llm_client: Option<Arc<dyn LLMClient>>,
        graph_store: Option<Arc<dyn GraphStore>>,
    ) -> Self {
        Self {
            preprocessor: Preprocessor::new(),
            classifier: Classifier::new(),
            intent_reasoner: IntentReasoner::new(llm_client),
            impact_analyzer: ImpactAnalyzer::new(graph_store),
            report_generator: ReportGenerator::new(),
        }
    }

    /// 创建默认 Pipeline（无 LLM，无图存储）
    pub fn default_pipeline() -> Self {
        Self::new(None, None)
    }

    /// 执行完整 Pipeline
    pub async fn execute(&self, input: &PipelineInput) -> PipelineOutput {
        let start = std::time::Instant::now();
        info!("[Pipeline] 开始分析 commit: {}", input.commit_hash);

        // Phase 1: 预处理
        info!("[Phase 1/5] 预处理 diff...");
        let stats = self.preprocessor.analyze(&input.diff, &input.repo_path);

        // Phase 2: 变更分类
        info!("[Phase 2/5] 变更分类...");
        let classification = self.classifier.classify(
            &input.diff,
            &input.commit_message,
            &stats,
        );

        // Phase 3: 意图推理
        info!("[Phase 3/5] 意图推理...");
        let intent = self.intent_reasoner
            .reason(
                &input.diff,
                &input.commit_message,
                &classification,
                &stats,
                &input.commit_hash,
            )
            .await;

        // Phase 4: 影响分析
        info!("[Phase 4/5] 影响分析...");
        let impact = self.impact_analyzer
            .analyze(
                &input.diff,
                &classification,
                &stats,
                &input.commit_hash,
            )
            .await;

        // Phase 5: 生成报告
        info!("[Phase 5/5] 生成报告...");
        let report = self.report_generator.generate(
            input,
            &stats,
            &classification,
            &intent,
            &impact,
        );
        let markdown = self.report_generator.render_markdown(&report);

        let elapsed = start.elapsed();
        info!(
            "[Pipeline] 完成! 耗时 {:?}, 变更类型: {}, 置信度: {:.0}%",
            elapsed,
            classification.primary_intent,
            classification.confidence.score * 100.0
        );

        PipelineOutput { report, markdown }
    }

    /// 仅执行预处理 + 分类（快速分析，跳过 LLM）
    pub async fn quick_analyze(&self, input: &PipelineInput) -> PipelineOutput {
        let stats = self.preprocessor.analyze(&input.diff, &input.repo_path);
        let classification = self.classifier.classify(
            &input.diff,
            &input.commit_message,
            &stats,
        );

        // 快速分析跳过 LLM
        let intent = self.intent_reasoner.reason_with_rules(
            &input.diff,
            &input.commit_message,
            &classification,
            &stats,
        );

        let impact = self.impact_analyzer
            .analyze(&input.diff, &classification, &stats, &input.commit_hash)
            .await;

        let report = self.report_generator.generate(input, &stats, &classification, &intent, &impact);
        let markdown = self.report_generator.render_markdown(&report);

        PipelineOutput { report, markdown }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_input() -> PipelineInput {
        PipelineInput {
            diff: r#"--- a/src/main.py
+++ b/src/main.py
@@ -1,3 +1,5 @@
+def new_feature():
+    return "hello"
"#
            .to_string(),
            commit_message: "feat: add new_feature function".to_string(),
            commit_hash: "abc1234".to_string(),
            author: "Test User".to_string(),
            timestamp: Utc::now(),
            repo_path: ".".to_string(),
        }
    }

    #[tokio::test]
    async fn test_pipeline_execution() {
        let pipeline = SemanticDiffPipeline::default_pipeline();
        let input = make_input();
        let output = pipeline.execute(&input).await;

        assert_eq!(output.report.commit_hash, "abc1234");
        assert_eq!(output.report.author, "Test User");
        assert!(!output.markdown.is_empty());
        assert!(output.markdown.contains("# Commit 变更分析报告"));
        assert!(output.markdown.contains("abc1234"));
        assert!(output.markdown.contains("feat: add new_feature function"));
    }

    #[tokio::test]
    async fn test_quick_analyze() {
        let pipeline = SemanticDiffPipeline::default_pipeline();
        let input = make_input();
        let output = pipeline.quick_analyze(&input).await;

        assert!(!output.markdown.is_empty());
    }
}
