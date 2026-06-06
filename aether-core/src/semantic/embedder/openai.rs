//! OpenAI Embedding API 封装
//!
//! 支持 OpenAI 兼容的 Embedding API（OpenAI、DeepSeek、Ollama 等皆兼容此格式）。
//! 默认使用 text-embedding-3-small 模型（1536 维）。

use crate::semantic::embedder::Embedder;
use crate::utils::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// OpenAI 兼容 Embedder
pub struct OpenAIEmbedder {
    api_base: String,
    api_key: String,
    model: String,
    dimension: usize,
    http_client: reqwest::Client,
}

/// Embedding API 请求体
#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    input: EmbeddingInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoding_format: Option<String>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum EmbeddingInput {
    Single(String),
    Batch(Vec<String>),
}

/// Embedding API 响应体
#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

impl OpenAIEmbedder {
    /// 已知模型的维度映射
    const KNOWN_DIMENSIONS: &'static [(&'static str, usize)] = &[
        ("text-embedding-3-small", 1536),
        ("text-embedding-3-large", 3072),
        ("text-embedding-ada-002", 1536),
        ("voyage-code-2", 1536),
    ];

    /// 创建 OpenAI Embedder
    pub fn new(
        api_base: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let model = model.into();
        let dimension = Self::resolve_dimension(&model);

        Self {
            api_base: api_base.into(),
            api_key: api_key.into(),
            model,
            dimension,
            http_client: reqwest::Client::new(),
        }
    }

    /// 使用默认模型 `text-embedding-3-small`
    pub fn with_default_model(api_key: impl Into<String>) -> Self {
        Self::new("https://api.openai.com/v1", api_key, "text-embedding-3-small")
    }

    /// 解析模型对应的维度
    fn resolve_dimension(model: &str) -> usize {
        Self::KNOWN_DIMENSIONS
            .iter()
            .find(|(name, _)| *name == model)
            .map(|(_, dim)| *dim)
            .unwrap_or(1536)
    }

    /// 发送 embedding 请求
    async fn call_api(&self, input: EmbeddingInput) -> Result<Vec<Vec<f32>>> {
        let url = format!("{}/embeddings", self.api_base.trim_end_matches('/'));

        let payload = EmbeddingRequest {
            model: self.model.clone(),
            input,
            encoding_format: Some("float".to_string()),
        };

        let mut req = self.http_client.post(&url).json(&payload);

        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let response = req
            .send()
            .await
            .map_err(|e| crate::utils::AetherError::LLM(format!("Embedding HTTP error: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::utils::AetherError::LLM(format!(
                "Embedding API error ({}): {}",
                status, body
            )));
        }

        let resp: EmbeddingResponse = response
            .json()
            .await
            .map_err(|e| {
                crate::utils::AetherError::LLM(format!("Embedding parse error: {}", e))
            })?;

        let embeddings: Vec<Vec<f32>> =
            resp.data.into_iter().map(|d| d.embedding).collect();

        Ok(embeddings)
    }
}

#[async_trait]
impl Embedder for OpenAIEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut results = self
            .call_api(EmbeddingInput::Single(text.to_string()))
            .await?;
        results.pop().ok_or_else(|| {
            crate::utils::AetherError::LLM("Embedding API returned empty result".to_string())
        })
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        self.call_api(EmbeddingInput::Batch(texts.to_vec())).await
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dimension_resolution() {
        assert_eq!(
            OpenAIEmbedder::resolve_dimension("text-embedding-3-small"),
            1536
        );
        assert_eq!(
            OpenAIEmbedder::resolve_dimension("text-embedding-3-large"),
            3072
        );
        assert_eq!(OpenAIEmbedder::resolve_dimension("unknown-model"), 1536);
    }
}
