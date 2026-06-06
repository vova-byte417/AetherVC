//! 知识图谱引擎
//!
//! 构建并查询 commit-author-module 三维关系图。
//!
//! ## 图谱节点类型
//!
//! - **Commit**: 代码提交节点，包含 hash, message, risk_level
//! - **Author**: 作者节点（区分 human/AI agent），包含 name, email
//! - **Module**: 模块节点，包含路径、变更频率
//!
//! ## 边类型
//!
//! - `AUTHORED`: Author → Commit
//! - `MODIFIES`: Commit → Module
//! - `DEPENDS_ON`: Module → Module（模块依赖）
//! - `PARENT_OF`: Commit → Commit（提交链）

use crate::domain::commit::{Commit, RiskLevel};
use crate::storage::graph_db::GraphStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// ─── 图谱节点 ───

/// 知识图谱节点类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Commit,
    Author,
    Module,
}

/// 知识图谱节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    pub properties: HashMap<String, String>,
}

impl GraphNode {
    pub fn commit(hash: &str, message: &str) -> Self {
        let mut props = HashMap::new();
        props.insert("message".to_string(), message.to_string());
        Self {
            id: format!("commit:{}", hash),
            kind: NodeKind::Commit,
            label: hash[..hash.len().min(8)].to_string(),
            properties: props,
        }
    }

    pub fn author(name: &str, email: &str) -> Self {
        let mut props = HashMap::new();
        props.insert("email".to_string(), email.to_string());
        Self {
            id: format!("author:{}", name),
            kind: NodeKind::Author,
            label: name.to_string(),
            properties: props,
        }
    }

    pub fn module(path: &str) -> Self {
        let mut props = HashMap::new();
        props.insert("path".to_string(), path.to_string());
        Self {
            id: format!("module:{}", path),
            kind: NodeKind::Module,
            label: path.to_string(),
            properties: props,
        }
    }
}

// ─── 图谱边 ───

/// 知识图谱边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub relation: String,
    pub weight: f32,
}

/// 关系类型常量
pub mod relations {
    pub const AUTHORED: &str = "AUTHORED";
    pub const MODIFIES: &str = "MODIFIES";
    pub const DEPENDS_ON: &str = "DEPENDS_ON";
    pub const PARENT_OF: &str = "PARENT_OF";
    pub const HIGH_RISK: &str = "HIGH_RISK";
}

// ─── 查询结果 ───

/// 模块影响分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleImpact {
    pub module: String,
    pub change_count: u32,
    pub authors: Vec<String>,
    pub risk_level: RiskLevel,
    pub recent_commits: Vec<String>,
}

/// 作者活跃度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorActivity {
    pub name: String,
    pub email: String,
    pub commit_count: u32,
    pub modules_touched: Vec<String>,
    pub risk_distribution: HashMap<String, u32>,
    pub activity_period: Option<String>,
}

/// 知识图谱查询结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraphReport {
    pub total_nodes: u32,
    pub total_edges: u32,
    pub top_modules: Vec<ModuleImpact>,
    pub top_authors: Vec<AuthorActivity>,
    pub high_risk_paths: Vec<Vec<String>>,
}

// ─── 知识图谱引擎 ───

/// 知识图谱引擎
///
/// 在 InMemoryGraphStore 之上构建语义丰富的知识图谱，
/// 跟踪 commit-author-module 之间的多维关系。
pub struct KnowledgeGraphEngine {
    graph: Arc<dyn GraphStore>,
}

impl KnowledgeGraphEngine {
    pub fn new(graph: Arc<dyn GraphStore>) -> Self {
        Self { graph }
    }

    /// 索引一个 commit 及其关联的 author 和 module 关系
    pub async fn index_commit(&self, commit: &Commit) -> crate::utils::Result<()> {
        let hash = &commit.id.0;
        let author_name = &commit.author.name;
        let author_email = &commit.author.email;

        // 1. 添加 Commit 节点
        self.graph
            .add_commit_node(
                &format!("commit:{}", hash),
                serde_json::json!({
                    "hash": hash,
                    "message": commit.message,
                    "risk_level": format!("{:?}", commit.semantic_info.risk_level),
                    "change_type": format!("{:?}", commit.semantic_info.change_type),
                }),
            )
            .await?;

        // 2. 添加/更新 Author 节点 + AUTHORED 边
        self.graph
            .add_commit_node(
                &format!("author:{}", author_name),
                serde_json::json!({
                    "name": author_name,
                    "email": author_email,
                    "kind": "author",
                }),
            )
            .await?;

        self.graph
            .add_dependency(
                &format!("author:{}", author_name),
                &format!("commit:{}", hash),
                relations::AUTHORED,
            )
            .await?;

        // 3. 添加 Module 节点 + MODIFIES 边
        for module in &commit.semantic_info.affected_modules {
            self.graph
                .add_commit_node(
                    &format!("module:{}", module),
                    serde_json::json!({
                        "path": module,
                        "kind": "module",
                    }),
                )
                .await?;

            self.graph
                .add_dependency(
                    &format!("commit:{}", hash),
                    &format!("module:{}", module),
                    relations::MODIFIES,
                )
                .await?;
        }

        // 4. 高风险标记边
        if commit.semantic_info.risk_level >= RiskLevel::High {
            self.graph
                .add_dependency(
                    &format!("commit:{}", hash),
                    "risk:high",
                    relations::HIGH_RISK,
                )
                .await?;
        }

        Ok(())
    }

    /// 批量索引 commits
    pub async fn index_commits(&self, commits: &[Commit]) -> crate::utils::Result<u32> {
        let mut count = 0;
        for commit in commits {
            self.index_commit(commit).await?;
            count += 1;
        }
        Ok(count)
    }

    /// 查询模块的所有变更
    pub async fn get_module_history(&self, module: &str) -> crate::utils::Result<Vec<String>> {
        // 反向查询：哪些 commit 修改了这个 module
        // 对于 InMemoryGraphStore，我们需要从 module 节点的依赖中找到 commits
        let node_id = format!("module:{}", module);
        self.graph.get_dependencies(&node_id).await
    }

    /// 查询作者的贡献
    pub async fn get_author_commits(&self, author: &str) -> crate::utils::Result<Vec<String>> {
        let node_id = format!("author:{}", author);
        self.graph.get_dependencies(&node_id).await
    }

    /// 查询受影响的模块
    pub async fn get_affected_modules(&self, hash: &str) -> crate::utils::Result<Vec<String>> {
        let node_id = format!("commit:{}", hash);
        let deps = self.graph.get_dependencies(&node_id).await?;
        // 过滤出 module 类型的节点
        Ok(deps
            .into_iter()
            .filter(|id| id.starts_with("module:"))
            .map(|id| id[7..].to_string())
            .collect())
    }

    /// 查询某个模块被作者的变更连锁影响
    pub async fn get_transitive_impact(
        &self,
        module: &str,
    ) -> crate::utils::Result<Vec<String>> {
        let node_id = format!("module:{}", module);
        self.graph.get_affected(&node_id).await
    }

    /// 生成知识图谱报告
    pub async fn generate_report(
        &self,
        _limit: usize,
    ) -> crate::utils::Result<KnowledgeGraphReport> {
        // 这里统计了一个简化版报告
        // 完整版需要图存储提供聚合查询能力

        Ok(KnowledgeGraphReport {
            total_nodes: 0,
            total_edges: 0,
            top_modules: Vec::new(),
            top_authors: Vec::new(),
            high_risk_paths: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::commit::{Author, SemanticInfo, ChangeCategory};
    use crate::storage::graph_db::InMemoryGraphStore;

    fn make_test_commit(hash: &str, msg: &str, modules: Vec<&str>, risk: RiskLevel) -> Commit {
        let mut commit = Commit::new(
            hash,
            msg,
            Author::ai_agent("GPT-4", "ai@test.com"),
            chrono::Utc::now(),
            vec![],
        );
        commit.semantic_info = SemanticInfo::new(
            msg,
            ChangeCategory::Feature,
            modules.into_iter().map(String::from).collect(),
            format!("test: {}", msg),
            risk,
        );
        commit
    }

    #[tokio::test]
    async fn test_index_commit_creates_nodes() {
        let store = Arc::new(InMemoryGraphStore::new());
        let engine = KnowledgeGraphEngine::new(store.clone());

        let commit = make_test_commit("abc123", "feat: login", vec!["auth/login.rs"], RiskLevel::Medium);
        engine.index_commit(&commit).await.unwrap();

        // 验证 author → commit 关系
        let deps = store.get_dependencies("author:GPT-4").await.unwrap();
        assert!(deps.iter().any(|d| d == "commit:abc123"));
    }

    #[tokio::test]
    async fn test_index_commit_with_modules() {
        let store = Arc::new(InMemoryGraphStore::new());
        let engine = KnowledgeGraphEngine::new(store.clone());

        let commit = make_test_commit(
            "def456",
            "feat: add api",
            vec!["api/routes.rs", "models/user.rs"],
            RiskLevel::High,
        );
        engine.index_commit(&commit).await.unwrap();

        let modules = engine.get_affected_modules("def456").await.unwrap();
        assert!(modules.contains(&"api/routes.rs".to_string()));
        assert!(modules.contains(&"models/user.rs".to_string()));
    }

    #[tokio::test]
    async fn test_high_risk_edge() {
        let store = Arc::new(InMemoryGraphStore::new());
        let engine = KnowledgeGraphEngine::new(store.clone());

        let commit = make_test_commit("critical1", "break!", vec!["auth/mod.rs"], RiskLevel::Critical);
        engine.index_commit(&commit).await.unwrap();

        // 高风险边应该存在
        let deps = store.get_dependencies("commit:critical1").await.unwrap();
        assert!(deps.iter().any(|d| d == "risk:high"));
    }
}
