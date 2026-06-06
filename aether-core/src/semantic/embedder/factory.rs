//! Embedder 工厂
//!
//! 根据配置自动创建合适的 Embedder 实例。

use crate::config::types::LLMConfig;
use crate::semantic::embedder::openai::OpenAIEmbedder;
use crate::semantic::embedder::retry::RetryEmbedder;
use crate::semantic::embedder::{Embedder, MockEmbedder};
use std::sync::Arc;

/// Embedder 工厂
pub struct EmbedderFactory;

impl EmbedderFactory {
    /// 根据 LLMConfig 创建 Embedder
    ///
    /// 支持的 provider:
    /// - "openai" / "deepseek" / "openai-compatible" → OpenAIEmbedder
    /// - "mock" 或未配置 → MockEmbedder
    ///
    /// 如果配置了 max_retries > 1，会自动包装 RetryEmbedder
    pub fn create(config: &LLMConfig) -> Arc<dyn Embedder> {
        let base: Arc<dyn Embedder> = match config.provider.as_str() {
            "openai" | "deepseek" | "openai-compatible" => {
                let api_base = if config.api_base.is_empty() {
                    match config.provider.as_str() {
                        "deepseek" => "https://api.deepseek.com/v1".to_string(),
                        _ => "https://api.openai.com/v1".to_string(),
                    }
                } else {
                    config.api_base.clone()
                };

                let embedding_model = if config.embedding_model.is_empty() {
                    "text-embedding-3-small".to_string()
                } else {
                    config.embedding_model.clone()
                };

                tracing::info!(
                    "[EmbedderFactory] 创建 OpenAIEmbedder: base={}, model={}",
                    api_base,
                    embedding_model
                );

                Arc::new(OpenAIEmbedder::new(api_base, &config.api_key, embedding_model))
            }
            _ => {
                tracing::warn!(
                    "[EmbedderFactory] provider={} 不明确，回退到 MockEmbedder",
                    config.provider
                );
                Arc::new(MockEmbedder::default())
            }
        };

        // 如果配置了重试，包装 RetryEmbedder
        if config.max_retries > 1 {
            tracing::info!(
                "[EmbedderFactory] 包装 RetryEmbedder，max_retries={}",
                config.max_retries
            );
            Arc::new(RetryEmbedder::new(base, config.max_retries))
        } else {
            base
        }
    }

    /// 创建测试用 MockEmbedder
    pub fn create_mock() -> Arc<dyn Embedder> {
        Arc::new(MockEmbedder::default())
    }
}
