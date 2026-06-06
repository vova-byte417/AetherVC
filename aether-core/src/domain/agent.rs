use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentId(pub Uuid);

impl AgentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AgentType {
    SemanticInterpreter,
    CrossCommitRecovery,
    Merge,
    MultiAgentCoordinator,
    ValidationRisk,
    Rollback,
    CommitIntelligence,
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::SemanticInterpreter => write!(f, "SemanticInterpreter"),
            AgentType::CrossCommitRecovery => write!(f, "CrossCommitRecovery"),
            AgentType::Merge => write!(f, "Merge"),
            AgentType::MultiAgentCoordinator => write!(f, "MultiAgentCoordinator"),
            AgentType::ValidationRisk => write!(f, "ValidationRisk"),
            AgentType::Rollback => write!(f, "Rollback"),
            AgentType::CommitIntelligence => write!(f, "CommitIntelligence"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Idle,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: Uuid,
    pub task_type: String,
    pub input: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl AgentTask {
    pub fn new(task_type: impl Into<String>, input: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            task_type: task_type.into(),
            input,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub task_id: Uuid,
    pub success: bool,
    pub output: serde_json::Value,
    pub error_message: Option<String>,
    pub execution_time_ms: u64,
}

impl AgentResult {
    pub fn success(task_id: Uuid, output: serde_json::Value, execution_time_ms: u64) -> Self {
        Self {
            task_id,
            success: true,
            output,
            error_message: None,
            execution_time_ms,
        }
    }

    pub fn failure(task_id: Uuid, error: impl Into<String>) -> Self {
        Self {
            task_id,
            success: false,
            output: serde_json::Value::Null,
            error_message: Some(error.into()),
            execution_time_ms: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    pub repository_path: String,
    pub current_branch: String,
    pub current_commit: String,
    pub user_id: String,
}

impl Default for TaskContext {
    fn default() -> Self {
        Self {
            repository_path: ".".to_string(),
            current_branch: "main".to_string(),
            current_commit: String::new(),
            user_id: "default".to_string(),
        }
    }
}

// ─── 多 Agent 协调相关类型 ───

/// Agent 身份信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub name: String,
    pub email: String,
    pub is_ai_agent: bool,
}

/// 冲突严重程度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConflictSeverity {
    Critical,
    High,
    Medium,
    Low,
}

/// 模块热点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleHotspot {
    pub module_path: String,
    pub agents_involved: Vec<String>,
    pub severity: ConflictSeverity,
    pub overlapping_functions: Vec<String>,
}

/// 合并排序项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeOrderItem {
    pub priority: u32,
    pub agent: String,
    pub commit_hash: String,
    pub description: String,
    pub reason: String,
}

/// 协调计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationPlan {
    pub summary: String,
    pub merge_order: Vec<MergeOrderItem>,
    pub requires_human_review: Vec<String>,
    pub auto_mergeable: Vec<String>,
}

/// Agent 冲突矩阵
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConflictMatrix {
    pub agents: Vec<AgentIdentity>,
    pub module_agent_counts: std::collections::HashMap<String, std::collections::HashMap<String, u32>>,
    pub hotspots: Vec<ModuleHotspot>,
    pub recommendation: CoordinationPlan,
}

// ─── 回滚相关类型 ───

/// 回滚动作类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RollbackAction {
    Revert { commit_hash: String },
    Reset { target_hash: String, hard: bool },
    NotifyOnly { reason: String },
}

/// 回滚状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RollbackStatus {
    PendingApproval,
    Executed,
    Failed(String),
    Cancelled,
}

/// 回滚请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackRequest {
    pub commit_hash: String,
    pub reason: String,
    pub action: RollbackAction,
    pub require_approval: bool,
}

/// 回滚记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackRecord {
    pub id: String,
    pub rolled_back_commit: String,
    pub revert_commit: Option<String>,
    pub reason: String,
    pub action: String,
    pub status: RollbackStatus,
    pub executed_at: DateTime<Utc>,
    pub agent_name: String,
}

// ─── Tag 验证相关类型 ───

/// Tag 排序方式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TagOrderBy {
    RiskAsc,
    RiskDesc,
    Chronological,
}

/// Tag 验证请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagValidationRequest {
    pub tags: Vec<String>,
    pub order_by: TagOrderBy,
    pub filter_keyword: Option<String>,
    pub max_tags: usize,
}

/// 验证结论
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationConclusion {
    Pass,
    ConditionalPass(Vec<String>),
    Fail(Vec<String>),
}

/// Tag 风险评估
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRiskAssessment {
    pub tag: String,
    pub commit_hash: String,
    pub risk_score: f32,
    pub risk_level: String,
    pub change_type: String,
    pub affected_modules: Vec<String>,
    pub agent_name: String,
}

/// Tag 验证报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagValidationReport {
    pub tag: String,
    pub commit_hash: String,
    pub agent: String,
    pub risk_assessment: TagRiskAssessment,
    pub overall_conclusion: String,
    pub verification_details: Option<serde_json::Value>,
}
