//! 集成测试模块

use aether_core::agents::orchestrator::AgentOrchestrator;
use aether_core::domain::agent::AgentTask;
use aether_core::domain::commit::{
    CurrentState,
};
use aether_core::domain::recovery::RecoveryRequest;
use aether_core::llm::client::MockLLMClient;
use aether_core::nlp::executor::CommandExecutor;
use aether_core::nlp::parser::NaturalLanguageParser;
use aether_core::semantic::embedder::MockEmbedder;
use aether_core::semantic::indexer::SemanticIndexer;
use aether_core::storage::git::{GitOperations, GitRepository};
use aether_core::storage::graph_db::InMemoryGraphStore;
use aether_core::storage::vector_db::InMemoryVectorStore;
use std::sync::Arc;

fn create_test_orchestrator() -> (Arc<AgentOrchestrator>, Arc<SemanticIndexer>) {
    let git_repo: Arc<dyn GitOperations> = Arc::new(GitRepository::new("."));
    let vector_store = Arc::new(InMemoryVectorStore::new());
    let graph_store = Arc::new(InMemoryGraphStore::new());
    let embedder = Arc::new(MockEmbedder::default());
    let llm_client = Arc::new(MockLLMClient::new());

    let agent_context = Arc::new(
        aether_core::agents::base::AgentContext::new(
            git_repo.clone(),
            vector_store.clone(),
            graph_store,
            embedder.clone(),
        )
        .with_llm(llm_client),
    );

    let orchestrator = Arc::new(AgentOrchestrator::new().with_all_agents(agent_context));
    let indexer = Arc::new(SemanticIndexer::new(embedder, vector_store, git_repo));

    (orchestrator, indexer)
}

#[tokio::test]
async fn test_full_recovery_workflow() {
    let (orchestrator, _indexer) = create_test_orchestrator();
    let executor = CommandExecutor::new(orchestrator, ".".to_string());

    let parser = NaturalLanguageParser::new();
    let parsed = parser.parse("把上周agent实现的用户画像分析模块恢复回来");

    assert_eq!(
        parsed.command_type,
        aether_core::nlp::parser::CommandType::Recovery
    );

    let state = CurrentState::new("main", "abc123");
    let result = executor.execute(&parsed, &state).await;

    // 在没有真实索引数据时，recovery 可能失败，但不应该 panic
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_full_merge_workflow() {
    let (orchestrator, _indexer) = create_test_orchestrator();
    let executor = CommandExecutor::new(orchestrator, ".".to_string());
    let parser = NaturalLanguageParser::new();

    let parsed = parser.parse("合并这些PR进行分析");
    let state = CurrentState::new("main", "HEAD");
    let result = executor.execute(&parsed, &state).await.unwrap();

    assert!(result.success);
}

#[tokio::test]
async fn test_full_search_workflow() {
    let (_orchestrator, indexer) = create_test_orchestrator();
    let parser = NaturalLanguageParser::new();

    let parsed = parser.parse("搜索认证相关的提交");
    assert_eq!(
        parsed.command_type,
        aether_core::nlp::parser::CommandType::Search
    );

    let results = indexer.search("authentication", 10, None).await.unwrap();
    // 空索引不应崩溃
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_orchestrator_all_agents_registered() {
    let (orchestrator, _) = create_test_orchestrator();
    let agents = orchestrator.registered_agents();
    assert_eq!(agents.len(), 6);

    use aether_core::domain::agent::AgentType;
    assert!(agents.contains(&AgentType::SemanticInterpreter));
    assert!(agents.contains(&AgentType::CrossCommitRecovery));
    assert!(agents.contains(&AgentType::Merge));
    assert!(agents.contains(&AgentType::MultiAgentCoordinator));
    assert!(agents.contains(&AgentType::ValidationRisk));
    assert!(agents.contains(&AgentType::Rollback));
}

#[tokio::test]
async fn test_nl_parser_all_command_types() {
    let parser = NaturalLanguageParser::new();

    let test_cases = vec![
        ("恢复用户模块", aether_core::nlp::parser::CommandType::Recovery),
        ("合并PR", aether_core::nlp::parser::CommandType::Merge),
        ("部署到测试环境", aether_core::nlp::parser::CommandType::Deploy),
        ("回滚上次提交", aether_core::nlp::parser::CommandType::Rollback),
        ("搜索认证", aether_core::nlp::parser::CommandType::Search),
        ("分析代码质量", aether_core::nlp::parser::CommandType::Analyze),
        ("查询最近提交", aether_core::nlp::parser::CommandType::Query),
    ];

    for (input, expected) in test_cases {
        let result = parser.parse(input);
        assert_eq!(
            result.command_type, expected,
            "Failed for input: '{}'", input
        );
    }
}
