//! 多 Agent 协调 Agent
//!
//! 核心职责：
//! 1. 感知多个 AI Agent 的并行提交活动
//! 2. 检测语义冲突（同一模块同时被多个 Agent 修改）
//! 3. 给出合并顺序建议和冲突解决方案

use crate::agents::base::{Agent, AgentContext, AgentStatus};
use crate::domain::agent::{
    AgentConflictMatrix, AgentIdentity, AgentResult, AgentTask, AgentType, ConflictSeverity,
    CoordinationPlan, MergeOrderItem, ModuleHotspot,
};
use crate::utils::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 多 Agent 协调 Agent
pub struct MultiAgentCoordinatorAgent {
    context: Arc<AgentContext>,
    status: Mutex<AgentStatus>,
}

impl MultiAgentCoordinatorAgent {
    pub fn new(context: Arc<AgentContext>) -> Self {
        Self {
            context,
            status: Mutex::new(AgentStatus::Idle),
        }
    }

    /// 收集活跃 Agent 的身份信息
    async fn collect_agents(&self) -> Result<Vec<AgentIdentity>> {
        let commits = self.context.git_repo.list_commits().await?;

        let mut agent_map: HashMap<String, AgentIdentity> = HashMap::new();

        for commit in &commits {
            let name = commit.author.name.clone();
            let email = commit.author.email.clone();
            let key = format!("{}:{}", name, email);

            if !agent_map.contains_key(&key) {
                // 简单启发式：email 包含 "ai" "bot" "agent" 视为 AI agent
                let is_ai = email.contains("ai")
                    || email.contains("bot")
                    || email.contains("agent")
                    || name.contains("Cline")
                    || name.contains("Copilot")
                    || name.contains("Claude");

                agent_map.insert(
                    key,
                    AgentIdentity {
                        name,
                        email,
                        is_ai_agent: is_ai,
                    },
                );
            }
        }

        Ok(agent_map.into_values().collect())
    }

    /// 构建模块 × Agent 变更矩阵
    async fn build_conflict_matrix(
        &self,
        agents: &[AgentIdentity],
    ) -> Result<AgentConflictMatrix> {
        let commits = self.context.git_repo.list_commits().await?;

        // module_path -> (agent_name -> change_count)
        let mut module_agent_counts: HashMap<String, HashMap<String, u32>> = HashMap::new();

        for commit in &commits {
            let agent_name = &commit.author.name;
            for module in &commit.semantic_info.affected_modules {
                module_agent_counts
                    .entry(module.clone())
                    .or_default()
                    .entry(agent_name.clone())
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
            }
        }

        // 识别热点模块
        let threshold = 2u32; // 可通过配置控制
        let mut hotspots = Vec::new();

        for (module, agent_counts) in &module_agent_counts {
            let agents_involved: Vec<String> = agent_counts
                .iter()
                .filter(|(_, count)| **count > 0)
                .map(|(name, _)| name.clone())
                .collect();

            if agents_involved.len() >= threshold as usize {
                let severity = if agents_involved.len() > 3 {
                    ConflictSeverity::Critical
                } else if agents_involved.len() > 2 {
                    ConflictSeverity::High
                } else {
                    ConflictSeverity::Medium
                };

                hotspots.push(ModuleHotspot {
                    module_path: module.clone(),
                    agents_involved,
                    severity,
                    overlapping_functions: Vec::new(), // 由 LLM 填充
                });
            }
        }

        // 生成协调建议
        let recommendation = self.generate_coordination_plan(&hotspots).await;

        Ok(AgentConflictMatrix {
            agents: agents.to_vec(),
            module_agent_counts,
            hotspots,
            recommendation,
        })
    }

    /// 生成协调计划（优先用 LLM，回退到规则）
    async fn generate_coordination_plan(
        &self,
        hotspots: &[ModuleHotspot],
    ) -> CoordinationPlan {
        if let Some(ref llm) = self.context.llm_client {
            let prompt = self.build_coordination_prompt(hotspots);
            if let Ok(response) = llm.complete(&prompt).await {
                return self.parse_llm_coordination_plan(&response.content);
            }
        }

        // 规则驱动的回退
        self.rule_based_coordination_plan(hotspots)
    }

    fn build_coordination_prompt(&self, hotspots: &[ModuleHotspot]) -> String {
        let template = self
            .context
            .prompt_templates
            .get("multi_agent_coordinator");

        let mut values = HashMap::new();
        values.insert("n".to_string(), hotspots.len().to_string());

        let submissions: Vec<String> = hotspots
            .iter()
            .map(|h| {
                format!(
                    "- 模块 {}: Agents={} (severity={:?})",
                    h.module_path,
                    h.agents_involved.join(", "),
                    h.severity
                )
            })
            .collect();
        values.insert("recent_submissions".to_string(), submissions.join("\n"));

        match template {
            Some(t) => t.render(&values),
            None => format!(
                "检测到 {} 个热点模块，需要协调以下 Agent：\n{}",
                hotspots.len(),
                submissions.join("\n")
            ),
        }
    }

    fn parse_llm_coordination_plan(&self, content: &str) -> CoordinationPlan {
        // 简单解析 LLM 输出，提取关键信息
        CoordinationPlan {
            summary: content.lines().next().unwrap_or("").to_string(),
            merge_order: self.extract_merge_order(content),
            requires_human_review: self.extract_section(content, "需要人工介入"),
            auto_mergeable: self.extract_section(content, "可自动合并"),
        }
    }

    fn extract_merge_order(&self, _content: &str) -> Vec<MergeOrderItem> {
        // 简化实现：从 LLM 输出中解析合并顺序
        // 完整实现应该解析结构化 JSON
        Vec::new()
    }

    fn extract_section(&self, content: &str, keyword: &str) -> Vec<String> {
        content
            .lines()
            .filter(|l| l.contains(keyword))
            .map(|l| l.to_string())
            .collect()
    }

    fn rule_based_coordination_plan(&self, hotspots: &[ModuleHotspot]) -> CoordinationPlan {
        let mut merge_order = Vec::new();
        let mut requires_human_review = Vec::new();
        let mut auto_mergeable = Vec::new();

        for (i, hotspot) in hotspots.iter().enumerate() {
            match hotspot.severity {
                ConflictSeverity::Critical | ConflictSeverity::High => {
                    requires_human_review.push(format!(
                        "{}: {} 涉及 {}",
                        hotspot.module_path,
                        hotspot.agents_involved.join(", "),
                        match hotspot.severity {
                            ConflictSeverity::Critical => "严重冲突",
                            _ => "高风险冲突",
                        }
                    ));
                }
                _ => {
                    let first_agent = hotspot.agents_involved.first().cloned().unwrap_or_default();
                    auto_mergeable.push(format!("{} (by {})", hotspot.module_path, first_agent));
                    merge_order.push(MergeOrderItem {
                        priority: i as u32 + 1,
                        agent: first_agent,
                        commit_hash: String::new(),
                        description: format!("低风险模块 {}", hotspot.module_path),
                        reason: "低风险自动合并".to_string(),
                    });
                }
            }
        }

        let summary = if requires_human_review.is_empty() {
            format!("所有 {} 个热点模块可自动合并", hotspots.len())
        } else {
            format!(
                "{} 个热点模块中 {} 个需人工介入",
                hotspots.len(),
                requires_human_review.len()
            )
        };

        CoordinationPlan {
            summary,
            merge_order,
            requires_human_review,
            auto_mergeable,
        }
    }
}

#[async_trait]
impl Agent for MultiAgentCoordinatorAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::MultiAgentCoordinator
    }

    async fn execute(&self, task: AgentTask) -> Result<AgentResult> {
        *self.status.lock().unwrap() = AgentStatus::Running;
        let start = std::time::Instant::now();

        let agents = self.collect_agents().await?;
        let matrix = self.build_conflict_matrix(&agents).await?;

        let output = serde_json::to_value(&matrix).unwrap_or_default();

        *self.status.lock().unwrap() = AgentStatus::Completed;

        Ok(AgentResult::success(
            task.id,
            output,
            start.elapsed().as_millis() as u64,
        ))
    }

    fn status(&self) -> AgentStatus {
        self.status.lock().unwrap().clone()
    }
}
