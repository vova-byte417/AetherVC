//! Retry LLM Client 装饰器
//!
//! 当真实 LLM 调用失败时自动重试（指数退避），最终仍失败则降级为规则分析。

use crate::domain::commit::SemanticInfo;
use crate::llm::client::{LLMClient, LLMResponse};
use crate::semantic::analyzer::{RuleBasedAnalyzer, SemanticAnalyzer};
use crate::utils::Result;
use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// 带重试和降级的 LLM 客户端包装器
pub struct RetryLLMClient {
    inner: Arc<dyn LLMClient>,
    max_retries: u32,
    base_backoff: Duration,
    pub degradation_count: AtomicU32,
}

impl RetryLLMClient {
    pub fn new(inner: Arc<dyn LLMClient>, max_retries: u32) -> Self {
        Self {
            inner,
            max_retries,
            base_backoff: Duration::from_secs(1),
            degradation_count: AtomicU32::new(0),
        }
    }

    pub fn is_degraded(&self) -> bool {
        self.degradation_count.load(Ordering::Relaxed) > 0
    }

    async fn try_with_retry<F, T>(&self, operation_name: &str, f: F) -> Result<T>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>,
    {
        let mut last_err = None;

        for attempt in 0..self.max_retries {
            if attempt > 0 {
                let wait = self.base_backoff * 2u32.pow(attempt - 1);
                tracing::warn!(
                    "[RetryLLM] {} 第 {}/{} 次重试，等待 {:?}",
                    operation_name,
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
                        "[RetryLLM] {} 调用失败 (attempt {}): {}",
                        operation_name,
                        attempt + 1,
                        e
                    );
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            crate::utils::AetherError::LLM(format!(
                "LLM {}: max retries ({}) exceeded",
                operation_name, self.max_retries
            ))
        }))
    }
}

#[async_trait]
impl LLMClient for RetryLLMClient {
    async fn complete(&self, prompt: &str) -> Result<LLMResponse> {
        let inner = Arc::clone(&self.inner);
        let prompt_owned = prompt.to_string();

        self.try_with_retry("complete", move || {
            let inner = Arc::clone(&inner);
            let p = prompt_owned.clone();
            Box::pin(async move { inner.complete(&p).await })
        })
        .await
    }

    async fn analyze_semantic(&self, diff: &str, message: &str) -> Result<SemanticInfo> {
        let inner = Arc::clone(&self.inner);
        let diff_owned = diff.to_string();
        let msg_owned = message.to_string();

        match self
            .try_with_retry("analyze_semantic", move || {
                let inner = Arc::clone(&inner);
                let d = diff_owned.clone();
                let m = msg_owned.clone();
                Box::pin(async move { inner.analyze_semantic(&d, &m).await })
            })
            .await
        {
            Ok(info) => Ok(info),
            Err(e) => {
                self.degradation_count.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    "[RetryLLM] LLM 语义分析降级到 RuleBasedAnalyzer，原因: {}. 已降级 {} 次",
                    e,
                    self.degradation_count.load(Ordering::Relaxed)
                );

                let analyzer = RuleBasedAnalyzer::new();
                analyzer.analyze(diff, message).await
            }
        }
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 始终成功的客户端
    struct AlwaysSuccessClient;
    #[async_trait::async_trait]
    impl LLMClient for AlwaysSuccessClient {
        async fn complete(&self, _prompt: &str) -> crate::utils::Result<LLMResponse> {
            Ok(LLMResponse {
                content: "success".into(),
                tokens_used: Some(10),
                model: "test".into(),
            })
        }
        async fn analyze_semantic(&self, _diff: &str, msg: &str) -> crate::utils::Result<SemanticInfo> {
            Ok(SemanticInfo::new(msg, crate::domain::commit::ChangeCategory::Feature,
                vec![], msg, crate::domain::commit::RiskLevel::Low))
        }
        fn model_name(&self) -> &str { "test" }
    }

    /// 第一次失败、之后成功的客户端
    struct FailThenSuccessClient {
        attempts: std::sync::atomic::AtomicU32,
    }
    impl FailThenSuccessClient {
        fn new() -> Self { Self { attempts: std::sync::atomic::AtomicU32::new(0) } }
    }
    #[async_trait::async_trait]
    impl LLMClient for FailThenSuccessClient {
        async fn complete(&self, _prompt: &str) -> crate::utils::Result<LLMResponse> {
            let n = self.attempts.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if n < 2 {
                Err(crate::utils::AetherError::LLM("transient error".into()))
            } else {
                Ok(LLMResponse { content: "ok".into(), tokens_used: Some(5), model: "test-fail-then-success".into() })
            }
        }
        async fn analyze_semantic(&self, _diff: &str, msg: &str) -> crate::utils::Result<SemanticInfo> {
            Ok(SemanticInfo::new(msg, crate::domain::commit::ChangeCategory::Feature,
                vec![], msg, crate::domain::commit::RiskLevel::Low))
        }
        fn model_name(&self) -> &str { "test-fail-then-success" }
    }

    /// 始终失败的客户端
    struct AlwaysFailClient;
    #[async_trait::async_trait]
    impl LLMClient for AlwaysFailClient {
        async fn complete(&self, _prompt: &str) -> crate::utils::Result<LLMResponse> {
            Err(crate::utils::AetherError::LLM("always fails".into()))
        }
        async fn analyze_semantic(&self, _diff: &str, _msg: &str) -> crate::utils::Result<SemanticInfo> {
            Err(crate::utils::AetherError::LLM("always fails".into()))
        }
        fn model_name(&self) -> &str { "always-fail" }
    }

    /// 始终成功的客户端 → 一次就返回
    #[tokio::test]
    async fn test_retry_success_first_try() {
        let client = RetryLLMClient::new(Arc::new(AlwaysSuccessClient), 3);
        let resp = client.complete("hello").await.unwrap();
        assert_eq!(resp.content, "success");
        assert_eq!(client.degradation_count.load(std::sync::atomic::Ordering::Relaxed), 0);
    }

    /// 失败后重试成功
    #[tokio::test]
    async fn test_retry_succeeds_after_failure() {
        let client = RetryLLMClient::new(Arc::new(FailThenSuccessClient::new()), 5);
        let resp = client.complete("retry-me").await.unwrap();
        assert_eq!(resp.content, "ok");
    }

    /// 全部重试失败 → 返回错误
    #[tokio::test]
    async fn test_retry_all_fail() {
        let client = RetryLLMClient::new(Arc::new(AlwaysFailClient), 2);
        let result = client.complete("fail").await;
        assert!(result.is_err());
    }

    /// analyze_semantic 降级到 RuleBasedAnalyzer
    #[tokio::test]
    async fn test_retry_analyze_semantic_fallback() {
        let client = RetryLLMClient::new(Arc::new(AlwaysFailClient), 1);
        let info = client.analyze_semantic("--- a/lib.rs\n+++ b/lib.rs\n+fn new() {}", "feat: test").await.unwrap();
        // 降级后仍能返回结果（规则分析器）
        assert!(!info.intent.is_empty());
        assert!(client.degradation_count.load(std::sync::atomic::Ordering::Relaxed) > 0);
    }

    /// is_degraded 检查
    #[tokio::test]
    async fn test_is_degraded() {
        let client = RetryLLMClient::new(Arc::new(AlwaysFailClient), 1);
        assert!(!client.is_degraded());
        let _ = client.complete("x").await;
        // complete 不增加 degradation_count（只有 analyze_semantic 降级时才加）
        assert!(!client.is_degraded());
    }

    /// model_name 透传
    #[test]
    fn test_model_name_passthrough() {
        let client = RetryLLMClient::new(Arc::new(AlwaysSuccessClient), 3);
        assert_eq!(client.model_name(), "test");
    }
}
