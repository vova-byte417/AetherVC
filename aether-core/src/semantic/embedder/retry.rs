//! Retry Embedder 装饰器
//!
//! 当真实 Embedder 调用失败时自动重试，最终仍失败则降级为 MockEmbedder。

use crate::semantic::embedder::{Embedder, MockEmbedder};
use crate::utils::Result;
use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// 带重试和降级的 Embedder 包装器
pub struct RetryEmbedder {
    inner: Arc<dyn Embedder>,
    max_retries: u32,
    base_backoff: Duration,
    fallback: MockEmbedder,
    pub degradation_count: AtomicU32,
}

impl RetryEmbedder {
    pub fn new(inner: Arc<dyn Embedder>, max_retries: u32) -> Self {
        let dimension = inner.dimension();
        Self {
            inner,
            max_retries,
            base_backoff: Duration::from_secs(1),
            fallback: MockEmbedder::new(dimension),
            degradation_count: AtomicU32::new(0),
        }
    }

    pub fn is_degraded(&self) -> bool {
        self.degradation_count.load(Ordering::Relaxed) > 0
    }

    async fn try_with_retry<F, T>(&self, f: F) -> Result<T>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>,
    {
        let mut last_err = None;

        for attempt in 0..self.max_retries {
            if attempt > 0 {
                let wait = self.base_backoff * 2u32.pow(attempt - 1);
                tracing::warn!(
                    "[RetryEmbedder] 第 {}/{} 次重试，等待 {:?}",
                    attempt,
                    self.max_retries,
                    wait
                );
                sleep(wait).await;
            }

            match f().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::warn!(
                        "[RetryEmbedder] 调用失败 (attempt {}): {}",
                        attempt + 1,
                        e
                    );
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            crate::utils::AetherError::LLM("Embedder: max retries exceeded".to_string())
        }))
    }
}

#[async_trait]
impl Embedder for RetryEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let inner = Arc::clone(&self.inner);
        let text_owned = text.to_string();

        match self
            .try_with_retry(move || {
                let inner = Arc::clone(&inner);
                let t = text_owned.clone();
                Box::pin(async move { inner.embed(&t).await })
            })
            .await
        {
            Ok(v) => Ok(v),
            Err(e) => {
                self.degradation_count.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    "[RetryEmbedder] 降级到 MockEmbedder，原因: {}",
                    e
                );
                self.fallback.embed(text).await
            }
        }
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let inner = Arc::clone(&self.inner);
        let texts_owned = texts.to_vec();

        match self
            .try_with_retry(move || {
                let inner = Arc::clone(&inner);
                let t = texts_owned.clone();
                Box::pin(async move { inner.embed_batch(&t).await })
            })
            .await
        {
            Ok(v) => Ok(v),
            Err(e) => {
                self.degradation_count.fetch_add(1, Ordering::Relaxed);
                tracing::warn!("[RetryEmbedder] 降级到 MockEmbedder (batch)，原因: {}", e);
                self.fallback.embed_batch(texts).await
            }
        }
    }

    fn dimension(&self) -> usize {
        self.inner.dimension()
    }
}
