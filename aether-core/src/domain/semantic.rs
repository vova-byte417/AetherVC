// 语义相关的类型已在 commit.rs 中定义
// 此模块作为领域模型的补充导出

pub use super::commit::{
    ChangeCategory, ChangeType, Commit, CommitId, CommitMetadata, FileChange, IndexReport,
    RiskLevel, SearchFilters, SearchResult, SemanticInfo, VectorStoreStats, CurrentState,
};
