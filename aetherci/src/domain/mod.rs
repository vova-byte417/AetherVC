//! AetherCI 领域模型
//!
//! 定义变更分析报告、置信度分数、变更实体等核心类型。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ─── 变更分类 ───

/// 变更分类（细粒度，覆盖功能/重构/安全/依赖等）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeIntent {
    /// 功能新增
    FeatureAddition,
    /// 功能修改
    FeatureModification,
    /// 功能删除
    FeatureRemoval,
    /// 重构（结构优化，行为不变）
    Refactor,
    /// 性能优化
    Performance,
    /// Bug 修复
    Bugfix,
    /// 安全加固
    SecurityHardening,
    /// 依赖更新
    DependencyUpdate,
    /// 配置变更
    ConfigChange,
    /// 文档变更
    Documentation,
    /// 测试变更
    Test,
    /// 未知/其他
    Unknown,
}

impl ChangeIntent {
    pub fn as_str(&self) -> &str {
        match self {
            ChangeIntent::FeatureAddition => "feature_addition",
            ChangeIntent::FeatureModification => "feature_modification",
            ChangeIntent::FeatureRemoval => "feature_removal",
            ChangeIntent::Refactor => "refactor",
            ChangeIntent::Performance => "performance",
            ChangeIntent::Bugfix => "bugfix",
            ChangeIntent::SecurityHardening => "security_hardening",
            ChangeIntent::DependencyUpdate => "dependency_update",
            ChangeIntent::ConfigChange => "config_change",
            ChangeIntent::Documentation => "documentation",
            ChangeIntent::Test => "test",
            ChangeIntent::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for ChangeIntent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChangeIntent::FeatureAddition => write!(f, "功能新增"),
            ChangeIntent::FeatureModification => write!(f, "功能修改"),
            ChangeIntent::FeatureRemoval => write!(f, "功能删除"),
            ChangeIntent::Refactor => write!(f, "重构"),
            ChangeIntent::Performance => write!(f, "性能优化"),
            ChangeIntent::Bugfix => write!(f, "Bug 修复"),
            ChangeIntent::SecurityHardening => write!(f, "安全加固"),
            ChangeIntent::DependencyUpdate => write!(f, "依赖更新"),
            ChangeIntent::ConfigChange => write!(f, "配置变更"),
            ChangeIntent::Documentation => write!(f, "文档变更"),
            ChangeIntent::Test => write!(f, "测试变更"),
            ChangeIntent::Unknown => write!(f, "未知"),
        }
    }
}

// ─── 置信度分数 ───

/// 带置信度分数的推理结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfidenceScore {
    /// 置信度 (0.0 ~ 1.0)
    pub score: f32,
    /// 推理依据
    pub rationale: String,
}

impl ConfidenceScore {
    pub fn new(score: f32, rationale: impl Into<String>) -> Self {
        Self {
            score: score.clamp(0.0, 1.0),
            rationale: rationale.into(),
        }
    }

    pub fn high(rationale: impl Into<String>) -> Self {
        Self::new(0.9, rationale)
    }

    pub fn medium(rationale: impl Into<String>) -> Self {
        Self::new(0.65, rationale)
    }

    pub fn low(rationale: impl Into<String>) -> Self {
        Self::new(0.35, rationale)
    }
}

// ─── 分类结果（流水线 Stage 2 输出） ───

/// 变更分类结果
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    /// 主要变更类型
    pub primary_intent: ChangeIntent,
    /// 置信度
    pub confidence: ConfidenceScore,
    /// 次要变更类型
    pub secondary_intents: Vec<ChangeIntent>,
}

// ─── 变更实体 ───

/// 变更实体（函数/类/模块等）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedEntity {
    /// 实体名称
    pub name: String,
    /// 实体类型（function / class / module / interface / variable）
    pub entity_type: String,
    /// 所在文件
    pub file_path: String,
    /// 变更操作（added / modified / deleted / renamed / moved）
    pub operation: EntityOperation,
    /// 变更前名称（用于 rename/move 检测）
    pub previous_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EntityOperation {
    Added,
    Modified,
    Deleted,
    Renamed,
    Moved,
}

impl std::fmt::Display for EntityOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntityOperation::Added => write!(f, "新增"),
            EntityOperation::Modified => write!(f, "修改"),
            EntityOperation::Deleted => write!(f, "删除"),
            EntityOperation::Renamed => write!(f, "重命名"),
            EntityOperation::Moved => write!(f, "移动"),
        }
    }
}

// ─── 影响分析 ───

/// 影响范围评估
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAssessment {
    /// 受影响的模块列表
    pub affected_modules: Vec<String>,
    /// 潜在风险列表
    pub risks: Vec<RiskItem>,
    /// 建议验证点
    pub suggested_validations: Vec<String>,
    /// 是否为破坏性变更
    pub is_breaking_change: bool,
}

/// 风险项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskItem {
    /// 风险描述
    pub description: String,
    /// 风险等级
    pub severity: RiskSeverity,
    /// 缓解建议
    pub mitigation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskSeverity {
    Critical,
    High,
    Medium,
    Low,
}

impl RiskSeverity {
    pub fn as_str(&self) -> &str {
        match self {
            RiskSeverity::Critical => "严重",
            RiskSeverity::High => "高",
            RiskSeverity::Medium => "中",
            RiskSeverity::Low => "低",
        }
    }
}

// ─── 历史上下文 ───

/// 历史上下文关联
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalContext {
    /// 相关历史 commit hash 列表
    pub related_commits: Vec<String>,
    /// 功能演化路径描述
    pub evolution_path: String,
}

// ─── 建议 ───

/// Review 与测试建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendations {
    /// Review 重点
    pub review_focus: Vec<String>,
    /// 推荐测试
    pub suggested_tests: Vec<String>,
}

// ─── Diff 统计 ───

/// Diff 预处理统计结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffStatistics {
    /// 变更文件数
    pub files_changed: u32,
    /// 新增行数
    pub additions: u32,
    /// 删除行数
    pub deletions: u32,
    /// 涉及的语言列表
    pub languages: Vec<String>,
    /// 变更实体列表
    pub entities: Vec<ChangedEntity>,
    /// 跨文件变更分组
    pub intent_groups: Vec<IntentGroup>,
}

/// 跨文件意图分组（相关变更聚类）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentGroup {
    /// 分组标签
    pub label: String,
    /// 涉及的文件
    pub files: Vec<String>,
    /// 涉及的实体
    pub entities: Vec<String>,
}

// ─── 核心报告 ───

/// AetherCI 变更分析报告（完整结构化输出）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitAnalysisReport {
    /// Commit 哈希
    pub commit_hash: String,
    /// 作者
    pub author: String,
    /// 时间
    pub timestamp: DateTime<Utc>,
    /// 变更摘要（一句话 + 类型）
    pub summary: CommitSummary,
    /// 详细变更内容
    pub detailed_changes: DetailedChanges,
    /// 意图与动机（LLM 推理）
    pub intent_analysis: IntentAnalysis,
    /// 影响范围与风险
    pub impact: ImpactAssessment,
    /// 历史上下文
    pub history: HistoricalContext,
    /// 建议
    pub recommendations: Recommendations,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitSummary {
    /// 一句话总结
    pub one_liner: String,
    /// 变更类型
    pub change_type: ChangeIntent,
    /// 置信度
    pub confidence: ConfidenceScore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedChanges {
    /// 变更实体列表
    pub entities: Vec<ChangedEntity>,
    /// 关键 diff 摘要（精简后的语义描述）
    pub key_diffs: Vec<String>,
    /// Diff 统计
    pub stats: DiffStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentAnalysis {
    /// 解决的问题描述
    pub problem_solved: String,
    /// 推断的动机
    pub inferred_motivation: String,
    /// 与项目架构的关系
    pub architectural_context: String,
    /// 置信度
    pub confidence: ConfidenceScore,
}

// ─── 流水线输入/输出 ───

/// 流水线输入
#[derive(Debug, Clone)]
pub struct PipelineInput {
    /// 原始 git diff
    pub diff: String,
    /// Commit message（如有）
    pub commit_message: String,
    /// Commit hash
    pub commit_hash: String,
    /// 作者
    pub author: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 项目根路径（用于读取完整文件上下文）
    pub repo_path: String,
}

/// 流水线输出
#[derive(Debug, Clone)]
pub struct PipelineOutput {
    /// 结构化报告
    pub report: CommitAnalysisReport,
    /// Markdown 文档
    pub markdown: String,
}
