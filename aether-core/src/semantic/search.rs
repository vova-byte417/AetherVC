use crate::domain::commit::{SearchFilters, SearchResult};
use crate::semantic::embedder::Embedder;
use crate::storage::vector_db::VectorStore;
use crate::utils::Result;
use std::sync::Arc;

/// 语义搜索引擎
pub struct SemanticSearchEngine {
    embedder: Arc<dyn Embedder>,
    vector_store: Arc<dyn VectorStore>,
}

impl SemanticSearchEngine {
    pub fn new(embedder: Arc<dyn Embedder>, vector_store: Arc<dyn VectorStore>) -> Self {
        Self {
            embedder,
            vector_store,
        }
    }

    /// 执行语义搜索
    pub async fn search(
        &self,
        query: &str,
        limit: usize,
        filters: Option<&SearchFilters>,
    ) -> Result<Vec<SearchResult>> {
        let embedding = self.embedder.embed(query).await?;
        self.vector_store
            .search_similar(&embedding, limit, filters)
            .await
    }

    /// 按提交哈希精确搜索
    pub async fn search_by_hash(&self, hash: &str) -> Result<Option<SearchResult>> {
        // 使用 hash 作为查询
        let embedding = self.embedder.embed(hash).await?;
        let results = self
            .vector_store
            .search_similar(&embedding, 10, None)
            .await?;

        Ok(results.into_iter().find(|r| r.commit_hash == hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::commit::{
        ChangeCategory, CommitMetadata, RiskLevel, SearchFilters,
    };
    use crate::semantic::embedder::MockEmbedder;
    use crate::storage::vector_db::InMemoryVectorStore;

    #[tokio::test]
    async fn test_search_with_filters() {
        let embedder = Arc::new(MockEmbedder::default());
        let vector_store = Arc::new(InMemoryVectorStore::new());

        // 添加测试数据
        let meta = CommitMetadata {
            commit_hash: "hash1".to_string(),
            author_name: "alice".to_string(),
            timestamp: chrono::Utc::now(),
            change_category: ChangeCategory::Bugfix,
            risk_level: RiskLevel::High,
            affected_modules: vec!["auth".to_string()],
            intent: "fix login bug".to_string(),
            summary: "fix login bug".to_string(),
            message: "fix: login bug".to_string(),
        };
        vector_store
            .store_commit("hash1", vec![1.0, 0.0], meta)
            .await
            .unwrap();

        let meta2 = CommitMetadata {
            commit_hash: "hash2".to_string(),
            author_name: "bob".to_string(),
            timestamp: chrono::Utc::now(),
            change_category: ChangeCategory::Feature,
            risk_level: RiskLevel::Low,
            affected_modules: vec!["core".to_string()],
            intent: "add new feature".to_string(),
            summary: "add new feature".to_string(),
            message: "feat: new feature".to_string(),
        };
        vector_store
            .store_commit("hash2", vec![0.0, 1.0], meta2)
            .await
            .unwrap();

        let engine = SemanticSearchEngine::new(embedder, vector_store);

        // 无过滤器搜索
        let results = engine.search("test", 10, None).await.unwrap();
        assert_eq!(results.len(), 2);

        // 只搜索高风险
        let filters = SearchFilters {
            risk_levels: Some(vec![RiskLevel::High]),
            ..Default::default()
        };
        let results = engine
            .search("test", 10, Some(&filters))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].commit_hash, "hash1");
    }
}
