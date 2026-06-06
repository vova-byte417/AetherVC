use crate::domain::commit::{
    ChangeCategory, Commit, CommitMetadata, IndexReport, SearchFilters, SearchResult, SemanticInfo,
};
use crate::llm::client::LLMClient;
use crate::semantic::analyzer::{RuleBasedAnalyzer, SemanticAnalyzer};
use crate::semantic::embedder::Embedder;
use crate::storage::git::GitOperations;
use crate::storage::vector_db::VectorStore;
use crate::utils::Result;

use std::sync::Arc;

/// 语义索引器 - 负责将 commit 索引到向量数据库
pub struct SemanticIndexer {
    embedder: Arc<dyn Embedder>,
    vector_store: Arc<dyn VectorStore>,
    git_repo: Arc<dyn GitOperations>,
    analyzer: Box<dyn SemanticAnalyzer>,
    llm_client: Option<Arc<dyn LLMClient>>,
}

impl SemanticIndexer {
    pub fn new(
        embedder: Arc<dyn Embedder>,
        vector_store: Arc<dyn VectorStore>,
        git_repo: Arc<dyn GitOperations>,
    ) -> Self {
        Self {
            embedder,
            vector_store,
            git_repo,
            analyzer: Box::new(RuleBasedAnalyzer::new()),
            llm_client: None,
        }
    }

    /// 设置 LLM 客户端（可选，用于增强语义分析）
    pub fn with_llm(mut self, llm_client: Arc<dyn LLMClient>) -> Self {
        self.llm_client = Some(llm_client);
        self
    }

    /// 索引单个 commit
    pub async fn index_commit(&self, commit: &Commit) -> Result<()> {
        // 1. 获取 diff
        let diff = self.git_repo.get_commit_diff(&commit.id.0).await?;

        // 2. 语义分析
        let mut semantic_info = self.analyzer.analyze(&diff, &commit.message).await?;

        // 3. 如果设置了 LLM，增强语义标签
        if let Some(ref llm) = self.llm_client {
            let enhanced = llm.analyze_semantic(&diff, &commit.message).await.ok();
            if let Some(info) = enhanced {
                // 合并 LLM 分析结果（用 LLM 的结果覆盖基本分析）
                if !info.intent.is_empty() {
                    semantic_info.intent = info.intent;
                }
                if !info.suggested_tags.is_empty() {
                    semantic_info.suggested_tags = info.suggested_tags;
                }
                if !info.related_historical_changes.is_empty() {
                    semantic_info.related_historical_changes = info.related_historical_changes;
                }
            }
        }

        // 4. 向量化语义摘要
        let embedding = self.embedder.embed(&semantic_info.semantic_summary).await?;

        // 5. 构建元数据
        let metadata = CommitMetadata {
            commit_hash: commit.id.0.clone(),
            author_name: commit.author.name.clone(),
            timestamp: commit.timestamp,
            change_category: semantic_info.change_type.clone(),
            risk_level: semantic_info.risk_level.clone(),
            affected_modules: semantic_info.affected_modules.clone(),
            intent: semantic_info.intent.clone(),
            summary: semantic_info.semantic_summary.clone(),
            message: commit.message.clone(),
        };

        // 6. 存储到向量数据库
        self.vector_store
            .store_commit(&commit.id.0, embedding, metadata)
            .await?;

        Ok(())
    }

    /// 批量索引所有 commit
    pub async fn index_all_commits(&self) -> Result<IndexReport> {
        let commits = self.git_repo.list_commits().await?;
        let mut report = IndexReport::new();

        for commit in &commits {
            match self.index_commit(commit).await {
                Ok(_) => report.successful += 1,
                Err(e) => {
                    report.failed += 1;
                    report
                        .errors
                        .push(format!("Commit {}: {}", commit.id, e));
                }
            }
        }

        Ok(report)
    }

    /// 语义搜索
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        filters: Option<&SearchFilters>,
    ) -> Result<Vec<SearchResult>> {
        let query_embedding = self.embedder.embed(query).await?;
        self.vector_store
            .search_similar(&query_embedding, limit, filters)
            .await
    }

    /// 获取索引统计
    pub async fn stats(&self) -> Result<crate::domain::commit::VectorStoreStats> {
        self.vector_store.stats().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::commit::{Author, Commit};
    use crate::semantic::embedder::MockEmbedder;
    use crate::storage::vector_db::InMemoryVectorStore;
    use tempfile::TempDir;

    /// 创建一个模拟的 GitOperations
    struct MockGitRepo;

    #[async_trait::async_trait]
    impl GitOperations for MockGitRepo {
        async fn list_commits(&self) -> Result<Vec<Commit>> {
            let commit = Commit::new(
                "abc123",
                "feat: add login module",
                Author::new("test", "test@test.com"),
                chrono::Utc::now(),
                vec![],
            );
            Ok(vec![commit])
        }

        async fn get_commit(&self, _hash: &str) -> Result<Option<Commit>> {
            Ok(None)
        }

        async fn get_commit_diff(&self, _hash: &str) -> Result<String> {
            Ok("diff --git a/auth/login.rs b/auth/login.rs\n+pub fn login() {}".to_string())
        }

        async fn current_state(&self) -> Result<crate::domain::commit::CurrentState> {
            Ok(crate::domain::commit::CurrentState::new("main", "abc123"))
        }

        fn repo_path(&self) -> &std::path::Path {
            std::path::Path::new(".")
        }
    }

    #[tokio::test]
    async fn test_indexer_single_commit() {
        let embedder = Arc::new(MockEmbedder::default());
        let vector_store = Arc::new(InMemoryVectorStore::new());
        let git_repo = Arc::new(MockGitRepo);

        let indexer = SemanticIndexer::new(embedder, vector_store.clone(), git_repo);
        let commits = indexer.git_repo.list_commits().await.unwrap();

        indexer.index_commit(&commits[0]).await.unwrap();

        let count = vector_store.count().await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_indexer_batch() {
        let embedder = Arc::new(MockEmbedder::default());
        let vector_store = Arc::new(InMemoryVectorStore::new());
        let git_repo = Arc::new(MockGitRepo);

        let indexer = SemanticIndexer::new(embedder, vector_store, git_repo);
        let report = indexer.index_all_commits().await.unwrap();

        assert_eq!(report.successful, 1);
        assert_eq!(report.failed, 0);
    }

    #[tokio::test]
    async fn test_semantic_search() {
        let embedder = Arc::new(MockEmbedder::default());
        let vector_store = Arc::new(InMemoryVectorStore::new());
        let git_repo = Arc::new(MockGitRepo);

        let indexer = SemanticIndexer::new(embedder, vector_store.clone(), git_repo);

        // 先索引
        indexer.index_all_commits().await.unwrap();

        // 搜索
        let results = indexer.search("login", 5, None).await.unwrap();
        assert!(!results.is_empty());

        // 相似度搜索应该能找到相关结果
        let found = results.iter().any(|r| r.commit_hash == "abc123");
        assert!(found);
    }
}
