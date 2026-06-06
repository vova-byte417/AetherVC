//! 配置类型定义
//!
//! 定义 `.aether/config.toml` 中所有配置项的结构体

use serde::{Deserialize, Serialize};

// ─── 顶层配置 ───

/// AetherVC 全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AetherConfig {
    /// 门控配置
    #[serde(default)]
    pub gate: GateConfig,
    /// 验证配置
    #[serde(default)]
    pub verify: VerifyConfig,
    /// LLM 配置
    #[serde(default)]
    pub llm: LLMConfig,
    /// 回滚配置
    #[serde(default)]
    pub rollback: RollbackConfig,
    /// 协调器配置
    #[serde(default)]
    pub coordinator: CoordinatorConfig,
    /// 存储配置
    #[serde(default)]
    pub storage: StorageConfig,
}

impl Default for AetherConfig {
    fn default() -> Self {
        Self {
            gate: GateConfig::default(),
            verify: VerifyConfig::default(),
            llm: LLMConfig::default(),
            rollback: RollbackConfig::default(),
            coordinator: CoordinatorConfig::default(),
            storage: StorageConfig::default(),
        }
    }
}

// ─── 门控配置 ───

/// 审核门控配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateConfig {
    /// 全局开关
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 自动通过的变更类型
    #[serde(default = "default_auto_pass")]
    pub auto_pass: Vec<String>,
    /// 必须人工审核的变更类型
    #[serde(default = "default_require_review")]
    pub require_review: Vec<String>,
    /// 模块路径配置
    #[serde(default)]
    pub modules: GateModuleConfig,
    /// 变更规模阈值
    #[serde(default)]
    pub thresholds: GateThresholds,
    /// 触发动作配置
    #[serde(default)]
    pub actions: GateActions,
}

fn default_enabled() -> bool { true }

fn default_auto_pass() -> Vec<String> {
    vec![
        "documentation".into(),
        "test".into(),
        "config_change".into(),
    ]
}

fn default_require_review() -> Vec<String> {
    vec![
        "security_hardening".into(),
        "feature_removal".into(),
    ]
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_pass: default_auto_pass(),
            require_review: default_require_review(),
            modules: GateModuleConfig::default(),
            thresholds: GateThresholds::default(),
            actions: GateActions::default(),
        }
    }
}

/// 模块路径配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateModuleConfig {
    /// 高风险模块：任何涉及这些模块的变更都需要审核
    #[serde(default = "default_high_risk_modules")]
    pub high_risk: Vec<String>,
    /// 低风险模块：自动通过
    #[serde(default = "default_low_risk_modules")]
    pub low_risk: Vec<String>,
}

fn default_high_risk_modules() -> Vec<String> {
    vec!["auth/".into(), "database/".into(), "payment/".into(), "core/".into()]
}

fn default_low_risk_modules() -> Vec<String> {
    vec!["docs/".into(), "tests/".into(), "assets/".into(), "examples/".into()]
}

impl Default for GateModuleConfig {
    fn default() -> Self {
        Self {
            high_risk: default_high_risk_modules(),
            low_risk: default_low_risk_modules(),
        }
    }
}

/// 变更规模阈值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateThresholds {
    /// 单个 commit 变更文件数上限
    #[serde(default = "default_max_files")]
    pub max_files_changed: u32,
    /// 新增行数上限
    #[serde(default = "default_max_added")]
    pub max_lines_added: u32,
    /// 删除行数上限
    #[serde(default = "default_max_deleted")]
    pub max_lines_deleted: u32,
    /// 每小时 AI 提交上限
    #[serde(default = "default_max_commits_per_hour")]
    pub max_commits_per_hour: u32,
}

fn default_max_files() -> u32 { 10 }
fn default_max_added() -> u32 { 500 }
fn default_max_deleted() -> u32 { 200 }
fn default_max_commits_per_hour() -> u32 { 10 }

impl Default for GateThresholds {
    fn default() -> Self {
        Self {
            max_files_changed: 10,
            max_lines_added: 500,
            max_lines_deleted: 200,
            max_commits_per_hour: 10,
        }
    }
}

/// 超阈值动作配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateActions {
    /// 超阈值动作
    #[serde(default = "default_on_exceeded")]
    pub on_threshold_exceeded: GateAction,
    /// 严重风险动作
    #[serde(default = "default_on_critical")]
    pub on_critical_risk: GateAction,
    /// 高风险动作
    #[serde(default = "default_on_high")]
    pub on_high_risk: GateAction,
}

fn default_on_exceeded() -> GateAction { GateAction::Block }
fn default_on_critical() -> GateAction { GateAction::Block }
fn default_on_high() -> GateAction { GateAction::Queue }

impl Default for GateActions {
    fn default() -> Self {
        Self {
            on_threshold_exceeded: GateAction::Block,
            on_critical_risk: GateAction::Block,
            on_high_risk: GateAction::Queue,
        }
    }
}

/// 门控动作
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GateAction {
    /// 阻止
    Block,
    /// 警告但放行
    Warn,
    /// 加入审核队列
    Queue,
}

// ─── 验证配置 ───

/// 验证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyConfig {
    /// 全局开关
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 基础验证
    #[serde(default)]
    pub basic: VerifyBasic,
    /// 测试验证
    #[serde(default)]
    pub testing: VerifyTesting,
    /// 高级验证
    #[serde(default)]
    pub advanced: VerifyAdvanced,
}

impl Default for VerifyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            basic: VerifyBasic::default(),
            testing: VerifyTesting::default(),
            advanced: VerifyAdvanced::default(),
        }
    }
}

/// 基础验证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyBasic {
    /// 编译检查
    #[serde(default = "default_true")]
    pub compile_check: bool,
    /// Lint 检查
    #[serde(default = "default_true")]
    pub lint_check: bool,
    /// 格式检查
    #[serde(default)]
    pub format_check: bool,
}

fn default_true() -> bool { true }

impl Default for VerifyBasic {
    fn default() -> Self {
        Self {
            compile_check: true,
            lint_check: true,
            format_check: false,
        }
    }
}

/// 测试验证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyTesting {
    /// 是否运行单元测试
    #[serde(default = "default_true")]
    pub run_unit_tests: bool,
    /// 测试命令
    #[serde(default = "default_test_command")]
    pub test_command: String,
    /// 只跑受影响的测试
    #[serde(default = "default_true")]
    pub run_affected_tests: bool,
    /// 覆盖率阈值
    #[serde(default = "default_coverage")]
    pub coverage_threshold: f32,
}

fn default_test_command() -> String { "cargo test".into() }
fn default_coverage() -> f32 { 0.8 }

impl Default for VerifyTesting {
    fn default() -> Self {
        Self {
            run_unit_tests: true,
            test_command: "cargo test".into(),
            run_affected_tests: true,
            coverage_threshold: 0.8,
        }
    }
}

/// 高级验证配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyAdvanced {
    /// 静态分析
    #[serde(default)]
    pub static_analysis: bool,
    /// 安全扫描
    #[serde(default)]
    pub security_scan: bool,
    /// 依赖检查
    #[serde(default)]
    pub dependency_check: bool,
}

impl Default for VerifyAdvanced {
    fn default() -> Self {
        Self {
            static_analysis: true,
            security_scan: true,
            dependency_check: true,
        }
    }
}

// ─── LLM 配置 ───

/// LLM 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    /// Provider: deepseek / openai / openai-compatible / claude / local
    #[serde(default = "default_provider")]
    pub provider: String,
    /// API Base URL（openai-compatible 时必填）
    #[serde(default)]
    pub api_base: String,
    /// API Key（支持环境变量引用 ${VAR_NAME}）
    #[serde(default)]
    pub api_key: String,
    /// 模型名称
    #[serde(default = "default_model")]
    pub model: String,
    /// 最大 token 数
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// 温度参数
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Embedding 模型名称
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    /// 失败重试次数（0 = 不重试，1 = 尝试一次，>1 = 多次重试）
    #[serde(default = "default_retries")]
    pub max_retries: u32,
    /// 降级策略
    #[serde(default)]
    pub fallback: LLMFallbackConfig,
}

fn default_provider() -> String { "deepseek".into() }
fn default_model() -> String { "deepseek-chat".into() }
fn default_max_tokens() -> u32 { 4000 }
fn default_temperature() -> f32 { 0.1 }
fn default_embedding_model() -> String { "text-embedding-3-small".into() }
fn default_retries() -> u32 { 3 }

impl Default for LLMConfig {
    fn default() -> Self {
        Self {
            provider: "deepseek".into(),
            api_base: String::new(),
            api_key: String::new(),
            model: "deepseek-chat".into(),
            max_tokens: 4000,
            temperature: 0.1,
            embedding_model: "text-embedding-3-small".into(),
            max_retries: 3,
            fallback: LLMFallbackConfig::default(),
        }
    }
}

/// LLM 降级配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMFallbackConfig {
    /// 降级策略: rules / error / skip
    #[serde(default = "default_fallback_strategy")]
    pub strategy: String,
    /// 是否启用缓存
    #[serde(default = "default_true")]
    pub cache_enabled: bool,
    /// 缓存有效期（小时）
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_hours: u32,
}

fn default_fallback_strategy() -> String { "rules".into() }
fn default_cache_ttl() -> u32 { 24 }

impl Default for LLMFallbackConfig {
    fn default() -> Self {
        Self {
            strategy: "rules".into(),
            cache_enabled: true,
            cache_ttl_hours: 24,
        }
    }
}

// ─── 门控决策结果 ───

/// 门控决策结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecision {
    /// commit hash
    pub commit_hash: String,
    /// 决策动作
    pub action: GateAction,
    /// 决策原因
    pub reason: String,
    /// 风险分数 (0.0~1.0)
    pub risk_score: f32,
    /// 决策时间
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl GateDecision {
    pub fn new(
        commit_hash: impl Into<String>,
        action: GateAction,
        reason: impl Into<String>,
        risk_score: f32,
    ) -> Self {
        Self {
            commit_hash: commit_hash.into(),
            action,
            reason: reason.into(),
            risk_score: risk_score.clamp(0.0, 1.0),
            timestamp: chrono::Utc::now(),
        }
    }
}

// ─── 回滚配置 ───

/// 回滚配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackConfig {
    /// 全局开关
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 编译失败自动回滚
    #[serde(default = "default_true")]
    pub auto_rollback_on_compile_failure: bool,
    /// 测试失败自动回滚
    #[serde(default)]
    pub auto_rollback_on_test_failure: bool,
    /// 测试失败率阈值
    #[serde(default = "default_test_failure_threshold")]
    pub test_failure_threshold: f32,
    /// 安全漏洞立即回滚
    #[serde(default = "default_true")]
    pub auto_rollback_on_security_cve: bool,
    /// 回滚是否需要人类确认
    #[serde(default = "default_true")]
    pub require_human_approval: bool,
    /// 每小时最大自动回滚次数
    #[serde(default = "default_max_rollbacks")]
    pub max_auto_rollbacks_per_hour: u32,
}

fn default_test_failure_threshold() -> f32 { 0.1 }
fn default_max_rollbacks() -> u32 { 3 }

impl Default for RollbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_rollback_on_compile_failure: true,
            auto_rollback_on_test_failure: false,
            test_failure_threshold: 0.1,
            auto_rollback_on_security_cve: true,
            require_human_approval: true,
            max_auto_rollbacks_per_hour: 3,
        }
    }
}

// ─── 协调器配置 ───

/// 多 Agent 协调器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorConfig {
    /// 全局开关
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 多少 Agent 同时修改同一模块视为热点
    #[serde(default = "default_hotspot_threshold")]
    pub hotspot_threshold: u32,
    /// 监控时间窗口（分钟）
    #[serde(default = "default_monitor_window")]
    pub monitor_window_minutes: u32,
    /// 低风险冲突自动解决
    #[serde(default = "default_true")]
    pub auto_resolve_low_risk: bool,
}

fn default_hotspot_threshold() -> u32 { 2 }
fn default_monitor_window() -> u32 { 120 }

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hotspot_threshold: 2,
            monitor_window_minutes: 120,
            auto_resolve_low_risk: true,
        }
    }
}

// ─── 存储配置 ───

/// 存储后端配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// 存储后端: "memory" | "persistent"
    #[serde(default = "default_storage_backend")]
    pub backend: String,
    /// 持久化数据目录（相对于仓库根目录，默认为 .aether）
    #[serde(default = "default_storage_data_dir")]
    pub data_dir: String,
}

fn default_storage_backend() -> String { "persistent".into() }
fn default_storage_data_dir() -> String { ".aether".into() }

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: "persistent".into(),
            data_dir: ".aether".into(),
        }
    }
}
