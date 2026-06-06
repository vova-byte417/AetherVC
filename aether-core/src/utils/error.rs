use thiserror::Error;

#[derive(Debug, Error)]
pub enum AetherError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Git error: {0}")]
    Git(String),

    #[error("Vector store error: {0}")]
    VectorStore(String),

    #[error("Graph store error: {0}")]
    GraphStore(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Embedding error: {0}")]
    Embedding(String),

    #[error("LLM error: {0}")]
    LLM(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Agent execution failed: {0}")]
    AgentError(String),

    #[error("Operation timeout: {0}")]
    Timeout(String),
}

pub type Result<T> = std::result::Result<T, AetherError>;
