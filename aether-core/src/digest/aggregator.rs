//! 摘要聚合器
//!
//! 负责从 Git 仓库中获取指定时间窗口内的 commit，并按维度聚合。

use super::{DigestGroupBy, DigestItem, DigestOptions, DigestReport, RiskDistribution, TimeWindow, TopicCluster};
use crate::storage::git::GitOperations;
use crate::utils::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// 摘要聚合器
pub struct DigestAggregator {
    git_repo: Arc<dyn GitOperations>,
}

impl DigestAggregator {
    pub fn new(git_repo: Arc<dyn GitOperations>) -> Self {
        Self { git_repo }
    }

    /// 执行聚合，生成摘要报告
    pub async fn aggregate(&self, options: &DigestOptions) -> Result<DigestReport> {
        // 1. 获取 commit 列表
        let commits = self.fetch_commits(options).await?;

        // 2. 统计基本信息
        let total = commits.len() as u32;
        let agents = self.extract_agents(&commits);
        let risk_dist = self.compute_risk_distribution(&commits);
        let module_heatmap = self.build_module_heatmap(&commits);

        // 3. 按风险等级分组
        let (high_risk, safe) = self.split_by_risk(&commits, &options.risk_threshold);

        // 4. 生成主题聚类
        let topic_clusters = self.cluster_topics(&commits);

        // 5. 生成一句话总结
        let summary = self.generate_summary(&commits, &topic_clusters, &risk_dist);

        // 6. 确定时间窗口
        let window = self.determine_window(&commits, options);

        Ok(DigestReport {
            id: Uuid::new_v4().to_string(),
            window,
            summary,
            total_commits: total,
            agents_involved: agents,
            topic_clusters,
            risk_distribution: risk_dist,
            module_heatmap,
            high_risk_items: high_risk,
            safe_items: safe,
            generated_at: Utc::now(),
        })
    }

    /// 从 Git 获取 commit 列表
    async fn fetch_commits(
        &self,
        options: &DigestOptions,
    ) -> Result<Vec<crate::domain::commit::Commit>> {
        let all_commits = self.git_repo.list_commits().await?;

        // 按时间范围或 commit 范围过滤
        let filtered: Vec<_> = all_commits
            .into_iter()
            .filter(|c| {
                if let Some(ref since) = options.since {
                    if c.timestamp < *since {
                        return false;
                    }
                }
                if let Some(ref until) = options.until {
                    if c.timestamp > *until {
                        return false;
                    }
                }
                true
            })
            .collect();

        // 如果有 commit_range，进一步过滤（简单实现：取前 N 个）
        if let Some(ref range) = options.commit_range {
            if range.contains("HEAD~") {
                if let Ok(n) = range
                    .trim_start_matches("HEAD~")
                    .split("..")
                    .next()
                    .unwrap_or("20")
                    .parse::<usize>()
                {
                    return Ok(filtered.into_iter().take(n).collect());
                }
            }
        }

        Ok(filtered)
    }

    /// 提取涉及的 Agent/作者
    fn extract_agents(&self, commits: &[crate::domain::commit::Commit]) -> Vec<String> {
        let mut agents = Vec::new();
        for c in commits {
            let name = c.author.name.clone();
            if !agents.contains(&name) {
                agents.push(name);
            }
        }
        agents
    }

    /// 计算风险分布
    fn compute_risk_distribution(
        &self,
        commits: &[crate::domain::commit::Commit],
    ) -> RiskDistribution {
        let mut dist = RiskDistribution::new();
        for c in commits {
            match c.semantic_info.risk_level {
                crate::domain::commit::RiskLevel::Critical => dist.critical += 1,
                crate::domain::commit::RiskLevel::High => dist.high += 1,
                crate::domain::commit::RiskLevel::Medium => dist.medium += 1,
                crate::domain::commit::RiskLevel::Low => dist.low += 1,
            }
        }
        dist
    }

    /// 构建模块热力图
    fn build_module_heatmap(
        &self,
        commits: &[crate::domain::commit::Commit],
    ) -> HashMap<String, u32> {
        let mut heatmap = HashMap::new();
        for c in commits {
            for module in &c.semantic_info.affected_modules {
                *heatmap.entry(module.clone()).or_insert(0) += 1;
            }
        }
        heatmap
    }

    /// 按风险等级分离
    fn split_by_risk(
        &self,
        commits: &[crate::domain::commit::Commit],
        threshold: &str,
    ) -> (Vec<DigestItem>, Vec<DigestItem>) {
        let threshold_level = match threshold {
            "low" => 0,
            "medium" => 1,
            "high" => 2,
            _ => 1,
        };

        let mut high_risk = Vec::new();
        let mut safe = Vec::new();

        for c in commits {
            let risk_num = match c.semantic_info.risk_level {
                crate::domain::commit::RiskLevel::Low => 0,
                crate::domain::commit::RiskLevel::Medium => 1,
                crate::domain::commit::RiskLevel::High => 2,
                crate::domain::commit::RiskLevel::Critical => 3,
            };

            let item = DigestItem {
                commit_hash: c.id.0.clone(),
                message: c.message.clone(),
                risk_level: c.semantic_info.risk_level.as_str().to_string(),
                affected_modules: c.semantic_info.affected_modules.clone(),
                summary: c.semantic_info.semantic_summary.clone(),
                author: c.author.name.clone(),
            };

            if risk_num >= threshold_level {
                high_risk.push(item);
            } else {
                safe.push(item);
            }
        }

        (high_risk, safe)
    }

    /// 按语义聚类 commit 为主题
    fn cluster_topics(
        &self,
        commits: &[crate::domain::commit::Commit],
    ) -> Vec<TopicCluster> {
        // 简单实现：按 change_type 聚类
        let mut clusters: HashMap<String, Vec<&crate::domain::commit::Commit>> = HashMap::new();

        for c in commits {
            let ct = c.semantic_info.change_type.to_string();
            clusters.entry(ct).or_default().push(c);
        }

        clusters
            .into_iter()
            .map(|(change_type, group)| {
                let hashes: Vec<String> = group.iter().map(|c| c.id.0.clone()).collect();
                let messages: Vec<&str> = group.iter().map(|c| c.message.as_str()).collect();
                let max_risk = group
                    .iter()
                    .map(|c| c.semantic_info.risk_level.clone())
                    .max_by_key(|r| match r {
                        crate::domain::commit::RiskLevel::Low => 0,
                        crate::domain::commit::RiskLevel::Medium => 1,
                        crate::domain::commit::RiskLevel::High => 2,
                        crate::domain::commit::RiskLevel::Critical => 3,
                    })
                    .unwrap_or(crate::domain::commit::RiskLevel::Low);

                TopicCluster {
                    label: format!("{} ({} commits)", change_type, group.len()),
                    summary: format!(
                        "涉及 {} 个 commit: {}",
                        group.len(),
                        messages.first().unwrap_or(&"")
                    ),
                    commit_hashes: hashes,
                    change_type,
                    risk_level: max_risk.as_str().to_string(),
                }
            })
            .collect()
    }

    /// 生成一句话总结
    fn generate_summary(
        &self,
        commits: &[crate::domain::commit::Commit],
        clusters: &[TopicCluster],
        risk_dist: &RiskDistribution,
    ) -> String {
        if commits.is_empty() {
            return "该时间窗口内没有代码变更。".to_string();
        }

        let total = commits.len();
        let cluster_descs: Vec<String> = clusters
            .iter()
            .take(3)
            .map(|c| format!("{}", c.label))
            .collect();

        let mut summary = format!(
            "共 {} 个 commit，主要涉及：{}。",
            total,
            cluster_descs.join("、")
        );

        if risk_dist.critical > 0 || risk_dist.high > 0 {
            summary.push_str(&format!(
                " ⚠️ 其中 {} 个高风险变更需要关注。",
                risk_dist.critical + risk_dist.high
            ));
        }

        summary
    }

    /// 确定时间窗口
    fn determine_window(
        &self,
        commits: &[crate::domain::commit::Commit],
        options: &DigestOptions,
    ) -> TimeWindow {
        let from = if let Some(since) = options.since {
            since
        } else {
            commits
                .last()
                .map(|c| c.timestamp)
                .unwrap_or_else(|| Utc::now())
        };

        let to = options.until.unwrap_or_else(|| {
            commits
                .first()
                .map(|c| c.timestamp)
                .unwrap_or_else(|| Utc::now())
        });

        TimeWindow { from, to }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::git::GitRepository;

    #[test]
    fn test_empty_aggregation() {
        let git_repo = Arc::new(GitRepository::new("."));
        let aggregator = DigestAggregator::new(git_repo);
        let risk_dist = aggregator.compute_risk_distribution(&[]);
        assert_eq!(risk_dist.critical, 0);
        assert_eq!(risk_dist.high, 0);
    }

    #[test]
    fn test_extract_agents() {
        let git_repo = Arc::new(GitRepository::new("."));
        let aggregator = DigestAggregator::new(git_repo);
        let agents = aggregator.extract_agents(&[]);
        assert!(agents.is_empty());
    }

    #[test]
    fn test_module_heatmap_empty() {
        let git_repo = Arc::new(GitRepository::new("."));
        let aggregator = DigestAggregator::new(git_repo);
        let heatmap = aggregator.build_module_heatmap(&[]);
        assert!(heatmap.is_empty());
    }
}
