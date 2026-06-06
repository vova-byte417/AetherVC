//! 门控引擎
//!
//! 根据 GatePolicy 对 commit 进行自动分级决策。

use crate::config::types::{GateAction, GateConfig, GateDecision};
use crate::domain::commit::{ChangeCategory, Commit, RiskLevel};
use crate::review::ReviewItem;
use crate::utils::Result;
use tracing::info;

/// 门控引擎
pub struct GateEngine {
    policy: GateConfig,
}

impl GateEngine {
    pub fn new(policy: GateConfig) -> Self {
        Self { policy }
    }

    /// 从默认策略创建（需要后续更新策略）
    pub fn with_defaults() -> Self {
        Self::new(GateConfig::default())
    }

    /// 更新策略
    pub fn update_policy(&mut self, policy: GateConfig) {
        self.policy = policy;
    }

    /// 获取当前策略引用
    pub fn policy(&self) -> &GateConfig {
        &self.policy
    }

    /// 检查单个 commit，返回门控决策
    pub fn check(&self, commit: &Commit, stats_opt: Option<&DiffStats>) -> GateDecision {
        if !self.policy.enabled {
            return GateDecision::new(
                &commit.id.0,
                GateAction::Warn,
                "门控已禁用，全部放行",
                0.0,
            );
        }

        let mut reasons = Vec::new();
        let mut risk_score = 0.0f32;

        // 1. 检查变更类型
        let change_type_str = commit.semantic_info.change_type.to_string();
        let type_action = self.check_change_type(&change_type_str);
        if type_action != GateAction::Warn {
            reasons.push(format!("变更类型触发：{}", change_type_str));
            risk_score += 0.4;
        }

        // 2. 检查涉及模块
        let module_action = self.check_modules(&commit.semantic_info.affected_modules);
        if module_action != GateAction::Warn {
            reasons.push(format!(
                "涉及高风险模块：{}",
                commit.semantic_info.affected_modules.join(", ")
            ));
            risk_score += 0.4;
        }

        // 3. 检查变更规模
        if let Some(stats) = stats_opt {
            let threshold_action = self.check_thresholds(stats);
            if threshold_action != GateAction::Warn {
                reasons.push(format!(
                    "变更规模超阈值：{} 文件, +{} -{} 行",
                    stats.files_changed, stats.added_lines, stats.deleted_lines
                ));
                risk_score += 0.3;
            }
        }

        // 4. 风险等级加权
        risk_score += match commit.semantic_info.risk_level {
            RiskLevel::Critical => 0.3,
            RiskLevel::High => 0.2,
            RiskLevel::Medium => 0.1,
            RiskLevel::Low => 0.0,
        };

        let risk_score = risk_score.min(1.0);

        // 5. 决策
        let action = self.decide_action(&reasons, risk_score, commit);

        let reason = if reasons.is_empty() {
            "所有检查通过，低风险变更".to_string()
        } else {
            reasons.join("；")
        };

        info!(
            "[Gate] commit={} risk_score={:.2} action={:?} reason={}",
            &commit.id.0[..commit.id.0.len().min(8)],
            risk_score,
            action,
            reason
        );

        GateDecision::new(&commit.id.0, action, reason, risk_score)
    }

    /// 检查变更类型
    fn check_change_type(&self, change_type: &str) -> GateAction {
        // 检查是否在 require_review 列表中
        if self.policy.require_review.iter().any(|t| t == change_type) {
            return self.policy.actions.on_high_risk.clone();
        }

        // 检查是否在 auto_pass 列表中
        if self.policy.auto_pass.iter().any(|t| t == change_type) {
            return GateAction::Warn; // 自动通过但记录
        }

        GateAction::Queue // 默认需要审核
    }

    /// 检查涉及模块
    fn check_modules(&self, modules: &[String]) -> GateAction {
        for module in modules {
            // 先检查高风险模块
            for high_risk in &self.policy.modules.high_risk {
                if module.starts_with(high_risk.trim_end_matches('/')) {
                    return self.policy.actions.on_high_risk.clone();
                }
            }
        }

        // 检查是否所有模块都在低风险列表中
        let all_low_risk = modules.iter().all(|m| {
            self.policy
                .modules
                .low_risk
                .iter()
                .any(|lr| m.starts_with(lr.trim_end_matches('/')))
        });

        if all_low_risk && !modules.is_empty() {
            return GateAction::Warn;
        }

        GateAction::Queue
    }

    /// 检查变更规模阈值
    fn check_thresholds(&self, stats: &DiffStats) -> GateAction {
        if stats.files_changed > self.policy.thresholds.max_files_changed
            || stats.added_lines > self.policy.thresholds.max_lines_added
            || stats.deleted_lines > self.policy.thresholds.max_lines_deleted
        {
            return self.policy.actions.on_threshold_exceeded.clone();
        }
        GateAction::Warn
    }

    /// 综合决策
    fn decide_action(
        &self,
        reasons: &[String],
        risk_score: f32,
        commit: &Commit,
    ) -> GateAction {
        // 严重风险：阻止
        if commit.semantic_info.risk_level == RiskLevel::Critical {
            return self.policy.actions.on_critical_risk.clone();
        }

        // 高风险 + 有触发原因
        if risk_score >= 0.5 && !reasons.is_empty() {
            return self.policy.actions.on_high_risk.clone();
        }

        // 无触发原因 → 放行
        if reasons.is_empty() {
            return GateAction::Warn;
        }

        GateAction::Queue
    }

    /// 生成审核项
    pub fn create_review_item(
        &self,
        commit: &Commit,
        decision: &GateDecision,
    ) -> Option<ReviewItem> {
        if decision.action == GateAction::Queue || decision.action == GateAction::Block {
            Some(ReviewItem::new(
                &commit.id.0,
                &commit.message,
                &commit.author.name,
                commit.semantic_info.risk_level.as_str(),
                &decision.reason,
                commit.semantic_info.affected_modules.clone(),
                &commit.semantic_info.semantic_summary,
            ))
        } else {
            None
        }
    }
}

/// Diff 统计信息（用于规模检查）
#[derive(Debug, Clone)]
pub struct DiffStats {
    pub files_changed: u32,
    pub added_lines: u32,
    pub deleted_lines: u32,
}

impl DiffStats {
    pub fn new(files_changed: u32, added_lines: u32, deleted_lines: u32) -> Self {
        Self {
            files_changed,
            added_lines,
            deleted_lines,
        }
    }

    /// 从 diff 文本解析统计信息
    pub fn from_diff(diff: &str) -> Self {
        let mut files_changed = 0u32;
        let mut added_lines = 0u32;
        let mut deleted_lines = 0u32;

        for line in diff.lines() {
            if line.starts_with("--- ") {
                files_changed += 1;
            } else if line.starts_with('+') && !line.starts_with("+++") {
                added_lines += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                deleted_lines += 1;
            }
        }

        Self {
            files_changed,
            added_lines,
            deleted_lines,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::GateConfig;
    use crate::domain::commit::{Author, Commit, RiskLevel, SemanticInfo};

    fn make_commit(risk: RiskLevel, modules: Vec<String>, change_type: crate::domain::commit::ChangeCategory) -> Commit {
        let mut commit = Commit::new(
            "abc123",
            "feat: test feature",
            Author::ai_agent("Cline", "agent-1"),
            chrono::Utc::now(),
            vec![],
        );
        commit.semantic_info = SemanticInfo::new(
            "test intent",
            change_type,
            modules,
            "test summary",
            risk,
        );
        commit
    }

    #[test]
    fn test_gate_disabled() {
        let mut policy = GateConfig::default();
        policy.enabled = false;
        let engine = GateEngine::new(policy);
        let commit = make_commit(RiskLevel::Critical, vec!["auth/".into()], crate::domain::commit::ChangeCategory::Breaking);
        let decision = engine.check(&commit, None);
        assert_eq!(decision.action, GateAction::Warn);
    }

    #[test]
    fn test_gate_critical_blocks() {
        let engine = GateEngine::with_defaults();
        let commit = make_commit(RiskLevel::Critical, vec!["auth/".into()], crate::domain::commit::ChangeCategory::Breaking);
        let decision = engine.check(&commit, None);
        assert_eq!(decision.action, GateAction::Block);
    }

    #[test]
    fn test_diff_stats_parsing() {
        let diff = r#"--- a/file1.rs
+++ b/file1.rs
+added line 1
+added line 2
-old line
--- a/file2.rs
+++ b/file2.rs
+another line"#;
        let stats = DiffStats::from_diff(diff);
        assert_eq!(stats.files_changed, 2);
        assert_eq!(stats.added_lines, 3);
        assert_eq!(stats.deleted_lines, 1);
    }

    #[test]
    fn test_review_item_creation() {
        let engine = GateEngine::with_defaults();
        let mut commit = make_commit(RiskLevel::High, vec!["auth/".into()], crate::domain::commit::ChangeCategory::Bugfix);
        commit.semantic_info = SemanticInfo::new(
            "修复认证 bug",
            crate::domain::commit::ChangeCategory::Bugfix,
            vec!["auth/".into()],
            "修复了登录认证中间件的问题",
            RiskLevel::High,
        );
        let decision = engine.check(&commit, None);
        let item = engine.create_review_item(&commit, &decision);
        assert!(item.is_some());
        assert_eq!(item.unwrap().status, crate::review::ReviewStatus::Pending);
    }
}
