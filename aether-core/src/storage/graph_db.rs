use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 图存储抽象
#[async_trait::async_trait]
pub trait GraphStore: Send + Sync {
    /// 添加 commit 节点
    async fn add_commit_node(&self, hash: &str, data: serde_json::Value) -> crate::utils::Result<()>;

    /// 添加依赖关系
    async fn add_dependency(&self, from: &str, to: &str, relation: &str) -> crate::utils::Result<()>;

    /// 查询影响范围
    async fn get_affected(&self, hash: &str) -> crate::utils::Result<Vec<String>>;

    /// 查询依赖
    async fn get_dependencies(&self, hash: &str) -> crate::utils::Result<Vec<String>>;
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
