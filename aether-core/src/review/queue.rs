//! 审核队列
//!
//! 管理待审核、已审核的变更项，支持增删改查。

use super::{ReviewHistoryEntry, ReviewItem, ReviewStatus};
use crate::utils::{AetherError, Result};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// 审核队列管理器
pub struct ReviewQueue {
    /// 所有审核项（key = queue_id）
    items: HashMap<String, ReviewItem>,
    /// 审核历史
    history: Vec<ReviewHistoryEntry>,
}

impl ReviewQueue {
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
            history: Vec::new(),
        }
    }

    /// 添加审核项到队列
    pub fn enqueue(&mut self, item: ReviewItem) {
        let id = item.id.clone();
        self.items.insert(id, item);
    }

    /// 获取待审核项列表
    pub fn pending_items(&self) -> Vec<&ReviewItem> {
        self.items
            .values()
            .filter(|item| item.status == ReviewStatus::Pending)
            .collect()
    }

    /// 获取所有审核项
    pub fn all_items(&self) -> Vec<&ReviewItem> {
        self.items.values().collect()
    }

    /// 按 ID 获取审核项
    pub fn get(&self, id: &str) -> Option<&ReviewItem> {
        self.items.get(id)
    }

    /// 获取审核项的可变引用
    pub fn get_mut(&mut self, id: &str) -> Option<&mut ReviewItem> {
        self.items.get_mut(id)
    }

    /// 批准审核项
    pub fn approve(
        &mut self,
        id: &str,
        reviewer: impl Into<String>,
        comment: Option<impl Into<String>>,
    ) -> Result<&ReviewItem> {
        let item = self
            .items
            .get_mut(id)
            .ok_or_else(|| AetherError::NotFound(format!("审核项不存在: {}", id)))?;

        if item.status != ReviewStatus::Pending {
            return Err(AetherError::InvalidInput(format!(
                "审核项 {} 状态为 {}，无法批准",
                id,
                item.status.as_str()
            )));
        }

        item.approve(reviewer, comment);

        self.history.push(ReviewHistoryEntry {
            action: "approved".into(),
            item: item.clone(),
            timestamp: Utc::now(),
        });

        Ok(item)
    }

    /// 拒绝审核项
    pub fn reject(
        &mut self,
        id: &str,
        reviewer: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<&ReviewItem> {
        let item = self
            .items
            .get_mut(id)
            .ok_or_else(|| AetherError::NotFound(format!("审核项不存在: {}", id)))?;

        if item.status != ReviewStatus::Pending {
            return Err(AetherError::InvalidInput(format!(
                "审核项 {} 状态为 {}，无法拒绝",
                id,
                item.status.as_str()
            )));
        }

        item.reject(reviewer, reason);

        self.history.push(ReviewHistoryEntry {
            action: "rejected".into(),
            item: item.clone(),
            timestamp: Utc::now(),
        });

        Ok(item)
    }

    /// 获取审核历史
    pub fn history(&self, since: Option<DateTime<Utc>>) -> Vec<&ReviewHistoryEntry> {
        self.history
            .iter()
            .filter(|entry| {
                if let Some(since_time) = since {
                    entry.timestamp >= since_time
                } else {
                    true
                }
            })
            .collect()
    }

    /// 统计信息
    pub fn stats(&self) -> QueueStats {
        let total = self.items.len() as u32;
        let pending = self
            .items
            .values()
            .filter(|i| i.status == ReviewStatus::Pending)
            .count() as u32;
        let approved = self
            .items
            .values()
            .filter(|i| i.status == ReviewStatus::Approved)
            .count() as u32;
        let rejected = self
            .items
            .values()
            .filter(|i| i.status == ReviewStatus::Rejected)
            .count() as u32;
        let skipped = self
            .items
            .values()
            .filter(|i| i.status == ReviewStatus::Skipped)
            .count() as u32;

        QueueStats {
            total,
            pending,
            approved,
            rejected,
            skipped,
        }
    }
}

impl Default for ReviewQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// 队列统计信息
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueueStats {
    pub total: u32,
    pub pending: u32,
    pub approved: u32,
    pub rejected: u32,
    pub skipped: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_item() -> ReviewItem {
        ReviewItem::new(
            "abc123",
            "fix: auth bug",
            "Cline",
            "high",
            "涉及高风险模块",
            vec!["auth/".into()],
            "修复认证 bug",
        )
    }

    #[test]
    fn test_enqueue_and_pending() {
        let mut queue = ReviewQueue::new();
        queue.enqueue(make_item());
        assert_eq!(queue.pending_items().len(), 1);
    }

    #[test]
    fn test_approve() {
        let mut queue = ReviewQueue::new();
        let item = make_item();
        let id = item.id.clone();
        queue.enqueue(item);

        let result = queue.approve(&id, "human-reviewer", Some("LGTM"));
        assert!(result.is_ok());
        assert!(queue.pending_items().is_empty());

        let stats = queue.stats();
        assert_eq!(stats.approved, 1);
    }

    #[test]
    fn test_reject() {
        let mut queue = ReviewQueue::new();
        let item = make_item();
        let id = item.id.clone();
        queue.enqueue(item);

        let result = queue.reject(&id, "human-reviewer", "需要补充测试");
        assert!(result.is_ok());
        assert_eq!(queue.stats().rejected, 1);
    }

    #[test]
    fn test_approve_non_pending_fails() {
        let mut queue = ReviewQueue::new();
        let item = make_item();
        let id = item.id.clone();
        queue.enqueue(item);
        queue.approve(&id, "r1", Some("ok")).unwrap();

        // 不应该能重复批准
        assert!(queue.approve(&id, "r2", Some("again")).is_err());
    }
}
