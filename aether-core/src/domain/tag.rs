use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::commit::{RiskLevel, SemanticInfo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: Uuid,
    pub name: String,
    pub commit_hash: String,
    pub semantic_info: SemanticInfo,
    pub validation_status: ValidationStatus,
    pub created_at: DateTime<Utc>,
    pub metadata: TagMetadata,
}

impl Tag {
    pub fn new(name: impl Into<String>, commit_hash: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            commit_hash: commit_hash.into(),
            semantic_info: SemanticInfo::default(),
            validation_status: ValidationStatus::Pending,
            created_at: Utc::now(),
            metadata: TagMetadata::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationStatus {
    Pending,
    Validated,
    Failed,
    InProgress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagMetadata {
    pub environment: String,
    pub deployment_status: DeploymentStatus,
    pub risk_score: f32,
}

impl Default for TagMetadata {
    fn default() -> Self {
        Self {
            environment: "dev".to_string(),
            deployment_status: DeploymentStatus::NotDeployed,
            risk_score: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentStatus {
    NotDeployed,
    Deployed,
    RolledBack,
    ShadowDeployed,
}

impl std::fmt::Display for DeploymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeploymentStatus::NotDeployed => write!(f, "not_deployed"),
            DeploymentStatus::Deployed => write!(f, "deployed"),
            DeploymentStatus::RolledBack => write!(f, "rolled_back"),
            DeploymentStatus::ShadowDeployed => write!(f, "shadow_deployed"),
        }
    }
}
