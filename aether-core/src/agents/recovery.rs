use crate::agents::base::{Agent, AgentContext, AgentStatus};
use crate::domain::agent::{AgentResult, AgentTask, AgentType};
use crate::domain::recovery::{RecoveryRequest, RecoveryResult, RecoveryStatus};
use crate::utils::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// 跨版本恢复 Agent
/// 核心功能：根据自然语言描述定位历史代码、生成兼容 patch、处理冲突
pub struct CrossCommitRecoveryAgent {
    context: Arc<AgentContext>,
    status: std::sync::Mutex<AgentStatus>,
}

impl CrossCommitRecoveryAgent {
    pub fn new(context: Arc<AgentContext>) -> Self {
        Self {
            context,
            status: std::sync::Mutex::new(AgentStatus::Idle),
        }
    }

    /// 第一步：定位目标 commit
    async fn locate_target_commit(&self, query: &str) -> Result<Vec<String>> {
        // 1. 向量化查询
        let query_embedding = self.context.embedder.embed(query).await?;

        // 2. 向量搜索
        let results = self
            .context
            .vector_store
            .search_similar(&query_embedding, 10, None)
            .await?;

        Ok(results.into_iter().map(|r| r.commit_hash).collect())
    }

    /// 第二步：分析 diff 并生成 patch
    async fn generate_patch(
        &self,
        target_commits: &[String],
        query: &str,
    ) -> Result<String> {
        let mut patches = Vec::new();

        for hash in target_commits.iter().take(3) {
            let diff = self.context.git_repo.get_commit_diff(hash).await?;
            patches.push(format!("--- Commit: {} ---\n{}", hash, diff));
        }

        // 如果有 LLM，使用 LLM 生成 patch
        if let Some(ref llm) = self.context.llm_client {
            let prompt = format!(
                "用户需求：{}\n\n相关的历史变更：\n{}\n\n请生成一个兼容当前代码库的 patch：",
                query,
                patches.join("\n\n")
            );
            let response = llm.complete(&prompt).await?;
            return Ok(response.content);
        }

        // 回退：直接返回找到的 diff
        Ok(patches.join("\n\n"))
    }

    /// 检测冲突
    async fn detect_conflicts(&self, patch: &str) -> Result<Vec<crate::domain::recovery::Conflict>> {
        let mut conflicts = Vec::new();

        // 简单冲突检测：提取 patch 中涉及的文件
        let mut files = std::collections::HashSet::new();
        for line in patch.lines() {
            if line.starts_with("--- a/") || line.starts_with("+++ b/") {
                if let Some(path) = line.split_whitespace().nth(1) {
                    files.insert(
                        path.trim_start_matches("a/")
                            .trim_start_matches("b/")
                            .to_string(),
                    );
                }
            }
        }

        for file in files {
            // 检查文件是否在后续 commit 中被修改过
            let search_results = self
                .context
                .vector_store
                .search_similar(
                    &self.context.embedder.embed(&file).await?,
                    3,
                    None,
                )
                .await?;

            if !search_results.is_empty() {
                conflicts.push(crate::domain::recovery::Conflict::new(
                    file,
                    format!("文件在后续 {} 个 commit 中被修改", search_results.len()),
                ));
            }
        }

        Ok(conflicts)
    }

    /// 生成恢复方案
    async fn build_recovery_plan(
        &self,
        target_commits: Vec<String>,
        patch: String,
        conflicts: Vec<crate::domain::recovery::Conflict>,
    ) -> RecoveryResult {
        let has_conflicts = !conflicts.is_empty();
        RecoveryResult {
            recovered_commit: target_commits.first().cloned().unwrap_or_default(),
            patch,
            conflicts,
            new_commit_hash: None,
            warnings: if has_conflicts {
                vec!["存在冲突，建议手动检查合并".to_string()]
            } else {
                vec![]
            },
        }
    }
}

#[async_trait]
impl Agent for CrossCommitRecoveryAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::CrossCommitRecovery
    }

    async fn execute(&self, task: AgentTask) -> Result<AgentResult> {
        *self.status.lock().unwrap() = AgentStatus::Running;
        let start = std::time::Instant::now();

        let request: RecoveryRequest = match serde_json::from_value(task.input) {
            Ok(req) => req,
            Err(e) => {
                return Ok(AgentResult::failure(
                    task.id,
                    format!("Invalid input: {}", e),
                ));
            }
        };

        // 步骤 1: 定位目标 commit
        let target_commits = match self
            .locate_target_commit(&request.natural_language_query)
            .await
        {
            Ok(c) => c,
            Err(e) => {
                return Ok(AgentResult::failure(
                    task.id,
                    format!("Failed to locate commit: {}", e),
                ));
            }
        };

        if target_commits.is_empty() {
            return Ok(AgentResult::failure(
                task.id,
                "No matching commits found",
            ));
        }

        // 步骤 2: 生成 patch
        let patch = match self
            .generate_patch(&target_commits, &request.natural_language_query)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                return Ok(AgentResult::failure(
                    task.id,
                    format!("Failed to generate patch: {}", e),
                ));
            }
        };

        // 步骤 3: 检测冲突
        let conflicts = match self.detect_conflicts(&patch).await {
            Ok(c) => c,
            Err(e) => vec![crate::domain::recovery::Conflict::new(
                "unknown",
                format!("Conflict detection error: {}", e),
            )],
        };

        // 步骤 4: 构建恢复方案
        let recovery_result = self
            .build_recovery_plan(target_commits, patch, conflicts)
            .await;

        let elapsed = start.elapsed().as_millis() as u64;
        *self.status.lock().unwrap() = AgentStatus::Completed;

        Ok(AgentResult::success(
            task.id,
            serde_json::to_value(&recovery_result).unwrap_or_default(),
            elapsed,
        ))
    }

    fn status(&self) -> AgentStatus {
        self.status.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::base::AgentContext;
    use crate::domain::recovery::RecoveryRequest;
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

    #[tokio::test]
    async fn test_fail_when_no_commits_found() {
        let agent = CrossCommitRecoveryAgent::new(test_context());
        let request = RecoveryRequest::new(
            "恢复用户画像模块",
            crate::domain::commit::CurrentState::new("main", "abc123"),
            "test-user",
        );

        let task = AgentTask::new("recover_commit", serde_json::to_value(request).unwrap());
        let result = agent.execute(task).await.unwrap();
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_agent_type() {
        let agent = CrossCommitRecoveryAgent::new(test_context());
        assert_eq!(agent.agent_type(), AgentType::CrossCommitRecovery);
    }
}
