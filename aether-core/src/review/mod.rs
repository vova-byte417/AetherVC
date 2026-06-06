//! 审核门控 (Review Gate)
//!
//! 根据策略自动分级 AI 变更，高风险进入审核队列，低风险自动放行。

pub mod gate;
pub mod queue;

pub use gate::GateEngine;
pub use queue::ReviewQueue;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 审核项状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReviewStatus {
    /// 待审核
    Pending,
    /// 已批准
    Approved,
    /// 已拒绝
    Rejected,
    /// 已跳过（自动放行）
    Skipped,
}

impl ReviewStatus {
    pub fn as_str(&self) -> &str {
        match self {
            ReviewStatus::Pending => "pending",
            ReviewStatus::Approved => "approved",
            ReviewStatus::Rejected => "rejected",
            ReviewStatus::Skipped => "skipped",
        }
    }
}

/// 审核队列项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewItem {
    /// 队列 ID
    pub id: String,
    /// commit hash
    pub commit_hash: String,
    /// commit message
    pub commit_message: String,
    /// 作者
    pub author: String,
    /// 风险等级
    pub risk_level: String,
    /// 触发审核的原因
    pub triggered_reason: String,
    /// 受影响的模块
    pub affected_modules: Vec<String>,
    /// 变更摘要
    pub change_summary: String,
    /// 审核状态
    pub status: ReviewStatus,
    /// 审核人
    pub reviewer: Option<String>,
    /// 审核意见
    pub review_comment: Option<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 解决时间
    pub resolved_at: Option<DateTime<Utc>>,
}

impl ReviewItem {
    pub fn new(
        commit_hash: impl Into<String>,
        commit_message: impl Into<String>,
        author: impl Into<String>,
        risk_level: impl Into<String>,
        triggered_reason: impl Into<String>,
        affected_modules: Vec<String>,
        change_summary: impl Into<String>,
    ) -> Self {
        Self {
            id: format!("Q-{}", &uuid::Uuid::new_v4().to_string()[..8]),
            commit_hash: commit_hash.into(),
            commit_message: commit_message.into(),
            author: author.into(),
            risk_level: risk_level.into(),
            triggered_reason: triggered_reason.into(),
            affected_modules,
            change_summary: change_summary.into(),
            status: ReviewStatus::Pending,
            reviewer: None,
            review_comment: None,
            created_at: Utc::now(),
            resolved_at: None,
        }
    }

    /// 批准此审核项
    pub fn approve(&mut self, reviewer: impl Into<String>, comment: Option<impl Into<String>>) {
        self.status = ReviewStatus::Approved;
        self.reviewer = Some(reviewer.into());
        self.review_comment = comment.map(|c| c.into());
        self.resolved_at = Some(Utc::now());
    }

    /// 拒绝此审核项
    pub fn reject(&mut self, reviewer: impl Into<String>, reason: impl Into<String>) {
        self.status = ReviewStatus::Rejected;
        self.reviewer = Some(reviewer.into());
        self.review_comment = Some(reason.into());
        self.resolved_at = Some(Utc::now());
    }

    /// 标记为跳过
    pub fn skip(&mut self) {
        self.status = ReviewStatus::Skipped;
        self.resolved_at = Some(Utc::now());
    }
}

/// 审核历史条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewHistoryEntry {
    pub action: String,
    pub item: ReviewItem,
    pub timestamp: DateTime<Utc>,
}
