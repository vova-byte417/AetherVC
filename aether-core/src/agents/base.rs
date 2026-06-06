use crate::domain::agent::{AgentResult, AgentTask, AgentType, TaskContext};
use crate::domain::commit::SemanticInfo;
use crate::llm::client::LLMClient;
use crate::llm::prompts::PromptTemplateManager;
use crate::semantic::embedder::Embedder;
use crate::semantic::indexer::SemanticIndexer;
use crate::storage::git::GitOperations;
use crate::storage::graph_db::GraphStore;
use crate::storage::vector_db::VectorStore;
use crate::utils::Result;
use std::sync::Arc;

/// Agent 基础 trait
#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    fn agent_type(&self) -> AgentType;
    async fn execute(&self, task: AgentTask) -> Result<AgentResult>;
    fn status(&self) -> AgentStatus;
}

/// Agent 上下文（依赖注入容器）
pub struct AgentContext {
    pub git_repo: Arc<dyn GitOperations>,
    pub vector_store: Arc<dyn VectorStore>,
    pub graph_store: Arc<dyn GraphStore>,
    pub embedder: Arc<dyn Embedder>,
    pub llm_client: Option<Arc<dyn LLMClient>>,
    pub prompt_templates: Arc<PromptTemplateManager>,
}

impl AgentContext {
    pub fn new(
        git_repo: Arc<dyn GitOperations>,
        vector_store: Arc<dyn VectorStore>,
        graph_store: Arc<dyn GraphStore>,
        embedder: Arc<dyn Embedder>,
    ) -> Self {
        Self {
            git_repo,
            vector_store,
            graph_store,
            embedder,
            llm_client: None,
            prompt_templates: Arc::new(PromptTemplateManager::new()),
        }
    }

    pub fn with_llm(mut self, llm: Arc<dyn LLMClient>) -> Self {
        self.llm_client = Some(llm);
        self
    }
}

/// Agent 状态
#[derive(Debug, Clone)]
pub enum AgentStatus {
    Idle,
    Running,
    Completed,
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::agent::AgentTask;
    use crate::llm::client::MockLLMClient;
    use crate::semantic::embedder::MockEmbedder;
    use crate::storage::git::GitRepository;
    use crate::storage::graph_db::InMemoryGraphStore;
    use crate::storage::vector_db::InMemoryVectorStore;

    #[test]
    fn test_agent_context_creation() {
        let context = create_test_context();
        assert!(context.llm_client.is_none());
    }

    #[test]
    fn test_agent_context_with_llm() {
        let git_repo = Arc::new(GitRepository::new("."));
        let vector_store = Arc::new(InMemoryVectorStore::new());
        let graph_store = Arc::new(InMemoryGraphStore::new());
        let embedder = Arc::new(MockEmbedder::default());
        let llm = Arc::new(MockLLMClient::new());

        let context = AgentContext::new(git_repo, vector_store, graph_store, embedder)
            .with_llm(llm);

        assert!(context.llm_client.is_some());
    }

    fn create_test_context() -> AgentContext {
        let git_repo = Arc::new(GitRepository::new("."));
        let vector_store = Arc::new(InMemoryVectorStore::new());
        let graph_store = Arc::new(InMemoryGraphStore::new());
        let embedder = Arc::new(MockEmbedder::default());

        AgentContext::new(git_repo, vector_store, graph_store, embedder)
    }
}
