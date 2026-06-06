//! 端到端工作流编排器
//!
//! 串联多个 Agent 完成完整的自动化工作流：
//! - Digest: 摘要 + 风险排序 + 门控 + 验证
//! - Coordinate: 冲突检测 + 合并分析 + 建议生成
//! - VerifyTags: 搜索 + 排序 + 验证 + 部署建议
//! - FullCI: 收到新 commit → 分析 → 门控 → 验证 → 回滚判断

use crate::agents::orchestrator::AgentOrchestrator;
use crate::config::types::AetherConfig;
use crate::digest::{DigestAggregator, DigestOptions, DigestSummarizer};
use crate::domain::agent::AgentTask;
use crate::review::{GateEngine, ReviewQueue};
use crate::storage::git::GitOperations;
use crate::verify::{VerificationRunner, VerifyMode};
use crate::utils::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 工作流类型
pub enum WorkflowType {
    /// 日常消化：摘要 + 风险排序 + 门控 + 验证
    Digest,
    /// 多 Agent 协调：冲突检测 + 合并分析 + 建议生成
    Coordinate,
    /// Tag 验证：搜索 + 排序 + 验证 + 部署建议
    VerifyTags,
    /// 完整 CI：分析 → 门控 → 验证 → 回滚判断
    FullCI,
}

/// 工作流结果
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkflowResult {
    pub workflow_type: String,
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub duration_ms: u64,
}

/// 端到端工作流编排器
pub struct WorkflowOrchestrator {
    agent_orchestrator: Arc<AgentOrchestrator>,
    pub gate_engine: GateEngine,
    pub verification_runner: VerificationRunner,
    pub review_queue: Mutex<ReviewQueue>,
    config: AetherConfig,
    repo_path: String,
}

impl WorkflowOrchestrator {
    pub fn new(
        agent_orchestrator: Arc<AgentOrchestrator>,
        config: AetherConfig,
        repo_path: impl Into<String>,
    ) -> Self {
        Self {
            agent_orchestrator,
            gate_engine: GateEngine::new(config.gate.clone()),
            verification_runner: VerificationRunner::new(config.verify.clone()),
            review_queue: Mutex::new(ReviewQueue::new()),
            config,
            repo_path: repo_path.into(),
        }
    }

    /// 执行工作流
    pub async fn execute(&self, workflow: WorkflowType) -> Result<WorkflowResult> {
        match workflow {
            WorkflowType::Digest => self.execute_digest_flow().await,
            WorkflowType::Coordinate => self.execute_coordinate_flow().await,
            WorkflowType::VerifyTags => self.execute_verify_tags_flow().await,
            WorkflowType::FullCI => self.execute_full_ci_flow().await,
        }
    }

    /// 工作流 1：日常消化流程
    async fn execute_digest_flow(&self) -> Result<WorkflowResult> {
        let start = std::time::Instant::now();

        // Step 1: 生成聚合摘要
        let git_repo = Arc::new(crate::storage::git::GitRepository::open(&self.repo_path)?);
        let aggregator = DigestAggregator::new(git_repo);
        let options = DigestOptions {
            since: None,
            until: None,
            group_by: crate::digest::DigestGroupBy::None,
            risk_threshold: String::new(),
            commit_range: None,
        };

        let report = aggregator.aggregate(&options).await?;

        // Step 2: 生成 Markdown 摘要
        let summarizer = DigestSummarizer::new();
        let markdown = summarizer.render_markdown(&report);

        // Step 3: 输出结果
        let data = serde_json::json!({
            "markdown": markdown,
            "total_commits": report.total_commits,
            "risk_distribution": {
                "critical": report.risk_distribution.critical,
                "high": report.risk_distribution.high,
                "medium": report.risk_distribution.medium,
                "low": report.risk_distribution.low,
            },
            "high_risk_count": report.high_risk_items.len(),
            "safe_count": report.safe_items.len(),
        });

        Ok(WorkflowResult {
            workflow_type: "digest".into(),
            success: true,
            message: format!(
                "摘要已生成: {} 个 commit, {} 个高风险项, {} 个安全项",
                report.total_commits,
                report.high_risk_items.len(),
                report.safe_items.len()
            ),
            data: Some(data),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// 工作流 2：多 Agent 协调流程
    async fn execute_coordinate_flow(&self) -> Result<WorkflowResult> {
        let start = std::time::Instant::now();

        let task = AgentTask::new("coordinate_agents", serde_json::json!({
            "mode": "full",
            "window_minutes": self.config.coordinator.monitor_window_minutes,
        }));

        let result = self.agent_orchestrator.execute_task(task).await?;

        let matrix = result.output;

        Ok(WorkflowResult {
            workflow_type: "coordinate".into(),
            success: result.success,
            message: "多 Agent 协调完成".into(),
            data: Some(matrix),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// 工作流 3：Tag 验证流程
    async fn execute_verify_tags_flow(&self) -> Result<WorkflowResult> {
        let start = std::time::Instant::now();

        let task = AgentTask::new("validate_tag", serde_json::json!({
            "keyword": "",
            "max_tags": 15,
            "order_by": "risk_asc",
        }));

        let result = self.agent_orchestrator.execute_task(task).await?;

        Ok(WorkflowResult {
            workflow_type: "verify_tags".into(),
            success: result.success,
            message: "Tag 验证完成".into(),
            data: Some(result.output),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// 工作流 4：完整 CI 流程
    async fn execute_full_ci_flow(&self) -> Result<WorkflowResult> {
        let start = std::time::Instant::now();

        // 简化实现：获取最新 commit 并运行完整流程
        let commits: Vec<crate::domain::commit::Commit> = crate::storage::git::GitRepository::open(&self.repo_path)?
            .list_commits()
            .await?;

        let latest_commit = match commits.first() {
            Some(c) => c,
            None => {
                return Ok(WorkflowResult {
                    workflow_type: "full_ci".into(),
                    success: true,
                    message: "No commits to analyze".into(),
                    data: None,
                    duration_ms: 0,
                });
            }
        };

        // 1. 语义分析
        let analyze_task = AgentTask::new("analyze_semantic", serde_json::json!({
            "diff": "",
            "message": latest_commit.message,
        }));
        let _analysis = self.agent_orchestrator.execute_task(analyze_task).await?;

        // 2. 门控检查
        let gate_decision = self.gate_engine.check(latest_commit, None);

        // 3. 如果需要验证，运行验证
        let verify_result = if matches!(gate_decision.action, crate::config::types::GateAction::Queue) {
            let mut runner = VerificationRunner::new(self.config.verify.clone());
            let report = runner
                .run(&latest_commit.id.0, VerifyMode::Smart, &self.repo_path)
                .await;
            Some(report)
        } else {
            None
        };

        // 4. 如果验证失败，触发回滚分析
        let rollback_needed = if let Some(ref report) = verify_result {
            let rollback_task = AgentTask::new("analyze_rollback", serde_json::json!({
                "commit_hash": latest_commit.id.0,
                "verification_result": serde_json::to_value(report).unwrap_or_default(),
            }));
            let rollback_result = self.agent_orchestrator.execute_task(rollback_task).await?;
            rollback_result
                .output
                .get("needs_rollback")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        } else {
            false
        };

        let data = serde_json::json!({
            "commit": latest_commit.id.0,
            "gate_action": format!("{:?}", gate_decision.action),
            "gate_reason": gate_decision.reason,
            "verification": verify_result.map(|r| serde_json::to_value(&r).unwrap_or_default()),
            "rollback_needed": rollback_needed,
        });

        Ok(WorkflowResult {
            workflow_type: "full_ci".into(),
            success: true,
            message: format!(
                "Full CI 完成: commit={}, gate={:?}, rollback_needed={}",
                &latest_commit.id.0[..8.min(latest_commit.id.0.len())],
                gate_decision.action,
                rollback_needed
            ),
            data: Some(data),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }
}
