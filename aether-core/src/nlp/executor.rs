//! 指令执行器 - 将解析后的命令转换并执行对应的 Agent 任务

use crate::agents::orchestrator::AgentOrchestrator;
use crate::domain::agent::AgentTask;
use crate::domain::commit::CurrentState;
use crate::domain::recovery::RecoveryRequest;
use crate::nlp::parser::{CommandType, ParsedCommand};
use crate::utils::Result;
use serde::{Deserialize, Serialize};

/// 指令执行器
pub struct CommandExecutor {
    orchestrator: std::sync::Arc<AgentOrchestrator>,
    _repo_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl CommandExecutor {
    pub fn new(orchestrator: std::sync::Arc<AgentOrchestrator>, repo_path: impl Into<String>) -> Self {
        Self {
            orchestrator,
            _repo_path: repo_path.into(),
        }
    }

    /// 执行解析后的指令
    pub async fn execute(
        &self,
        parsed: &ParsedCommand,
        current_state: &CurrentState,
    ) -> Result<ExecutionResult> {
        match parsed.command_type {
            CommandType::Recovery => self.execute_recovery(parsed, current_state).await,
            CommandType::Merge => self.execute_merge(parsed).await,
            CommandType::Search => self.execute_search(parsed).await,
            CommandType::Query => self.execute_query(parsed).await,
            _ => Ok(ExecutionResult {
                success: true,
                message: format!("Command type {:?} acknowledged", parsed.command_type),
                data: None,
            }),
        }
    }

    async fn execute_recovery(
        &self,
        parsed: &ParsedCommand,
        current_state: &CurrentState,
    ) -> Result<ExecutionResult> {
        let recovery_request = RecoveryRequest::new(
            &parsed.raw_text,
            current_state.clone(),
            "user",
        );

        let task = AgentTask::new(
            "recover_commit",
            serde_json::to_value(recovery_request).unwrap_or_default(),
        );

        let result = self.orchestrator.execute_task(task).await?;

        Ok(ExecutionResult {
            success: result.success,
            message: if result.success {
                "恢复操作完成".to_string()
            } else {
                result
                    .error_message
                    .unwrap_or_else(|| "恢复失败".to_string())
            },
            data: if result.success {
                Some(result.output)
            } else {
                None
            },
        })
    }

    async fn execute_merge(&self, parsed: &ParsedCommand) -> Result<ExecutionResult> {
        let task = AgentTask::new(
            "merge_prs",
            serde_json::json!({
                "prs": [],
                "description": parsed.raw_text,
            }),
        );

        let result = self.orchestrator.execute_task(task).await?;

        Ok(ExecutionResult {
            success: result.success,
            message: if result.success {
                "合并分析完成".to_string()
            } else {
                result
                    .error_message
                    .unwrap_or_else(|| "合并分析失败".to_string())
            },
            data: if result.success {
                Some(result.output)
            } else {
                None
            },
        })
    }

    async fn execute_search(&self, parsed: &ParsedCommand) -> Result<ExecutionResult> {
        Ok(ExecutionResult {
            success: true,
            message: format!("搜索: {}", parsed.raw_text),
            data: None,
        })
    }

    async fn execute_query(&self, parsed: &ParsedCommand) -> Result<ExecutionResult> {
        Ok(ExecutionResult {
            success: true,
            message: format!("查询: {}", parsed.raw_text),
            data: None,
        })
    }
}
