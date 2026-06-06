use crate::agents::base::{Agent, AgentContext, AgentStatus};
use crate::domain::agent::{AgentResult, AgentTask, AgentType};
use crate::domain::commit::SemanticInfo;
use crate::semantic::analyzer::SemanticAnalyzer;
use crate::utils::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// 语义解释 Agent
/// 负责分析代码变更，生成结构化的语义信息
pub struct SemanticInterpreterAgent {
    context: Arc<AgentContext>,
    status: std::sync::Mutex<AgentStatus>,
}

impl SemanticInterpreterAgent {
    pub fn new(context: Arc<AgentContext>) -> Self {
        Self {
            context,
            status: std::sync::Mutex::new(AgentStatus::Idle),
        }
    }

    async fn analyze_diff(&self, diff: &str, commit_message: &str) -> Result<SemanticInfo> {
        // 优先使用 LLM 分析
        if let Some(ref llm) = self.context.llm_client {
            return llm.analyze_semantic(diff, commit_message).await;
        }

        // 回退到规则分析
        let analyzer = crate::semantic::analyzer::RuleBasedAnalyzer::new();
        analyzer.analyze(diff, commit_message).await
    }
}

#[async_trait]
impl Agent for SemanticInterpreterAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::SemanticInterpreter
    }

    async fn execute(&self, task: AgentTask) -> Result<AgentResult> {
        *self.status.lock().unwrap() = AgentStatus::Running;

        let start = std::time::Instant::now();

        let diff = task
            .input
            .get("diff")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let message = task
            .input
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let result = match self.analyze_diff(diff, message).await {
            Ok(info) => AgentResult::success(
                task.id,
                serde_json::to_value(&info).unwrap_or_default(),
                start.elapsed().as_millis() as u64,
            ),
            Err(e) => AgentResult::failure(task.id, e.to_string()),
        };

        *self.status.lock().unwrap() = if result.success {
            AgentStatus::Completed
        } else {
            AgentStatus::Failed
        };

        Ok(result)
    }

    fn status(&self) -> AgentStatus {
        self.status.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::base::AgentContext;
    use crate::domain::agent::AgentTask;
    use crate::llm::client::MockLLMClient;
    use crate::semantic::embedder::MockEmbedder;
    use crate::storage::git::GitRepository;
    use crate::storage::graph_db::InMemoryGraphStore;
    use crate::storage::vector_db::InMemoryVectorStore;

    fn test_context() -> Arc<AgentContext> {
        Arc::new(
            AgentContext::new(
                Arc::new(GitRepository::new(".")),
                Arc::new(InMemoryVectorStore::new()),
                Arc::new(InMemoryGraphStore::new()),
                Arc::new(MockEmbedder::default()),
            )
            .with_llm(Arc::new(MockLLMClient::new())),
        )
    }

    #[tokio::test]
    async fn test_semantic_interpreter_with_mock_llm() {
        let agent = SemanticInterpreterAgent::new(test_context());
        let task = AgentTask::new(
            "analyze_semantic",
            serde_json::json!({
                "diff": "diff --git a/src/main.rs ...",
                "message": "feat: add login module"
            }),
        );

        let result = agent.execute(task).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_semantic_interpreter_without_llm() {
        let context = Arc::new(AgentContext::new(
            Arc::new(GitRepository::new(".")),
            Arc::new(InMemoryVectorStore::new()),
            Arc::new(InMemoryGraphStore::new()),
            Arc::new(MockEmbedder::default()),
        ));

        let agent = SemanticInterpreterAgent::new(context);
        let task = AgentTask::new(
            "analyze_semantic",
            serde_json::json!({
                "diff": "--- a/auth.rs\n+++ b/auth.rs",
                "message": "feat: add auth"
            }),
        );

        let result = agent.execute(task).await.unwrap();
        assert!(result.success);
    }
}
