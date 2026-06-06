use crate::llm::client::{LLMClient, LLMResponse};
use crate::domain::commit::SemanticInfo;
use crate::utils::Result;
use async_trait::async_trait;

/// OpenAI 兼容的 LLM 客户端
pub struct OpenAICompatibleClient {
    api_base: String,
    api_key: String,
    model: String,
    http_client: reqwest::Client,
}

impl OpenAICompatibleClient {
    pub fn new(api_base: impl Into<String>, api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_base: api_base.into(),
            api_key: api_key.into(),
            model: model.into(),
            http_client: reqwest::Client::new(),
        }
    }
}

#[derive(serde::Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    max_tokens: u32,
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

#[async_trait]
impl LLMClient for OpenAICompatibleClient {
    async fn complete(&self, prompt: &str) -> Result<LLMResponse> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            temperature: 0.3,
            max_tokens: 4096,
        };

        let response = self
            .http_client
            .post(format!("{}/chat/completions", self.api_base))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| crate::utils::AetherError::LLM(format!("HTTP error: {}", e)))?;

        let chat_response: ChatResponse = response
            .json()
            .await
            .map_err(|e| crate::utils::AetherError::LLM(format!("Parse error: {}", e)))?;

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

    async fn analyze_semantic(&self, diff: &str, message: &str) -> Result<SemanticInfo> {
        let prompt = format!(
            r#"你是一个代码语义分析专家。分析以下代码变更：

Commit Message: {}
Diff:
{}

请以JSON格式输出分析结果：
{{
  "intent": "功能描述",
  "change_type": "feature|refactor|bugfix|performance|breaking",
  "affected_modules": ["module1"],
  "semantic_summary": "详细语义描述",
  "risk_level": "low|medium|high"
}}"#,
            message, diff
        );

        let response = self.complete(&prompt).await?;

        // 尝试解析 JSON
        let info: serde_json::Value = serde_json::from_str(&response.content)
            .unwrap_or(serde_json::json!({
                "intent": message,
                "change_type": "feature",
                "affected_modules": [],
                "semantic_summary": "Unknown",
                "risk_level": "low"
            }));

        Ok(SemanticInfo::new(
            info["intent"].as_str().unwrap_or(message),
            match info["change_type"].as_str().unwrap_or("feature") {
                "feature" => crate::domain::commit::ChangeCategory::Feature,
                "refactor" => crate::domain::commit::ChangeCategory::Refactor,
                "bugfix" => crate::domain::commit::ChangeCategory::Bugfix,
                "performance" => crate::domain::commit::ChangeCategory::Performance,
                "breaking" => crate::domain::commit::ChangeCategory::Breaking,
                "documentation" => crate::domain::commit::ChangeCategory::Documentation,
                "test" => crate::domain::commit::ChangeCategory::Test,
                _ => crate::domain::commit::ChangeCategory::Feature,
            },
            info["affected_modules"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            info["semantic_summary"].as_str().unwrap_or("Unknown").to_string(),
            match info["risk_level"].as_str().unwrap_or("low") {
                "low" => crate::domain::commit::RiskLevel::Low,
                "medium" => crate::domain::commit::RiskLevel::Medium,
                "high" => crate::domain::commit::RiskLevel::High,
                "critical" => crate::domain::commit::RiskLevel::Critical,
                _ => crate::domain::commit::RiskLevel::Low,
            },
        ))
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}
