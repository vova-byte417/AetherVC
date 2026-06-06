//! 智能回滚 Agent
//!
//! 核心职责：
//! 1. 监控验证结果，判定是否需要回滚
//! 2. 生成回滚方案（git revert / git reset）
//! 3. 执行回滚并记录历史
//! 4. 更新 Agent 信誉分

use crate::agents::base::{Agent, AgentContext, AgentStatus};
use crate::config::types::RollbackConfig;
use crate::domain::agent::{
    AgentResult, AgentTask, AgentType, RollbackAction, RollbackRecord, RollbackStatus,
};
use crate::utils::Result;
use async_trait::async_trait;
use chrono::Utc;
use std::sync::{Arc, Mutex};

/// 智能回滚 Agent
pub struct RollbackAgent {
    context: Arc<AgentContext>,
    status: Mutex<AgentStatus>,
    /// 回滚历史记录
    history: Mutex<Vec<RollbackRecord>>,
    /// Agent 信誉分记录
    reputation: Mutex<std::collections::HashMap<String, f32>>,
    config: RollbackConfig,
}

impl RollbackAgent {
    pub fn new(context: Arc<AgentContext>, config: RollbackConfig) -> Self {
        Self {
            context,
            status: Mutex::new(AgentStatus::Idle),
            history: Mutex::new(Vec::new()),
            reputation: Mutex::new(std::collections::HashMap::new()),
            config,
        }
    }

    /// 根据验证报告判断是否需要回滚
    async fn analyze_need_rollback(
        &self,
        commit_hash: &str,
        verification_result: &serde_json::Value,
    ) -> Result<Option<RollbackAction>> {
        if !self.config.enabled {
            return Ok(None);
        }

        let checks = verification_result
            .get("checks")
            .and_then(|v| v.as_array());

        let mut compile_failed = false;
        let mut test_failed_count = 0u32;
        let mut test_total = 0u32;
        let mut security_issue = false;

        if let Some(checks) = checks {
            for check in checks {
                let name = check.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let status = check.get("status").and_then(|v| v.as_str()).unwrap_or("");

                let failed = status == "failed";

                if name.contains("compile") && failed {
                    compile_failed = true;
                }
                if name.contains("test") {
                    test_total += 1;
                    if failed {
                        test_failed_count += 1;
                    }
                }
                if name.contains("security") && failed {
                    security_issue = true;
                }
            }
        }

        // 编译失败 → 立即回滚
        if compile_failed && self.config.auto_rollback_on_compile_failure {
            return Ok(Some(RollbackAction::Revert {
                commit_hash: commit_hash.to_string(),
            }));
        }

        // 安全漏洞 → 立即回滚
        if security_issue && self.config.auto_rollback_on_security_cve {
            return Ok(Some(RollbackAction::Revert {
                commit_hash: commit_hash.to_string(),
            }));
        }

        // 测试失败率超阈值 → 建议回滚
        if test_total > 0 {
            let failure_rate = test_failed_count as f32 / test_total as f32;
            if failure_rate > self.config.test_failure_threshold {
                if self.config.auto_rollback_on_test_failure {
                    return Ok(Some(RollbackAction::Revert {
                        commit_hash: commit_hash.to_string(),
                    }));
                } else {
                    // 仅通知
                    return Ok(Some(RollbackAction::NotifyOnly {
                        reason: format!(
                            "测试失败率 {:.0}% 超过阈值 {:.0}%",
                            failure_rate * 100.0,
                            self.config.test_failure_threshold * 100.0
                        ),
                    }));
                }
            }
        }

        Ok(None)
    }

    /// 执行回滚操作
    async fn execute_rollback(
        &self,
        commit_hash: &str,
        action: &RollbackAction,
        require_approval: bool,
    ) -> Result<RollbackRecord> {
        let record_id = format!("RB-{}", Utc::now().format("%Y%m%d-%H%M%S"));

        if require_approval && self.config.require_human_approval {
            // 需要人工确认
            return Ok(RollbackRecord {
                id: record_id,
                rolled_back_commit: commit_hash.to_string(),
                revert_commit: None,
                reason: format!("{:?}", action),
                action: format!("{:?}", action),
                status: RollbackStatus::PendingApproval,
                executed_at: Utc::now(),
                agent_name: "unknown".to_string(),
            });
        }

        let revert_result = match action {
            RollbackAction::Revert { commit_hash: ch } => {
                // 执行 git revert
                self.perform_git_revert(ch).await
            }
            RollbackAction::Reset {
                target_hash,
                hard: _,
            } => {
                // 执行 git reset（危险操作，需要特别确认）
                self.perform_git_reset(target_hash).await
            }
            RollbackAction::NotifyOnly { .. } => Ok(None),
        };

        let status = match &revert_result {
            Ok(Some(_)) => RollbackStatus::Executed,
            Ok(None) => RollbackStatus::Cancelled,
            Err(e) => RollbackStatus::Failed(e.to_string()),
        };

        let record = RollbackRecord {
            id: record_id,
            rolled_back_commit: commit_hash.to_string(),
            revert_commit: revert_result.ok().flatten(),
            reason: format!("{:?}", action),
            action: format!("{:?}", action),
            status,
            executed_at: Utc::now(),
            agent_name: "unknown".to_string(),
        };

        // 更新 Agent 信誉分
        if matches!(record.status, RollbackStatus::Executed) {
            self.update_reputation(&record.agent_name, -0.05);
        }

        Ok(record)
    }

    /// 执行 git revert
    async fn perform_git_revert(&self, _commit_hash: &str) -> Result<Option<String>> {
        // 实际项目中通过 git2 库执行 revert
        // 这里返回模拟的新 revert commit hash
        let new_hash = format!("revert-{}", Utc::now().timestamp());
        tracing::info!("[RollbackAgent] 执行 git revert: new_commit={}", new_hash);
        Ok(Some(new_hash))
    }

    /// 执行 git reset
    async fn perform_git_reset(&self, _target_hash: &str) -> Result<Option<String>> {
        tracing::warn!("[RollbackAgent] git reset 是危险操作，需要额外确认");
        Ok(None)
    }

    /// 更新 Agent 信誉分
    fn update_reputation(&self, agent_name: &str, delta: f32) {
        let mut rep = self.reputation.lock().unwrap();
        let score = rep.entry(agent_name.to_string()).or_insert(1.0);
        *score = (*score + delta).clamp(0.0, 1.0);
        tracing::info!(
            "[RollbackAgent] Agent '{}' 信誉分更新: {} → {}",
            agent_name,
            *score - delta,
            *score
        );
    }

    /// 获取 Agent 信誉分
    pub fn get_reputation(&self, agent_name: &str) -> f32 {
        self.reputation
            .lock()
            .unwrap()
            .get(agent_name)
            .copied()
            .unwrap_or(1.0)
    }

    /// 获取回滚历史
    pub fn get_history(&self) -> Vec<RollbackRecord> {
        self.history.lock().unwrap().clone()
    }
}

#[async_trait]
impl Agent for RollbackAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Rollback
    }

    async fn execute(&self, task: AgentTask) -> Result<AgentResult> {
        *self.status.lock().unwrap() = AgentStatus::Running;
        let start = std::time::Instant::now();

        let task_type = task.task_type.as_str();
        let commit_hash = task
            .input
            .get("commit_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("HEAD");

        match task_type {
            "analyze_rollback" => {
                let verification = task.input.get("verification_result").cloned();

                let result = if let Some(ref v) = verification {
                    self.analyze_need_rollback(commit_hash, v).await?
                } else {
                    // 没有验证结果，假设需要人工决策
                    Some(RollbackAction::NotifyOnly {
                        reason: "缺少验证结果，建议人工检查".to_string(),
                    })
                };

                *self.status.lock().unwrap() = AgentStatus::Completed;

                Ok(AgentResult::success(
                    task.id,
                    serde_json::json!({
                        "commit_hash": commit_hash,
                        "needs_rollback": result.is_some(),
                        "suggested_action": result,
                    }),
                    start.elapsed().as_millis() as u64,
                ))
            }
            "execute_rollback" => {
                let action_json = task.input.get("action");
                let require_approval = task
                    .input
                    .get("require_approval")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);

                let action = if let Some(a) = action_json {
                    match a.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                        "revert" => RollbackAction::Revert {
                            commit_hash: commit_hash.to_string(),
                        },
                        "reset" => RollbackAction::Reset {
                            target_hash: a
                                .get("target")
                                .and_then(|v| v.as_str())
                                .unwrap_or("HEAD~1")
                                .to_string(),
                            hard: a.get("hard").and_then(|v| v.as_bool()).unwrap_or(false),
                        },
                        _ => RollbackAction::Revert {
                            commit_hash: commit_hash.to_string(),
                        },
                    }
                } else {
                    RollbackAction::Revert {
                        commit_hash: commit_hash.to_string(),
                    }
                };

                let record = self
                    .execute_rollback(commit_hash, &action, require_approval)
                    .await?;

                self.history.lock().unwrap().push(record.clone());

                *self.status.lock().unwrap() = AgentStatus::Completed;

                Ok(AgentResult::success(
                    task.id,
                    serde_json::to_value(&record).unwrap_or_default(),
                    start.elapsed().as_millis() as u64,
                ))
            }
            "rollback_history" => {
                let history = self.get_history();
                *self.status.lock().unwrap() = AgentStatus::Completed;

                Ok(AgentResult::success(
                    task.id,
                    serde_json::to_value(&history).unwrap_or_default(),
                    start.elapsed().as_millis() as u64,
                ))
            }
            "reputation" => {
                let rep: std::collections::HashMap<String, f32> =
                    self.reputation.lock().unwrap().clone();
                *self.status.lock().unwrap() = AgentStatus::Completed;

                Ok(AgentResult::success(
                    task.id,
                    serde_json::to_value(&rep).unwrap_or_default(),
                    start.elapsed().as_millis() as u64,
                ))
            }
            _ => {
                *self.status.lock().unwrap() = AgentStatus::Completed;

                Ok(AgentResult::failure(
                    task.id,
                    format!("Unknown rollback task type: {}", task_type),
                ))
            }
        }
    }

    fn status(&self) -> AgentStatus {
        self.status.lock().unwrap().clone()
    }
}
