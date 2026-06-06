# AetherVC — AI-Native Semantic Version Control

**专为 AI 大规模代码生成时代设计的下一代版本控制系统**

在传统 Git 之上构建语义智能层，让人类通过自然语言与版本控制交互，实现跨 commit 恢复、自动合并、语义搜索等能力。

[![CI](https://github.com/aethervc/aether-vc/actions/workflows/ci.yml/badge.svg)](https://github.com/aethervc/aether-vc/actions/workflows/ci.yml)

---

## 1. 快速开始

### 环境要求

- **Rust 1.75+**（推荐通过 [rustup](https://rustup.rs) 安装）
- **Git** 仓库
- **Ubuntu 24.04** 或兼容 Linux
- 可选：OpenAI / Claude API Key（用于 LLM 增强）

### 安装

```bash
# 克隆仓库
git clone https://github.com/aethervc/aether-vc.git
cd aether-vc

# 编译
cargo build --release

# 运行测试
cargo test --all-features

# 仅 aether-core 测试
cargo test -p aether-core

# 仅 aetherci 测试
cargo test -p aetherci -- --nocapture

# 安装到系统
cargo install --path aether-cli
```

### 初始化

```bash
# 在任意 Git 仓库中初始化
cd your-project
aether init

# 索引所有历史提交（构建语义向量库）
aether index
```

---

## 2. 命令参考

| 命令 | 功能 | 示例 |
|------|------|------|
| `aether init` | 初始化 AetherVC | `aether init` |
| `aether index` | 构建语义索引 | `aether index` |
| `aether do <指令>` | 自然语言命令 | `aether do "恢复上周删除的用户画像模块"` |
| `aether search <查询>` | 语义搜索提交 | `aether search "JWT 认证相关改动"` |
| `aether recover <描述>` | 恢复历史功能 | `aether recover "用户画像分析模块"` |
| `aether merge <PRs>` | 分析 PR 合并 | `aether merge --prs #12 #15 #23` |
| `aether analyze <范围>` | 语义 Diff 分析 + 生成变更报告 | `aether analyze HEAD~1..HEAD` |
| `aether auto --watch` | 自动监控新 commit 并生成报告 | `aether auto --watch` |
| `aether history <名称>` | 查询函数/类演化历史 | `aether history login` |

### 使用示例

```bash
# 场景1：语义恢复
aether recover "把上周 agent 实现的用户画像分析模块恢复回来，它在昨天的重构中被删掉了"

# 场景2：语义搜索
aether search "认证模块的 JWT token 生成逻辑"

# 场景3：批量 PR 分析
aether do "分析 #12 #15 #23 这三个 PR，给出合并顺序和风险建议"

# 场景4：自然语言查询
aether do "最近哪些提交涉及数据库迁移"

# 场景5：语义分析 commit（AetherCI）
aether analyze HEAD~1..HEAD

# 场景6：快速分析（跳过 LLM）
aether analyze --quick

# 场景7：自动监控模式
aether auto --watch

# 场景8：查询功能演化历史
aether history "UserService" -l 20
```

---

## 3. AI Agent 自动调用 AetherVC

### 3.1 核心思路

AetherVC 的命令设计天然适合 AI Agent 调用。每个命令都是**无交互的原子操作**，输入和输出都是**结构化 JSON**，不需要人工确认。

### 3.2 AI Agent 调用模式

#### CLI 模式（最简单）

AI Agent 直接执行 shell 命令并解析输出：

```bash
# AI Agent 在终端中执行
aether search "authentication bug" --limit 5

# 输出示例（可用于 AI 解析）
# 1. a1b2c3d4 (score: 0.89) - fix login authentication bug
# 2. e5f6g7h8 (score: 0.76) - refactor auth middleware
# ...
```

#### JSON 输出模式（推荐）

通过环境变量开启 JSON 输出（Phase 2 功能），AI 直接解析结构化结果：

```bash
AETHER_OUTPUT=json aether search "authentication bug" --limit 5
```

```json
{
  "results": [
    {
      "commit_hash": "a1b2c3d4",
      "score": 0.89,
      "intent": "fix login authentication bug",
      "change_category": "bugfix",
      "risk_level": "low"
    }
  ],
  "total": 5
}
```

### 3.3 多 Agent 协作工作流

```
┌─────────────────────────────────────────────────────┐
│                 Human Supervisor                     │
│  "实现用户画像功能, 修复登录 bug, 重构数据库层"       │
└──────────────────────┬──────────────────────────────┘
                       │ 分解任务
          ┌────────────┼────────────┐
          ▼            ▼            ▼
     ┌─────────┐ ┌─────────┐ ┌─────────┐
     │ Agent A  │ │ Agent B  │ │ Agent C  │
     │ 用户画像  │ │ 修复登录 │ │ 重构DB   │
     └────┬─────┘ └────┬─────┘ └────┬─────┘
          │ 提交代码      │ 提交代码    │ 提交代码
          ▼            ▼            ▼
     ┌─────────────────────────────────────────────┐
     │              AetherVC                       │
     │  ┌─────────────────────────────────────┐   │
     │  │ 1. SemanticInterpreter: 理解每个变更 │   │
     │  │ 2. Merge Agent: 分析冲突、自动合并   │   │
     │  │ 3. Cross-Commit Recovery: 失败回滚   │   │
     │  └─────────────────────────────────────┘   │
     └─────────────────────────────────────────────┘
```

### 3.4 Prompt 模板（给 AI Agent 使用）

以下 Prompt 可以直接给 Claude、GPT-4o、Grok 等模型，让它自主调用 AetherVC：

#### 通用 Agent Prompt

```
你是一个集成 AetherVC 的代码 Agent。你可以通过命令行调用 AetherVC 来管理版本控制。

可用命令：
- aether search "<query>"         语义搜索历史提交
- aether recover "<description>"  恢复历史功能
- aether do "<natural language>"  执行自然语言操作
- aether index                    重建语义索引

规则：
1. 每次提交代码后，运行 `aether index` 更新语义索引
2. 在修改已有功能前，先用 `aether search` 查找相关历史变更
3. 遇到冲突时，用 `aether merge` 分析合并方案
4. 所有 AetherVC 输出都是你可以直接解析的结构化文本

现在开始工作。当前仓库在 {REPO_PATH}。
```

#### 场景专用 Prompt

**场景1：AI 恢复被删除的代码**

```
你需要恢复项目中关于 {FEATURE_NAME} 的功能代码。
它可能在之前的某个提交中被删除或重构了。

步骤：
1. 运行: aether search "{FEATURE_NAME}"
2. 分析搜索结果中 score 最高的 3 个提交
3. 运行: aether recover "{FEATURE_NAME}"
4. 检查生成的 patch 中的冲突
5. 应用 patch 并创建新提交

如果 aether 返回了冲突警告，仔细检查并手动解决。
```

**场景2：AI 批量合并 PR**

```
你需要分析以下 PR 的合并方案：{PR_LIST}

步骤：
1. 运行: aether merge --prs {PR_IDS}
2. 查看 merge_order（合并优先级）
3. auto_mergeable 中的 PR 直接合并
4. needs_review 中的 PR 生成审核要点
5. 按 risk_assessment 的建议执行
```

**场景3：AI 多 Agent 协调**

```
你是 Agent Coordinator。当前有 {N} 个 Agent 同时修改代码库。

在每次 Agent 提交后执行：
1. aether index                    # 更新语义索引
2. aether search "最近5个提交"      # 检查是否有冲突模块
3. 如有冲突，运行: aether merge --prs {冲突PR列表}
4. 按 Merge Agent 的建议协调各 Agent

始终按低风险 → 高风险的顺序处理提交。
```

### 3.5 程序化调用（Rust API）

如果 AI Agent 是用 Rust 编写的，可以直接使用 `aether-core` 库：

```rust
use aether_core::semantic::indexer::SemanticIndexer;
use aether_core::semantic::embedder::MockEmbedder;
use aether_core::storage::vector_db::InMemoryVectorStore;
use aether_core::storage::git::GitRepository;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化
    let git = Arc::new(GitRepository::open(".")?);
    let store = Arc::new(InMemoryVectorStore::new());
    let embedder = Arc::new(MockEmbedder::default());
    let indexer = SemanticIndexer::new(embedder, store, git);

    // 索引所有提交
    let report = indexer.index_all_commits().await?;
    println!("索引: {} 成功, {} 失败", report.successful, report.failed);

    // 语义搜索
    let results = indexer.search("authentication", 5, None).await?;
    for r in results {
        println!("{} (score: {:.2}): {}", r.commit_hash, r.score, r.summary);
    }

    Ok(())
}
```

### 3.6 Cursor / Copilot / Cline 集成

在项目的 `.cursorrules` 或 `.github/copilot-instructions.md` 中添加：

```markdown
## Version Control

This project uses AetherVC for semantic version control. Always use these commands:

- Before modifying existing code, search history:
  `aether search "related feature description"`

- After commit, update index:
  `aether index`

- When asked to recover deleted code:
  `aether recover "description of the feature"`

- To analyze merge conflicts:
  `aether do "analyze merge conflicts between branch X and Y"`

Do NOT use bare `git` commands for history search — always use `aether search`.
```

---

## 4. AetherCommit Intelligence (AetherCI)

**代码变更语义理解与意图文档生成系统**，AetherVC 的核心子模块。

AetherCI 不依赖 commit message，而是通过 AST 解析 + 语义向量 + LLM 推理，真正"理解"每次 commit 的代码变更，自动生成结构化分析报告。

### 4.1 分析能力

| 维度 | 说明 |
|------|------|
| **改了什么** | 函数/类/接口级实体检测 + 重构识别 |
| **为什么改** | LLM 多轮意图推理（带置信度） |
| **影响什么** | 依赖图影响分析 + 回归风险预测 |
| **历史上下文** | 跨 commit 关联 + 功能演化路径 |

### 4.2 五阶段流水线

```
预处理         分类器          意图推理        影响分析        报告生成
 diff →    变更类型分类  →   LLM/规则推理  →  依赖+风险  →  Markdown/JSON
实体检测    (12种类型)      (降级策略)      (风险等级)      (PRD模板)
```

### 4.3 输出报告模板

生成的 Markdown 报告包含 6 个章节：

1. **变更摘要** — 一句话总结 + 变更类型 + 置信度
2. **详细变更内容** — 实体列表、统计概览、变更分组
3. **变更意图与动机** — LLM 推理结果（解决的问题、推断动机、架构上下文）
4. **影响范围与风险** — 受影响模块、风险矩阵、验证建议
5. **历史上下文** — 相关 commit、演化路径
6. **建议** — Review 重点、测试推荐

### 4.4 变更分类（12 种）

`功能新增` | `功能修改` | `功能删除` | `重构` | `性能优化` | `Bug 修复` | `安全加固` | `依赖更新` | `配置变更` | `文档变更` | `测试变更` | `未知`

### 4.5 程序化调用（Rust API）

```rust
use aetherci::SemanticDiffPipeline;
use aetherci::domain::PipelineInput;

let pipeline = SemanticDiffPipeline::default_pipeline();
let output = pipeline.execute(&input).await;

println!("{}", output.markdown);  // 人类可读
// output.report                  // 结构化 JSON
```

---

## 5. 架构概览

```
应用层    CLI / REST API / Web UI
   ↓
NL 层     NaturalLanguageParser → CommandExecutor
   ↓
Agent 层  SemanticInterpreter | Recovery | Merge | CommitIntelligence | Orchestrator
   ↓
语义层    SemanticAnalyzer | Embedder | Indexer | SearchEngine | AetherCI Pipeline
   ↓
存储层    Git(git2) | VectorStore | GraphStore
```

详见 [架构文档](docs/ARCHITECTURE.md)

## 6. 测试

```bash
# 全部测试
cargo test --all-features

# 仅 aether-core 测试
cargo test -p aether-core --lib

# 仅 aetherci 测试
cargo test -p aetherci

# 带输出
cargo test --all-features -- --nocapture
```

## 7. CI/CD

GitHub Actions 在 Ubuntu 24.04 上自动执行：

- `cargo fmt --check` — 代码格式检查
- `cargo clippy` — 静态分析
- `cargo test --all-features` — 全部测试
- `cargo build --release` — 发布构建

## 8. MVP 功能清单

| 模块 | 状态 |
|------|------|
| 语义索引（commit 自动向量化） | ✅ |
| 语义搜索（余弦相似度） | ✅ |
| Cross-Commit Recovery Agent | ✅ |
| Merge Agent（冲突检测+风险分级） | ✅ |
| Semantic Interpreter Agent | ✅ |
| CommitIntelligence Agent（代码变更语义理解） | ✅ |
| AetherCI 五阶段流水线（预处理→分类→推理→影响→报告） | ✅ |
| Agent Orchestrator | ✅ |
| NL 指令解析器（中英文） | ✅ |
| CLI 命令（9 个子命令） | ✅ |
| Prompt 模板库（6 个核心模板） | ✅ |
| REST API | 🚧 骨架 |
| Multi-Agent Coordinator | 📋 Phase 2 |
| Web UI | 📋 Phase 2 |

## 9. License

MIT
