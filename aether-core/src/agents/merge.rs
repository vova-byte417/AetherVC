use crate::agents::base::{Agent, AgentContext, AgentStatus};
use crate::domain::agent::{AgentResult, AgentTask, AgentType};
use crate::domain::merge::{
    MergeRecommendation, MergeRequest, MergeStatus, PullRequest, RiskAssessment, RiskFactor,
};
use crate::domain::commit::RiskLevel;
use crate::utils::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// 智能合并 Agent
/// 分析 PR 列表，给出合并优先级和风险评估
pub struct MergeAgent {
    context: Arc<AgentContext>,
    status: std::sync::Mutex<AgentStatus>,
}

impl MergeAgent {
    pub fn new(context: Arc<AgentContext>) -> Self {
        Self {
            context,
            status: std::sync::Mutex::new(AgentStatus::Idle),
        }
    }

    /// 分析 PR 依赖关系和冲突
    async fn analyze_prs(&self, prs: &[PullRequest]) -> Vec<(String, RiskLevel, Vec<String>)> {
        let mut analyzed = Vec::new();

        for pr in prs {
            let risk = self.assess_pr_risk(pr);
            let conflicts = self.find_conflicts(pr, prs);

            analyzed.push((pr.id.clone(), risk, conflicts));
        }

        analyzed
    }

    /// 评估单个 PR 的风险等级
    fn assess_pr_risk(&self, pr: &PullRequest) -> RiskLevel {
        let info = &pr.semantic_info;

        // 高风险：包含 breaking changes、修改核心模块、大量文件变更
        if info.risk_level == RiskLevel::Critical
            || info.change_type == crate::domain::commit::ChangeCategory::Breaking
        {
            return RiskLevel::High;
        }

        if pr.files_changed.len() > 10 {
            return RiskLevel::High;
        }

        if pr.files_changed.len() > 5 {
            return RiskLevel::Medium;
        }

        // 低风险：小的 bugfix、documentation
        if info.change_type == crate::domain::commit::ChangeCategory::Bugfix
            || info.change_type == crate::domain::commit::ChangeCategory::Documentation
        {
            return RiskLevel::Low;
        }

        RiskLevel::Medium
    }

    /// 找出与指定 PR 冲突的其他 PR
    fn find_conflicts(&self, pr: &PullRequest, all_prs: &[PullRequest]) -> Vec<String> {
        let mut conflicts = Vec::new();
        let files: std::collections::HashSet<_> = pr.files_changed.iter().collect();

        for other in all_prs {
            if other.id == pr.id {
                continue;
            }
            let other_files: std::collections::HashSet<_> = other.files_changed.iter().collect();
            if files.intersection(&other_files).count() > 0 {
                conflicts.push(other.id.clone());
            }
        }

        conflicts
    }

    /// 生成合并优先级
    async fn build_merge_order(&self, prs: &[PullRequest]) -> Vec<String> {
        // 按风险从低到高排序（低风险先合并）
        let mut ordered: Vec<(&PullRequest, RiskLevel)> = prs
            .iter()
            .map(|pr| (pr, self.assess_pr_risk(pr)))
            .collect();

        ordered.sort_by_key(|(_, risk)| match risk {
            RiskLevel::Low => 0,
            RiskLevel::Medium => 1,
            RiskLevel::High => 2,
            RiskLevel::Critical => 3,
        });

        ordered.into_iter().map(|(pr, _)| pr.id.clone()).collect()
    }

    /// 生成风险评估报告
    async fn generate_report(
        &self,
        _prs: &[PullRequest],
        analyzed: &[(String, RiskLevel, Vec<String>)],
        _merge_order: &[String],
    ) -> RiskAssessment {
        let mut risk_factors = Vec::new();
        let mut high_risk_count = 0;

        for (id, risk, conflicts) in analyzed {
            match risk {
                RiskLevel::High | RiskLevel::Critical => {
                    high_risk_count += 1;
                    risk_factors.push(RiskFactor {
                        description: format!("PR {} 风险等级 {:?}，冲突 PR: {:?}", id, risk, conflicts),
                        severity: risk.clone(),
                        affected_files: Vec::new(),
                    });
                }
                _ => {}
            }
        }

        let overall = if high_risk_count == 0 {
            RiskLevel::Low
        } else if high_risk_count <= 2 {
            RiskLevel::Medium
        } else {
            RiskLevel::High
        };

        let recommendation = if overall == RiskLevel::Low {
            MergeRecommendation::AutoMerge
        } else if overall == RiskLevel::Medium {
            MergeRecommendation::ManualReview
        } else {
            MergeRecommendation::Postpone
        };

        RiskAssessment {
            overall_risk: overall,
            risk_factors,
            recommendation,
        }
    }
}

#[async_trait]
impl Agent for MergeAgent {
    fn agent_type(&self) -> AgentType {
        AgentType::Merge
    }

    async fn execute(&self, task: AgentTask) -> Result<AgentResult> {
        *self.status.lock().unwrap() = AgentStatus::Running;
        let start = std::time::Instant::now();

        let prs: Vec<PullRequest> = match task.input.get("prs") {
            Some(val) => match serde_json::from_value(val.clone()) {
                Ok(p) => p,
                Err(e) => {
                    return Ok(AgentResult::failure(task.id, format!("Invalid PRs: {}", e)));
                }
            },
            None => {
                return Ok(AgentResult::failure(task.id, "Missing 'prs' field"));
            }
        };

        // 1. 分析 PR
        let analyzed = self.analyze_prs(&prs).await;

        // 2. 生成合并顺序
        let merge_order = self.build_merge_order(&prs).await;

        // 3. 生成风险评估
        let report = self.generate_report(&prs, &analyzed, &merge_order).await;

        let elapsed = start.elapsed().as_millis() as u64;
        *self.status.lock().unwrap() = AgentStatus::Completed;

        Ok(AgentResult::success(
            task.id,
            serde_json::json!({
                "merge_order": merge_order,
                "auto_mergeable": analyzed.iter()
                    .filter(|(_, risk, _)| risk == &RiskLevel::Low)
                    .map(|(id, _, _)| id)
                    .collect::<Vec<_>>(),
                "needs_review": analyzed.iter()
                    .filter(|(_, risk, _)| risk == &RiskLevel::Medium || risk == &RiskLevel::High)
                    .map(|(id, _, _)| id)
                    .collect::<Vec<_>>(),
                "risk_assessment": report,
            }),
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
    use crate::domain::commit::{Author, SemanticInfo, ChangeCategory, RiskLevel as CRisk};
    use crate::domain::merge::PRStatus;
    use crate::llm::client::MockLLMClient;
    use crate::semantic::embedder::MockEmbedder;
    use crate::storage::git::GitRepository;
    use crate::storage::graph_db::InMemoryGraphStore;
    use crate::storage::vector_db::InMemoryVectorStore;

    fn make_test_pr(id: &str, title: &str, files: Vec<&str>, risk: CRisk, cat: ChangeCategory) -> PullRequest {
        let mut pr = PullRequest::new(id, title, "feature", "main", Author::new("test", "t@t.com"));
        pr.files_changed = files.into_iter().map(|s| s.to_string()).collect();
        pr.semantic_info = SemanticInfo::new(title, cat, vec![], "", risk);
        pr
    }

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
    async fn test_merge_order_low_risk_first() {
        let agent = MergeAgent::new(test_context());
        let prs = vec![
            make_test_pr("1", "high risk change", vec!["a.rs", "b.rs", "c.rs"], CRisk::High, ChangeCategory::Breaking),
            make_test_pr("2", "bugfix", vec!["x.rs"], CRisk::Low, ChangeCategory::Bugfix),
        ];

        let task = AgentTask::new("merge_prs", serde_json::json!({"prs": prs}));
        let result = agent.execute(task).await.unwrap();
        assert!(result.success);

        let output = result.output;
        let order: Vec<String> = serde_json::from_value(output["merge_order"].clone()).unwrap();
        // 低风险应该排在前面
        assert_eq!(order[0], "2");
    }

    #[tokio::test]
    async fn test_merge_with_no_prs() {
        let agent = MergeAgent::new(test_context());
        let task = AgentTask::new("merge_prs", serde_json::json!({"prs": []}));

        let result = agent.execute(task).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_merge_missing_prs() {
        let agent = MergeAgent::new(test_context());
        let task = AgentTask::new("merge_prs", serde_json::json!({}));

        let result = agent.execute(task).await.unwrap();
        assert!(!result.success);
    }
}
