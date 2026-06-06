//! 变更摘要引擎 (Digest Engine)
//!
//! 将一段时间内的多个 commit 聚合为一份人类可读的摘要报告。

pub mod aggregator;
pub mod summarizer;

pub use aggregator::DigestAggregator;
pub use summarizer::DigestSummarizer;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 变更摘要报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestReport {
    /// 报告 ID
    pub id: String,
    /// 时间窗口
    pub window: TimeWindow,
    /// LLM 生成的一句话总结
    pub summary: String,
    /// 总 commit 数
    pub total_commits: u32,
    /// 涉及的 Agent/作者
    pub agents_involved: Vec<String>,
    /// 变更主题聚类
    pub topic_clusters: Vec<TopicCluster>,
    /// 风险分布
    pub risk_distribution: RiskDistribution,
    /// 模块热力图（模块路径 → 变更次数）
    pub module_heatmap: HashMap<String, u32>,
    /// 需要关注的高风险变更
    pub high_risk_items: Vec<DigestItem>,
    /// 安全的变更
    pub safe_items: Vec<DigestItem>,
    /// 生成时间
    pub generated_at: DateTime<Utc>,
}

/// 时间窗口
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeWindow {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

/// 变更主题聚类
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicCluster {
    /// 主题标签，如"认证流程修复"
    pub label: String,
    /// 主题摘要
    pub summary: String,
    /// 涉及的 commit hash
    pub commit_hashes: Vec<String>,
    /// 变更类型
    pub change_type: String,
    /// 风险等级
    pub risk_level: String,
}

/// 风险分布
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskDistribution {
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
}

impl RiskDistribution {
    pub fn new() -> Self {
        Self {
            critical: 0,
            high: 0,
            medium: 0,
            low: 0,
        }
    }
}

/// 摘要中的单个变更项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestItem {
    pub commit_hash: String,
    pub message: String,
    pub risk_level: String,
    pub affected_modules: Vec<String>,
    pub summary: String,
    pub author: String,
}

/// 摘要配置选项
#[derive(Debug, Clone)]
pub struct DigestOptions {
    /// 时间窗口开始
    pub since: Option<DateTime<Utc>>,
    /// 时间窗口结束
    pub until: Option<DateTime<Utc>>,
    /// commit 范围（如 HEAD~20..HEAD）
    pub commit_range: Option<String>,
    /// 分组维度
    pub group_by: DigestGroupBy,
    /// 风险等级下限（低于此级别的变更归入 safe_items）
    pub risk_threshold: String,
}

impl Default for DigestOptions {
    fn default() -> Self {
        Self {
            since: None,
            until: None,
            commit_range: None,
            group_by: DigestGroupBy::None,
            risk_threshold: "medium".into(),
        }
    }
}

/// 分组维度
#[derive(Debug, Clone, PartialEq)]
pub enum DigestGroupBy {
    None,
    Agent,
    Module,
}

impl DigestOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_range(mut self, range: impl Into<String>) -> Self {
        self.commit_range = Some(range.into());
        self
    }

    pub fn with_group_by(mut self, group_by: DigestGroupBy) -> Self {
        self.group_by = group_by;
        self
    }
}
