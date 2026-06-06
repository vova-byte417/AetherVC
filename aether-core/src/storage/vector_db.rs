use crate::domain::commit::{
    CommitMetadata, IndexReport, SearchFilters, SearchResult, VectorStoreStats,
};
use crate::utils::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 向量存储抽象
#[async_trait::async_trait]
pub trait VectorStore: Send + Sync {
    /// 存储 commit 向量
    async fn store_commit(
        &self,
        commit_hash: &str,
        embedding: Vec<f32>,
        metadata: CommitMetadata,
    ) -> Result<()>;

    /// 批量存储
    async fn store_batch(
        &self,
        entries: Vec<(String, Vec<f32>, CommitMetadata)>,
    ) -> Result<()>;

    /// 相似度搜索
    async fn search_similar(
        &self,
        embedding: &[f32],
        limit: usize,
        filters: Option<&SearchFilters>,
    ) -> Result<Vec<SearchResult>>;

    /// 删除向量
    async fn delete(&self, commit_hash: &str) -> Result<()>;

    /// 获取统计信息
    async fn stats(&self) -> Result<VectorStoreStats>;

    /// 总条目数
    async fn count(&self) -> Result<usize>;
}

/// 内存向量存储（用于开发/测试）
pub struct InMemoryVectorStore {
    entries: RwLock<HashMap<String, (Vec<f32>, CommitMetadata)>>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// 计算余弦相似度
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot_product / (norm_a * norm_b)
    }
}

impl Default for InMemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn store_commit(
        &self,
        commit_hash: &str,
        embedding: Vec<f32>,
        metadata: CommitMetadata,
    ) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.insert(commit_hash.to_string(), (embedding, metadata));
        Ok(())
    }

    async fn store_batch(
        &self,
        batch: Vec<(String, Vec<f32>, CommitMetadata)>,
    ) -> Result<()> {
        let mut entries = self.entries.write().await;
        for (hash, embedding, metadata) in batch {
            entries.insert(hash, (embedding, metadata));
        }
        Ok(())
    }

    async fn search_similar(
        &self,
        embedding: &[f32],
        limit: usize,
        filters: Option<&SearchFilters>,
    ) -> Result<Vec<SearchResult>> {
        let entries = self.entries.read().await;

        let mut scored: Vec<(f32, &CommitMetadata)> = entries
            .iter()
            .map(|(_, (e, m))| (Self::cosine_similarity(embedding, e), m))
            .collect();

        // 应用过滤器
        if let Some(f) = filters {
            scored.retain(|(_, m)| {
                // 按分类过滤
                if let Some(ref cats) = f.change_categories {
                    if !cats.iter().any(|c| *c == m.change_category) {
                        return false;
                    }
                }
                // 按风险过滤
                if let Some(ref risks) = f.risk_levels {
                    if !risks.iter().any(|r| *r == m.risk_level) {
                        return false;
                    }
                }
                // 按作者过滤
                if let Some(ref authors) = f.authors {
                    if !authors.iter().any(|a| a == &m.author_name) {
                        return false;
                    }
                }
                // 按模块过滤
                if let Some(ref modules) = f.modules {
                    if !modules
                        .iter()
                        .any(|mod_name| m.affected_modules.iter().any(|am| am == mod_name))
                    {
                        return false;
                    }
                }
                true
            });
        }

        scored.sort_by(|(a, _), (b, _)| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scored
            .into_iter()
            .take(limit)
            .map(|(score, meta)| SearchResult {
                commit_hash: meta.commit_hash.clone(),
                score,
                intent: meta.intent.clone(),
                summary: meta.summary.clone(),
                change_category: meta.change_category.clone(),
                risk_level: meta.risk_level.clone(),
                affected_modules: meta.affected_modules.clone(),
                author_name: meta.author_name.clone(),
                timestamp: meta.timestamp,
            })
            .collect())
    }

    async fn delete(&self, commit_hash: &str) -> Result<()> {
        let mut entries = self.entries.write().await;
        entries.remove(commit_hash);
        Ok(())
    }

    async fn stats(&self) -> Result<VectorStoreStats> {
        let entries = self.entries.read().await;
        Ok(VectorStoreStats {
            total_points: entries.len() as u64,
            indexed_modules: Vec::new(),
            last_indexed_at: None,
        })
    }

    async fn count(&self) -> Result<usize> {
        let entries = self.entries.read().await;
        Ok(entries.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::commit::{ChangeCategory, RiskLevel};

    fn make_metadata(hash: &str, author: &str) -> CommitMetadata {
        CommitMetadata {
            commit_hash: hash.to_string(),
            author_name: author.to_string(),
            timestamp: chrono::Utc::now(),
            change_category: ChangeCategory::Feature,
            risk_level: RiskLevel::Low,
            affected_modules: vec!["core".to_string()],
            intent: "test".to_string(),
            summary: "test commit".to_string(),
            message: "feat: test".to_string(),
        }
    }

    #[tokio::test]
    async fn test_store_and_search() {
        let store = InMemoryVectorStore::new();

        // 存储两个 commit
        store
            .store_commit("abc", vec![1.0, 0.0, 0.0], make_metadata("abc", "alice"))
            .await
            .unwrap();

        store
            .store_commit("def", vec![0.0, 1.0, 0.0], make_metadata("def", "bob"))
            .await
            .unwrap();

        // 搜索最相似的
        let results = store
            .search_similar(&[1.0, 0.0, 0.0], 1, None)
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].commit_hash, "abc");
        assert!(results[0].score > 0.9);
    }

    #[tokio::test]
    async fn test_store_batch() {
        let store = InMemoryVectorStore::new();
        let entries = vec![
            ("a".to_string(), vec![1.0, 0.0], make_metadata("a", "x")),
            ("b".to_string(), vec![0.0, 1.0], make_metadata("b", "y")),
        ];
        store.store_batch(entries).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_delete() {
        let store = InMemoryVectorStore::new();
        store
            .store_commit("abc", vec![1.0, 0.0], make_metadata("abc", "x"))
            .await
            .unwrap();

        store.delete("abc").await.unwrap();
        assert_eq!(store.count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_filter_search() {
        let store = InMemoryVectorStore::new();

        let mut meta = make_metadata("abc", "alice");
        meta.change_category = ChangeCategory::Bugfix;
        store.store_commit("abc", vec![1.0, 0.0], meta).await.unwrap();

        let meta2 = make_metadata("def", "bob");
        store.store_commit("def", vec![0.9, 0.1], meta2).await.unwrap();

        // 只搜索 Bugfix
        let filters = SearchFilters {
            change_categories: Some(vec![ChangeCategory::Bugfix]),
            ..Default::default()
        };

        let results = store
            .search_similar(&[1.0, 0.0], 5, Some(&filters))
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].commit_hash, "abc");
    }
}
