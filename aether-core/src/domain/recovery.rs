use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::commit::CurrentState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryRequest {
    pub id: Uuid,
    pub natural_language_query: String,
    pub target_commit_hint: Option<String>,
    pub current_state: CurrentState,
    pub status: RecoveryStatus,
    pub result: Option<RecoveryResult>,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryStatus {
    Pending,
    Analyzing,
    GeneratingPatch,
    ResolvingConflicts,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryResult {
    pub recovered_commit: String,
    pub patch: String,
    pub conflicts: Vec<Conflict>,
    pub new_commit_hash: Option<String>,
    pub warnings: Vec<String>,
}

impl RecoveryRequest {
    pub fn new(
        query: impl Into<String>,
        current_state: CurrentState,
        user_id: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            natural_language_query: query.into(),
            target_commit_hint: None,
            current_state,
            status: RecoveryStatus::Pending,
            result: None,
            created_at: Utc::now(),
            created_by: user_id.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub file_path: String,
    pub description: String,
    pub resolution_suggestion: Option<String>,
}

impl Conflict {
    pub fn new(file_path: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            file_path: file_path.into(),
            description: description.into(),
            resolution_suggestion: None,
        }
    }
}

// CurrentState 的定义和实现见 domain/commit.rs
