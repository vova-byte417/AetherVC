use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommitId(pub String);

impl CommitId {
    pub fn new(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }
}

impl std::fmt::Display for CommitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub id: CommitId,
    pub message: String,
    pub author: Author,
    pub timestamp: DateTime<Utc>,
    pub parent_hashes: Vec<String>,
    pub semantic_info: SemanticInfo,
    pub files: Vec<FileChange>,
}

impl Commit {
    pub fn new(
        hash: impl Into<String>,
        message: impl Into<String>,
        author: Author,
        timestamp: DateTime<Utc>,
        parent_hashes: Vec<String>,
    ) -> Self {
        Self {
            id: CommitId::new(hash),
            message: message.into(),
            author,
            timestamp,
            parent_hashes,
            semantic_info: SemanticInfo::default(),
            files: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: String,
    pub is_ai_agent: bool,
    pub agent_id: Option<String>,
}

impl Author {
    pub fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            email: email.into(),
            is_ai_agent: false,
            agent_id: None,
        }
    }

    pub fn ai_agent(name: impl Into<String>, agent_id: impl Into<String>) -> Self {
        let id: String = agent_id.into();
        Self {
            name: name.into(),
            email: format!("{}@aether.ai", id),
            is_ai_agent: true,
            agent_id: Some(id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub change_type: ChangeType,
    pub additions: u32,
    pub deletions: u32,
    pub diff: String,
}

impl FileChange {
    pub fn new(path: impl Into<String>, change_type: ChangeType, diff: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            change_type,
            additions: 0,
            deletions: 0,
            diff: diff.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticInfo {
    pub intent: String,
    pub change_type: ChangeCategory,
    pub affected_modules: Vec<String>,
    pub semantic_summary: String,
    pub risk_level: RiskLevel,
    pub suggested_tags: Vec<String>,
    pub related_historical_changes: Vec<String>,
    /// 向量表示（维度取决于 embedder）
    pub embedding: Option<Vec<f32>>,
}

impl Default for SemanticInfo {
    fn default() -> Self {
        Self {
            intent: String::new(),
            change_type: ChangeCategory::Feature,
            affected_modules: Vec::new(),
            semantic_summary: String::new(),
            risk_level: RiskLevel::Low,
            suggested_tags: Vec::new(),
            related_historical_changes: Vec::new(),
            embedding: None,
        }
    }
}

impl SemanticInfo {
    pub fn new(
        intent: impl Into<String>,
        change_type: ChangeCategory,
        affected_modules: Vec<String>,
        semantic_summary: impl Into<String>,
        risk_level: RiskLevel,
    ) -> Self {
        Self {
            intent: intent.into(),
            change_type,
            affected_modules,
            semantic_summary: semantic_summary.into(),
            risk_level,
            suggested_tags: Vec::new(),
            related_historical_changes: Vec::new(),
            embedding: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeCategory {
    Feature,
    Refactor,
    Bugfix,
    Performance,
    Breaking,
    Documentation,
    Test,
}

impl std::fmt::Display for ChangeCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeCategory::Feature => write!(f, "feature"),
            ChangeCategory::Refactor => write!(f, "refactor"),
            ChangeCategory::Bugfix => write!(f, "bugfix"),
            ChangeCategory::Performance => write!(f, "performance"),
            ChangeCategory::Breaking => write!(f, "breaking"),
            ChangeCategory::Documentation => write!(f, "documentation"),
            ChangeCategory::Test => write!(f, "test"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn as_str(&self) -> &str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "critical",
        }
    }
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// --- 搜索相关类型 ---

/// 搜索结果，用于向量搜索返回
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub commit_hash: String,
    pub score: f32,
    pub intent: String,
    pub summary: String,
    pub change_category: ChangeCategory,
    pub risk_level: RiskLevel,
    pub affected_modules: Vec<String>,
    pub author_name: String,
    pub timestamp: DateTime<Utc>,
}

/// 搜索过滤条件
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchFilters {
    pub change_categories: Option<Vec<ChangeCategory>>,
    pub risk_levels: Option<Vec<RiskLevel>>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub authors: Option<Vec<String>>,
    pub modules: Option<Vec<String>>,
}

/// Commit 元数据（存储到向量数据库的 payload）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitMetadata {
    pub commit_hash: String,
    pub author_name: String,
    pub timestamp: DateTime<Utc>,
    pub change_category: ChangeCategory,
    pub risk_level: RiskLevel,
    pub affected_modules: Vec<String>,
    pub intent: String,
    pub summary: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VectorStoreStats {
    pub total_points: u64,
    pub indexed_modules: Vec<String>,
    pub last_indexed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexReport {
    pub successful: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

impl IndexReport {
    pub fn new() -> Self {
        Self::default()
    }
}

/// 当前代码库状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentState {
    pub current_branch: String,
    pub current_commit: String,
    pub working_directory_clean: bool,
}

impl CurrentState {
    pub fn new(branch: impl Into<String>, commit: impl Into<String>) -> Self {
        Self {
            current_branch: branch.into(),
            current_commit: commit.into(),
            working_directory_clean: true,
        }
    }
}
