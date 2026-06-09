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

    /// 执行 git revert（真实 git2 操作）
    async fn perform_git_revert(&self, commit_hash: &str) -> Result<Option<String>> {
        use git2::{Oid, Repository};

        let repo_path = self.context.git_repo.repo_path();
        let repo = Repository::open(repo_path).map_err(|e| {
            crate::utils::AetherError::Git(format!(
                "[RollbackAgent] 无法打开仓库 {}: {}",
                repo_path.display(),
                e
            ))
        })?;

        let oid = Oid::from_str(commit_hash).map_err(|e| {
            crate::utils::AetherError::Git(format!(
                "[RollbackAgent] 无效的 commit hash '{}': {}",
                commit_hash, e
            ))
        })?;

        let commit = repo.find_commit(oid).map_err(|e| {
            crate::utils::AetherError::Git(format!(
                "[RollbackAgent] 找不到 commit '{}': {}",
                commit_hash, e
            ))
        })?;

        // 获取 HEAD commit
        let head = repo.head().map_err(|e| {
            crate::utils::AetherError::Git(format!(
                "[RollbackAgent] 无法获取 HEAD: {}",
                e
            ))
        })?;
        let head_commit = head.peel_to_commit().map_err(|e| {
            crate::utils::AetherError::Git(format!("[RollbackAgent] HEAD 解析失败: {}", e))
        })?;

        // 使用 git revert 产生反向 patch 并写入 index
        // git2 0.19 API: revert_commit(commit, our_commit, mainline, merge_options)
        let mut index = repo
            .revert_commit(&commit, &head_commit, 1, None)
            .map_err(|e| {
                crate::utils::AetherError::Git(format!(
                    "[RollbackAgent] revert 失败 '{}': {}",
                    commit_hash, e
                ))
            })?;

        // 检查冲突
        if index.has_conflicts() {
            return Err(crate::utils::AetherError::AgentError(
                "Revert 存在冲突，需要手动解决".to_string(),
            ));
        }

        // 写入 tree
        let tree_id = index.write_tree_to(&repo).map_err(|e| {
            crate::utils::AetherError::Git(format!(
                "[RollbackAgent] 写入 tree 失败: {}",
                e
            ))
        })?;
        let tree = repo.find_tree(tree_id).map_err(|e| {
            crate::utils::AetherError::Git(format!("[RollbackAgent] 查找 tree 失败: {}", e))
        })?;

        // 创建 revert commit
        let signature = repo.signature().unwrap_or_else(|_| {
            git2::Signature::now("AetherVC", "rollback@aether.vc").unwrap()
        });
        let message = format!(
            "Revert \"{}\"\n\nThis reverts commit {}.\n[Auto-reverted by AetherVC RollbackAgent]",
            commit.message().unwrap_or("unknown"),
            commit_hash
        );

        let new_oid = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                &message,
                &tree,
                &[&head_commit],
            )
            .map_err(|e| {
                crate::utils::AetherError::Git(format!(
                    "[RollbackAgent] 创建 revert commit 失败: {}",
                    e
                ))
            })?;

        let new_hash = new_oid.to_string();
        tracing::info!(
            "[RollbackAgent] 已创建 revert commit: {} (revert {})",
            &new_hash[..8.min(new_hash.len())],
            &commit_hash[..8.min(commit_hash.len())]
        );
        Ok(Some(new_hash))
    }

    /// 执行 git reset（带 snapshot 保护）
    async fn perform_git_reset(&self, target_hash: &str) -> Result<Option<String>> {
        use git2::{Oid, Repository, ResetType};

        let repo_path = self.context.git_repo.repo_path();
        let repo = Repository::open(repo_path).map_err(|e| {
            crate::utils::AetherError::Git(format!(
                "[RollbackAgent] 无法打开仓库 {}: {}",
                repo_path.display(),
                e
            ))
        })?;

        // 先创建 snapshot 保护（backup branch）
        let snapshot_name = format!(
            "aether-snapshot-{}",
            Utc::now().format("%Y%m%d-%H%M%S")
        );
        let head = repo.head().map_err(|e| {
            crate::utils::AetherError::Git(format!("[RollbackAgent] HEAD 获取失败: {}", e))
        })?;
        let head_oid = head.target().ok_or_else(|| {
            crate::utils::AetherError::Git("[RollbackAgent] HEAD 无 target".into())
        })?;

        // 创建 snapshot branch
        let head_commit_obj = repo.find_commit(head_oid).map_err(|e| {
            crate::utils::AetherError::Git(format!(
                "[RollbackAgent] 查找 HEAD commit 失败: {}",
                e
            ))
        })?;
        match repo.branch(&snapshot_name, &head_commit_obj, false) {
            Ok(_) => {
                tracing::info!(
                    "[RollbackAgent] 已创建 snapshot branch: {}",
                    snapshot_name
                );
            }
            Err(e) => {
                tracing::warn!(
                    "[RollbackAgent] 创建 snapshot branch 失败: {}",
                    e
                );
            }
        }

        // 执行 reset
        let target_oid = Oid::from_str(target_hash).map_err(|e| {
            crate::utils::AetherError::Git(format!(
                "[RollbackAgent] 无效的 target hash '{}': {}",
                target_hash, e
            ))
        })?;
        let target_commit = repo.find_commit(target_oid).map_err(|e| {
            crate::utils::AetherError::Git(format!(
                "[RollbackAgent] 找不到 target commit '{}': {}",
                target_hash, e
            ))
        })?;

        repo.reset(target_commit.as_object(), ResetType::Hard, None)
            .map_err(|e| {
                crate::utils::AetherError::Git(format!(
                    "[RollbackAgent] git reset 失败: {}",
                    e
                ))
            })?;

        tracing::info!(
            "[RollbackAgent] 已执行 git reset --hard to {}; snapshot 保存在 {}",
            target_hash,
            snapshot_name
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::base::AgentContext;
    use crate::llm::client::MockLLMClient;
    use crate::semantic::embedder::MockEmbedder;
    use crate::storage::git::GitRepository;
    use crate::storage::graph_db::InMemoryGraphStore;
    use crate::storage::vector_db::InMemoryVectorStore;

    fn test_context() -> Arc<AgentContext> {
        Arc::new(
            AgentContext::new(
                Arc::new(GitRepository::new(".")),
                Arc::new(InMemoryVectorStore::new()),
                Arc::new(InMemoryGraphStore::new()),
                Arc::new(MockEmbedder::default()),
            )
            .with_llm(Arc::new(MockLLMClient::new())),
        )
    }

    fn test_config() -> RollbackConfig {
        RollbackConfig {
            enabled: true,
            auto_rollback_on_compile_failure: true,
            auto_rollback_on_test_failure: false,
            test_failure_threshold: 0.1,
            auto_rollback_on_security_cve: true,
            require_human_approval: false,
            max_auto_rollbacks_per_hour: 3,
        }
    }

    /// 编译失败 + auto_rollback_on_compile_failure=true → 返回 Revert
    #[tokio::test]
    async fn test_analyze_compile_failure() {
        let agent = RollbackAgent::new(test_context(), test_config());
        let result = agent
            .analyze_need_rollback(
                "abc123",
                &serde_json::json!({
                    "checks": [
                        {"name": "compile", "status": "failed", "duration_ms": 500},
                        {"name": "lint", "status": "passed", "duration_ms": 300},
                    ]
                }),
            )
            .await
            .unwrap();
        assert!(result.is_some());
        match result.unwrap() {
            RollbackAction::Revert { commit_hash } => assert_eq!(commit_hash, "abc123"),
            _ => panic!("Expected Revert action"),
        }
    }

    /// 安全漏洞 + auto_rollback_on_security_cve=true → 返回 Revert
    #[tokio::test]
    async fn test_analyze_security_issue() {
        let agent = RollbackAgent::new(test_context(), test_config());
        let result = agent
            .analyze_need_rollback(
                "def456",
                &serde_json::json!({
                    "checks": [
                        {"name": "compile", "status": "passed"},
                        {"name": "security_scan", "status": "failed"},
                    ]
                }),
            )
            .await
            .unwrap();
        assert!(result.is_some());
        match result.unwrap() {
            RollbackAction::Revert { commit_hash } => assert_eq!(commit_hash, "def456"),
            _ => panic!("Expected Revert action for security issue"),
        }
    }

    /// 测试失败率 50% > 阈值 10% → 返回 NotifyOnly
    #[tokio::test]
    async fn test_analyze_test_failure_threshold_exceeded() {
        let agent = RollbackAgent::new(test_context(), test_config());
        let result = agent
            .analyze_need_rollback(
                "ghi789",
                &serde_json::json!({
                    "checks": [
                        {"name": "compile", "status": "passed"},
                        {"name": "test_a", "status": "failed"},
                        {"name": "test_b", "status": "passed"},
                    ]
                }),
            )
            .await
            .unwrap();
        // auto_rollback_on_test_failure=false, 所以应返回 NotifyOnly
        assert!(result.is_some());
        match result.unwrap() {
            RollbackAction::NotifyOnly { .. } => {}
            _ => panic!("Expected NotifyOnly action"),
        }
    }

    /// 全部通过 → 无需回滚
    #[tokio::test]
    async fn test_analyze_all_passed() {
        let agent = RollbackAgent::new(test_context(), test_config());
        let result = agent
            .analyze_need_rollback(
                "jkl012",
                &serde_json::json!({
                    "checks": [
                        {"name": "compile", "status": "passed"},
                        {"name": "test_a", "status": "passed"},
                    ]
                }),
            )
            .await
            .unwrap();
        assert!(result.is_none());
    }

    /// config.enabled=false → 永远不触发回滚
    #[tokio::test]
    async fn test_analyze_disabled_config() {
        let mut cfg = test_config();
        cfg.enabled = false;
        let agent = RollbackAgent::new(test_context(), cfg);
        let result = agent
            .analyze_need_rollback(
                "abc",
                &serde_json::json!({
                    "checks": [{"name": "compile", "status": "failed"}]
                }),
            )
            .await
            .unwrap();
        assert!(result.is_none());
    }

    /// 更新信誉分
    #[test]
    fn test_update_reputation() {
        let agent = RollbackAgent::new(test_context(), test_config());
        agent.update_reputation("Cline", -0.05);
        let score = agent.get_reputation("Cline");
        assert!((score - 0.95).abs() < 1e-5);
    }

    /// execute "analyze_rollback" 任务
    #[tokio::test]
    async fn test_execute_analyze_rollback() {
        let agent = RollbackAgent::new(test_context(), test_config());
        let task = AgentTask::new(
            "analyze_rollback",
            serde_json::json!({
                "commit_hash": "abc123",
                "verification_result": {
                    "checks": [
                        {"name": "compile", "status": "failed"}
                    ]
                }
            }),
        );
        let result = agent.execute(task).await.unwrap();
        assert!(result.success);
        assert_eq!(
            result.output.get("needs_rollback").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    /// execute "reputation" 任务
    #[tokio::test]
    async fn test_execute_reputation() {
        let agent = RollbackAgent::new(test_context(), test_config());
        agent.update_reputation("Claude", 0.0); // 初始化
        let task = AgentTask::new("reputation", serde_json::json!({}));
        let result = agent.execute(task).await.unwrap();
        assert!(result.success);
    }

    /// execute 未知任务类型 → 失败
    #[tokio::test]
    async fn test_execute_unknown_task() {
        let agent = RollbackAgent::new(test_context(), test_config());
        let task = AgentTask::new("unknown_task", serde_json::json!({}));
        let result = agent.execute(task).await.unwrap();
        assert!(!result.success);
    }

    /// agent_type 返回正确的 AgentType
    #[test]
    fn test_agent_type() {
        let agent = RollbackAgent::new(test_context(), test_config());
        assert_eq!(agent.agent_type(), AgentType::Rollback);
    }

    /// 历史记录初始为空
    #[test]
    fn test_history_starts_empty() {
        let agent = RollbackAgent::new(test_context(), test_config());
        assert!(agent.get_history().is_empty());
    }

    /// 信誉分初始为 1.0
    #[test]
    fn test_reputation_starts_at_one() {
        let agent = RollbackAgent::new(test_context(), test_config());
        assert_eq!(agent.get_reputation("unknown_agent"), 1.0);
    }
}
