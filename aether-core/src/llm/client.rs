use crate::domain::commit::SemanticInfo;
use crate::utils::Result;
use serde::{Deserialize, Serialize};

/// LLM 响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    pub content: String,
    pub tokens_used: Option<u32>,
    pub model: String,
}

/// LLM 客户端抽象
#[async_trait::async_trait]
pub trait LLMClient: Send + Sync {
    /// 发送对话补全请求
    async fn complete(&self, prompt: &str) -> Result<LLMResponse>;

    /// 分析代码变更语义
    async fn analyze_semantic(&self, diff: &str, message: &str) -> Result<SemanticInfo>;

    /// 模型名称
    fn model_name(&self) -> &str;
}

/// Mock LLM 客户端（用于测试）
pub struct MockLLMClient {
    model: String,
}

impl MockLLMClient {
    pub fn new() -> Self {
        Self {
            model: "mock-llm".to_string(),
        }
    }

    pub fn with_model(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
        }
    }
}

impl Default for MockLLMClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl LLMClient for MockLLMClient {
    async fn complete(&self, _prompt: &str) -> Result<LLMResponse> {
        Ok(LLMResponse {
            content: "Mock response".to_string(),
            tokens_used: Some(0),
            model: self.model.clone(),
        })
    }

    async fn analyze_semantic(&self, _diff: &str, message: &str) -> Result<SemanticInfo> {
        use crate::domain::commit::{ChangeCategory, RiskLevel};

        // 基于 commit message 做简单分析
        let (change_type, risk) = if message.starts_with("feat") {
            (ChangeCategory::Feature, RiskLevel::Medium)
        } else if message.starts_with("fix") {
            (ChangeCategory::Bugfix, RiskLevel::Low)
        } else if message.starts_with("refactor") {
            (ChangeCategory::Refactor, RiskLevel::Medium)
        } else if message.starts_with("perf") {
            (ChangeCategory::Performance, RiskLevel::Low)
        } else if message.contains("breaking") {
            (ChangeCategory::Breaking, RiskLevel::Critical)
        } else {
            (ChangeCategory::Feature, RiskLevel::Low)
        };

        Ok(SemanticInfo::new(
            message.to_string(),
            change_type,
            vec![],
            format!("Mock analysis: {}", message),
            risk,
        ))
    }

    fn model_name(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_llm_complete() {
        let client = MockLLMClient::new();
        let response = client.complete("Hello").await.unwrap();
        assert_eq!(response.content, "Mock response");
        assert_eq!(response.model, "mock-llm");
    }

    #[tokio::test]
    async fn test_mock_llm_analyze_semantic() {
        let client = MockLLMClient::new();
        let info = client
            .analyze_semantic("diff...", "feat: add login")
            .await
            .unwrap();
        assert_eq!(info.change_type, crate::domain::commit::ChangeCategory::Feature);
    }
}
