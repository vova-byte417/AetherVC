//! 真实 LLM / Embedder 集成测试
//!
//! 这些测试**仅当**设置了真实 API Key 时运行。
//! 默认 `cargo test` 会跳过它们（标记 `#[ignore]`）。
//! 运行方式：
//!   $env:DEEPSEEK_API_KEY="sk-..." ; cargo test --test real_llm_tests -- --ignored --nocapture
//!
//! CI 中通过 `cargo test -- --ignored` 启用。

use aether_core::config::types::LLMConfig;
use aether_core::llm::client::LLMClient;
use aether_core::llm::factory::LLMFactory;
use aether_core::llm::providers::deepseek::DeepSeekClient;
use aether_core::semantic::embedder::openai::OpenAIEmbedder;
use aether_core::semantic::embedder::Embedder;

/// 检查是否有真实 API Key
fn has_api_key() -> bool {
    std::env::var("DEEPSEEK_API_KEY").is_ok() || std::env::var("OPENAI_API_KEY").is_ok()
}

/// 获取 API Key（优先 DeepSeek）
fn get_api_key() -> Option<String> {
    std::env::var("DEEPSEEK_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .ok()
}

// ═══════════════════════════════════════════════════════════════
// LLM Factory 集成测试
// ═══════════════════════════════════════════════════════════════

/// 在无 API Key 时，from_config_or_env 回退到 MockLLMClient
#[tokio::test]
async fn test_factory_fallback_to_mock() {
    // 如果用户已设置了 API Key，直接跳过这个测试
    if has_api_key() {
        eprintln!("Skipping fallback test: API key is set");
        return;
    }

    let config = LLMConfig::default();
    let client = LLMFactory::from_config_or_env(&config);
    // 应该回退到 MockLLMClient
    let resp = client.complete("test").await.unwrap();
    assert!(!resp.model.is_empty());
    // Mock 客户端返回 "mock-llm" 模型名
    assert!(client.model_name().contains("mock"));
}

/// 有 API Key 时创建真实客户端
#[tokio::test]
#[ignore = "requires DEEPSEEK_API_KEY or OPENAI_API_KEY"]
async fn test_factory_from_config_or_env_real() {
    let _key = get_api_key().expect("API key required");
    let mut config = LLMConfig::default();
    config.api_key = get_api_key().unwrap();

    let client = LLMFactory::from_config_or_env(&config);
    // 真实客户端不应是 mock
    assert!(!client.model_name().contains("mock"));
}

// ═══════════════════════════════════════════════════════════════
// DeepSeek 真实调用测试
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "requires DEEPSEEK_API_KEY"]
async fn test_real_deepseek_completion() {
    let api_key = std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY not set");
    let client = DeepSeekClient::new(api_key, "deepseek-chat");
    let resp = client.complete("用一句话回复：1+1等于几？").await.unwrap();
    assert!(!resp.content.is_empty());
    println!("DeepSeek 回复: {}", resp.content);
    assert!(resp.tokens_used.unwrap_or(0) > 0);
}

// ═══════════════════════════════════════════════════════════════
// OpenAI 真实 Embedding 测试
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
#[ignore = "requires DEEPSEEK_API_KEY or OPENAI_API_KEY"]
async fn test_real_embedding_single() {
    let api_key = get_api_key().expect("API key not set");
    let api_base = if std::env::var("DEEPSEEK_API_KEY").is_ok() {
        "https://api.deepseek.com/v1"
    } else {
        "https://api.openai.com/v1"
    };

    let embedder = OpenAIEmbedder::new(api_base, api_key, "text-embedding-3-small");
    let vec = embedder.embed("hello world").await.unwrap();

    // text-embedding-3-small → 1536 维
    assert_eq!(vec.len(), 1536);
    // 验证向量非全零
    let sum: f32 = vec.iter().sum();
    assert!(sum != 0.0, "嵌入向量不应全为零");
    println!("Embedding 维度: {}, 前5值: {:?}", vec.len(), &vec[..5]);
}

#[tokio::test]
#[ignore = "requires DEEPSEEK_API_KEY or OPENAI_API_KEY"]
async fn test_real_embedding_batch() {
    let api_key = get_api_key().expect("API key not set");
    let api_base = if std::env::var("DEEPSEEK_API_KEY").is_ok() {
        "https://api.deepseek.com/v1"
    } else {
        "https://api.openai.com/v1"
    };

    let embedder = OpenAIEmbedder::new(api_base, api_key, "text-embedding-3-small");
    let texts: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
    let vecs = embedder.embed_batch(&texts).await.unwrap();

    assert_eq!(vecs.len(), 3);
    assert_eq!(vecs[0].len(), 1536);

    // 不同文本的向量应不同
    assert_ne!(vecs[0], vecs[1]);
}

// ═══════════════════════════════════════════════════════════════
// Embedder 工厂在 mock 模式下维度正确
// ═══════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_embedder_factory_dimension() {
    use aether_core::semantic::embedder::factory::EmbedderFactory;

    let config = LLMConfig::default();
    let embedder = EmbedderFactory::create(&config);
    // 即使没有 API key，dimension 也应返回合理值
    assert!(embedder.dimension() > 0);
    let vec = embedder.embed("test").await.unwrap();
    assert_eq!(vec.len(), embedder.dimension());
}
