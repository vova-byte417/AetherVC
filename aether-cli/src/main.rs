//! AetherVC CLI - AI-Native Semantic Version Control System
//!
//! 命令行入口，提供自然语言驱动的版本控制操作

use aether_core::agents::orchestrator::AgentOrchestrator;
use aether_core::llm::client::MockLLMClient;
use aether_core::nlp::context::ContextManager;
use aether_core::nlp::executor::CommandExecutor;
use aether_core::nlp::parser::NaturalLanguageParser;
use aether_core::semantic::embedder::MockEmbedder;
use aether_core::semantic::indexer::SemanticIndexer;
use aether_core::storage::git::{GitOperations, GitRepository};
use aether_core::storage::graph_db::InMemoryGraphStore;
use aether_core::storage::vector_db::InMemoryVectorStore;
use aether_core::domain::commit::{Commit, CurrentState};

use aetherci::pipeline::orchestrator::SemanticDiffPipeline;
use aetherci::domain::PipelineInput;

use chrono::Utc;
use clap::{Parser, Subcommand};
use std::sync::Arc;
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
    }
}

fn create_app_context(repo_path: &str) -> anyhow::Result<(
    Arc<AgentOrchestrator>,
    Arc<SemanticIndexer>,
    Arc<ContextManager>,
)> {
    let git_repo: Arc<dyn GitOperations> = Arc::new(GitRepository::new(repo_path));
    let vector_store = Arc::new(InMemoryVectorStore::new());
    let graph_store = Arc::new(InMemoryGraphStore::new());
    let embedder = Arc::new(MockEmbedder::default());
    let llm_client = Arc::new(MockLLMClient::new());

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

    Ok((orchestrator, indexer, context_manager))
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

async fn cmd_do(command: &str, repo_path: &str) -> anyhow::Result<()> {
    let (orchestrator, _indexer, _context_manager) = create_app_context(repo_path)?;
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
    let (_orchestrator, indexer, _ctx) = create_app_context(repo_path)?;

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
    let (_orchestrator, indexer, _ctx) = create_app_context(repo_path)?;

    println!("正在构建语义索引...");
    let report = indexer.index_all_commits().await?;

    println!("  索引完成: {} 成功, {} 失败", report.successful, report.failed);
    for error in &report.errors {
        println!("    ✗ {}", error);
    }

    Ok(())
}

async fn cmd_recover(description: &str, repo_path: &str) -> anyhow::Result<()> {
    let (orchestrator, _indexer, _ctx) = create_app_context(repo_path)?;
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
    let (orchestrator, _indexer, _ctx) = create_app_context(repo_path)?;
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
        // 带 LLM 的 Pipeline
        let llm = Arc::new(MockLLMClient::new());
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
