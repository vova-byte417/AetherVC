use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Prompt 模板
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub template: String,
    pub variables: Vec<String>,
}

impl PromptTemplate {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        template: impl Into<String>,
        variables: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            template: template.into(),
            variables,
        }
    }

    /// 渲染模板
    pub fn render(&self, values: &HashMap<String, String>) -> String {
        let mut result = self.template.clone();
        for var in &self.variables {
            let placeholder = format!("{{{}}}", var);
            let value = values.get(var).map(|s| s.as_str()).unwrap_or("");
            result = result.replace(&placeholder, value);
        }
        result
    }
}

/// Prompt 模板管理器
pub struct PromptTemplateManager {
    templates: HashMap<String, PromptTemplate>,
}

impl PromptTemplateManager {
    pub fn new() -> Self {
        let mut manager = Self {
            templates: HashMap::new(),
        };
        manager.load_default_templates();
        manager
    }

    fn load_default_templates(&mut self) {
        // 1. Semantic Interpreter Agent
        self.add_template(PromptTemplate::new(
            "semantic_interpreter",
            "Semantic Interpreter",
            "分析代码变更的语义",
            r#"你是一个极度专业的代码语义分析专家。
用户会给你一段代码变更或自然语言描述。

请按以下JSON格式输出：
{
  "intent": "功能描述（一句话）",
  "change_type": "feature|refactor|bugfix|performance|breaking",
  "affected_modules": ["module1", "module2"],
  "semantic_summary": "详细但简洁的变更语义",
  "risk_level": "low|medium|high",
  "suggested_tags": ["tag1", "tag2"],
  "related_historical_changes": ["commit-hash or description"]
}

当前变更：
{user_input_or_diff}"#,
            vec!["user_input_or_diff".to_string()],
        ));

        // 2. Merge Agent
        self.add_template(PromptTemplate::new(
            "merge_agent",
            "Merge Agent",
            "智能PR合并分析和建议",
            r#"你是一个资深的AI代码合并专家，负责处理海量AI Agent提交的Pull Request。

当前有以下PRs需要处理：
{list_of_prs_with_summary}

请执行以下步骤：
1. 分析依赖关系和潜在冲突
2. 提出最优合并顺序
3. 对低风险PR给出自动合并建议
4. 对高风险PR给出详细审查要点和建议方案

输出格式：
- 合并优先级排序
- 可自动合并的PR列表 + 理由
- 需要人工介入的PR + 风险点
- 整体风险评估"#,
            vec!["list_of_prs_with_summary".to_string()],
        ));

        // 3. Cross-Commit Recovery Agent
        self.add_template(PromptTemplate::new(
            "cross_commit_recovery",
            "Cross-Commit Recovery",
            "跨版本智能恢复",
            r#"用户想要恢复之前commit中的某个功能，但该功能在后续commit中被删除或修改。

已知信息：
- 原始commit: {commit1_info}
- 当前状态: {current_state}
- 用户需求: {user_natural_language_request}

请：
1. 定位最相关的代码片段（即使被重构也能找到）
2. 生成兼容当前代码库的patch
3. 预测可能产生的冲突和副作用
4. 输出最终建议的恢复方案（包含完整代码diff）"#,
            vec![
                "commit1_info".to_string(),
                "current_state".to_string(),
                "user_natural_language_request".to_string(),
            ],
        ));

        // 4. Multi-Agent Coordinator Agent
        self.add_template(PromptTemplate::new(
            "multi_agent_coordinator",
            "Multi-Agent Coordinator",
            "多Agent冲突协调",
            r#"你负责协调多个AI Agent同时向同一个代码库提交变更。

当前情况：
- 活跃Agent数量: {n}
- 最近提交: {recent_submissions}

任务：
- 检测冲突模块
- 组织"Agent会议"（如果需要多个Agent讨论同一模块）
- 制定本次迭代的全局变更计划
- 决定哪些提交应该立即合并，哪些应该暂缓

输出必须包含：
- 当前全局状态总结
- 冲突解决建议
- 下一轮提交指导原则"#,
            vec!["n".to_string(), "recent_submissions".to_string()],
        ));

        // 5. Validation & Risk Agent
        self.add_template(PromptTemplate::new(
            "validation_risk",
            "Validation & Risk Agent",
            "Tag/Commit 验证与风险评估",
            r#"针对即将部署的Tag/Commit进行全面风险评估。

Tag/Commit信息：{tag_info}

请分析：
- 功能覆盖范围
- 潜在回归风险
- 性能/安全/稳定性影响
- 建议验证策略（影子部署、小流量、A/B测试等）
- 最终推荐：Deploy / Hold / Rollback

输出结构化报告。"#,
            vec!["tag_info".to_string()],
        ));

        // 6. Commit Intelligence (AetherCI) Agent
        self.add_template(PromptTemplate::new(
            "commit_intelligence",
            "Commit Intelligence",
            "代码变更意图深度推理",
            r#"你是一个代码变更意图分析专家。请分析以下代码变更，推断作者的意图和动机。

变更类型（预分类）：{change_type}
Commit Message：{msg}
变更统计：{stats}

Code Diff：
{diff}

请深入分析并以 JSON 格式回答：
{
    "problem_solved": "这个变更解决了什么问题？（1-2句话，从功能层面描述）",
    "inferred_motivation": "作者可能的动机是什么？（2-3句话，考虑技术债务、架构演进、性能瓶颈等）",
    "architectural_context": "这个变更与项目整体架构/之前 commit 的关系？（说明影响的层次）",
    "confidence": 0.0~1.0 的置信度分数
}

注意：
- 请识别重构（改名、提取、移动）而非简单的行变更
- 关注跨文件的一致性变更
- 如果涉及 API 变更，请标注兼容性影响

只输出 JSON，不要包含其他内容。"#,
            vec![
                "change_type".to_string(),
                "msg".to_string(),
                "stats".to_string(),
                "diff".to_string(),
            ],
        ));
    }

    fn add_template(&mut self, template: PromptTemplate) {
        self.templates.insert(template.id.clone(), template);
    }

    /// 获取模板
    pub fn get(&self, id: &str) -> Option<&PromptTemplate> {
        self.templates.get(id)
    }

    /// 列出所有模板
    pub fn list(&self) -> Vec<&PromptTemplate> {
        self.templates.values().collect()
    }

    /// 渲染模板
    pub fn render(&self, id: &str, values: &HashMap<String, String>) -> String {
        self.get(id)
            .map(|t| t.render(values))
            .unwrap_or_else(|| format!("Template '{}' not found", id))
    }

    /// 获取7个核心Agent的模板ID列表
    pub fn core_agent_templates() -> Vec<&'static str> {
        vec![
            "semantic_interpreter",
            "merge_agent",
            "cross_commit_recovery",
            "multi_agent_coordinator",
            "validation_risk",
            "commit_intelligence",
        ]
    }
}

impl Default for PromptTemplateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_render() {
        let template = PromptTemplate::new(
            "test",
            "Test",
            "A test template",
            "Hello {name}! Current diff: {diff}",
            vec!["name".to_string(), "diff".to_string()],
        );

        let mut values = HashMap::new();
        values.insert("name".to_string(), "Alice".to_string());
        values.insert("diff".to_string(), "-- test diff".to_string());

        let result = template.render(&values);
        assert_eq!(result, "Hello Alice! Current diff: -- test diff");
    }

    #[test]
    fn test_default_templates_loaded() {
        let manager = PromptTemplateManager::new();
        let templates = manager.list();
        assert_eq!(templates.len(), 6);
    }

    #[test]
    fn test_get_core_templates() {
        let manager = PromptTemplateManager::new();
        for id in PromptTemplateManager::core_agent_templates() {
            assert!(manager.get(id).is_some(), "Missing template: {}", id);
        }
    }

    #[test]
    fn test_render_semantic_interpreter() {
        let manager = PromptTemplateManager::new();
        let mut values = HashMap::new();
        values.insert(
            "user_input_or_diff".to_string(),
            "diff --git a/src/main.rs ...".to_string(),
        );

        let rendered = manager.render("semantic_interpreter", &values);
        assert!(rendered.contains("diff --git"));
        assert!(rendered.contains("代码语义分析专家"));
    }
}
