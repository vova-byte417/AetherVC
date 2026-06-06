use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::commit::{Author, RiskLevel, SemanticInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeRequest {
    pub id: Uuid,
    pub pull_requests: Vec<PullRequest>,
    pub merge_order: Vec<String>,
    pub conflicts: Vec<super::recovery::Conflict>,
    pub status: MergeStatus,
    pub risk_assessment: Option<RiskAssessment>,
    pub created_at: DateTime<Utc>,
}

impl MergeRequest {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            pull_requests: Vec::new(),
            merge_order: Vec::new(),
            conflicts: Vec::new(),
            status: MergeStatus::Pending,
            risk_assessment: None,
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PullRequest {
    pub id: String,
    pub title: String,
    pub source_branch: String,
    pub target_branch: String,
    pub author: Author,
    pub semantic_info: SemanticInfo,
    pub files_changed: Vec<String>,
    pub status: PRStatus,
}

impl PullRequest {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        source: impl Into<String>,
        target: impl Into<String>,
        author: Author,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            source_branch: source.into(),
            target_branch: target.into(),
            author,
            semantic_info: SemanticInfo::default(),
            files_changed: Vec::new(),
            status: PRStatus::Open,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PRStatus {
    Open,
    ReadyForMerge,
    Merged,
    Closed,
    Conflicted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeStatus {
    Pending,
    Analyzing,
    AutoMerging,
    RequiresManualIntervention,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub overall_risk: RiskLevel,
    pub risk_factors: Vec<RiskFactor>,
    pub recommendation: MergeRecommendation,
}

impl RiskAssessment {
    pub fn new(overall_risk: RiskLevel, recommendation: MergeRecommendation) -> Self {
        Self {
            overall_risk,
            risk_factors: Vec::new(),
            recommendation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskFactor {
    pub description: String,
    pub severity: RiskLevel,
    pub affected_files: Vec<String>,
}

impl RiskFactor {
    pub fn new(description: impl Into<String>, severity: RiskLevel) -> Self {
        Self {
            description: description.into(),
            severity,
            affected_files: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MergeRecommendation {
    AutoMerge,
    ManualReview,
    Postpone,
    Reject,
}

impl Default for MergeRequest {
    fn default() -> Self {
        Self::new()
    }
}
