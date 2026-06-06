//! AetherCommit Intelligence (AetherCI)
//!
//! AetherVC 的核心子模块，提供代码变更语义理解与意图文档生成能力。
//!
//! ## 主要组件
//!
//! - `domain` - 领域模型（报告、置信度、变更实体等）
//! - `pipeline` - 五阶段语义 Diff 理解流水线
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use aetherci::pipeline::orchestrator::SemanticDiffPipeline;
//! use aetherci::domain::PipelineInput;
//!
//! let pipeline = SemanticDiffPipeline::default_pipeline();
//! let input = PipelineInput { /* ... */ };
//! let output = pipeline.execute(&input).await;
//! println!("{}", output.markdown);
//! ```

pub mod domain;
pub mod pipeline;

// 便捷重导出
pub use pipeline::orchestrator::SemanticDiffPipeline;
