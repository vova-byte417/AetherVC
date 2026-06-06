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
