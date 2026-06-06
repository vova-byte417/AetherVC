//! LLM Provider 工厂
//!
//! 根据配置自动创建对应的 LLM 客户端实例。

use crate::config::types::LLMConfig;
use crate::llm::client::LLMClient;
use crate::llm::providers::deepseek::DeepSeekClient;
use crate::llm::providers::openai::OpenAICompatibleClient;
use std::sync::Arc;

/// LLM 客户端工厂
pub struct LLMFactory;

impl LLMFactory {
    /// 根据配置创建 LLM 客户端
    ///
    /// 支持 provider: "deepseek", "openai", "openai-compatible"
    pub fn create(config: &LLMConfig) -> crate::utils::Result<Arc<dyn LLMClient>> {
        let api_key = if config.api_key.is_empty() {
            return Err(crate::utils::AetherError::Config(
                "LLM api_key not configured".to_string(),
            ));
        } else {
            &config.api_key
        };

        let model = if config.model.is_empty() {
            "deepseek-chat"
        } else {
            &config.model
        };
        let provider = if config.provider.is_empty() {
            "deepseek"
        } else {
            &config.provider
        };

        match provider.to_lowercase().as_str() {
            "deepseek" => {
                tracing::info!("Creating DeepSeek client with model: {}", model);
                Ok(Arc::new(DeepSeekClient::new(api_key, model)))
            }
            "openai" => {
                let api_base = if config.api_base.is_empty() {
                    "https://api.openai.com/v1"
                } else {
                    &config.api_base
                };
                tracing::info!(
                    "Creating OpenAI client with base: {}, model: {}",
                    api_base,
                    model
                );
                Ok(Arc::new(OpenAICompatibleClient::new(
                    api_base, api_key, model,
                )))
            }
            "openai-compatible" => {
                if config.api_base.is_empty() {
                    return Err(crate::utils::AetherError::Config(
                        "LLM api_base required for openai-compatible provider".to_string(),
                    ));
                }
                tracing::info!(
                    "Creating OpenAI-compatible client with base: {}, model: {}",
                    config.api_base,
                    model
                );
                Ok(Arc::new(OpenAICompatibleClient::new(
                    &config.api_base,
                    api_key,
                    model,
                )))
            }
            _ => Err(crate::utils::AetherError::Config(format!(
                "Unknown LLM provider: {}. Supported: deepseek, openai, openai-compatible",
                provider
            ))),
        }
    }

    /// 从环境变量创建客户端（快速启动用）
    ///
    /// 检测 DEEPSEEK_API_KEY 环境变量
    pub fn from_env() -> crate::utils::Result<Arc<dyn LLMClient>> {
        let api_key = std::env::var("DEEPSEEK_API_KEY").map_err(|_| {
            crate::utils::AetherError::Config(
                "DEEPSEEK_API_KEY environment variable not set".to_string(),
            )
        })?;

        let model =
            std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string());

        Ok(Arc::new(DeepSeekClient::new(api_key, model)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_unknown_provider() {
        let mut config = LLMConfig::default();
        config.provider = "unknown".to_string();
        config.api_key = "sk-test".to_string();

        let result = LLMFactory::create(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_factory_missing_api_key() {
        let config = LLMConfig::default();
        let result = LLMFactory::create(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_factory_deepseek_creates() {
        let mut config = LLMConfig::default();
        config.api_key = "sk-test".to_string();
        config.model = "deepseek-chat".to_string();

        let result = LLMFactory::create(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_factory_from_env_fails_without_key() {
        // 确保环境变量未设置
        std::env::remove_var("DEEPSEEK_API_KEY");
        let result = LLMFactory::from_env();
        assert!(result.is_err());
    }
}
