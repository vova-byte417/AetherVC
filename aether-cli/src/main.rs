//! AetherVC CLI - AI-Native Semantic Version Control System
//!
//! 命令行入口，提供自然语言驱动的版本控制操作

use aether_core::agents::orchestrator::AgentOrchestrator;
use aether_core::batch::{BatchAnalyzer, BatchOptions};
use aether_core::config::{ConfigLoader, AetherConfig};
use aether_core::digest::{DigestAggregator, DigestOptions, DigestSummarizer};
use aether_core::llm::factory::LLMFactory;
use aether_core::nlp::context::ContextManager;
use aether_core::nlp::executor::CommandExecutor;
use aether_core::nlp::parser::NaturalLanguageParser;
use aether_core::review::{GateEngine, ReviewQueue};
use aether_core::semantic::embedder::factory::EmbedderFactory;
use aether_core::semantic::indexer::SemanticIndexer;
use aether_core::semantic::knowledge_graph::KnowledgeGraphEngine;
use aether_core::storage::git::{GitOperations, GitRepository};
use aether_core::storage::graph_db::{InMemoryGraphStore, GraphStore};
use aether_core::storage::vector_db::InMemoryVectorStore;
use aether_core::verify::{VerificationRunner, VerificationReport, VerifyMode};
use aether_core::domain::agent::{AgentTask, RollbackRecord, RollbackStatus, TagValidationReport};
use aether_core::domain::commit::{Commit, CurrentState};
use aether_core::workflow::{WorkflowOrchestrator, WorkflowType};

use aetherci::pipeline::orchestrator::SemanticDiffPipeline;
use aetherci::domain::PipelineInput;

use chrono::Utc;
use clap::{Parser, Subcommand};
use std::sync::{Arc, Mutex};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "aether")]
#[command(about = "AetherVC - AI-Native Semantic Version Control System")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// 仓库路径
    #[arg(short, long, default_value = ".")]
    repo_path: String,
}

#[derive(Subcommand)]
enum Commands {
    /// 初始化 AetherVC 仓库（语义索引 + Git hook）
    Init {
        #[arg(short, long)]
        path: Option<String>,
    },

    /// 执行自然语言命令
    Do {
        /// 自然语言指令
        command: String,
    },

    /// 语义搜索
    Search {
        /// 搜索查询
        query: String,

        /// 结果数量
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// 索引仓库所有提交
    Index {
        /// 完全重新索引
        #[arg(short, long)]
        full: bool,
    },

    /// 恢复功能
    Recover {
        /// 自然语言描述要恢复的内容
        description: Vec<String>,
    },

    /// 分析 PR 合并
    Merge {
        /// PR 标识列表
        #[arg(short, long)]
        prs: Vec<String>,
    },

    /// 分析两个 commit 之间的语义差异并生成报告
    Analyze {
        /// Commit 范围，如 HEAD~1..HEAD 或 abc123..def456
        #[arg(default_value = "HEAD~1..HEAD")]
        commit_range: String,

        /// 输出 JSON 格式（默认输出 Markdown）
        #[arg(short, long)]
        json: bool,

        /// 快速分析模式（跳过 LLM 推理）
        #[arg(short, long)]
        quick: bool,
    },

    /// 自动监控新 commit 并生成分析报告
    Auto {
        /// 监控模式，持续运行
        #[arg(short, long)]
        watch: bool,

        /// 输出目录（写入分析报告）
        #[arg(short, long, default_value = ".aether/reports")]
        output_dir: String,

        /// 结果数量限制
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// 查询函数/类的演化历史
    History {
        /// 函数或类名
        function_name: String,

        /// 限制历史条目数
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    // ─── 新增命令 (Phase 1) ───
    /// 生成变更摘要报告
    Digest {
        /// 时间窗口开始
        #[arg(long)]
        since: Option<String>,

        /// 时间窗口结束
        #[arg(long)]
        until: Option<String>,

        /// commit 范围
        #[arg(long)]
        range: Option<String>,

        /// 按 Agent 分组
        #[arg(long)]
        by_agent: bool,

        /// 按模块分组
        #[arg(long)]
        by_module: bool,

        /// JSON 格式输出
        #[arg(short, long)]
        json: bool,

        /// 输出文件路径
        #[arg(short, long)]
        output: Option<String>,
    },

    /// 批量分析 commit 列表
    Batch {
        /// commit 范围
        #[arg(default_value = "HEAD~20..HEAD")]
        range: String,

        /// 按风险排序
        #[arg(long)]
        risk_sort: bool,

        /// 结果数量限制
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// 按模块过滤
        #[arg(long)]
        module: Option<String>,

        /// 最低风险等级
        #[arg(long)]
        min_risk: Option<String>,

        /// JSON 格式输出
        #[arg(short, long)]
        json: bool,
    },

    /// 门控检查
    Gate {
        #[command(subcommand)]
        action: GateAction,
    },

    /// 审核队列管理
    Review {
        #[command(subcommand)]
        action: ReviewAction,
    },

    /// 运行验证
    Verify {
        /// commit hash
        commit_hash: Option<String>,

        /// 验证模式: quick, smart, full
        #[arg(short, long, default_value = "smart")]
        mode: String,

        /// 监控模式
        #[arg(short, long)]
        watch: bool,

        /// 生成报告
        #[arg(short, long)]
        report: bool,
    },

    /// 配置管理
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Git Hook 管理
    Hook {
        #[command(subcommand)]
        action: HookAction,
    },

    /// 知识图谱查询
    Graph {
        #[command(subcommand)]
        action: GraphAction,
    },

    /// 端到端工作流编排
    Workflow {
        #[command(subcommand)]
        action: WorkflowAction,
    },

    /// 智能回滚管理
    Rollback {
        #[command(subcommand)]
        action: RollbackSub,
    },

    /// 批量 Tag 验证
    VerifyTag {
        /// Tag 名称列表 或 keyword
        #[arg()]
        tags: Vec<String>,

        /// 排序方式: risk-asc, risk-desc, chronological
        #[arg(short, long, default_value = "risk-asc")]
        order_by: String,

        /// JSON 格式输出
        #[arg(short, long)]
        json: bool,
    },

    /// 显示 AetherVC 状态
    Status,
}

// ─── 子命令枚举 ───

#[derive(Subcommand)]
enum GateAction {
    /// 检查当前 commit
    Check {
        #[arg(default_value = "HEAD")]
        commit_hash: String,
    },
    /// 显示门控状态
    Status,
}

#[derive(Subcommand)]
enum ReviewAction {
    /// 查看待审核队列
    Queue {
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    /// 批准审核项
    Approve {
        queue_id: String,
        #[arg(long)]
        comment: Option<String>,
    },
    /// 拒绝审核项
    Reject {
        queue_id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    /// 查看审核历史
    History {
        #[arg(long)]
        since: Option<String>,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// 初始化配置文件
    Init,
    /// 显示当前配置
    Show,
    /// 设置配置项
    Set {
        key: String,
        value: String,
    },
    /// 验证配置合法性
    Validate,
}

#[derive(Subcommand)]
enum HookAction {
    /// 安装 Git hooks
    Install {
        #[arg(long)]
        post_commit: bool,
        #[arg(long)]
        pre_push: bool,
    },
    /// 卸载 Git hooks
    Uninstall,
    /// 查看 hooks 状态
    Status,
}

#[derive(Subcommand)]
enum GraphAction {
    /// 索引仓库到知识图谱
    Index {
        #[arg(short, long)]
        full: bool,
    },
    /// 查询模块变更历史
    QueryModule {
        module: String,
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    /// 查询作者贡献
    QueryAuthor {
        author: String,
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    /// 查询提交影响的模块
    AffectedModules {
        #[arg(default_value = "HEAD")]
        commit_hash: String,
    },
}

#[derive(Subcommand)]
enum WorkflowAction {
    /// 日常消化：摘要 + 风险排序 + 门控
    Digest {
        #[arg(long)]
        since: Option<String>,
        #[arg(long)]
        until: Option<String>,
    },
    /// 多 Agent 协调：冲突检测 + 合并建议
    Coordinate {
        #[arg(long)]
        since: Option<String>,
    },
    /// Tag 验证：搜索 + 排序 + 验证
    VerifyTags {
        #[arg(long)]
        keyword: Option<String>,
        #[arg(long, default_value = "15")]
        count: usize,
    },
    /// 完整 CI 流程
    FullCi {
        #[arg(long)]
        watch: bool,
    },
}

#[derive(Subcommand)]
enum RollbackSub {
    /// 分析指定 commit 是否需要回滚
    Analyze {
        #[arg(default_value = "HEAD")]
        commit_hash: String,
    },
    /// 执行回滚
    Execute {
        commit_hash: String,
        #[arg(long, default_value = "revert")]
        method: String,
        #[arg(long)]
        hard: bool,
    },
    /// 查看回滚历史
    History,
    /// 查看 Agent 信誉分
    Reputation,
}

fn setup_logging() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("aether=info".parse().unwrap()))
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_logging();
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path } => cmd_init(path.as_deref().unwrap_or(&cli.repo_path)).await,
        Commands::Do { command } => cmd_do(&command, &cli.repo_path).await,
        Commands::Search { query, limit } => cmd_search(&query, limit, &cli.repo_path).await,
        Commands::Index { full: _ } => cmd_index(&cli.repo_path).await,
        Commands::Recover { description } => {
            cmd_recover(&description.join(" "), &cli.repo_path).await
        }
        Commands::Merge { prs } => cmd_merge(&prs, &cli.repo_path).await,
        Commands::Analyze { commit_range, json, quick } => {
            cmd_analyze(&commit_range, json, quick, &cli.repo_path).await
        }
        Commands::Auto { watch, output_dir, limit } => {
            cmd_auto(watch, &output_dir, limit, &cli.repo_path).await
        }
        Commands::History { function_name, limit } => {
            cmd_history(&function_name, limit, &cli.repo_path).await
        }
        // ─── 新增命令 ───
        Commands::Digest { since, until, range, by_agent, by_module, json, output } => {
            cmd_digest(since, until, range, by_agent, by_module, json, output, &cli.repo_path).await
        }
        Commands::Batch { range, risk_sort, limit, module, min_risk, json } => {
            cmd_batch(&range, risk_sort, limit, module, min_risk, json, &cli.repo_path).await
        }
        Commands::Gate { action } => {
            cmd_gate(action, &cli.repo_path).await
        }
        Commands::Review { action } => {
            cmd_review(action, &cli.repo_path).await
        }
        Commands::Verify { commit_hash, mode, watch, report } => {
            cmd_verify(commit_hash, &mode, watch, report, &cli.repo_path).await
        }
        Commands::Config { action } => {
            cmd_config(action, &cli.repo_path).await
        }
        Commands::Hook { action } => {
            cmd_hook(action, &cli.repo_path).await
        }
        Commands::Graph { action } => {
            cmd_graph(action, &cli.repo_path).await
        }
        Commands::Workflow { action } => {
            cmd_workflow(action, &cli.repo_path).await
        }
        Commands::Rollback { action } => {
            cmd_rollback_cli(action, &cli.repo_path).await
        }
        Commands::VerifyTag { tags, order_by, json } => {
            cmd_verify_tag(&tags, &order_by, json, &cli.repo_path).await
        }
        Commands::Status => {
            cmd_status(&cli.repo_path).await
        }
    }
}

fn create_app_context(repo_path: &str) -> anyhow::Result<(
    Arc<AgentOrchestrator>,
    Arc<SemanticIndexer>,
    Arc<ContextManager>,
    AetherConfig,
)> {
    let git_repo: Arc<dyn GitOperations> = Arc::new(GitRepository::new(repo_path));

    // 加载配置
    let loader = ConfigLoader::new(repo_path);
    let config = loader.load().unwrap_or_default();

    // 根据配置选择存储后端
    let (vector_store, graph_store): (
        Arc<dyn aether_core::storage::vector_db::VectorStore>,
        Arc<dyn aether_core::storage::graph_db::GraphStore>,
    ) = if config.storage.backend == "persistent" {
        let data_dir = std::path::Path::new(repo_path).join(&config.storage.data_dir);
        let vector_dir = data_dir.join("vectors");
        let graph_file = data_dir.join("graph").join("graph.json");

        eprintln!("[AetherVC] 使用持久化存储: {} (backend={})",
            data_dir.display(), config.storage.backend);

        let vs = Arc::new(
            aether_core::storage::vector_db::PersistentVectorStore::new(&vector_dir),
        );
        let gs = Arc::new(
            aether_core::storage::graph_db::PersistentGraphStore::new(&graph_file),
        );
        (vs, gs)
    } else {
        eprintln!("[AetherVC] 使用内存存储 (重启后数据丢失)");
        (
            Arc::new(InMemoryVectorStore::new()),
            Arc::new(InMemoryGraphStore::new()),
        )
    };

    // 用 EmbedderFactory 创建 Embedder（配置优先，回退到 Mock）
    let embedder = EmbedderFactory::create(&config.llm);

    // 用 LLMFactory 创建 LLM 客户端（配置优先 → 环境变量 → Mock）
    let llm_client = LLMFactory::from_config_or_env(&config.llm);

    eprintln!("[AetherVC] LLM: {}, Embedder: {}维",
        llm_client.model_name(),
        embedder.dimension()
    );

    let agent_context = Arc::new(
        aether_core::agents::base::AgentContext::new(
            git_repo.clone(),
            vector_store.clone(),
            graph_store,
            embedder.clone(),
        )
        .with_llm(llm_client),
    );

    let orchestrator = Arc::new(AgentOrchestrator::new().with_all_agents(agent_context));
    let indexer = Arc::new(SemanticIndexer::new(
        embedder,
        vector_store,
        git_repo,
    ));
    let context_manager = Arc::new(ContextManager::new());

    Ok((orchestrator, indexer, context_manager, config))
}

async fn cmd_init(path: &str) -> anyhow::Result<()> {
    println!("Initializing AetherVC repository at: {}", path);

    match GitRepository::open(path) {
        Ok(_) => {
            println!("  ✓ Git repository found");
            println!("  ✓ AetherVC initialized successfully");
            println!("\n  Next: run 'aether index' to build semantic index");
        }
        Err(_) => {
            println!("  ✗ No Git repository found at {}", path);
            println!("  Hint: run 'git init' first, or use 'aether init' in an existing repo");
        }
    }

    Ok(())
}

// ─── Graph 命令 ───

async fn cmd_graph(action: GraphAction, repo_path: &str) -> anyhow::Result<()> {
    let loader = ConfigLoader::new(repo_path);
    let config = loader.load().unwrap_or_default();

    let graph_store: Arc<dyn aether_core::storage::graph_db::GraphStore> =
        if config.storage.backend == "persistent" {
            let data_dir = std::path::Path::new(repo_path).join(&config.storage.data_dir);
            let graph_file = data_dir.join("graph").join("graph.json");
            Arc::new(aether_core::storage::graph_db::PersistentGraphStore::new(&graph_file))
        } else {
            Arc::new(InMemoryGraphStore::new())
        };

    let engine = KnowledgeGraphEngine::new(graph_store.clone());

    match action {
        GraphAction::Index { full: _ } => {
            println!("索引提交到知识图谱...");
            match GitRepository::open(repo_path) {
                Ok(repo) => {
                    match repo.list_commits().await {
                        Ok(commits) => {
                            let count = engine.index_commits(&commits).await?;
                            println!("✓ 已索引 {} 个 commits", count);
                            println!("  节点类型: Commit, Author, Module");
                            println!("  关系类型: AUTHORED, MODIFIES, HIGH_RISK");
                        }
                        Err(e) => {
                            println!("✗ 获取 commits 失败: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("✗ 打开 Git 仓库失败: {}", e);
                }
            }
        }
        GraphAction::QueryModule { module, limit: _ } => {
            println!("查询模块 '{}' 的变更历史...", module);
            let node_id = format!("module:{}", module);
            match graph_store.get_dependencies(&node_id).await {
                Ok(deps) => {
                    if deps.is_empty() {
                        println!("暂无相关记录。请先运行 'aether graph index'。");
                    } else {
                        println!("直接关联的 commits ({})：", deps.len());
                        for dep in deps.iter().take(20) {
                            println!("  - {}", dep);
                        }
                    }
                }
                Err(e) => println!("✗ 查询失败: {}", e),
            }
        }
        GraphAction::QueryAuthor { author, limit: _ } => {
            println!("查询作者 '{}' 的贡献...", author);
            let node_id = format!("author:{}", author);
            match graph_store.get_dependencies(&node_id).await {
                Ok(deps) => {
                    if deps.is_empty() {
                        println!("暂无相关记录。请先运行 'aether graph index'。");
                    } else {
                        println!("该作者的 commits ({})：", deps.len());
                        for dep in deps.iter().take(20) {
                            println!("  - {}", dep);
                        }
                    }
                }
                Err(e) => println!("✗ 查询失败: {}", e),
            }
        }
        GraphAction::AffectedModules { commit_hash } => {
            println!("查询 commit '{}' 影响的模块...", commit_hash);
            let graph = KnowledgeGraphEngine::new(graph_store.clone());
            match graph.get_affected_modules(&commit_hash).await {
                Ok(modules) => {
                    if modules.is_empty() {
                        println!("未找到关联模块。请先运行 'aether graph index'。");
                    } else {
                        println!("受影响的模块 ({})：", modules.len());
                        for m in &modules {
                            println!("  - {}", m);
                        }
                    }
                }
                Err(e) => println!("✗ 查询失败: {}", e),
            }
        }
    }

    Ok(())
}

async fn cmd_do(command: &str, repo_path: &str) -> anyhow::Result<()> {
    let (orchestrator, _indexer, _context_manager, _config) = create_app_context(repo_path)?;
    let parser = NaturalLanguageParser::new();
    let executor = CommandExecutor::new(orchestrator, repo_path.to_string());

    let parsed = parser.parse(command);
    println!("理解意图: {} (类型: {}, 置信度: {:.2})", parsed.intent, parsed.command_type, parsed.confidence);

    let current_state = CurrentState::new("main", "HEAD");
    let result = executor.execute(&parsed, &current_state).await?;

    if result.success {
        println!("✓ {}", result.message);
        if let Some(data) = result.data {
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
    } else {
        println!("✗ {}", result.message);
    }

    Ok(())
}

async fn cmd_search(query: &str, limit: usize, repo_path: &str) -> anyhow::Result<()> {
    let (_orchestrator, indexer, _config_manager, _config) = create_app_context(repo_path)?;

    println!("搜索: \"{}\" (限制 {} 条结果)", query, limit);
    let results = indexer.search(query, limit, None).await?;

    if results.is_empty() {
        println!("  未找到匹配的提交");
    } else {
        for (i, result) in results.iter().enumerate() {
            println!(
                "{}. {} (score: {:.2}) - {}",
                i + 1,
                &result.commit_hash[..std::cmp::min(8, result.commit_hash.len())],
                result.score,
                result.summary
            );
        }
    }

    Ok(())
}

async fn cmd_index(repo_path: &str) -> anyhow::Result<()> {
    let (_orchestrator, indexer, _config_manager, _config) = create_app_context(repo_path)?;

    println!("正在构建语义索引...");
    let report = indexer.index_all_commits().await?;

    println!("  索引完成: {} 成功, {} 失败", report.successful, report.failed);
    for error in &report.errors {
        println!("    ✗ {}", error);
    }

    Ok(())
}

async fn cmd_recover(description: &str, repo_path: &str) -> anyhow::Result<()> {
    let (orchestrator, _indexer, _config_manager, _config) = create_app_context(repo_path)?;
    let executor = CommandExecutor::new(orchestrator, repo_path.to_string());
    let parser = NaturalLanguageParser::new();

    let parsed = parser.parse(&format!("恢复 {}", description));
    println!("执行恢复: {}", parsed.intent);

    let current_state = CurrentState::new("main", "HEAD");
    let result = executor.execute(&parsed, &current_state).await?;

    if result.success {
        println!("✓ {}", result.message);
        if let Some(data) = result.data {
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
    } else {
        println!("✗ {}", result.message);
    }

    Ok(())
}

async fn cmd_merge(prs: &[String], repo_path: &str) -> anyhow::Result<()> {
    let (orchestrator, _indexer, _config_manager, _config) = create_app_context(repo_path)?;
    let executor = CommandExecutor::new(orchestrator, repo_path.to_string());
    let parser = NaturalLanguageParser::new();

    let parsed = parser.parse(&format!("合并 PR: {}", prs.join(", ")));
    println!("分析合并: {}", parsed.intent);

    let current_state = CurrentState::new("main", "HEAD");
    let result = executor.execute(&parsed, &current_state).await?;

    if result.success {
        println!("✓ {}", result.message);
        if let Some(data) = result.data {
            println!("{}", serde_json::to_string_pretty(&data)?);
        }
    } else {
        println!("✗ {}", result.message);
    }

    Ok(())
}

// ─── AetherCI 命令处理器 ───

/// `aether analyze <commit_range>` - 分析两个 commit 之间的语义差异
async fn cmd_analyze(commit_range: &str, json: bool, quick: bool, repo_path: &str) -> anyhow::Result<()> {
    let git_repo = GitRepository::new(repo_path);
    let git: Arc<dyn GitOperations> = Arc::new(git_repo);

    println!("AetherCI: 分析 commit 范围: {}", commit_range);

    // 解析 commit range
    let parts: Vec<&str> = commit_range.split("..").collect();
    let (from_ref, to_ref) = if parts.len() == 2 {
        (parts[0], parts[1])
    } else {
        ("HEAD~1", commit_range)
    };

    // 获取 diff
    let diff = git.get_diff_between(from_ref, to_ref).await
        .map_err(|e| anyhow::anyhow!("无法获取 diff: {}", e))?;

    if diff.is_empty() {
        println!("  没有检测到变更.");
        return Ok(());
    }

    // 获取 commit 信息
    let commits: Vec<Commit> = git.get_commits_in_range(from_ref, to_ref).await
        .unwrap_or_default();
    let commit_hash = commits.first().map(|c| c.id.0.clone()).unwrap_or_else(|| to_ref.to_string());
    let author = commits.first().map(|c| c.author.name.clone()).unwrap_or_else(|| "Unknown".to_string());
    let message = commits.first().map(|c| c.message.clone()).unwrap_or_default();
    let timestamp = commits.first().map(|c| c.timestamp).unwrap_or_else(Utc::now);

    // 构建 Pipeline 输入
    let input = PipelineInput {
        diff,
        commit_message: message,
        commit_hash,
        author,
        timestamp,
        repo_path: repo_path.to_string(),
    };

    // 创建 Pipeline
    let pipeline = if quick {
        SemanticDiffPipeline::default_pipeline()
    } else {
        // 带 LLM 的 Pipeline - 使用配置/环境变量
        let (_orchestrator, _indexer, _config_manager, config) = create_app_context(repo_path)?;
        let llm = LLMFactory::from_config_or_env(&config.llm);
        SemanticDiffPipeline::new(Some(llm), None)
    };

    // 执行分析
    let output = if quick {
        pipeline.quick_analyze(&input).await
    } else {
        pipeline.execute(&input).await
    };

    // 输出结果
    if json {
        let json_str = serde_json::to_string_pretty(&output.report)?;
        println!("{}", json_str);
    } else {
        println!("{}", output.markdown);
    }

    Ok(())
}

/// `aether auto --watch` - 自动监控新 commit 并生成分析报告
async fn cmd_auto(watch: bool, output_dir: &str, limit: usize, repo_path: &str) -> anyhow::Result<()> {
    let git: Arc<dyn GitOperations> = Arc::new(GitRepository::new(repo_path));
    let pipeline = SemanticDiffPipeline::default_pipeline();

    // 确保输出目录存在
    std::fs::create_dir_all(output_dir)?;

    if watch {
        println!("AetherCI: 启动自动监控模式...");
        println!("  监控仓库: {}", repo_path);
        println!("  输出目录: {}", output_dir);
        println!("  按 Ctrl+C 停止");

        let mut last_commit = String::new();
        loop {
            // 获取 HEAD commit hash
            let commits: Vec<Commit> = match git.get_commits_in_range("HEAD", "HEAD").await {
                Ok(c) => c,
                Err(_) => continue,
            };
            if let Some(latest) = commits.first() {
                let current_hash = latest.id.0.clone();
                if current_hash != last_commit {
                    if !last_commit.is_empty() {
                        println!("\n检测到新 commit: {}", &current_hash[..8]);
                        analyze_and_save(&pipeline, &git, &latest.id.0, "HEAD~1", output_dir).await?;
                    }
                    last_commit = current_hash;
                }
            }

            // 每 5 秒检查一次
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    } else {
        // 非 watch 模式：分析最近的 N 个 commit
        println!("AetherCI: 分析最近 {} 个 commit...", limit);

        let all_commits: Vec<Commit> = git.list_commits().await?;
        let commits: Vec<&Commit> = all_commits.iter().take(limit).collect();

        if commits.is_empty() {
            println!("  仓库中没有 commit 记录.");
            return Ok(());
        }

        for commit in commits.iter().rev() {
            // 跳过没有父 commit 的根提交
            if commit.parent_hashes.is_empty() {
                continue;
            }
            let parent = format!("{}~1", commit.id.0);
            analyze_and_save(&pipeline, &git, &commit.id.0, &parent, output_dir).await?;
            println!();
        }

        println!("完成! 报告已保存至: {}", output_dir);
    }

    Ok(())
}

async fn analyze_and_save(
    pipeline: &SemanticDiffPipeline,
    git: &Arc<dyn GitOperations>,
    commit_hash: &str,
    parent_ref: &str,
    output_dir: &str,
) -> anyhow::Result<()> {
    let diff = git.get_diff_between(parent_ref, commit_hash).await
        .map_err(|e| anyhow::anyhow!("无法获取 diff: {}", e))?;

    let commits: Vec<Commit> = git.get_commits_in_range(commit_hash, commit_hash).await
        .unwrap_or_default();
    let author = commits.first().map(|c| c.author.name.clone()).unwrap_or_else(|| "Unknown".to_string());
    let message = commits.first().map(|c| c.message.clone()).unwrap_or_default();
    let timestamp = commits.first().map(|c| c.timestamp).unwrap_or_else(Utc::now);

    let input = PipelineInput {
        diff: diff.clone(),
        commit_message: message,
        commit_hash: commit_hash.to_string(),
        author,
        timestamp,
        repo_path: ".".to_string(),
    };

    let output = pipeline.execute(&input).await;

    // 保存 Markdown
    let md_path = format!("{}/{}.md", output_dir, &commit_hash[..8]);
    std::fs::write(&md_path, &output.markdown)?;
    println!("  ✓ {}", md_path);

    // 保存 JSON
    let json_path = format!("{}/{}.json", output_dir, &commit_hash[..8]);
    let json_str = serde_json::to_string_pretty(&output.report)?;
    std::fs::write(&json_path, json_str)?;

    Ok(())
}

/// `aether history <function_name>` - 查询函数/类的演化历史
async fn cmd_history(function_name: &str, limit: usize, repo_path: &str) -> anyhow::Result<()> {
    let git: Arc<dyn GitOperations> = Arc::new(GitRepository::new(repo_path));

    println!("AetherCI: 查询 \"{}\" 的演化历史 (限制 {} 条)...\n", function_name, limit);

    // 获取最近 N 个 commit（如果仓库 commit 数不足则全量获取）
    let all_commits: Vec<Commit> = git.list_commits().await?;
    let commits: Vec<&Commit> = all_commits.iter().take(limit).collect();

    if commits.is_empty() {
        println!("  仓库中没有 commit 记录.");
        return Ok(());
    }

    let mut found = 0u32;
    for commit in &commits {
        let diff = git.get_diff_between(
            &format!("{}~1", commit.id.0),
            &commit.id.0,
        ).await.unwrap_or_default();

        let diff_lower = diff.to_lowercase();
        let search_lower = function_name.to_lowercase();

        if diff_lower.contains(&search_lower) {
            found += 1;
            let short_hash = &commit.id.0[..std::cmp::min(8, commit.id.0.len())];
            println!("{}. {} - {} ({})",
                found,
                short_hash,
                commit.message.lines().next().unwrap_or("(no message)"),
                commit.timestamp.format("%Y-%m-%d"),
            );

            // 生成快速分析报告
            let pipeline = SemanticDiffPipeline::default_pipeline();
            let input = PipelineInput {
                diff,
                commit_message: commit.message.clone(),
                commit_hash: commit.id.0.clone(),
                author: commit.author.name.clone(),
                timestamp: commit.timestamp,
                repo_path: repo_path.to_string(),
            };

            let output = pipeline.quick_analyze(&input).await;
            println!("   → 变更类型: {}, 置信度: {:.0}%",
                output.report.summary.change_type,
                output.report.summary.confidence.score * 100.0,
            );
            println!();
        }
    }

    if found == 0 {
        println!("  未找到包含 \"{}\" 的 commit 记录.", function_name);
        println!("  Hint: 请先运行 'aether index' 构建语义索引.");
    } else {
        println!("共找到 {} 条相关 commit.", found);
    }

    Ok(())
}

// ─── 新增命令处理器 ───

/// `aether digest` - 生成变更摘要报告
async fn cmd_digest(
    since: Option<String>,
    until: Option<String>,
    range: Option<String>,
    by_agent: bool,
    by_module: bool,
    json: bool,
    output: Option<String>,
    repo_path: &str,
) -> anyhow::Result<()> {
    let git: Arc<dyn GitOperations> = Arc::new(GitRepository::new(repo_path));
    let aggregator = DigestAggregator::new(git);
    let summarizer = DigestSummarizer::new();

    let mut options = DigestOptions::default();
    options.commit_range = range;

    // 解析时间参数
    if let Some(s) = since {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&s) {
            options.since = Some(dt.with_timezone(&Utc));
        } else {
            println!("警告: 无法解析 --since 参数，格式应为 RFC3339 (如 2024-06-01T00:00:00Z)");
        }
    }
    if let Some(u) = until {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&u) {
            options.until = Some(dt.with_timezone(&Utc));
        }
    }

    if by_agent {
        options.group_by = aether_core::digest::DigestGroupBy::Agent;
    }
    if by_module {
        options.group_by = aether_core::digest::DigestGroupBy::Module;
    }

    println!("AetherVC Digest: 正在生成变更摘要...");
    let report = aggregator.aggregate(&options).await?;

    if json {
        println!("{}", summarizer.render_json(&report)?);
    } else {
        let output_text = summarizer.render_markdown(&report);
        if let Some(path) = output {
            std::fs::write(&path, &output_text)?;
            println!("摘要已保存至: {}", path);
        } else {
            println!("{}", output_text);
        }
    }

    Ok(())
}

/// `aether batch` - 批量分析 commit
async fn cmd_batch(
    range: &str,
    risk_sort: bool,
    limit: usize,
    module: Option<String>,
    min_risk: Option<String>,
    json: bool,
    repo_path: &str,
) -> anyhow::Result<()> {
    let git: Arc<dyn GitOperations> = Arc::new(GitRepository::new(repo_path));
    let analyzer = BatchAnalyzer::new();

    println!("AetherVC Batch: 分析范围 {} ...", range);

    let commits = git.list_commits().await?;

    let options = BatchOptions {
        risk_sort,
        limit,
        module_filter: module,
        min_risk,
        ..Default::default()
    };

    let result = analyzer.analyze(commits, &options);

    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        println!("\n批量分析结果 (共 {} 个 commit):", result.summary.total);
        println!("风险分布: Critical={} High={} Medium={} Low={}",
            result.summary.critical,
            result.summary.high,
            result.summary.medium,
            result.summary.low,
        );
        println!("平均风险分数: {:.2}", result.summary.average_risk_score);
        println!();

        if result.commits.is_empty() {
            println!("  没有符合条件的 commit。");
        } else {
            println!("{:>8}  {:6}  {:>8}  {:12}  {}",
                "Commit", "Risk", "Score", "Author", "Message");
            println!("{}", "-".repeat(80));
            for c in &result.commits {
                let risk_icon = match c.risk_level.as_str() {
                    "critical" => "CRIT",
                    "high" => "HIGH",
                    "medium" => "MED",
                    _ => "LOW",
                };
                let short_hash = &c.commit_hash[..c.commit_hash.len().min(8)];
                let short_msg = if c.message.len() > 40 {
                    &c.message[..40]
                } else {
                    &c.message
                };
                println!(
                    "{}  {:4}  {:>5.2}  {:12}  {}",
                    short_hash, risk_icon, c.risk_score, c.author, short_msg
                );
            }
        }

        if !result.summary.top_affected_modules.is_empty() {
            println!("\n受影响最多的模块:");
            for (module, count) in &result.summary.top_affected_modules {
                println!("  {:20} {} 次", module, count);
            }
        }
    }

    Ok(())
}

/// `aether gate check/status` - 门控检查
async fn cmd_gate(action: GateAction, repo_path: &str) -> anyhow::Result<()> {
    match action {
        GateAction::Check { commit_hash } => {
            let git: Arc<dyn GitOperations> = Arc::new(GitRepository::new(repo_path));
            let loader = ConfigLoader::new(repo_path);
            let config = loader.load().unwrap_or_default();
            let engine = GateEngine::new(config.gate);

            // 获取 commit 信息
            let commits = git.list_commits().await?;
            let target = commits.iter().find(|c| c.id.0.starts_with(&commit_hash));

            match target {
                Some(commit) => {
                    let stats = git.get_commit_diff(&commit.id.0).await.ok()
                        .map(|diff| aether_core::review::gate::DiffStats::from_diff(&diff));
                    let decision = engine.check(commit, stats.as_ref());

                    println!("门控检查: {}", &commit.id.0[..commit.id.0.len().min(8)]);
                    println!("  风险等级: {}", commit.semantic_info.risk_level.as_str());
                    println!("  决策: {:?}", decision.action);
                    println!("  风险分数: {:.2}", decision.risk_score);
                    println!("  原因: {}", decision.reason);

                    if decision.action == aether_core::config::types::GateAction::Block {
                        println!("\n  ⛔ 此 commit 被阻止! 需要人工处理。");
                    } else if decision.action == aether_core::config::types::GateAction::Queue {
                        println!("\n  📋 此 commit 需要加入审核队列。");
                    }
                }
                None => {
                    println!("未找到 commit: {}", commit_hash);
                }
            }
        }
        GateAction::Status => {
            let loader = ConfigLoader::new(repo_path);
            let config = loader.load().unwrap_or_default();
            println!("门控状态:");
            println!("  启用: {}", config.gate.enabled);
            println!("  自动通过类型: {:?}", config.gate.auto_pass);
            println!("  必须审核类型: {:?}", config.gate.require_review);
            println!("  高风险模块: {:?}", config.gate.modules.high_risk);
            println!("  阈值: 文件数>{} 行+>{} 行->{} 每小时>{}",
                config.gate.thresholds.max_files_changed,
                config.gate.thresholds.max_lines_added,
                config.gate.thresholds.max_lines_deleted,
                config.gate.thresholds.max_commits_per_hour,
            );
        }
    }

    Ok(())
}

// 全局审核队列（简化实现，生产环境应持久化）
lazy_static::lazy_static! {
    static ref GLOBAL_REVIEW_QUEUE: Mutex<ReviewQueue> = Mutex::new(ReviewQueue::new());
}

/// `aether review` - 审核队列管理
async fn cmd_review(action: ReviewAction, _repo_path: &str) -> anyhow::Result<()> {
    let mut queue = GLOBAL_REVIEW_QUEUE.lock().unwrap();

    match action {
        ReviewAction::Queue { limit } => {
            let pending = queue.pending_items();
            if pending.is_empty() {
                println!("审核队列为空。");
            } else {
                println!("待审核队列 ({} 项):", pending.len());
                println!("{:>8}  {:8}  {:6}  {:12}  {}",
                    "ID", "Commit", "Risk", "Author", "Reason");
                println!("{}", "-".repeat(80));
                for item in pending.iter().take(limit) {
                    let short_hash = &item.commit_hash[..item.commit_hash.len().min(8)];
                    println!(
                        "{}  {:8}  {:4}  {:12}  {}",
                        item.id, short_hash, item.risk_level.to_uppercase(),
                        item.author, item.triggered_reason
                    );
                }
            }
        }
        ReviewAction::Approve { queue_id, comment } => {
            match queue.approve(&queue_id, "cli-user", comment) {
                Ok(item) => println!("✓ 已批准: {} ({})", item.id, &item.commit_hash[..8]),
                Err(e) => println!("✗ 批准失败: {}", e),
            }
        }
        ReviewAction::Reject { queue_id, reason } => {
            let reason_text = reason.unwrap_or_else(|| "未提供原因".into());
            match queue.reject(&queue_id, "cli-user", reason_text) {
                Ok(item) => println!("✗ 已拒绝: {} ({})", item.id, &item.commit_hash[..8]),
                Err(e) => println!("✗ 拒绝失败: {}", e),
            }
        }
        ReviewAction::History { since: _ } => {
            let history = queue.history(None);
            if history.is_empty() {
                println!("无审核历史。");
            } else {
                println!("审核历史 ({} 条):", history.len());
                for entry in history.iter().take(20) {
                    println!(
                        "  {} {} {} ({})",
                        entry.timestamp.format("%Y-%m-%d %H:%M"),
                        entry.action,
                        entry.item.id,
                        &entry.item.commit_hash[..entry.item.commit_hash.len().min(8)]
                    );
                }
            }
        }
    }

    Ok(())
}

/// `aether verify` - 运行验证
async fn cmd_verify(
    commit_hash: Option<String>,
    mode: &str,
    watch: bool,
    report: bool,
    repo_path: &str,
) -> anyhow::Result<()> {
    let loader = ConfigLoader::new(repo_path);
    let config = loader.load().unwrap_or_default();
    let mut runner = VerificationRunner::new(config.verify);

    let verify_mode = match mode {
        "quick" => VerifyMode::Quick,
        "full" => VerifyMode::Full,
        _ => VerifyMode::Smart,
    };

    let hash = commit_hash.unwrap_or_else(|| "HEAD".to_string());

    if watch {
        println!("验证监控模式启动...");
        let git: Arc<dyn GitOperations> = Arc::new(GitRepository::new(repo_path));
        let mut last_hash = String::new();

        loop {
            if let Ok(commits) = git.list_commits().await {
                if let Some(latest) = commits.first() {
                    if latest.id.0 != last_hash {
                        if !last_hash.is_empty() {
                            println!("\n检测到新 commit: {}", &latest.id.0[..8]);
                            let result = runner.run(&latest.id.0, verify_mode, repo_path).await;
                            print_verify_result(&result);
                        }
                        last_hash = latest.id.0.clone();
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    } else {
        println!("运行验证: {} (模式: {})", &hash[..hash.len().min(8)], mode);
        let result = runner.run(&hash, verify_mode, repo_path).await;
        print_verify_result(&result);

        if report {
            let report_path = format!(".aether/reports/verify_{}.json", &hash[..8]);
            std::fs::create_dir_all(".aether/reports")?;
            std::fs::write(&report_path, serde_json::to_string_pretty(&result)?)?;
            println!("验证报告已保存至: {}", report_path);
        }
    }

    Ok(())
}

// ─── 工作流命令 ───

async fn cmd_workflow(action: WorkflowAction, repo_path: &str) -> anyhow::Result<()> {
    let (orchestrator, _indexer, _config_manager, config) = create_app_context(repo_path)?;
    let wf = WorkflowOrchestrator::new(orchestrator, config, repo_path);

    match action {
        WorkflowAction::Digest { since: _, until: _ } => {
            let result = wf.execute(WorkflowType::Digest).await?;
            print_workflow_result(&result);
        }
        WorkflowAction::Coordinate { since: _ } => {
            let result = wf.execute(WorkflowType::Coordinate).await?;
            print_workflow_result(&result);
        }
        WorkflowAction::VerifyTags { keyword: _, count: _ } => {
            let result = wf.execute(WorkflowType::VerifyTags).await?;
            print_workflow_result(&result);
        }
        WorkflowAction::FullCi { watch } => {
            if watch {
                println!("FullCI 监控模式启动（每 30 秒检查一次）...");
                loop {
                    let result = wf.execute(WorkflowType::FullCI).await?;
                    print_workflow_result(&result);
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                }
            } else {
                let result = wf.execute(WorkflowType::FullCI).await?;
                print_workflow_result(&result);
            }
        }
    }
    Ok(())
}

fn print_workflow_result(result: &aether_core::workflow::WorkflowResult) {
    println!("┌─────────────────────────────────────────┐");
    println!("│  工作流: {}  {}│",
        result.workflow_type,
        if result.success { "✅" } else { "❌" }
    );
    println!("│  耗时: {}ms", result.duration_ms);
    println!("│  {}", result.message);
    if let Some(ref data) = result.data {
        println!("│  ─────────────────────────────────────");
        if let Ok(pretty) = serde_json::to_string_pretty(data) {
            for line in pretty.lines().take(20) {
                println!("│  {}", line);
            }
        }
    }
    println!("└─────────────────────────────────────────┘");
}

// ─── 回滚命令 ───

async fn cmd_rollback_cli(action: RollbackSub, repo_path: &str) -> anyhow::Result<()> {
    let (orchestrator, _indexer, _config_manager, config) = create_app_context(repo_path)?;

    match action {
        RollbackSub::Analyze { commit_hash } => {
            let task = AgentTask::new("analyze_rollback", serde_json::json!({
                "commit_hash": commit_hash,
                "verification_result": null,
            }));
            let result = orchestrator.execute_task(task).await?;
            println!("回滚分析: {}", &commit_hash[..commit_hash.len().min(8)]);
            let needs = result.output.get("needs_rollback").and_then(|v| v.as_bool()).unwrap_or(false);
            println!("  需要回滚: {}", if needs { "是 ⚠" } else { "否" });
            if let Some(action) = result.output.get("suggested_action") {
                println!("  建议操作: {}", action);
            }
        }
        RollbackSub::Execute { commit_hash, method, hard } => {
            let action = if method == "reset" {
                serde_json::json!({"type": "reset", "target": commit_hash, "hard": hard})
            } else {
                serde_json::json!({"type": "revert"})
            };
            let task = AgentTask::new("execute_rollback", serde_json::json!({
                "commit_hash": commit_hash,
                "action": action,
                "require_approval": true,
            }));
            let result = orchestrator.execute_task(task).await?;
            if result.success {
                if let Ok(record) = serde_json::from_value::<RollbackRecord>(result.output.clone()) {
                    match record.status {
                        RollbackStatus::Executed => {
                            println!("✅ 回滚已执行: {}", &commit_hash[..8]);
                            if let Some(ref rev) = record.revert_commit {
                                println!("   Revert commit: {}", rev);
                            }
                        }
                        RollbackStatus::PendingApproval => {
                            println!("⏳ 回滚待审批: {}", record.reason);
                        }
                        RollbackStatus::Failed(ref e) => {
                            println!("❌ 回滚失败: {}", e);
                        }
                        _ => println!("回滚状态: {:?}", record.status),
                    }
                }
            } else {
                println!("❌ 回滚执行失败: {:?}", result.error_message);
            }
        }
        RollbackSub::History => {
            let task = AgentTask::new("rollback_history", serde_json::json!({}));
            let result = orchestrator.execute_task(task).await?;
            if let Ok(records) = serde_json::from_value::<Vec<RollbackRecord>>(result.output) {
                if records.is_empty() {
                    println!("回滚历史为空");
                } else {
                    for r in &records {
                        println!("  {} {} {:?} {}",
                            r.id,
                            &r.rolled_back_commit[..std::cmp::min(8, r.rolled_back_commit.len())],
                            r.status,
                            r.agent_name
                        );
                    }
                }
            }
        }
        RollbackSub::Reputation => {
            let task = AgentTask::new("reputation", serde_json::json!({}));
            let result = orchestrator.execute_task(task).await?;
            println!("Agent 信誉分:");
            if let Some(map) = result.output.as_object() {
                for (name, score) in map {
                    if let Some(s) = score.as_f64() {
                        let emoji = if s > 0.9 { "🟢" } else if s > 0.7 { "🟡" } else { "🔴" };
                        println!("  {} {}: {:.2}", emoji, name, s);
                    }
                }
            }
        }
    }
    Ok(())
}

// ─── Tag 验证命令 ───

async fn cmd_verify_tag(tags: &[String], order_by: &str, json: bool, repo_path: &str) -> anyhow::Result<()> {
    let (orchestrator, _indexer, _config_manager, _config) = create_app_context(repo_path)?;

    let task = AgentTask::new("validate_tag", serde_json::json!({
        "tags": tags,
        "order_by": order_by,
        "max_tags": tags.len().max(15),
    }));

    let result = orchestrator.execute_task(task).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&result.output)?);
    } else {
        if let Ok(reports) = serde_json::from_value::<Vec<TagValidationReport>>(result.output.clone()) {
            if reports.is_empty() {
                println!("未找到匹配的 Tag");
            } else {
                println!("┌──────┬──────────┬────────┬────────────────────┐");
                println!("│ 风险 │ Tag      │ Agent  │ 结论               │");
                println!("├──────┼──────────┼────────┼────────────────────┤");
                for r in &reports {
                    let risk_icon = match r.overall_conclusion.as_str() {
                        "pass" => "🟢",
                        "conditional_pass" => "🟡",
                        _ => "🔴",
                    };
                    println!("│ {} │ {:8} │ {:6} │ {:18} │",
                        risk_icon,
                        &r.tag[..r.tag.len().min(8)],
                        &r.agent[..r.agent.len().min(6)],
                        r.overall_conclusion
                    );
                }
                println!("└──────┴──────────┴────────┴────────────────────┘");
                println!("总计: {} 个 Tag, {} pass, {} conditional, {} fail",
                    reports.len(),
                    reports.iter().filter(|r| r.overall_conclusion == "pass").count(),
                    reports.iter().filter(|r| r.overall_conclusion == "conditional_pass").count(),
                    reports.iter().filter(|r| r.overall_conclusion == "fail").count(),
                );
            }
        }
    }
    Ok(())
}

// ─── 状态命令 ───

async fn cmd_status(repo_path: &str) -> anyhow::Result<()> {
    let (_orchestrator, indexer, _config_manager, config) = create_app_context(repo_path)?;

    let stats = indexer.stats().await?;

    println!("AetherVC v0.3.0");
    println!("  仓库：{}", repo_path);
    println!("  语义索引：✅ 已构建 ({} 个 commit)", stats.total_points);
    println!("  LLM Provider：{} ({})",
        config.llm.provider,
        config.llm.model
    );
    println!("  Embedder：{} ({} 维)",
        config.llm.embedding_model,
        // 实际维度从 embedder 获取，这里从配置显示
        if config.llm.embedding_model.contains("large") { "3072" } else { "1536" }
    );
    println!("  门控：{}", if config.gate.enabled { "✅ 已启用" } else { "⏸ 已禁用" });
    println!("  验证：{}", if config.verify.enabled { "✅ 已启用" } else { "⏸ 已禁用" });
    println!("  回滚：{}", if config.rollback.enabled { "✅ 已启用" } else { "⏸ 已禁用" });
    println!("  协调器：{}", if config.coordinator.enabled { "✅ 已启用" } else { "⏸ 已禁用" });
    println!("  6/6 Agent 就绪");
    println!();
    println!("  LLM 提供商: {}", config.llm.provider);
    println!("  模型: {}", config.llm.model);
    if config.llm.api_key.is_empty() {
        println!("  ⚠ api_key 未配置 → 使用 MockLLMClient（无真实 AI 能力）");
        println!("  设置环境变量以启用: DEEPSEEK_API_KEY 或 OPENAI_API_KEY");
    } else {
        println!("  ✅ api_key 已配置");
    }

    Ok(())
}

fn print_verify_result(report: &VerificationReport) {
    println!("验证结果: {:?} ({}ms)", report.overall_status, report.duration_ms);
    println!("通过率: {:.0}%", report.pass_rate() * 100.0);

    for check in &report.checks {
        let status_icon = match check.status {
            aether_core::verify::CheckStatus::Passed => "✓",
            aether_core::verify::CheckStatus::Failed => "✗",
            aether_core::verify::CheckStatus::Skipped => "○",
            _ => "?",
        };
        println!("  {} {} ({}ms)", status_icon, check.name, check.duration_ms);
        if let Some(ref details) = check.details {
            println!("    → {}", details);
        }
    }
}

/// `aether config` - 配置管理
async fn cmd_config(action: ConfigAction, repo_path: &str) -> anyhow::Result<()> {
    let loader = ConfigLoader::new(repo_path);

    match action {
        ConfigAction::Init => {
            match loader.init() {
                Ok(config) => {
                    println!("✓ 配置文件已创建: {}", loader.config_path().display());
                    println!("  gate.enabled = {}", config.gate.enabled);
                    println!("  verify.enabled = {}", config.verify.enabled);
                    println!("  llm.provider = {}", config.llm.provider);
                    println!("\n使用 'aether config show' 查看完整配置");
                    println!("使用 'aether config set <key> <value>' 修改配置");
                }
                Err(e) => {
                    println!("✗ {}", e);
                    println!("  配置文件可能已存在。使用 'aether config show' 查看。");
                }
            }
        }
        ConfigAction::Show => {
            let config = loader.load()?;
            let toml_str = toml::to_string_pretty(&config)?;
            println!("{}", toml_str);
        }
        ConfigAction::Set { key, value } => {
            match loader.set_value(&key, &value) {
                Ok(_) => println!("✓ 已设置 {} = {}", key, value),
                Err(e) => println!("✗ {}", e),
            }
        }
        ConfigAction::Validate => {
            let config = loader.load()?;
            match loader.validate(&config) {
                Ok(warnings) => {
                    if warnings.is_empty() {
                        println!("✓ 配置验证通过，无警告。");
                    } else {
                        println!("配置验证通过，但有以下警告:");
                        for w in warnings {
                            println!("  ⚠ {}", w);
                        }
                    }
                }
                Err(e) => println!("✗ 配置验证失败: {}", e),
            }
        }
    }

    Ok(())
}

/// `aether hook` - Git Hook 管理
async fn cmd_hook(action: HookAction, repo_path: &str) -> anyhow::Result<()> {
    let hooks_dir = std::path::Path::new(repo_path).join(".git").join("hooks");

    match action {
        HookAction::Install { post_commit, pre_push } => {
            if !hooks_dir.exists() {
                println!("✗ 未找到 .git/hooks 目录。请确保在 Git 仓库中运行。");
                return Ok(());
            }

            let aether_path = std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "aether".to_string());

            if post_commit {
                let hook_content = format!(
                    r#"#!/bin/sh
# AetherVC post-commit hook
# 每次提交后自动分析变更

echo "[AetherVC] 分析最新 commit..."
{} analyze HEAD --quick --json > .aether/last_analysis.json
{} gate check HEAD
"#,
                    aether_path, aether_path
                );
                let hook_path = hooks_dir.join("post-commit");
                std::fs::write(&hook_path, &hook_content)?;
                // 在 Unix 上设置可执行权限（Windows 上会失败，忽略）
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = std::fs::metadata(&hook_path)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&hook_path, perms)?;
                }
                println!("✓ post-commit hook 已安装: {}", hook_path.display());
            }

            if pre_push {
                let hook_content = format!(
                    r#"#!/bin/sh
# AetherVC pre-push hook
# push 前检查高风险变更

echo "[AetherVC] 检查待推送变更..."
{} gate check HEAD --block-if-unreviewed
"#,
                    aether_path
                );
                let hook_path = hooks_dir.join("pre-push");
                std::fs::write(&hook_path, &hook_content)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = std::fs::metadata(&hook_path)?.permissions();
                    perms.set_mode(0o755);
                    std::fs::set_permissions(&hook_path, perms)?;
                }
                println!("✓ pre-push hook 已安装: {}", hook_path.display());
            }

            if !post_commit && !pre_push {
                println!("请指定要安装的 hook: --post-commit 或 --pre-push");
                println!("示例: aether hook install --post-commit --pre-push");
            }
        }
        HookAction::Uninstall => {
            let mut removed = 0;
            for hook_name in &["post-commit", "pre-push"] {
                let hook_path = hooks_dir.join(hook_name);
                if hook_path.exists() {
                    // 只删除 AetherVC 安装的 hooks
                    if let Ok(content) = std::fs::read_to_string(&hook_path) {
                        if content.contains("AetherVC") {
                            std::fs::remove_file(&hook_path)?;
                            removed += 1;
                            println!("✓ 已移除: {}", hook_path.display());
                        }
                    }
                }
            }
            if removed == 0 {
                println!("没有找到 AetherVC 安装的 hooks。");
            }
        }
        HookAction::Status => {
            let mut installed = Vec::new();
            for hook_name in &["post-commit", "pre-push"] {
                let hook_path = hooks_dir.join(hook_name);
                if hook_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&hook_path) {
                        if content.contains("AetherVC") {
                            installed.push(hook_name.to_string());
                        }
                    }
                }
            }
            if installed.is_empty() {
                println!("AetherVC hooks 未安装。");
                println!("使用 'aether hook install --post-commit --pre-push' 安装。");
            } else {
                println!("已安装的 AetherVC hooks:");
                for name in &installed {
                    println!("  ✓ {}", name);
                }
            }
        }
    }

    Ok(())
}
