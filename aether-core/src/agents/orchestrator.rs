use crate::agents::base::{Agent, AgentContext, AgentStatus};
use crate::config::types::RollbackConfig;
use crate::domain::agent::{AgentResult, AgentTask, AgentType};
use crate::utils::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Agent 编排器
/// 负责管理所有 Agent，路由任务到对应的 Agent
pub struct AgentOrchestrator {
    agents: HashMap<AgentType, Arc<dyn Agent>>,
}

impl AgentOrchestrator {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// 注册 Agent
    pub fn register(&mut self, agent: Arc<dyn Agent>) {
        self.agents.insert(agent.agent_type(), agent);
    }

    /// 从上下文创建所有默认 Agent 并注册（全部 6 个）
    pub fn with_all_agents(mut self, context: Arc<AgentContext>) -> Self {
        self.register(Arc::new(super::semantic_interpreter::SemanticInterpreterAgent::new(context.clone())));
        self.register(Arc::new(super::recovery::CrossCommitRecoveryAgent::new(context.clone())));
        self.register(Arc::new(super::merge::MergeAgent::new(context.clone())));
        self.register(Arc::new(super::coordinator::MultiAgentCoordinatorAgent::new(context.clone())));
        self.register(Arc::new(super::validation::ValidationRiskAgent::new(context.clone())));
        self.register(Arc::new(super::rollback::RollbackAgent::new(context, RollbackConfig::default())));
        self
    }

    /// 执行任务
    pub async fn execute_task(&self, task: AgentTask) -> Result<AgentResult> {
        let agent_type = self.resolve_agent_type(&task);

        let agent = self.agents.get(&agent_type).ok_or_else(|| {
            crate::utils::AetherError::AgentError(format!(
                "Agent not found for type: {:?}",
                agent_type
            ))
        })?;

        agent.execute(task).await
    }

    /// 根据任务类型解析需要哪个 Agent
    fn resolve_agent_type(&self, task: &AgentTask) -> AgentType {
        match task.task_type.as_str() {
            "analyze_semantic" => AgentType::SemanticInterpreter,
            "recover_commit" => AgentType::CrossCommitRecovery,
            "merge_prs" => AgentType::Merge,
            "coordinate_agents" => AgentType::MultiAgentCoordinator,
            "validate_tag" => AgentType::ValidationRisk,
            "rollback" => AgentType::Rollback,
            _ => AgentType::SemanticInterpreter, // 默认
        }
    }

    /// 获取已注册的 Agent 列表
    pub fn registered_agents(&self) -> Vec<AgentType> {
        self.agents.keys().cloned().collect()
    }
}

impl Default for AgentOrchestrator {
    fn default() -> Self {
        Self::new()
    }
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

    #[test]
    fn test_orchestrator_creation() {
        let orch = AgentOrchestrator::new();
        assert!(orch.registered_agents().is_empty());
    }

    #[test]
    fn test_orchestrator_with_agents() {
        let orch = AgentOrchestrator::new().with_all_agents(test_context());
        let agents = orch.registered_agents();
        assert_eq!(agents.len(), 6);
        assert!(agents.contains(&AgentType::SemanticInterpreter));
        assert!(agents.contains(&AgentType::CrossCommitRecovery));
        assert!(agents.contains(&AgentType::Merge));
        assert!(agents.contains(&AgentType::MultiAgentCoordinator));
        assert!(agents.contains(&AgentType::ValidationRisk));
        assert!(agents.contains(&AgentType::Rollback));
    }

    #[tokio::test]
    async fn test_execute_semantic_task() {
        let orch = AgentOrchestrator::new().with_all_agents(test_context());
        let task = AgentTask::new(
            "analyze_semantic",
            serde_json::json!({
                "diff": "--- a/lib.rs\n+++ b/lib.rs",
                "message": "feat: test"
            }),
        );

        let result = orch.execute_task(task).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_execute_merge_task() {
        let orch = AgentOrchestrator::new().with_all_agents(test_context());
        let task = AgentTask::new(
            "merge_prs",
            serde_json::json!({"prs": []}),
        );

        let result = orch.execute_task(task).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_execute_unknown_task_type() {
        let orch = AgentOrchestrator::new().with_all_agents(test_context());
        let task = AgentTask::new(
            "unknown_task",
            serde_json::json!({}),
        );

        let result = orch.execute_task(task).await.unwrap();
        assert!(result.success); // 回退到 SemanticInterpreter
    }
}
