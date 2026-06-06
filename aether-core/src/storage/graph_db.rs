use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 图存储抽象
#[async_trait::async_trait]
pub trait GraphStore: Send + Sync {
    /// 添加 commit 节点
    async fn add_commit_node(&self, hash: &str, data: serde_json::Value) -> crate::utils::Result<()>;

    /// 添加依赖关系
    async fn add_dependency(&self, from: &str, to: &str, relation: &str) -> crate::utils::Result<()>;

    /// 删除节点及其所有关联边
    async fn remove_node(&self, hash: &str) -> crate::utils::Result<()>;

    /// 查询影响范围
    async fn get_affected(&self, hash: &str) -> crate::utils::Result<Vec<String>>;

    /// 查询依赖
    async fn get_dependencies(&self, hash: &str) -> crate::utils::Result<Vec<String>>;

    /// 获取所有节点 ID
    async fn all_nodes(&self) -> crate::utils::Result<Vec<String>>;
}

/// 内存图存储（MVP 实现）
pub struct InMemoryGraphStore {
    // 邻接表: node -> [(neighbor, relation)]
    adjacent: RwLock<HashMap<String, Vec<(String, String)>>>,
    // 节点数据
    nodes: RwLock<HashMap<String, serde_json::Value>>,
}

impl InMemoryGraphStore {
    pub fn new() -> Self {
        Self {
            adjacent: RwLock::new(HashMap::new()),
            nodes: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryGraphStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl GraphStore for InMemoryGraphStore {
    async fn add_commit_node(&self, hash: &str, data: serde_json::Value) -> crate::utils::Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(hash.to_string(), data);

        let mut adj = self.adjacent.write().await;
        adj.entry(hash.to_string()).or_default();

        Ok(())
    }

    async fn add_dependency(&self, from: &str, to: &str, relation: &str) -> crate::utils::Result<()> {
        let mut adj = self.adjacent.write().await;
        adj.entry(from.to_string())
            .or_default()
            .push((to.to_string(), relation.to_string()));
        Ok(())
    }

    async fn remove_node(&self, hash: &str) -> crate::utils::Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.remove(hash);
        let mut adj = self.adjacent.write().await;
        adj.remove(hash);
        Ok(())
    }

    async fn get_affected(&self, hash: &str) -> crate::utils::Result<Vec<String>> {
        let adj = self.adjacent.read().await;
        let mut affected = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![hash.to_string()];

        while let Some(node) = queue.pop() {
            if visited.contains(&node) {
                continue;
            }
            visited.insert(node.clone());

            if let Some(neighbors) = adj.get(&node) {
                for (neighbor, _) in neighbors {
                    if !visited.contains(neighbor) {
                        queue.push(neighbor.clone());
                        affected.push(neighbor.clone());
                    }
                }
            }
        }

        Ok(affected)
    }

    async fn get_dependencies(&self, hash: &str) -> crate::utils::Result<Vec<String>> {
        let adj = self.adjacent.read().await;
        if let Some(neighbors) = adj.get(hash) {
            Ok(neighbors.iter().map(|(n, _)| n.clone()).collect())
        } else {
            Ok(Vec::new())
        }
    }

    async fn all_nodes(&self) -> crate::utils::Result<Vec<String>> {
        let nodes = self.nodes.read().await;
        Ok(nodes.keys().cloned().collect())
    }
}

// ─── 持久化图存储 ───

/// 持久化图存储序列化格式
#[derive(serde::Serialize, serde::Deserialize)]
struct PersistentGraphData {
    nodes: HashMap<String, serde_json::Value>,
    /// adjacency: from -> [(to, relation), ...]
    edges: HashMap<String, Vec<(String, String)>>,
}

/// 文件系统持久化的图存储
///
/// 数据存储在 `.aether/graph/graph.json` 文件中。
/// 内存中维护缓存以加速查询。
pub struct PersistentGraphStore {
    /// 数据文件路径
    file_path: PathBuf,
    /// 内存缓存
    adjacent: RwLock<HashMap<String, Vec<(String, String)>>>,
    nodes: RwLock<HashMap<String, serde_json::Value>>,
}

impl PersistentGraphStore {
    /// 创建持久化图存储
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            file_path: path.into(),
            adjacent: RwLock::new(HashMap::new()),
            nodes: RwLock::new(HashMap::new()),
        }
    }

    /// 从磁盘加载数据
    async fn load(&self) -> crate::utils::Result<()> {
        if !self.file_path.exists() {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.file_path).await.map_err(|e| {
            crate::utils::AetherError::Storage(format!(
                "读取图数据文件 {} 失败: {}",
                self.file_path.display(),
                e
            ))
        })?;

        let data: PersistentGraphData = serde_json::from_str(&content).map_err(|e| {
            crate::utils::AetherError::Storage(format!("解析图数据文件失败: {}", e))
        })?;

        let mut nodes = self.nodes.write().await;
        *nodes = data.nodes;

        let mut adj = self.adjacent.write().await;
        *adj = data.edges;

        Ok(())
    }

    /// 保存数据到磁盘
    async fn save(&self) -> crate::utils::Result<()> {
        // 确保父目录存在
        if let Some(parent) = self.file_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                crate::utils::AetherError::Storage(format!(
                    "无法创建图存储目录 {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        let nodes = self.nodes.read().await;
        let adj = self.adjacent.read().await;

        let data = PersistentGraphData {
            nodes: nodes.clone(),
            edges: adj.clone(),
        };

        let json = serde_json::to_string_pretty(&data).map_err(|e| {
            crate::utils::AetherError::Storage(format!("序列化图数据失败: {}", e))
        })?;

        tokio::fs::write(&self.file_path, json).await.map_err(|e| {
            crate::utils::AetherError::Storage(format!("写入图数据文件失败: {}", e))
        })?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl GraphStore for PersistentGraphStore {
    async fn add_commit_node(&self, hash: &str, data: serde_json::Value) -> crate::utils::Result<()> {
        // 如果文件存在，先加载
        if self.nodes.read().await.is_empty() && self.file_path.exists() {
            self.load().await?;
        }

        {
            let mut nodes = self.nodes.write().await;
            nodes.insert(hash.to_string(), data);
        }
        {
            let mut adj = self.adjacent.write().await;
            adj.entry(hash.to_string()).or_default();
        }

        self.save().await
    }

    async fn add_dependency(&self, from: &str, to: &str, relation: &str) -> crate::utils::Result<()> {
        if self.adjacent.read().await.is_empty() && self.file_path.exists() {
            self.load().await?;
        }

        {
            let mut adj = self.adjacent.write().await;
            adj.entry(from.to_string())
                .or_default()
                .push((to.to_string(), relation.to_string()));
        }

        self.save().await
    }

    async fn remove_node(&self, hash: &str) -> crate::utils::Result<()> {
        if self.nodes.read().await.is_empty() && self.file_path.exists() {
            self.load().await?;
        }

        {
            let mut nodes = self.nodes.write().await;
            nodes.remove(hash);
        }
        {
            let mut adj = self.adjacent.write().await;
            adj.remove(hash);
        }

        self.save().await
    }

    async fn get_affected(&self, hash: &str) -> crate::utils::Result<Vec<String>> {
        if self.adjacent.read().await.is_empty() && self.file_path.exists() {
            self.load().await?;
        }

        let adj = self.adjacent.read().await;
        let mut affected = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![hash.to_string()];

        while let Some(node) = queue.pop() {
            if visited.contains(&node) {
                continue;
            }
            visited.insert(node.clone());

            if let Some(neighbors) = adj.get(&node) {
                for (neighbor, _) in neighbors {
                    if !visited.contains(neighbor) {
                        queue.push(neighbor.clone());
                        affected.push(neighbor.clone());
                    }
                }
            }
        }

        Ok(affected)
    }

    async fn get_dependencies(&self, hash: &str) -> crate::utils::Result<Vec<String>> {
        if self.adjacent.read().await.is_empty() && self.file_path.exists() {
            self.load().await?;
        }

        let adj = self.adjacent.read().await;
        if let Some(neighbors) = adj.get(hash) {
            Ok(neighbors.iter().map(|(n, _)| n.clone()).collect())
        } else {
            Ok(Vec::new())
        }
    }

    async fn all_nodes(&self) -> crate::utils::Result<Vec<String>> {
        if self.nodes.read().await.is_empty() && self.file_path.exists() {
            self.load().await?;
        }

        let nodes = self.nodes.read().await;
        Ok(nodes.keys().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_and_query() {
        let store = InMemoryGraphStore::new();

        store
            .add_commit_node("a", serde_json::json!({"msg": "commit a"}))
            .await
            .unwrap();
        store
            .add_commit_node("b", serde_json::json!({"msg": "commit b"}))
            .await
            .unwrap();
        store
            .add_dependency("a", "b", "parent")
            .await
            .unwrap();

        let deps = store.get_dependencies("a").await.unwrap();
        assert_eq!(deps, vec!["b"]);

        let affected = store.get_affected("a").await.unwrap();
        assert_eq!(affected, vec!["b"]);
    }
}
