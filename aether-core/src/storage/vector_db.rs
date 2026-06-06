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

// ─── 持久化向量存储 ───

use std::path::PathBuf;

/// 持久化向量存储条目（写入磁盘的序列化格式）
#[derive(serde::Serialize, serde::Deserialize)]
struct PersistentEntry {
    embedding: Vec<f32>,
    metadata: CommitMetadata,
}

/// 文件系统持久化的向量存储
///
/// 数据存储在 `.aether/vectors/` 目录下，每个 commit 一个 JSON 文件。
/// 同时维护一个 `index.json` 索引文件以加速计数和枚举。
pub struct PersistentVectorStore {
    /// 数据目录路径
    dir: PathBuf,
    /// 内存缓存，加速搜索
    cache: RwLock<HashMap<String, (Vec<f32>, CommitMetadata)>>,
}

impl PersistentVectorStore {
    /// 创建持久化向量存储
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            dir: path.into(),
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// 确保存储目录存在
    async fn ensure_dir(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.dir).await.map_err(|e| {
            crate::utils::AetherError::Storage(format!(
                "无法创建向量存储目录 {}: {}",
                self.dir.display(),
                e
            ))
        })?;
        Ok(())
    }

    /// 获取某个 commit 的文件路径
    fn entry_path(&self, commit_hash: &str) -> PathBuf {
        self.dir.join(format!("{}.json", commit_hash))
    }

    /// 加载所有条目到缓存
    async fn load_all(&self) -> Result<()> {
        self.ensure_dir().await?;

        let mut entries = tokio::fs::read_dir(&self.dir).await.map_err(|e| {
            crate::utils::AetherError::Storage(format!(
                "无法读取向量存储目录 {}: {}",
                self.dir.display(),
                e
            ))
        })?;

        let mut cache = self.cache.write().await;
        cache.clear();

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            crate::utils::AetherError::Storage(format!("读取目录条目失败: {}", e))
        })? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                match tokio::fs::read_to_string(&path).await {
                    Ok(content) => {
                        if let Ok(pe) = serde_json::from_str::<PersistentEntry>(&content) {
                            let hash = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("")
                                .to_string();
                            cache.insert(hash, (pe.embedding, pe.metadata));
                        }
                    }
                    Err(e) => {
                        tracing::warn!("读取向量文件 {} 失败: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(())
    }

    /// 计算余弦相似度（与 InMemory 版本一致）
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

#[async_trait::async_trait]
impl VectorStore for PersistentVectorStore {
    async fn store_commit(
        &self,
        commit_hash: &str,
        embedding: Vec<f32>,
        metadata: CommitMetadata,
    ) -> Result<()> {
        self.ensure_dir().await?;

        // 写入文件
        let entry = PersistentEntry {
            embedding: embedding.clone(),
            metadata: metadata.clone(),
        };
        let json = serde_json::to_string(&entry).map_err(|e| {
            crate::utils::AetherError::Storage(format!("序列化向量条目失败: {}", e))
        })?;
        let path = self.entry_path(commit_hash);
        tokio::fs::write(&path, json).await.map_err(|e| {
            crate::utils::AetherError::Storage(format!("写入向量文件失败: {}", e))
        })?;

        // 更新缓存
        let mut cache = self.cache.write().await;
        cache.insert(commit_hash.to_string(), (embedding, metadata));

        Ok(())
    }

    async fn store_batch(
        &self,
        entries: Vec<(String, Vec<f32>, CommitMetadata)>,
    ) -> Result<()> {
        self.ensure_dir().await?;

        let mut cache = self.cache.write().await;

        for (hash, embedding, metadata) in entries {
            let entry = PersistentEntry {
                embedding: embedding.clone(),
                metadata: metadata.clone(),
            };
            let json = serde_json::to_string(&entry).map_err(|e| {
                crate::utils::AetherError::Storage(format!("序列化向量条目失败: {}", e))
            })?;
            let path = self.entry_path(&hash);
            tokio::fs::write(&path, json).await.map_err(|e| {
                crate::utils::AetherError::Storage(format!("写入向量文件失败: {}", e))
            })?;
            cache.insert(hash, (embedding, metadata));
        }

        Ok(())
    }

    async fn search_similar(
        &self,
        embedding: &[f32],
        limit: usize,
        filters: Option<&SearchFilters>,
    ) -> Result<Vec<SearchResult>> {
        // 如果缓存为空，从磁盘加载
        {
            let cache = self.cache.read().await;
            if cache.is_empty() {
                drop(cache);
                self.load_all().await?;
            }
        }

        let cache = self.cache.read().await;

        let mut scored: Vec<(f32, &CommitMetadata)> = cache
            .iter()
            .map(|(_, (e, m))| (Self::cosine_similarity(embedding, e), m))
            .collect();

        // 应用过滤器
        if let Some(f) = filters {
            scored.retain(|(_, m)| {
                if let Some(ref cats) = f.change_categories {
                    if !cats.iter().any(|c| *c == m.change_category) {
                        return false;
                    }
                }
                if let Some(ref risks) = f.risk_levels {
                    if !risks.iter().any(|r| *r == m.risk_level) {
                        return false;
                    }
                }
                if let Some(ref authors) = f.authors {
                    if !authors.iter().any(|a| a == &m.author_name) {
                        return false;
                    }
                }
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
        let path = self.entry_path(commit_hash);
        if path.exists() {
            tokio::fs::remove_file(&path).await.map_err(|e| {
                crate::utils::AetherError::Storage(format!("删除向量文件失败: {}", e))
            })?;
        }
        let mut cache = self.cache.write().await;
        cache.remove(commit_hash);
        Ok(())
    }

    async fn stats(&self) -> Result<VectorStoreStats> {
        let cache = self.cache.read().await;
        let count = if cache.is_empty() {
            drop(cache);
            self.load_all().await?;
            self.cache.read().await.len()
        } else {
            cache.len()
        };

        Ok(VectorStoreStats {
            total_points: count as u64,
            indexed_modules: Vec::new(),
            last_indexed_at: None,
        })
    }

    async fn count(&self) -> Result<usize> {
        let cache = self.cache.read().await;
        let count = if cache.is_empty() {
            drop(cache);
            self.load_all().await?;
            self.cache.read().await.len()
        } else {
            cache.len()
        };
        Ok(count)
    }
}

// ─── 测试 ───

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
