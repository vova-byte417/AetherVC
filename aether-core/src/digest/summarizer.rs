//! 摘要生成器
//!
//! 将聚合后的数据渲染为 Markdown 格式的人类可读报告。

use super::{DigestItem, DigestReport, RiskDistribution, TimeWindow, TopicCluster};
use chrono::Utc;
use std::collections::HashMap;

/// 摘要生成器
pub struct DigestSummarizer;

impl DigestSummarizer {
    pub fn new() -> Self {
        Self
    }

    /// 将 DigestReport 渲染为 Markdown
    pub fn render_markdown(&self, report: &DigestReport) -> String {
        let mut md = String::new();

        // 标题
        md.push_str("# AetherVC 变更摘要\n\n");
        md.push_str(&format!(
            "> 由 AetherVC Digest Engine 自动生成 | {}\n\n",
            Utc::now().format("%Y-%m-%d %H:%M:%S")
        ));

        // 概览表格
        md.push_str("## 概览\n\n");
        md.push_str("| 属性 | 内容 |\n");
        md.push_str("|------|------|\n");
        md.push_str(&format!(
            "| **时间窗口** | {} ~ {} |\n",
            report.window.from.format("%Y-%m-%d %H:%M"),
            report.window.to.format("%Y-%m-%d %H:%M")
        ));
        md.push_str(&format!("| **总 Commit 数** | {} |\n", report.total_commits));
        if !report.agents_involved.is_empty() {
            md.push_str(&format!(
                "| **涉及 Agent** | {} |\n",
                report.agents_involved.join(", ")
            ));
        }
        md.push('\n');

        // 一句话总结
        md.push_str(&format!("**一句话总结**：{}\n\n", report.summary));

        // 风险分布
        md.push_str("## 风险分布\n\n");
        md.push_str("| 等级 | 数量 |\n");
        md.push_str("|------|------|\n");
        md.push_str(&format!(
            "| 🔴 Critical | {} |\n",
            report.risk_distribution.critical
        ));
        md.push_str(&format!(
            "| 🟠 High | {} |\n",
            report.risk_distribution.high
        ));
        md.push_str(&format!(
            "| 🟡 Medium | {} |\n",
            report.risk_distribution.medium
        ));
        md.push_str(&format!(
            "| 🟢 Low | {} |\n",
            report.risk_distribution.low
        ));
        md.push('\n');

        // 主题聚类
        if !report.topic_clusters.is_empty() {
            md.push_str("## 变更主题\n\n");
            for cluster in &report.topic_clusters {
                md.push_str(&format!(
                    "### {} ({})\n\n",
                    cluster.label, cluster.risk_level
                ));
                md.push_str(&format!("{}\n\n", cluster.summary));
                md.push_str("涉及 commit：\n");
                for hash in &cluster.commit_hashes {
                    md.push_str(&format!("- `{}`\n", &hash[..hash.len().min(8)]));
                }
                md.push('\n');
            }
        }

        // 模块热力图
        if !report.module_heatmap.is_empty() {
            md.push_str("## 模块热力图\n\n");
            md.push_str("| 模块 | 变更次数 |\n");
            md.push_str("|------|----------|\n");

            // 按变更次数降序排列
            let mut sorted: Vec<_> = report.module_heatmap.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));

            for (module, count) in sorted.iter().take(15) {
                let bar = "█".repeat((**count as usize).min(20));
                md.push_str(&format!("| `{}` | {} {} |\n", module, bar, count));
            }
            md.push('\n');
        }

        // 高风险变更
        if !report.high_risk_items.is_empty() {
            md.push_str("## ⚠️ 需要关注的变更\n\n");
            md.push_str("| Commit | 风险 | 模块 | 摘要 |\n");
            md.push_str("|--------|------|------|------|\n");
            for item in &report.high_risk_items {
                let short_hash = &item.commit_hash[..item.commit_hash.len().min(8)];
                let modules = item.affected_modules.join(", ");
                let risk_icon = match item.risk_level.as_str() {
                    "critical" => "🔴",
                    "high" => "🟠",
                    "medium" => "🟡",
                    _ => "🟢",
                };
                md.push_str(&format!(
                    "| `{}` | {} {} | {} | {} |\n",
                    short_hash,
                    risk_icon,
                    item.risk_level,
                    modules,
                    if item.summary.len() > 50 {
                        &item.summary[..50]
                    } else {
                        &item.summary
                    }
                ));
            }
            md.push('\n');
        }

        // 安全变更
        if !report.safe_items.is_empty() {
            md.push_str(&format!(
                "## ✅ 安全的变更（{} 个）\n\n",
                report.safe_items.len()
            ));
            let mut modules: Vec<String> = Vec::new();
            for item in &report.safe_items {
                for m in &item.affected_modules {
                    if !modules.contains(m) {
                        modules.push(m.clone());
                    }
                }
            }
            if !modules.is_empty() {
                md.push_str(&format!("涉及模块：{}\n\n", modules.join("、")));
            }

            for item in report.safe_items.iter().take(10) {
                let short_hash = &item.commit_hash[..item.commit_hash.len().min(8)];
                md.push_str(&format!(
                    "- `{}` {} — {}\n",
                    short_hash,
                    item.author,
                    if item.message.len() > 60 {
                        &item.message[..60]
                    } else {
                        &item.message
                    }
                ));
            }
            if report.safe_items.len() > 10 {
                md.push_str(&format!(
                    "\n... 还有 {} 个安全变更未列出\n",
                    report.safe_items.len() - 10
                ));
            }
            md.push('\n');
        }

        md
    }

    /// 将 DigestReport 渲染为 JSON 字符串
    pub fn render_json(&self, report: &DigestReport) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(report)
    }

    /// 简单文本输出（用于终端）
    pub fn render_text(&self, report: &DigestReport) -> String {
        let mut text = String::new();

        text.push_str(&format!(
            "AetherVC 变更摘要 | {} ~ {}\n",
            report.window.from.format("%Y-%m-%d %H:%M"),
            report.window.to.format("%Y-%m-%d %H:%M")
        ));
        text.push_str(&format!("总 commit 数：{}\n", report.total_commits));
        if !report.agents_involved.is_empty() {
            text.push_str(&format!("涉及 Agent：{}\n", report.agents_involved.join(", ")));
        }
        text.push_str(&format!("\n一句话总结：{}\n", report.summary));

        text.push_str(&format!(
            "\n风险分布：Critical={} High={} Medium={} Low={}\n",
            report.risk_distribution.critical,
            report.risk_distribution.high,
            report.risk_distribution.medium,
            report.risk_distribution.low
        ));

        if !report.high_risk_items.is_empty() {
            text.push_str(&format!(
                "\n⚠️ 需要关注的变更（{} 个）：\n",
                report.high_risk_items.len()
            ));
            for item in &report.high_risk_items {
                let short_hash = &item.commit_hash[..item.commit_hash.len().min(8)];
                text.push_str(&format!(
                    "  {} {} - {} [{}]\n",
                    short_hash,
                    item.risk_level.to_uppercase(),
                    item.summary,
                    item.affected_modules.join(", ")
                ));
            }
        }

        if !report.module_heatmap.is_empty() {
            text.push_str("\n模块热力图：\n");
            let mut sorted: Vec<_> = report.module_heatmap.iter().collect();
            sorted.sort_by(|a, b| b.1.cmp(a.1));
            for (module, count) in sorted.iter().take(10) {
                let bar = "█".repeat((**count as usize).min(20));
                text.push_str(&format!("  {:20} {} ({})\n", module, bar, count));
            }
        }

        text
    }
}

impl Default for DigestSummarizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::digest::TopicCluster;

    fn make_report() -> DigestReport {
        DigestReport {
            id: "test-1".into(),
            window: TimeWindow {
                from: Utc::now(),
                to: Utc::now(),
            },
            summary: "测试摘要".into(),
            total_commits: 3,
            agents_involved: vec!["Cline".into(), "Copilot".into()],
            topic_clusters: vec![TopicCluster {
                label: "bugfix (2 commits)".into(),
                summary: "修复了登录 bug".into(),
                commit_hashes: vec!["abc123".into(), "def456".into()],
                change_type: "bugfix".into(),
                risk_level: "medium".into(),
            }],
            risk_distribution: RiskDistribution {
                critical: 0,
                high: 1,
                medium: 2,
                low: 0,
            },
            module_heatmap: {
                let mut m = HashMap::new();
                m.insert("auth/".into(), 2);
                m.insert("api/".into(), 1);
                m
            },
            high_risk_items: vec![DigestItem {
                commit_hash: "abc123".into(),
                message: "fix auth bug".into(),
                risk_level: "high".into(),
                affected_modules: vec!["auth/".into()],
                summary: "修复认证中间件 bug".into(),
                author: "Cline".into(),
            }],
            safe_items: vec![],
            generated_at: Utc::now(),
        }
    }

    #[test]
    fn test_render_markdown() {
        let summarizer = DigestSummarizer::new();
        let md = summarizer.render_markdown(&make_report());
        assert!(md.contains("AetherVC 变更摘要"));
        assert!(md.contains("auth/"));
        assert!(md.contains("bugfix"));
    }

    #[test]
    fn test_render_json() {
        let summarizer = DigestSummarizer::new();
        let json = summarizer.render_json(&make_report()).unwrap();
        assert!(json.contains("测试摘要"));
        assert!(json.contains("abc123"));
    }

    #[test]
    fn test_render_text() {
        let summarizer = DigestSummarizer::new();
        let text = summarizer.render_text(&make_report());
        assert!(text.contains("AetherVC 变更摘要"));
        assert!(text.contains("HIGH"));
    }
}
