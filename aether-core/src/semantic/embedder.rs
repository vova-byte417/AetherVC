use crate::utils::Result;

/// 嵌入器抽象 - 将文本转换为向量
#[async_trait::async_trait]
pub trait Embedder: Send + Sync {
    /// 单条文本向量化
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// 批量向量化
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;

    /// 向量维度
    fn dimension(&self) -> usize;
}

/// Mock 嵌入器（用于测试，返回固定维度向量）
pub struct MockEmbedder {
    dimension: usize,
}

impl MockEmbedder {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    /// 基于文本内容生成伪随机但确定性的向量
    fn text_to_vec(&self, text: &str) -> Vec<f32> {
        let mut v = vec![0.0; self.dimension];
        let bytes = text.as_bytes();
        for (i, &b) in bytes.iter().enumerate() {
            if i < self.dimension {
                // 映射到 [-1.0, 1.0]
                v[i] = (b as f32 / 127.5) - 1.0;
            }
        }
        // 正则化
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            v.iter_mut().for_each(|x| *x /= norm);
        }
        v
    }
}

impl Default for MockEmbedder {
    fn default() -> Self {
        Self::new(384)
    }
}

#[async_trait::async_trait]
impl Embedder for MockEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        Ok(self.text_to_vec(text))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts.iter().map(|t| self.text_to_vec(t)).collect())
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_embedder_consistency() {
        let embedder = MockEmbedder::default();
        let v1 = embedder.embed("hello").await.unwrap();
        let v2 = embedder.embed("hello").await.unwrap();
        assert_eq!(v1, v2);
    }

    #[tokio::test]
    async fn test_mock_embedder_different() {
        let embedder = MockEmbedder::default();
        let v1 = embedder.embed("hello").await.unwrap();
        let v2 = embedder.embed("world").await.unwrap();
        // 不同文本应该产生不同向量
        assert_ne!(v1, v2);
    }

    #[tokio::test]
    async fn test_batch_embed() {
        let embedder = MockEmbedder::default();
        let texts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let vs = embedder.embed_batch(&texts).await.unwrap();
        assert_eq!(vs.len(), 3);
        assert_eq!(vs[0].len(), 384);
    }
}
