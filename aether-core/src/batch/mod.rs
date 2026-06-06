//! 批量分析
//!
//! 支持跨多个 commit 的风险排序、模块过滤等功能。

use crate::domain::commit::{Commit, RiskLevel};
use crate::utils::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 批量分析器
pub struct BatchAnalyzer;

/// 排序后的批量分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchAnalysisResult {
    /// 按风险排序后的 commit 列表
    pub commits: Vec<ScoredCommit>,
    /// 统计概览
    pub summary: BatchSummary,
}

/// 带风险评分的 commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredCommit {
    pub commit_hash: String,
    pub message: String,
    pub author: String,
    pub risk_level: String,
    pub risk_score: f32,
    pub affected_modules: Vec<String>,
    pub change_type: String,
    pub summary: String,
}

/// 批量分析摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSummary {
    pub total: u32,
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub average_risk_score: f32,
    pub top_affected_modules: Vec<(String, u32)>,
}

/// 批量分析选项
#[derive(Debug, Clone)]
pub struct BatchOptions {
    /// 是否按风险排序
    pub risk_sort: bool,
    /// 结果数量限制
    pub limit: usize,
    /// 仅显示指定模块
    pub module_filter: Option<String>,
    /// 最低风险等级
    pub min_risk: Option<String>,
}

impl Default for BatchOptions {
    fn default() -> Self {
        Self {
            risk_sort: false,
            limit: 50,
            module_filter: None,
            min_risk: None,
        }
    }
}

/// 风险评分模型
impl BatchAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// 对 commit 列表进行分析和排序
    pub fn analyze(&self, commits: Vec<Commit>, options: &BatchOptions) -> BatchAnalysisResult {
        // 1. 计算每个 commit 的风险评分
        let mut scored: Vec<ScoredCommit> = commits
            .iter()
            .map(|c| self.score_commit(c))
            .collect();

        // 2. 按模块过滤
        if let Some(ref module) = options.module_filter {
            scored.retain(|s| {
                s.affected_modules
                    .iter()
                    .any(|m| m.contains(module.as_str()))
            });
        }

        // 3. 按最低风险过滤
        if let Some(ref min_risk) = options.min_risk {
            let min_level = risk_to_num(min_risk);
            scored.retain(|s| risk_to_num(&s.risk_level) >= min_level);
        }

        // 4. 排序
        if options.risk_sort {
            scored.sort_by(|a, b| b.risk_score.partial_cmp(&a.risk_score).unwrap_or(std::cmp::Ordering::Equal));
        }

        // 5. 截断
        let total = scored.len() as u32;
        scored.truncate(options.limit);

        // 6. 计算摘要
        let summary = self.compute_summary(&scored, &commits, total);

        BatchAnalysisResult {
            commits: scored,
            summary,
        }
    }

    /// 计算单个 commit 的风险评分
    fn score_commit(&self, commit: &Commit) -> ScoredCommit {
        let mut score = 0.0f32;

        // 变更类型权重
        score += match commit.semantic_info.change_type.to_string().as_str() {
            "breaking" => 1.0,
            "feature" => 0.5,
            "refactor" => 0.4,
            "bugfix" => 0.3,
            "performance" => 0.5,
            "documentation" => 0.1,
            "test" => 0.1,
            _ => 0.3,
        };

        // 模块敏感度权重
        for module in &commit.semantic_info.affected_modules {
            score += match module.as_str() {
                m if m.starts_with("auth")
                    || m.starts_with("database")
                    || m.starts_with("payment") =>
                    1.0,
                m if m.starts_with("api") || m.starts_with("models") => 0.6,
                _ => 0.2,
            };
        }

        // 风险等级额外加权
        score += match commit.semantic_info.risk_level {
            RiskLevel::Critical => 0.3,
            RiskLevel::High => 0.2,
            RiskLevel::Medium => 0.1,
            RiskLevel::Low => 0.0,
        };

        let risk_score = score.min(1.0);

        ScoredCommit {
            commit_hash: commit.id.0.clone(),
            message: commit.message.clone(),
            author: commit.author.name.clone(),
            risk_level: commit.semantic_info.risk_level.as_str().to_string(),
            risk_score,
            affected_modules: commit.semantic_info.affected_modules.clone(),
            change_type: commit.semantic_info.change_type.to_string(),
            summary: commit.semantic_info.semantic_summary.clone(),
        }
    }

    /// 计算批量摘要
    fn compute_summary(
        &self,
        scored: &[ScoredCommit],
        all_commits: &[Commit],
        total: u32,
    ) -> BatchSummary {
        let mut critical = 0u32;
        let mut high = 0u32;
        let mut medium = 0u32;
        let mut low = 0u32;
        let mut module_counts: HashMap<String, u32> = HashMap::new();

        for s in scored {
            match s.risk_level.as_str() {
                "critical" => critical += 1,
                "high" => high += 1,
                "medium" => medium += 1,
                _ => low += 1,
            }
            for m in &s.affected_modules {
                *module_counts.entry(m.clone()).or_insert(0) += 1;
            }
        }

        let total_score: f32 = scored.iter().map(|s| s.risk_score).sum();
        let avg_score = if scored.is_empty() {
            0.0
        } else {
            total_score / scored.len() as f32
        };

        let mut sorted_modules: Vec<_> = module_counts.into_iter().collect();
        sorted_modules.sort_by(|a, b| b.1.cmp(&a.1));
        let top_affected_modules: Vec<_> = sorted_modules.into_iter().take(10).collect();

        BatchSummary {
            total,
            critical,
            high,
            medium,
            low,
            average_risk_score: avg_score,
            top_affected_modules,
        }
    }
}

impl Default for BatchAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// 风险等级转数字（越高越严重）
fn risk_to_num(risk: &str) -> u32 {
    match risk.to_lowercase().as_str() {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::commit::{Author, ChangeCategory, RiskLevel};

    fn make_commit(
        hash: &str,
        risk: RiskLevel,
        modules: Vec<&str>,
        change_type: ChangeCategory,
    ) -> Commit {
        let mut c = Commit::new(
            hash,
            "test commit",
            Author::ai_agent("Cline", "agent-1"),
            chrono::Utc::now(),
            vec![],
        );
        c.semantic_info.risk_level = risk;
        c.semantic_info.affected_modules = modules.into_iter().map(|s| s.to_string()).collect();
        c.semantic_info.change_type = change_type;
        c.semantic_info.semantic_summary = "test".into();
        c
    }

    #[test]
    fn test_analyze_empty() {
        let analyzer = BatchAnalyzer::new();
        let result = analyzer.analyze(vec![], &BatchOptions::default());
        assert_eq!(result.summary.total, 0);
    }

    #[test]
    fn test_analyze_risk_sort() {
        let analyzer = BatchAnalyzer::new();
        let commits = vec![
            make_commit("a", RiskLevel::Low, vec!["tests"], ChangeCategory::Documentation),
            make_commit("b", RiskLevel::Critical, vec!["auth"], ChangeCategory::Breaking),
            make_commit("c", RiskLevel::Medium, vec!["api"], ChangeCategory::Feature),
        ];

        let opts = BatchOptions {
            risk_sort: true,
            ..Default::default()
        };

        let result = analyzer.analyze(commits, &opts);
        assert_eq!(result.commits.len(), 3);
        // 风险最高的排第一
        assert_eq!(result.commits[0].risk_level, "critical");
        assert_eq!(result.commits[2].risk_level, "low");
    }

    #[test]
    fn test_module_filter() {
        let analyzer = BatchAnalyzer::new();
        let commits = vec![
            make_commit("a", RiskLevel::High, vec!["auth"], ChangeCategory::Bugfix),
            make_commit("b", RiskLevel::Low, vec!["docs"], ChangeCategory::Documentation),
        ];

        let opts = BatchOptions {
            module_filter: Some("auth".into()),
            ..Default::default()
        };

        let result = analyzer.analyze(commits, &opts);
        assert_eq!(result.commits.len(), 1);
        assert_eq!(result.commits[0].commit_hash, "a");
    }

    #[test]
    fn test_min_risk_filter() {
        let analyzer = BatchAnalyzer::new();
        let commits = vec![
            make_commit("a", RiskLevel::High, vec!["auth"], ChangeCategory::Bugfix),
            make_commit("b", RiskLevel::Low, vec!["docs"], ChangeCategory::Documentation),
        ];

        let opts = BatchOptions {
            min_risk: Some("high".into()),
            ..Default::default()
        };

        let result = analyzer.analyze(commits, &opts);
        assert_eq!(result.commits.len(), 1);
    }
}
