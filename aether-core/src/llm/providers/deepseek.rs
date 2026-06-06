//! DeepSeek LLM Provider
//!
//! DeepSeek API 兼容 OpenAI Chat Completions 格式。
//! Base URL: https://api.deepseek.com
//!
//! 支持模型: deepseek-chat, deepseek-reasoner

use crate::llm::client::{LLMClient, LLMResponse};
use crate::domain::commit::SemanticInfo;
use crate::utils::Result;
use async_trait::async_trait;

/// DeepSeek LLM 客户端
///
/// ## 使用示例
///
/// ```rust,ignore
/// let client = DeepSeekClient::new(
///     "sk-your-api-key",
///     "deepseek-chat"
/// );
/// let response = client.complete("Hello").await?;
/// ```
pub struct DeepSeekClient {
    api_key: String,
    model: String,
    http_client: reqwest::Client,
}

impl DeepSeekClient {
    /// DeepSeek API 地址
    const API_BASE: &'static str = "https://api.deepseek.com";

    /// 创建 DeepSeek 客户端
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            http_client: reqwest::Client::new(),
        }
    }

    /// 使用默认模型 deepseek-chat
    pub fn with_default_model(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "deepseek-chat")
    }

    /// 使用 deepseek-reasoner 模型（推理增强）
    pub fn with_reasoner(api_key: impl Into<String>) -> Self {
        Self::new(api_key, "deepseek-reasoner")
    }

    /// 构建 API 请求
    fn build_request(&self, system_prompt: Option<&str>, user_message: &str) -> ChatRequest {
        let mut messages = Vec::new();

        if let Some(sys) = system_prompt {
            messages.push(ChatMessage {
                role: "system".to_string(),
                content: sys.to_string(),
            });
        }

        messages.push(ChatMessage {
            role: "user".to_string(),
            content: user_message.to_string(),
        });

        ChatRequest {
            model: self.model.clone(),
            messages,
            temperature: 0.3,
            max_tokens: 4096,
            stream: false,
        }
    }

    /// 核心 API 调用
    async fn call_api(&self, system_prompt: Option<&str>, user_message: &str) -> Result<LLMResponse> {
        let request = self.build_request(system_prompt, user_message);
        let url = format!("{}/chat/completions", Self::API_BASE);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| crate::utils::AetherError::LLM(format!("DeepSeek HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::utils::AetherError::LLM(format!(
                "DeepSeek API error ({}): {}",
                status, body
            )));
        }

        let chat_response: ChatResponse = response
            .json()
            .await
            .map_err(|e| crate::utils::AetherError::LLM(format!("DeepSeek response parse error: {}", e)))?;

        let content = chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_default();

        Ok(LLMResponse {
            content,
            tokens_used: chat_response.usage.map(|u| u.total_tokens),
            model: self.model.clone(),
        })
    }

    /// 提取 JSON（处理 markdown code block 包裹的情况）
    fn extract_json(content: &str) -> String {
        let content = content.trim();

        // 尝试找到 ```json ... ``` 块
        if let Some(start) = content.find("```json") {
            let inner = &content[start + 7..];
            if let Some(end) = inner.find("```") {
                return inner[..end].trim().to_string();
            }
        }
        if let Some(start) = content.find("```") {
            let inner = &content[start + 3..];
            if let Some(end) = inner.find("```") {
                return inner[..end].trim().to_string();
            }
        }

        // 尝试找到第一个 { 到最后一个 }
        if let (Some(start), Some(end)) = (content.find('{'), content.rfind('}')) {
            return content[start..=end].to_string();
        }

        content.to_string()
    }
}

// ─── OpenAI 兼容的请求/响应类型 ───

#[derive(serde::Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(serde::Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(serde::Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(serde::Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(serde::Deserialize)]
struct ChoiceMessage {
    content: String,
}

#[derive(serde::Deserialize)]
struct Usage {
    total_tokens: u32,
}

// ─── LLMClient trait 实现 ───

#[async_trait]
impl LLMClient for DeepSeekClient {
    async fn complete(&self, prompt: &str) -> Result<LLMResponse> {
        self.call_api(None, prompt).await
    }

    /// 语义分析：使用结构化的 system prompt 引导 DeepSeek 输出 JSON
    async fn analyze_semantic(&self, diff: &str, message: &str) -> Result<SemanticInfo> {
        let system_prompt = r#"你是一个精确的代码变更分析引擎。你的任务是分析 git diff 并输出严格的 JSON 格式结果。

JSON 格式要求：
{
  "intent": "一句话描述这次变更做了什么（中文）",
  "change_type": "feature|bugfix|refactor|performance|breaking|documentation|test|security|dependency|config",
  "affected_modules": ["受影响的模块名列表"],
  "semantic_summary": "2-3句话详细描述变更内容、影响范围和潜在风险（中文）",
  "risk_level": "low|medium|high|critical"
}

风险等级判定标准：
- critical: 涉及数据库 schema 变更、认证/授权逻辑、支付流程、破坏性 API 变更
- high: 核心业务逻辑修改、大量文件变更（>10个文件）、API 接口签名变更
- medium: 一般功能修改、中等规模重构（3-10个文件）
- low: 文档、测试、格式化、小修复

只输出 JSON，不要输出解释文字。"#;

        let user_message = format!(
            "分析以下 Git 变更：\n\nCommit Message: {}\n\nDiff:\n{}",
            message,
            if diff.len() > 8000 { &diff[..8000] } else { diff }
        );

        let response = self.call_api(Some(system_prompt), &user_message).await?;
        let json_str = Self::extract_json(&response.content);

        // 解析 JSON，失败时降级为规则分析
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap_or_else(|_| {
            serde_json::json!({
                "intent": message,
                "change_type": infer_change_type(message),
                "affected_modules": [],
                "semantic_summary": message,
                "risk_level": "low"
            })
        });

        Ok(SemanticInfo::new(
            parsed["intent"].as_str().unwrap_or(message),
            parse_change_type(parsed["change_type"].as_str().unwrap_or("feature")),
            parsed["affected_modules"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default(),
            parsed["semantic_summary"].as_str().unwrap_or("Unknown").to_string(),
            parse_risk_level(parsed["risk_level"].as_str().unwrap_or("low")),
        ))
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

/// 降级用：从 message 推断变更类型
fn infer_change_type(message: &str) -> &str {
    let msg = message.to_lowercase();
    if msg.starts_with("feat") || msg.contains("add") { "feature" }
    else if msg.starts_with("fix") || msg.contains("bug") { "bugfix" }
    else if msg.starts_with("refactor") { "refactor" }
    else if msg.starts_with("perf") || msg.contains("optimize") { "performance" }
    else if msg.contains("breaking") { "breaking" }
    else if msg.starts_with("doc") { "documentation" }
    else if msg.starts_with("test") { "test" }
    else if msg.starts_with("security") { "security" }
    else { "feature" }
}

fn parse_change_type(ct: &str) -> crate::domain::commit::ChangeCategory {
    match ct {
        "feature" => crate::domain::commit::ChangeCategory::Feature,
        "bugfix" => crate::domain::commit::ChangeCategory::Bugfix,
        "refactor" => crate::domain::commit::ChangeCategory::Refactor,
        "performance" => crate::domain::commit::ChangeCategory::Performance,
        "breaking" => crate::domain::commit::ChangeCategory::Breaking,
        "documentation" => crate::domain::commit::ChangeCategory::Documentation,
        "test" => crate::domain::commit::ChangeCategory::Test,
        _ => crate::domain::commit::ChangeCategory::Feature,
    }
}

fn parse_risk_level(rl: &str) -> crate::domain::commit::RiskLevel {
    match rl {
        "low" => crate::domain::commit::RiskLevel::Low,
        "medium" => crate::domain::commit::RiskLevel::Medium,
        "high" => crate::domain::commit::RiskLevel::High,
        "critical" => crate::domain::commit::RiskLevel::Critical,
        _ => crate::domain::commit::RiskLevel::Low,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_change_type() {
        assert_eq!(infer_change_type("feat: add login"), "feature");
        assert_eq!(infer_change_type("fix: auth bug"), "bugfix");
        assert_eq!(infer_change_type("refactor: cleanup"), "refactor");
        assert_eq!(infer_change_type("docs: update README"), "documentation");
    }

    #[test]
    fn test_extract_json_from_markdown() {
        let md = r#"```json
{"key": "value"}
```"#;
        assert_eq!(DeepSeekClient::extract_json(md), r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_plain() {
        let json = r#"{"key": "value"}"#;
        assert_eq!(DeepSeekClient::extract_json(json), r#"{"key": "value"}"#);
    }

    #[test]
    fn test_extract_json_with_text() {
        let text = "Some text\n{\"key\": \"value\"}\nMore text";
        assert_eq!(DeepSeekClient::extract_json(text), r#"{"key": "value"}"#);
    }
}
