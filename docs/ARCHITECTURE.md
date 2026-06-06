# AetherVC - AI-Native Semantic Version Control System

## 技术架构文档 v0.1 (MVP)

### 架构概览

```
┌─────────────────────────────────────────────────────────┐
│                    应用层                                │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │   CLI    │  │ REST API │  │  Web UI  │              │
│  └──────────┘  └──────────┘  └──────────┘              │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│              自然语言接口层                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ NL 指令解析器 │  │ 对话管理器   │  │ 指令执行器   │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│                  Agent 系统层                            │
│  ┌──────────────────────────────────────────────────┐  │
│  │ SemanticInterpreterAgent / CrossCommitRecoveryAgent │  │
│  │ MergeAgent / AgentOrchestrator                    │  │
│  └──────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│              语义与知识层                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ 语义分析引擎 │  │ 向量索引管理 │  │ Embedder     │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
                           ↓
┌─────────────────────────────────────────────────────────┐
│                   存储层                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Git (git2)   │  │ VectorStore  │  │ GraphStore   │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### 模块结构

| Crate | 描述 |
|-------|------|
| `aether-core` | 核心库：领域模型、存储抽象、语义分析、Agent 系统、LLM 集成、NLP 层 |
| `aether-cli` | 命令行工具：init/do/search/index/recover/merge 子命令 |
| `aether-api` | REST API 服务（Phase 5） |
| `aether-web` | Web 前端（Phase 5） |

### 技术选型

| 组件 | 方案 | 说明 |
|------|------|------|
| 语言 | Rust | 高性能、安全、编译时检查 |
| 异步运行时 | tokio | 高性能异步 I/O |
| Git 操作 | git2 | libgit2 绑定 |
| 向量存储 | InMemoryVectorStore (Qdrant for prod) | MVP 使用内存实现 |
| 图存储 | InMemoryGraphStore (Neo4j for prod) | MVP 使用邻接表 |
| 嵌入器 | MockEmbedder (OpenAI API for prod) | 测试用确定性向量 |
| LLM | MockLLMClient (OpenAI/Claude for prod) | 统一 trait 接口 |
| CLI | clap | 声明式命令行解析 |
| Web | axum | 高性能 Web 框架 |
| 序列化 | serde + serde_json | JSON 序列化 |
| 测试 | cargo test + tempfile | Rust 内置测试 |

### 核心领域模型

```
Commit ──────────── 聚合根：提交信息 + 语义信息
  ├── SemanticInfo: intent, change_type, risk_level, embedding
  ├── FileChange: path, change_type, diff
  └── Author: name, email, is_ai_agent

RecoveryRequest ─── 恢复请求
  ├── natural_language_query
  ├── current_state
  └── RecoveryResult: patch, conflicts

MergeRequest ─────── 合并分析
  ├── PullRequest[]
  └── RiskAssessment: overall_risk, recommendation

Agent ────────────── Agent 实体
  ├── AgentType: 6 种核心 Agent
  └── AgentTask: input/output
```

### 6 大核心 Agent

| Agent | 职责 | 状态 |
|-------|------|------|
| SemanticInterpreterAgent | 语义分析，diff → 结构化语义 | 已实现 |
| CrossCommitRecoveryAgent | 跨版本智能恢复 | 已实现 |
| MergeAgent | PR 排序、冲突检测、风险评估 | 已实现 |
| MultiAgentCoordinator | 多 Agent 协调（Phase 2） | 待实现 |
| ValidationRiskAgent | Tag/Commit 验证（Phase 2） | 待实现 |
| RollbackAgent | 智能回滚（Phase 2） | 待实现 |

### Prompt 模板库

5 个高质量 Prompt 模板已内置：
1. `semantic_interpreter` - 代码语义分析
2. `cross_commit_recovery` - 跨版本恢复
3. `merge_agent` - 智能 PR 合并
4. `multi_agent_coordinator` - 多 Agent 协调
5. `validation_risk` - 验证与风险评估

### API 设计

#### CLI 命令

```
aether init          # 初始化 AetherVC 仓库
aether do <command>  # 自然语言命令
aether search <q>    # 语义搜索
aether index         # 构建语义索引
aether recover <desc> # 恢复功能
aether merge <prs>   # 分析 PR 合并
```

### 测试策略

- 单元测试：每个模块包含 `#[cfg(test)] mod tests`
- 覆盖率目标：>80%
- 测试 Mock：MockEmbedder、MockLLMClient、InMemory 存储
- CI/CD：GitHub Actions 三平台编译 + 测试

### MVP 已交付功能

- [x] 项目结构 + Cargo 工作空间
- [x] 领域模型（Commit, SemanticInfo, RecoveryRequest, MergeRequest, Agent）
- [x] Git 操作封装（git2）
- [x] InMemory 向量存储 + 语义搜索
- [x] InMemory 图存储
- [x] Mock 嵌入器（确定性向量）
- [x] 规则驱动的语义分析器
- [x] LLM 客户端抽象 + Mock 实现
- [x] Prompt 模板管理器（5 个模板）
- [x] SemanticInterpreterAgent
- [x] CrossCommitRecoveryAgent
- [x] MergeAgent
- [x] AgentOrchestrator
- [x] NL 指令解析器（规则驱动）
- [x] 对话上下文管理器
- [x] 指令执行器
- [x] CLI 命令（init/do/search/index/recover/merge）
- [ ] REST API 服务
- [ ] Web UI
- [ ] MultiAgentCoordinator
- [ ] ValidationRiskAgent
- [ ] RollbackAgent
