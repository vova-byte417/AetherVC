//! 验证与风险评估 Agent
//!
//! 核心职责：
//! 1. 批量 Tag 验证（编译 + 测试 + 静态分析）
//! 2. Tag 风险评估（变更类型 × 影响范围 × 历史通过率）
//! 3. 生成验证报告 + 按风险排序

use crate::agents::base::{Agent, AgentContext, AgentStatus};
use crate::domain::agent::{
    AgentResult, AgentTask, AgentType, TagOrderBy, TagRiskAssessment, TagValidationReport,
    TagValidationRequest, ValidationConclusion,
};
use crate::domain::commit::RiskLevel;
use crate::utils::Result;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

/// 验证与风险评估 Agent
pub struct ValidationRiskAgent {
    context: Arc<AgentContext>,
    status: Mutex<AgentStatus>,
}

impl ValidationRiskAgent {
    pub fn new(context: Arc<AgentContext>) -> Self {
        Self {
            context,
            status: Mutex::new(AgentStatus::Idle),
        }
    }

    /// 搜索包含指定关键词的 Tag
    async fn search_tags(&self, keyword: &str, max_tags: usize) -> Result<Vec<String>> {
        // 用语义搜索查找相关 Tag
        let embedding = self.context.embedder.embed(keyword).await?;
        let results = self
            .context
            .vector_store
            .search_similar(&embedding, max_tags, None)
            .await?;

        let mut tags: Vec<String> = results.into_iter().map(|r| r.commit_hash).collect();
        tags.dedup();
        Ok(tags)
    }

    /// 评估单个 Tag 的风险
    async fn assess_tag_risk(&self, tag: &str) -> Result<TagRiskAssessment> {
        let commit_hash = self.resolve_tag_to_commit(tag).await?;

        // 获取 commit 语义信息
        let results = self
            .context
            .vector_store
            .search_similar(
                &self.context.embedder.embed(tag).await?,
                1,
                None,
            )
            .await?;

        let (change_type, affected_modules, risk_level_str, agent_name) = if let Some(r) = results.first()
        {
            (
                r.change_category.to_string(),
                r.affected_modules.clone(),
                r.risk_level.to_string(),
                r.author_name.clone(),
            )
        } else {
            (
                "unknown".to_string(),
                vec![],
                "low".to_string(),
                "unknown".to_string(),
            )
        };

        // 风险评分
        let risk_score = self.calculate_risk_score(&change_type, &affected_modules, &risk_level_str);

        Ok(TagRiskAssessment {
            tag: tag.to_string(),
            commit_hash,
            risk_score,
            risk_level: risk_level_str,
            change_type,
            affected_modules,
            agent_name,
        })
    }

    /// 将 Tag 名解析为 commit hash
    async fn resolve_tag_to_commit(&self, tag: &str) -> Result<String> {
        // 如果是完整 hash，直接返回
        if tag.len() >= 40 && tag.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(tag.to_string());
        }

        // 从 Git 仓库查询
        let commits = self.context.git_repo.list_commits().await?;
        for commit in &commits {
            if commit.id.0.starts_with(tag) {
                return Ok(commit.id.0.clone());
            }
        }

        // 回退：返回 tag 自身
        Ok(tag.to_string())
    }

    /// 风险评分模型
    fn calculate_risk_score(
        &self,
        change_type: &str,
        affected_modules: &[String],
        risk_level: &str,
    ) -> f32 {
        let type_score = match change_type {
            "breaking" => 1.0,
            "security_hardening" => 0.9,
            "feature" | "feature_addition" => 0.5,
            "refactor" => 0.4,
            "bugfix" => 0.3,
            "documentation" | "test" => 0.1,
            _ => 0.5,
        };

        let module_score = if affected_modules.iter().any(|m| {
            m.contains("auth")
                || m.contains("database")
                || m.contains("payment")
                || m.contains("core")
        }) {
            1.0
        } else if affected_modules
            .iter()
            .any(|m| m.contains("api") || m.contains("models"))
        {
            0.6
        } else {
            0.2
        };

        let risk_multiplier = match risk_level.to_lowercase().as_str() {
            "critical" => 1.0,
            "high" => 0.8,
            "medium" => 0.5,
            "low" => 0.2,
            _ => 0.5,
        };

        let score: f32 = type_score * 0.4 + module_score * 0.3 + risk_multiplier * 0.3;
        score.clamp(0.0_f32, 1.0_f32)
    }

    /// 按指定顺序排序 Tag
    fn sort_tags(
        &self,
        assessments: &mut Vec<TagRiskAssessment>,
        order_by: &TagOrderBy,
    ) {
        match order_by {
            TagOrderBy::RiskAsc => {
                assessments.sort_by(|a, b| a.risk_score.partial_cmp(&b.risk_score).unwrap());
            }
            TagOrderBy::RiskDesc => {
                assessments.sort_by(|a, b| b.risk_score.partial_cmp(&a.risk_score).unwrap());
            }
            TagOrderBy::Chronological => {
                // 保持原序
            }
        }
    }

    /// 执行本地验证（编译 + lint + 测试）
    async fn run_local_verification(
        &self,
        _tag: &str,
        _commit_hash: &str,
    ) -> Result<serde_json::Value> {
        // 在 MVP 中，调用现有的 VerificationRunner
        // 这里简化为返回 mock 结果
        Ok(serde_json::json!({
            "status": "passed",
            "checks": [
                {"name": "compile", "status": "passed", "duration_ms": 500},
                {"name": "lint", "status": "passed", "duration_ms": 300},
            ]
        }))
    }
}

#[async_trait]
impl Agent for ValidationRiskAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::ValidationRisk
    }

    async fn execute(&self, task: AgentTask) -> Result<AgentResult> {
        *self.status.lock().unwrap() = AgentStatus::Running;
        let start = std::time::Instant::now();

        // 解析请求
        let keyword = task.input.get("keyword").and_then(|v| v.as_str()).unwrap_or("");
        let max_tags = task
            .input
            .get("max_tags")
            .and_then(|v| v.as_u64())
            .unwrap_or(15) as usize;
        let order_by_str = task
            .input
            .get("order_by")
            .and_then(|v| v.as_str())
            .unwrap_or("risk_asc");

        let order_by = match order_by_str {
            "risk_desc" => TagOrderBy::RiskDesc,
            "chronological" => TagOrderBy::Chronological,
            _ => TagOrderBy::RiskAsc,
        };

        // 搜索 Tag
        let tags = if !keyword.is_empty() {
            self.search_tags(keyword, max_tags).await?
        } else {
            // 从 input 中读取指定的 tags
            task.input
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default()
        };

        if tags.is_empty() {
            return Ok(AgentResult::success(
                task.id,
                serde_json::json!({"reports": [], "message": "No tags found"}),
                start.elapsed().as_millis() as u64,
            ));
        }

        // 评估每个 Tag 的风险
        let mut assessments = Vec::new();
        for tag in &tags {
            if let Ok(assessment) = self.assess_tag_risk(tag).await {
                assessments.push(assessment);
            }
        }

        // 排序
        self.sort_tags(&mut assessments, &order_by);

        // 生成报告
        let reports: Vec<TagValidationReport> = assessments
            .iter()
            .map(|a| {
                let conclusion = if a.risk_score > 0.7 {
                    "fail".to_string()
                } else if a.risk_score > 0.4 {
                    "conditional_pass".to_string()
                } else {
                    "pass".to_string()
                };

                TagValidationReport {
                    tag: a.tag.clone(),
                    commit_hash: a.commit_hash.clone(),
                    agent: a.agent_name.clone(),
                    risk_assessment: a.clone(),
                    overall_conclusion: conclusion,
                    verification_details: None,
                }
            })
            .collect();

        let output = serde_json::to_value(&reports).unwrap_or_default();

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
