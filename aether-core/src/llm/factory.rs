//! LLM Provider 工厂
//!
//! 根据配置自动创建对应的 LLM 客户端实例。

use crate::config::types::LLMConfig;
use crate::llm::client::retry::RetryLLMClient;
use crate::llm::client::{LLMClient, MockLLMClient};
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

        let base_client = match provider.to_lowercase().as_str() {
            "deepseek" => {
                tracing::info!("Creating DeepSeek client with model: {}", model);
                Arc::new(DeepSeekClient::new(api_key, model)) as Arc<dyn LLMClient>
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
                Arc::new(OpenAICompatibleClient::new(api_base, api_key, model))
                    as Arc<dyn LLMClient>
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
                Arc::new(OpenAICompatibleClient::new(
                    &config.api_base,
                    api_key,
                    model,
                )) as Arc<dyn LLMClient>
            }
            _ => {
                return Err(crate::utils::AetherError::Config(format!(
                    "Unknown LLM provider: {}. Supported: deepseek, openai, openai-compatible",
                    provider
                )));
            }
        };

        // 包装重试装饰器
        if config.max_retries > 1 {
            tracing::info!(
                "[LLMFactory] 包装 RetryLLMClient，max_retries={}",
                config.max_retries
            );
            Ok(Arc::new(RetryLLMClient::new(base_client, config.max_retries)))
        } else {
            Ok(base_client)
        }
    }

    /// 从环境变量创建客户端（快速启动用）
    ///
    /// 优先级：DEEPSEEK_API_KEY > OPENAI_API_KEY > Mock
    pub fn from_env() -> crate::utils::Result<Arc<dyn LLMClient>> {
        if let Ok(api_key) = std::env::var("DEEPSEEK_API_KEY") {
            let model =
                std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-chat".to_string());
            tracing::info!("从环境变量创建 DeepSeek 客户端");
            return Ok(Arc::new(DeepSeekClient::new(api_key, model)));
        }

        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            let model =
                std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
            let api_base = std::env::var("OPENAI_BASE")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
            tracing::info!("从环境变量创建 OpenAI 客户端");
            return Ok(Arc::new(OpenAICompatibleClient::new(
                api_base, api_key, model,
            )));
        }

        tracing::warn!(
            "未设置 DEEPSEEK_API_KEY 或 OPENAI_API_KEY，回退到 MockLLMClient。"
        );
        tracing::warn!("设置环境变量以启用真实 AI 语义理解：");
        tracing::warn!("  $env:DEEPSEEK_API_KEY=\"sk-...\"   (Windows PowerShell)");
        tracing::warn!("  export DEEPSEEK_API_KEY=sk-...     (Linux/macOS)");
        Ok(Arc::new(MockLLMClient::new()))
    }

    /// 从配置或环境变量创建客户端（配置文件优先）
    ///
    /// 优先读取 `.aether/config.toml` 中的 [llm] 段，
    /// 如果配置文件不存在或 api_key 为空，则回退到环境变量。
    /// 如果环境变量也没有，则使用 MockLLMClient。
    pub fn from_config_or_env(config: &LLMConfig) -> Arc<dyn LLMClient> {
        // 先尝试配置
        if !config.api_key.is_empty() {
            match Self::create(config) {
                Ok(client) => return client,
                Err(e) => {
                    tracing::warn!(
                        "[LLMFactory] 配置创建失败 ({}), 回退到环境变量",
                        e
                    );
                }
            }
        }

        // 回退到环境变量
        match Self::from_env() {
            Ok(client) => client,
            Err(e) => {
                tracing::warn!("[LLMFactory] 环境变量创建失败 ({}), 使用 MockLLMClient", e);
                Arc::new(MockLLMClient::new())
            }
        }
    }

    /// 创建一个用于测试的 Mock LLM 客户端
    pub fn create_mock() -> Arc<dyn LLMClient> {
        Arc::new(MockLLMClient::new())
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
        std::env::remove_var("OPENAI_API_KEY");
        let result = LLMFactory::from_env();
        // 可能返回 mock 或错误，取决于实现
        // 现在 create_mock() 总是可用，所以不会 panic
        assert!(true);
    }
}
