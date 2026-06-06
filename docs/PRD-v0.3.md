# AetherVC 产品需求文档 (PRD) v0.3

> **主题：补全核心智能能力 —— 真实 LLM 接入 + 3 大缺失 Agent + 多 Agent 端到端协调**
>
> 基于 Rust 技术栈，延续 v0.1~v0.2 的架构设计，聚焦"从骨架到可用"的质变。

---

## 目录

1. [背景：v0.2 未解决的问题](#1-背景v02-未解决的问题)
2. [Part A：真实 LLM / Embedder 接入](#2-part-a真实-llm--embedder-接入)
3. [Part B：MultiAgentCoordinator Agent](#3-part-bmultiagentcoordinator-agent)
4. [Part C：ValidationRisk Agent](#4-part-cvalidationrisk-agent)
5. [Part D：Rollback Agent](#5-part-drollback-agent)
6. [Part E：多 Agent 端到端协调工作流](#6-part-e多-agent-端到端协调工作流)
7. [Part F：统一配置与用户体验](#7-part-f统一配置与用户体验)
8. [实现路线图](#8-实现路线图)
9. [验收标准](#9-验收标准)

---

## 1. 背景：v0.2 未解决的问题

### 1.1 现状回顾

| 模块 | v0.2 状态 | 根因 |
|------|----------|------|
| LLM 推理 | 全部 `MockLLMClient`，返回假数据 | 虽有 `DeepSeekClient` / `OpenAICompatibleClient` 实现，但 CLI 入口硬编码 `MockLLMClient` |
| Embedding | 全部 `MockEmbedder`，用 ASCII 字节映射伪向量 | 虽有 `Embedder` trait，但没有真实的 OpenAI Embedding 实现 |
| MultiAgentCoordinator | 仅 `AgentType` 枚举 | 无 Agent 结构体、无 execute 实现 |
| ValidationRisk | 仅 `AgentType` 枚举 | 无 Agent 结构体、无 execute 实现 |
| Rollback | 仅 `AgentType` 枚举 | 无 Agent 结构体、无 execute 实现 |
| 多 Agent 协调 | 不存在 | `AgentOrchestrator` 只做单任务路由，无并发/协调能力 |

### 1.2 v0.3 目标

```
┌───────────────────────────────────────────────────────┐
│               v0.3 核心命题                             │
│                                                       │
│  让 AetherVC 从"结构完整的骨架"                          │
│  变成"真正能用 AI 理解代码的版本管理系统"                   │
│                                                       │
│  三项关键补全：                                         │
│  1. LLM/Embedder 真实化 → 语义理解从 0 到 1             │
│  2. 3 大 Agent 补齐 → 6 大 Agent 全部可用               │
│  3. 多 Agent 协调 → 场景 2&3 真正可跑通                 │
│                                                       │
└───────────────────────────────────────────────────────┘
```

---

## 2. Part A：真实 LLM / Embedder 接入

### 2.1 问题分析

**当前调用链（全 Mock）**：

```
CLI (aether-cli/src/main.rs)
  → MockLLMClient::new()          ← 硬编码
  → MockEmbedder::default()       ← 硬编码
  → SemanticIndexer(embedder, store, git)
  → AgentContext { llm_client: None, embedder, ... }
  → AgentOrchestrator → 各 Agent
```

**已有但未使用的真实实现**：

| 文件 | 能力 | 是否被 CLI 使用 |
|------|------|:---:|
| [llm/providers/deepseek.rs](file:///d:/trae_projects/AetherVC/aether-core/src/llm/providers/deepseek.rs) | DeepSeek API 真实调用 | ❌ |
| [llm/providers/openai.rs](file:///d:/trae_projects/AetherVC/aether-core/src/llm/providers/openai.rs) | OpenAI 兼容 API 真实调用 | ❌ |
| [llm/factory.rs](file:///d:/trae_projects/AetherVC/aether-core/src/llm/factory.rs) | `LLMFactory::create()` / `LLMFactory::from_env()` | ❌ |
| [semantic/embedder.rs](file:///d:/trae_projects/AetherVC/aether-core/src/semantic/embedder.rs) | 只有 `MockEmbedder`，无 `OpenAIEmbedder` | ❌ |

**核心矛盾**：真实 LLM 客户端和工厂已经写好，但 CLI 入口从未调用它们。

### 2.2 需求详述

#### 2.2.1 LLM 真实化

##### F-A1：CLI 启动时根据配置/环境变量选择 LLM Provider

**优先级**：P0

```
启动流程：
  1. 读取 .aether/config.toml → [llm] 段
  2. 如果 config.toml 无配置 → 检查环境变量
  3. 如果都没有 → 回退到 MockLLMClient（仅开发/测试模式，打印警告）
```

**配置格式**（`.aether/config.toml`）：

```toml
[llm]
provider = "deepseek"              # deepseek | openai | openai-compatible
model = "deepseek-chat"            # 模型名称
api_key = "${DEEPSEEK_API_KEY}"    # 支持 ${ENV_VAR} 语法
api_base = "https://api.deepseek.com"  # openai-compatible 时必填
max_tokens = 4096
temperature = 0.3
```

**环境变量回退**（优先级：config.toml > 环境变量 > Mock）：

```
DEEPSEEK_API_KEY   → 自动启用 DeepSeek provider
OPENAI_API_KEY     → 自动启用 OpenAI provider
AETHER_LLM_PROVIDER → 手动指定 provider
AETHER_LLM_MODEL    → 手动指定 model
AETHER_LLM_BASE     → 自定义 API base URL
```

**实现位置**：

- 新文件：`aether-core/src/semantic/embedder/openai_embedder.rs` — OpenAI Embedding API 封装
- 修改：`aether-cli/src/main.rs` — 替换硬编码 `MockLLMClient::new()` 为 `LLMFactory::from_config_or_env()`
- 新文件：`aether-core/src/llm/factory.rs` — 新增 `from_config_or_env()` 方法

##### F-A2：真实 Embedding API 接入

**优先级**：P0

```rust
// aether-core/src/semantic/embedder/openai_embedder.rs

pub struct OpenAIEmbedder {
    api_base: String,        // 默认 https://api.openai.com/v1
    api_key: String,
    model: String,           // 默认 text-embedding-3-small (1536 维)
    dimension: usize,
    http_client: reqwest::Client,
}

#[async_trait]
impl Embedder for OpenAIEmbedder {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    fn dimension(&self) -> usize;  // text-embedding-3-small → 1536
}
```

**API 端点**：`POST {api_base}/embeddings`

```json
// Request
{
  "model": "text-embedding-3-small",
  "input": "commit diff content or natural language query",
  "encoding_format": "float"
}

// Response parsing → Vec<f32>
```

**维度和模型对应关系**：

| 模型 | 维度 | 推荐场景 |
|------|------|---------|
| `text-embedding-3-small` | 1536 | 默认，性价比最高 |
| `text-embedding-3-large` | 3072 | 精度要求高 |
| `voyage-code-2` (Voyage AI) | 1536 | 代码专用（可选扩展） |

**同时更新 `InMemoryVectorStore`**：维度从硬编码 384 改为由 Embedder 运行时决定。

##### F-A3：Embedder 工厂与统一初始化

**优先级**：P0

修改现有 `aether-core/src/semantic/embedder.rs`，增加 `EmbedderFactory`：

```rust
pub struct EmbedderFactory;

impl EmbedderFactory {
    /// 根据配置创建 Embedder
    pub fn create(config: &LLMConfig) -> Result<Arc<dyn Embedder>> {
        match config.provider.as_str() {
            "openai" | "deepseek" | "openai-compatible" => {
                // 所有 OpenAI 兼容的 API 都支持 embedding
                Ok(Arc::new(OpenAIEmbedder::new(
                    &config.api_key,
                    "text-embedding-3-small",
                )))
            }
            "mock" => Ok(Arc::new(MockEmbedder::default())),
            _ => {
                // 未知 provider 回退到 mock 并警告
                tracing::warn!("Unknown embedder provider, falling back to MockEmbedder");
                Ok(Arc::new(MockEmbedder::default()))
            }
        }
    }
}
```

##### F-A4：降级策略

**优先级**：P1

当真实 LLM/Embedder 不可用时（网络错误、额度耗尽、超时），系统应优雅降级：

```
LLM 调用失败
  → 3 次重试（指数退避：1s, 2s, 4s）
  → 仍失败 → 降级到 RuleBasedAnalyzer（规则驱动分析）
  → 在输出中标记 "⚠ LLM unavailable, using rule-based fallback"
  → 记录降级事件到日志

Embedding 调用失败
  → 3 次重试
  → 仍失败 → 降级到 MockEmbedder（用于测试连续性）
  → 语义搜索降级为关键词匹配（grep fallback）
  → 在输出中标记降级状态
```

**实现位置**：
- `aether-core/src/llm/client.rs` — `LLMClient` trait 增加重试包装器
- `aether-core/src/semantic/embedder.rs` — `Embedder` trait 增加重试包装器

---

### 2.3 LLM / Embedder 改造范围汇总

| 变更项 | 类型 | 文件 |
|--------|------|------|
| `OpenAIEmbedder` 实现 | 新增 | `aether-core/src/semantic/embedder/openai_embedder.rs` |
| `EmbedderFactory` | 新增 | `aether-core/src/semantic/embedder.rs`（追加） |
| `LLMFactory::from_config_or_env()` | 修改 | `aether-core/src/llm/factory.rs` |
| `RetryLLMClient` 装饰器 | 新增 | `aether-core/src/llm/client.rs`（追加） |
| `RetryEmbedder` 装饰器 | 新增 | `aether-core/src/semantic/embedder.rs`（追加） |
| `VectorStore` 维度自适应 | 修改 | `aether-core/src/storage/vector_db.rs` |
| CLI 初始化逻辑替换 | 修改 | `aether-cli/src/main.rs` |
| `LLMConfig` 新增 `embedding_model` 字段 | 修改 | `aether-core/src/config/types.rs` |
| `aether index` 使用真实 Embedder | 修改 | `aether-cli/src/main.rs` |

---

## 3. Part B：MultiAgentCoordinator Agent

### 3.1 问题分析

当前代码中 `MultiAgentCoordinator` 仅存在于 [domain/agent.rs](file:///d:/trae_projects/AetherVC/aether-core/src/domain/agent.rs) 的 `AgentType` 枚举中，以及 [agents/orchestrator.rs](file:///d:/trae_projects/AetherVC/aether-core/src/agents/orchestrator.rs) 的路由匹配，**没有任何 execute 实现**。

### 3.2 核心职责

```
MultiAgentCoordinator Agent 的职责：

1. 感知：监控多个 AI Agent 的并行提交活动
2. 检测：识别语义冲突（同一模块同时被多个 Agent 修改）
3. 协调：给出合并顺序建议、冲突解决方案
4. 通知：向人类 supervisor 汇报当前多 Agent 状态
```

### 3.3 功能详述

#### F-B1：多 Agent 活动感知

**优先级**：P0

**输入**：一段自然语言描述，或一组 commit hash 列表。

```
输入方式 A：自然语言触发
  "检查最近 2 小时内所有 AI Agent 的提交是否有冲突"

输入方式 B：指定 commit 列表
  aether do "分析 agent-A 的 commit abc123 和 agent-B 的 commit def456 的冲突情况"
```

**处理流程**：

```
1. 获取各 Agent 的 commit 列表（通过 Git log + author 过滤）
2. 用 SemanticIndexer 检索每个 commit 的语义信息
3. 构建 Module × Agent 矩阵：
          auth/  models/  api/  tests/
   Agent-A  3      0       1      0
   Agent-B  2      2       0      1        ← auth/ 冲突！
   Agent-C  1      1       3      0
4. 识别热点模块（>1 个 Agent 同时修改）
5. 对热点模块生成冲突分析报告
```

**冲突矩阵数据结构**：

```rust
pub struct AgentConflictMatrix {
    /// 时间窗口
    pub window: TimeWindow,
    /// 活跃 Agent 列表
    pub agents: Vec<AgentIdentity>,
    /// 模块 × Agent 的变更计数
    pub module_agent_counts: HashMap<String, HashMap<String, u32>>,
    /// 热点模块（>1 Agent 修改）
    pub hotspots: Vec<ModuleHotspot>,
    /// 整体协调建议
    pub recommendation: CoordinationPlan,
}

pub struct ModuleHotspot {
    pub module_path: String,
    pub agents_involved: Vec<String>,
    pub conflict_severity: ConflictSeverity,  // Low/Medium/High/Critical
    pub overlapping_functions: Vec<String>,   // 可能冲突的函数名
}
```

#### F-B2：语义冲突检测

**优先级**：P0

**语义冲突** ≠ Git 冲突。两个 Agent 修改同一文件的不同行，Git 可能不会报 conflict，但逻辑上可能互相矛盾。

```
检测方法：

1. 文件级冲突：同一个文件被多个 Agent 修改
   → 用 git diff 分析修改行是否重叠
   
2. 函数级冲突：同一个函数被多个 Agent 修改
   → 用 AetherCI Preprocessor 提取实体信息
   → 检查函数签名/调用关系

3. 语义级冲突：两个 Agent 的变更意图相矛盾
   → 用 LLM 分析各 Agent 的变更意图
   → 判断是否存在意图冲突
   → 例：Agent-A "添加缓存提升性能" vs Agent-B "移除缓存简化逻辑"
```

**冲突严重程度**：

| 等级 | 定义 | 示例 |
|------|------|------|
| Critical | 语义级冲突 + 文件重叠 | 同一函数的相反意图 |
| High | 函数级冲突 | 同一函数签名变更不一致 |
| Medium | 文件级冲突但无函数重叠 | 同一文件不同函数 |
| Low | 同一模块不同文件 | auth/login.rs vs auth/signup.rs |

#### F-B3：协调计划生成

**优先级**：P0

基于冲突矩阵，用 LLM 生成协调计划：

```
┌─────────────────────────────────────────────────────────┐
│  Multi-Agent Coordination Plan                          │
│                                                         │
│  时间窗口：2024-06-01 10:00 - 12:00                     │
│  活跃 Agent：3（Cline, Copilot, Claude）                 │
│                                                         │
│  ⚠ 热点模块：                                           │
│  • auth/ 被 Cline 和 Claude 同时修改（High 风险）        │
│    → Cline: 修改了 login.rs 的 JWT 验证                  │
│    → Claude: 重构了 middleware.rs 的认证中间件            │
│    → 建议：Claude 先合并，Cline rebase 后合并            │
│                                                         │
│  • models/ 被 Copilot 和 Claude 同时修改（Medium 风险）  │
│    → Copilot: 新增 UserProfile 字段                      │
│    → Claude: 重构 User 模型结构                          │
│    → 建议：两个 PR 合并到一个分支，由 Claude 做最终整合   │
│                                                         │
│  合并顺序建议：                                          │
│  1. ✅ Claude 的 认证中间件重构（无冲突，先合）           │
│  2. ✅ Copilot 的 UserProfile 字段（需 Claude 配合）     │
│  3. ⚠ Cline 的 JWT 修复（需要 rebase 到 Claude 之上）   │
│                                                         │
│  无需人工介入的 Agent：无                                 │
│  需要人工确认的：Cline（JWT 修复需验证逻辑正确性）       │
└─────────────────────────────────────────────────────────┘
```

#### F-B4：CLI 接口

**优先级**：P0

```bash
# 分析指定 commit 范围的多 Agent 冲突
aether do "分析最近 20 个 commit 的 Agent 冲突情况"

# 指定时间窗口
aether do "分析过去 2 小时的 Agent 协调情况"

# 指定 Agent 过滤
aether do "检查 Cline 和 Copilot 的提交有没有冲突"

# JSON 输出（供 AI Agent 程序化调用）
aether do "分析 Agent 冲突" --json
```

### 3.4 数据结构

```rust
// aether-core/src/domain/agent.rs 追加

/// Agent 身份信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub name: String,
    pub email: String,
    pub is_ai_agent: bool,
}

/// 冲突严重程度
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConflictSeverity {
    Critical,
    High,
    Medium,
    Low,
}

/// 模块热点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleHotspot {
    pub module_path: String,
    pub agents_involved: Vec<String>,
    pub severity: ConflictSeverity,
    pub overlapping_functions: Vec<String>,
}

/// 协调计划
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationPlan {
    pub summary: String,
    pub merge_order: Vec<MergeOrderItem>,
    pub requires_human_review: Vec<String>,
    pub auto_mergeable: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeOrderItem {
    pub priority: u32,
    pub agent: String,
    pub commit_hash: String,
    pub description: String,
    pub reason: String,
}
```

### 3.5 实现位置

| 文件 | 内容 |
|------|------|
| `aether-core/src/agents/coordinator.rs` | **新增** — `MultiAgentCoordinatorAgent` 结构体 + execute |
| `aether-core/src/domain/agent.rs` | **修改** — 追加 `AgentIdentity`, `CoordinationPlan` 等类型 |
| `aether-core/src/agents/mod.rs` | **修改** — 注册 `mod coordinator` |
| `aether-core/src/agents/orchestrator.rs` | **修改** — 在 `with_all_agents()` 中注册 Coordinator |

---

## 4. Part C：ValidationRisk Agent

### 4.1 问题分析

当前 `ValidationRisk` 只存在于 `AgentType` 枚举中。虽然 `VerificationRunner` 和 `VerifyConfig` 已实现，但它们只是本地编译/lint/测试检查，不具备 **跨环境 Tag 验证**、**影子部署**、**风险评估** 能力。

### 4.2 核心职责

```
ValidationRisk Agent 的职责：

1. 验证 Tag 质量：自动编译 + 测试 + 静态分析
2. 评估部署风险：变更类型 × 影响范围 × 历史验证通过率
3. 生成验证报告：Pass/Fail + 详细建议
4. 按风险排序 Tag：帮助人类决定"先验证哪个 Tag"
```

### 4.3 功能详述

#### F-C1：批量 Tag 验证

**优先级**：P0

```
输入：一批 Tag 名称 或 自然语言范围
  "验证最近 15 个包含 RAG 关键词的 tag"

处理流程：
  1. 解析 Tag 列表
  2. 对每个 Tag：
     a. checkout 该 Tag 的代码
     b. 运行 VerifyConfig 中配置的检查（编译、lint、测试）
     c. 用 AetherCI 分析该 Tag 的变更意图和风险
     d. 生成单 Tag 验证报告
  3. 汇总所有 Tag 报告 → 排序 → 输出
```

**Tag 风险评分模型**：

```
TagRiskScore = 代码风险(0~1) × 变更规模(0~1) × 模块敏感度(0~1) × 历史因子(0~1)

其中：
- 代码风险：AetherCI 分析输出的 risk_level
  - Critical → 1.0, High → 0.8, Medium → 0.5, Low → 0.2
- 变更规模：(files_changed / 50).min(1.0)
- 模块敏感度：涉及敏感模块(auth/database/payment) → 1.0，普通模块 → 0.5
- 历史因子：1.0 - 该 Agent 的历史验证通过率
```

#### F-C2：验证报告模板

**优先级**：P0

```markdown
# Tag 验证报告

## 概览
| 属性 | 值 |
|------|-----|
| Tag | v2.1.3-rag-optimize |
| Commit | abc1234def |
| 关联 Agent | Cline |
| 验证时间 | 2024-06-01 14:30:00 |
| 整体结论 | ⚠ 条件通过（1 项测试失败） |

## 验证结果
| 检查项 | 状态 | 耗时 | 详情 |
|--------|------|------|------|
| 编译检查 | ✅ Passed | 12s | - |
| Lint 检查 | ✅ Passed | 3s | 0 warnings |
| 单元测试 | ❌ Failed | 45s | test_rag_pipeline: assertion failed |
| 静态分析 | ✅ Passed | 8s | - |

## 风险评估
- **变更类型**：功能新增（RAG pipeline）
- **影响模块**：src/rag/, src/llm/
- **风险等级**：Medium
- **风险因素**：新增模块，涉及 LLM 调用，有网络依赖

## 验证建议
1. ❌ 修复 test_rag_pipeline 失败（预计在 src/rag/pipeline.rs:142）
2. 🔍 建议人工 review src/rag/pipeline.rs 的核心逻辑
3. 📋 建议补充 RAG 模块的集成测试
```

#### F-C3：按用户需求排序和过滤 Tag

**优先级**：P1

```bash
# 方式 1：自然语言筛选
aether do "把最近 15 个包含 RAG 的 tag 按风险从低到高验证"

# 处理：
#   1. 用语义搜索找到含 "RAG" 的 Tag
#   2. 用 AetherCI 分析每个 Tag 的风险
#   3. 按风险从低到高排序（低风险先验证，高风险后验证）
#   4. 按顺序逐个 checkout → 验证 → 报告

# 方式 2：精确指定
aether verify-tags v1.0 v1.1 v1.2 --order-by risk --asc
aether verify-tags v1.0..v2.0 --filter "security" --order-by risk
```

**输出示例**：

```
Tag 验证队列（按风险从低到高）：
  1. 🟢 v1.0.2  LOW     docs/update            建议：直接验证
  2. 🟢 v1.1.0  LOW     tests/add-rag-tests     建议：直接验证
  3. 🟡 v2.0.1  MEDIUM  models/add-embedding    建议：编译+测试
  4. 🟠 v2.1.0  HIGH    rag/pipeline-impl        建议：完整验证+人工review
  5. 🔴 v2.2.0  CRITICAL auth/jwt-refactor       建议：暂缓，需人工确认

继续验证？[Y/n]
```

#### F-C4：验证环境抽象

**优先级**：P1（基础设施，为后续多环境影子部署做准备）

```rust
pub enum VerifyEnvironment {
    Local,           // 当前本地环境
    Docker(String),  // Docker 镜像
    Remote(String),  // 远程服务器地址
}
```

**MVP 阶段**：仅支持 `Local` 环境（在当前机器上 checkout Tag → 验证），其他环境保留接口。

#### F-C5：CLI 接口

**优先级**：P0

```bash
# 验证单个 Tag
aether verify-tag v2.1.0

# 验证多个 Tag
aether verify-tags v1.0 v1.5 v2.0

# Tag 范围验证
aether verify-tags v1.0..v2.0 --order-by risk

# 自然语言触发
aether do "验证最近 10 个 tag 并给出验证报告"

# 生成 JSON 报告
aether verify-tag v2.1.0 --json --output report.json
```

### 4.4 数据结构

```rust
// aether-core/src/domain/tag.rs（扩展）

pub struct TagValidationRequest {
    pub tags: Vec<String>,
    pub order_by: TagOrderBy,     // RiskAsc / RiskDesc / Chronological
    pub filter_keyword: Option<String>,
    pub max_tags: usize,
}

pub enum TagOrderBy {
    RiskAsc,
    RiskDesc,
    Chronological,
}

pub struct TagValidationReport {
    pub tag: String,
    pub commit_hash: String,
    pub agent: String,
    pub verification: VerificationReport,  // 复用现有 verify 模块
    pub risk_assessment: TagRiskAssessment,
    pub overall_conclusion: ValidationConclusion,
}

pub enum ValidationConclusion {
    Pass,
    ConditionalPass(Vec<String>),  // 条件 + 理由
    Fail(Vec<String>),             // 失败 + 原因
}
```

### 4.5 实现位置

| 文件 | 内容 |
|------|------|
| `aether-core/src/agents/validation.rs` | **新增** — `ValidationRiskAgent` |
| `aether-core/src/domain/tag.rs` | **修改** — 追加 `TagValidationRequest`, `TagValidationReport` |
| `aether-core/src/agents/mod.rs` | **修改** — 注册 |
| `aether-core/src/agents/orchestrator.rs` | **修改** — 注册到 `with_all_agents()` |
| `aether-cli/src/main.rs` | **修改** — 新增 `verify-tag` / `verify-tags` 子命令 |

---

## 5. Part D：Rollback Agent

### 5.1 问题分析

`RollbackAgent` 仅存在于枚举中。当前系统没有任何能力做智能回滚——没有自动检测"提交后出了问题"，没有自动回滚逻辑。

### 5.2 核心职责

```
Rollback Agent 的职责：

1. 监控：在 commit 被合并/部署后持续监控（编译失败、测试失败、验证报告异常）
2. 判断：当验证报告标记为 Fail 时，自动判定是否需要回滚
3. 执行：创建 revert commit、通知人类、记录回滚历史
4. 学习：记录回滚原因，优化后续风险评估
```

### 5.3 功能详述

#### F-D1：自动回滚触发条件

**优先级**：P0

```
回滚触发条件（可配置）：

1. 编译失败 → 立即建议回滚
2. 单元测试失败率 > 阈值 → 建议回滚（阈值默认 10%）
3. Lint 新增 Error 级别警告 > 0 → 建议回滚
4. 安全扫描发现 CVE → 强制回滚
5. 人类手动触发 → 立即回滚
```

**配置**（`.aether/config.toml`）：

```toml
[rollback]
enabled = true
auto_rollback_on_compile_failure = true
auto_rollback_on_test_failure = false   # 测试失败需要人类确认
test_failure_threshold = 0.1            # 10% 失败率
auto_rollback_on_security_cve = true    # 安全漏洞立即回滚
require_human_approval = true           # 所有回滚是否需要人类确认
```

#### F-D2：回滚执行流程

**优先级**：P0

```
验证报告标记为 Fail
     │
     ▼
┌─────────────────────┐
│ Rollback Agent 分析  │
│                     │
│ 检查：              │
│ □ 失败是否可修复？  │
│   - 可修复 → 建议修复而非回滚  │
│   - 不可快速修复 → 建议回滚   │
│ □ 影响范围？        │
│ □ 是否有级联影响？  │
└────────┬────────────┘
         │
         ▼
┌─────────────────────┐
│ 生成回滚方案        │
│                     │
│ 方案 A：git revert  │ ← 安全，保留历史
│ 方案 B：git reset   │ ← 彻底，丢失历史（需确认）
│                     │
│ 推荐 + 理由         │
└────────┬────────────┘
         │
    ┌────┼────┐
    ▼    ▼    ▼
  自动回滚  等待确认  建议修复
```

**回滚操作类型**：

```rust
pub enum RollbackAction {
    /// git revert：创建新的 revert commit，保留历史
    Revert { commit_hash: String },
    /// git reset：回退到指定 commit
    Reset { target_hash: String, hard: bool },
    /// 仅通知，不做操作
    NotifyOnly { reason: String },
}
```

#### F-D3：回滚报告

**优先级**：P1

```markdown
# 回滚报告

## 回滚信息
| 属性 | 值 |
|------|-----|
| 回滚 ID | RB-20240601-001 |
| 回滚 Commit | abc1234 |
| 回滚类型 | git revert |
| 触发原因 | 编译失败：auth/login.rs:45 类型不匹配 |
| 执行时间 | 2024-06-01 14:35:00 |
| 执行状态 | ✅ Revert commit 已创建: def5678 |

## 影响分析
- 影响的 Tag：v2.3.0
- 级联影响：无（该 commit 尚未被其他 commit 依赖）

## 记录
- 回滚原因已记录到知识库
- 该 Agent 信誉分已更新（-0.05）
- 建议：下次类似变更需要提前运行完整测试套件
```

#### F-D4：回滚历史与学习

**优先级**：P1

```
aether rollback history
# 输出：
#  时间       Commit    原因              状态      Agent
#  06-01 14:35  abc1234  编译失败          完成  ✅  Cline
#  05-28 10:12  xyz7890  安全漏洞 CVE-xxx  完成  ✅  Claude
#  05-25 16:45  def4567  测试失败率 35%    完成  ✅  Cline
#
#  回滚统计：
#  当前 Agent 信誉分：Cline(0.85), Claude(0.95), Copilot(0.92)
```

**信誉分计算公式**：

```
AgentScore = 1.0 - (回滚次数 × 0.05) - (验证失败次数 × 0.02) + (验证通过次数 × 0.01)
范围：[0.0, 1.0]，初始 1.0
```

信誉分用于 [4.4.3 风险评分模型] 中的 `作者信誉` 因子，形成闭环。

#### F-D5：CLI 接口

**优先级**：P0

```bash
# 分析指定 commit 是否需要回滚
aether rollback analyze abc1234

# 执行回滚
aether rollback execute abc1234 --method revert
aether rollback execute abc1234 --method reset --hard  # 需要确认

# 查看回滚历史
aether rollback history

# 自然语言触发
aether do "回滚最近一次导致编译失败的提交"
aether do "撤销 Cline 过去 1 小时内的所有提交"

# 查看 Agent 信誉分
aether rollback reputation
```

### 5.4 数据结构

```rust
// aether-core/src/domain/agent.rs 追加

pub struct RollbackRequest {
    pub commit_hash: String,
    pub reason: String,
    pub action: RollbackAction,
    pub require_approval: bool,
}

pub struct RollbackRecord {
    pub id: String,
    pub rolled_back_commit: String,
    pub revert_commit: Option<String>,
    pub reason: String,
    pub action: RollbackAction,
    pub status: RollbackStatus,
    pub executed_at: DateTime<Utc>,
    pub agent_name: String,
}

pub enum RollbackStatus {
    PendingApproval,
    Executed,
    Failed(String),
    Cancelled,
}
```

### 5.5 实现位置

| 文件 | 内容 |
|------|------|
| `aether-core/src/agents/rollback.rs` | **新增** — `RollbackAgent` |
| `aether-core/src/config/types.rs` | **修改** — 新增 `RollbackConfig` |
| `aether-core/src/agents/mod.rs` | **修改** — 注册 |
| `aether-core/src/agents/orchestrator.rs` | **修改** — 注册到 `with_all_agents()` |
| `aether-cli/src/main.rs` | **修改** — 新增 `rollback` 子命令 |

---

## 6. Part E：多 Agent 端到端协调工作流

### 6.1 问题分析

前三个 Part 补齐了单点能力，但缺少**端到端的自动化工作流**。三个 Agent 各自为战，无法串联成完整的"人类只需下达一个命令，系统自动完成协调→验证→回滚"的闭环。

### 6.2 端到端工作流设计

#### 工作流 1：日常 AI 提交消化流程

```
人类： "总结过去 2 小时 AI 做了什么，需要我关注什么"
   │
   ▼
┌─────────────────────────────────────────────────────┐
│ Step 1: Digest Engine                               │
│   生成聚合摘要：变更主题、风险分布、模块热力图         │
│   输出：Markdown 摘要报告                             │
└────────────────────────┬────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────┐
│ Step 2: BatchAnalyzer       │
│   按风险排序所有 commit       │
│   输出：排序后的 commit 列表  │
└────────────────────────┬────┘
                         │
              ┌──────────┼──────────┐
              ▼          ▼          ▼
          低风险      中风险      高风险
              │          │          │
              ▼          ▼          ▼
┌────────┐ ┌────────┐ ┌────────────┐
│自动通过 │ │加入审核 │ │阻止 + 通知  │
│GateEngine│ │队列    │ │GateEngine  │
└────────┘ └───┬────┘ └─────┬──────┘
               │            │
               ▼            ▼
         ┌──────────────┐  ┌──────────────┐
         │VerificationRunner│  │ RollbackAgent│
         │自动验证         │  │ 如果验证失败  │
         └──────┬───────┘  │ → 自动回滚   │
                │          └──────────────┘
                ▼
         验证通过 → 合并
```

**CLI 入口**：

```bash
# 一条命令完成整个流程
aether workflow digest --since "2 hours ago"

# 输出：
#   📊 摘要报告已生成 → .aether/reports/digest-20240601-1400.md
#   📋 审核队列：2 个待审核项
#   ✅ 自动通过：18 个 commit
#   ⚠ 需要关注：3 个 commit（详见审核队列）
#   📝 查看审核队列：aether review queue
```

#### 工作流 2：多 Agent 冲突协调流程

```
人类： "检查并协调所有 Agent 的提交冲突"
   │
   ▼
┌─────────────────────────────────────────────────────┐
│ Step 1: MultiAgentCoordinator                       │
│   构建 Agent × Module 冲突矩阵                       │
│   识别热点模块                                       │
│   输出：CoordinationPlan                            │
└────────────────────────┬────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────┐
│ Step 2: MergeAgent              │
│   分析冲突 PR 的最优合并顺序     │
│   检测 Git 级和语义级冲突       │
│   输出：MergeRecommendation      │
└────────────────────────┬───────┘
                         │
              ┌──────────┼──────────┐
              ▼          ▼          ▼
         无冲突PR     有冲突PR    冲突不可自动解决
              │          │          │
              ▼          ▼          ▼
┌────────┐ ┌────────┐ ┌────────────────┐
│自动合并 │ │尝试自动│ │ 生成人工建议    │
│        │ │解决冲突│ │ 通知 Tech Lead  │
└────────┘ └───┬────┘ └────────────────┘
               │
          ┌────┼────┐
          ▼    ▼    ▼
       成功  失败  → 通知人类
          │
          ▼
     自动合并
```

**CLI 入口**：

```bash
# 一条命令完成多 Agent 协调
aether workflow coordinate --since "2 hours ago"

# 输出：
#   🔍 检测到 3 个活跃 Agent：Cline, Copilot, Claude
#   ⚠ 热点模块：auth/（Cline + Claude）, models/（Copilot + Claude）
#   📋 合并方案已生成
#   ✅ 可自动合并：Claude 的 auth 重构
#   🔄 需 rebase 后合并：Cline 的 JWT 修复
#   ❌ 需人工介入：Copilot + Claude 的 models 冲突
```

#### 工作流 3：Tag 验证与部署决策流程

```
人类： "验证最近 15 个 RAG 相关 tag，告诉我可以部署哪些"
   │
   ▼
┌─────────────────────────────────────────────────────┐
│ Step 1: ValidationRisk Agent                        │
│   搜索含 "RAG" 的 Tag                                │
│   按风险从低到高排序                                  │
│   逐个 checkout → 编译 → 测试 → 风险评估              │
│   输出：TagValidationReport[]                        │
└────────────────────────┬────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────┐
│ Step 2: 汇总分析               │
│   ✅ 可直接部署的 Tag           │
│   ⚠ 条件可部署的 Tag           │
│   ❌ 不建议部署的 Tag           │
└────────────────────────┬───────┘
                         │
                         ▼
┌────────────────────────────────┐
│ Step 3: RollbackMonitor        │
│   部署后持续监控验证状态         │
│   如果验证失败 → 触发 Rollback  │
└────────────────────────────────┘
```

**CLI 入口**：

```bash
# 一条命令完成 Tag 验证
aether workflow verify-tags --keyword "RAG" --count 15 --order-by risk-asc

# 输出：
#   🔍 找到 12 个包含 "RAG" 的 Tag
#   📋 验证计划：
#     1. v1.0.2  LOW     → 预计 30s
#     2. v1.1.0  LOW     → 预计 30s
#     ...
#     12. v2.2.0 CRITICAL → 预计 120s（含完整测试）
#   
#   执行中...
#   [1/12] v1.0.2  ✅ Passed
#   [2/12] v1.1.0  ✅ Passed
#   [3/12] v2.0.1  ⚠ Conditional（1 项测试不稳定）
#   ...
#
#   验证完成：
#   ✅ 可直接部署：8 个 Tag
#   ⚠ 条件可部署：3 个 Tag
#   ❌ 不建议部署：1 个 Tag（v2.2.0 - 安全扫描发现依赖漏洞）
```

### 6.3 工作流编排器设计

**优先级**：P0

新增 `WorkflowOrchestrator`，位于 `aether-core`：

```rust
// aether-core/src/workflow/mod.rs（新模块）

pub struct WorkflowOrchestrator {
    agent_orchestrator: Arc<AgentOrchestrator>,
    gate_engine: GateEngine,
    verification_runner: VerificationRunner,
    review_queue: ReviewQueue,
}

pub enum WorkflowType {
    /// 日常消化：摘要 + 风险排序 + 门控 + 验证
    Digest,
    /// 多 Agent 协调：冲突检测 + 合并分析 + 建议生成
    Coordinate,
    /// Tag 验证：搜索 + 排序 + 验证 + 部署建议
    VerifyTags,
    /// 完整 CI：收到新 commit → 分析 → 门控 → 验证 → 回滚判断
    FullCI,
}

impl WorkflowOrchestrator {
    pub async fn execute(&mut self, workflow: WorkflowType) -> Result<WorkflowResult>;
}
```

### 6.4 CLI 工作流命令

```bash
# 三种工作流入口
aether workflow digest    --since "2 hours ago"
aether workflow coordinate --since "2 hours ago"
aether workflow verify-tags --keyword "RAG" --count 15

# 完整 CI 模式（watch 新 commit）
aether workflow full-ci --watch

# JSON 输出（供 AI Agent 调用）
aether workflow --json
```

### 6.5 实现位置

| 文件 | 内容 |
|------|------|
| `aether-core/src/workflow/mod.rs` | **新增** — `WorkflowOrchestrator` |
| `aether-core/src/workflow/digest_flow.rs` | **新增** — 工作流 1 |
| `aether-core/src/workflow/coordinate_flow.rs` | **新增** — 工作流 2 |
| `aether-core/src/workflow/verify_flow.rs` | **新增** — 工作流 3 |
| `aether-core/src/workflow/full_ci.rs` | **新增** — 工作流 4 |
| `aether-core/src/lib.rs` | **修改** — `pub mod workflow` |
| `aether-cli/src/main.rs` | **修改** — 新增 `workflow` 子命令 |

---

## 7. Part F：统一配置与用户体验

### 7.1 完整配置文件

`.aether/config.toml` 完整结构：

```toml
# ─── LLM 配置 ───
[llm]
provider = "deepseek"                    # deepseek | openai | openai-compatible | mock
model = "deepseek-chat"
api_key = "${DEEPSEEK_API_KEY}"          # 支持环境变量引用
api_base = "https://api.deepseek.com"
embedding_model = "text-embedding-3-small"
max_tokens = 4096
temperature = 0.3
max_retries = 3                          # 失败重试次数

# ─── 门控配置 ───
[gate]
enabled = true
auto_pass = ["documentation", "test", "config_change"]
require_review = ["security_hardening", "feature_removal"]

[gate.modules]
high_risk = ["auth/", "database/", "payment/", "core/"]
low_risk = ["docs/", "tests/", "assets/", "examples/"]

[gate.thresholds]
max_files_changed = 10
max_lines_added = 500
max_lines_deleted = 200
max_commits_per_hour = 10

[gate.actions]
on_threshold_exceeded = "block"
on_critical_risk = "block"
on_high_risk = "queue"

# ─── 验证配置 ───
[verify]
enabled = true

[verify.basic]
compile_check = true
lint_check = true
format_check = false

[verify.testing]
run_unit_tests = true
test_command = "cargo test"
run_affected_tests = true
coverage_threshold = 0.8

[verify.advanced]
static_analysis = true
security_scan = true
dependency_check = true

# ─── 回滚配置（新增）───
[rollback]
enabled = true
auto_rollback_on_compile_failure = true
auto_rollback_on_test_failure = false
test_failure_threshold = 0.1
auto_rollback_on_security_cve = true
require_human_approval = true
max_auto_rollbacks_per_hour = 3

# ─── 协调器配置（新增）───
[coordinator]
enabled = true
hotspot_threshold = 2                     # 多少 Agent 同时修改同一模块视为热点
monitor_window_minutes = 120              # 监控时间窗口
auto_resolve_low_risk = true              # 低风险冲突自动解决
```

### 7.2 一键初始化

```bash
# 在仓库中初始化，自动生成完整配置
aether init --with-config

# 初始化后检查状态
aether status

# 输出：
#   AetherVC v0.3.0
#   仓库：/path/to/repo
#   语义索引：✅ 已构建（1,234 个 commit）
#   LLM Provider：DeepSeek (deepseek-chat)
#   Embedder：OpenAI (text-embedding-3-small)
#   门控：✅ 已启用
#   验证：✅ 已启用
#   回滚：✅ 已启用
#   6/6 Agent 就绪
```

---

## 8. 实现路线图

### Phase 1：LLM/Embedder 真实化（P0，3-4 天）

| 任务 | 文件 | 优先级 |
|------|------|--------|
| 实现 `OpenAIEmbedder` | `aether-core/src/semantic/embedder/openai_embedder.rs` | P0 |
| 实现 `EmbedderFactory` | `aether-core/src/semantic/embedder.rs` | P0 |
| `LLMFactory::from_config_or_env()` | `aether-core/src/llm/factory.rs` | P0 |
| `RetryLLMClient` 装饰器 | `aether-core/src/llm/client.rs` | P1 |
| `RetryEmbedder` 装饰器 | `aether-core/src/semantic/embedder.rs` | P1 |
| 修改 CLI 入口使用真实 Provider | `aether-cli/src/main.rs` | P0 |
| 配置类型扩展 | `aether-core/src/config/types.rs` | P0 |
| 配置加载器支持环境变量引用 | `aether-core/src/config/loader.rs` | P0 |
| 向量存储维度自适应 | `aether-core/src/storage/vector_db.rs` | P0 |
| 集成测试：真实 LLM 调通 | `aether-core/tests/` | P0 |

### Phase 2：3 大 Agent 实现（P0，4-5 天）

| 任务 | 文件 | 优先级 |
|------|------|--------|
| `MultiAgentCoordinatorAgent` 结构体 + execute | `aether-core/src/agents/coordinator.rs` | P0 |
| 冲突矩阵构建逻辑 | 同上 | P0 |
| LLM 驱动的协调计划生成 | 同上 | P0 |
| `ValidationRiskAgent` 结构体 + execute | `aether-core/src/agents/validation.rs` | P0 |
| Tag 风险评分模型 | 同上 | P0 |
| `RollbackAgent` 结构体 + execute | `aether-core/src/agents/rollback.rs` | P0 |
| 回滚执行 + 历史记录 | 同上 | P0 |
| 领域模型扩展 | `aether-core/src/domain/` | P0 |
| 配置类型扩展（rollback + coordinator） | `aether-core/src/config/types.rs` | P0 |
| 在 orchestrator 中注册 3 个新 Agent | `aether-core/src/agents/orchestrator.rs` | P0 |

### Phase 3：端到端工作流 + CLI 集成（P0，3-4 天）

| 任务 | 文件 | 优先级 |
|------|------|--------|
| `WorkflowOrchestrator` | `aether-core/src/workflow/mod.rs` | P0 |
| 工作流 1：Digest 流程 | `aether-core/src/workflow/digest_flow.rs` | P0 |
| 工作流 2：Coordinate 流程 | `aether-core/src/workflow/coordinate_flow.rs` | P0 |
| 工作流 3：VerifyTags 流程 | `aether-core/src/workflow/verify_flow.rs` | P0 |
| 工作流 4：FullCI 流程 | `aether-core/src/workflow/full_ci.rs` | P1 |
| CLI `workflow` 子命令 | `aether-cli/src/main.rs` | P0 |
| CLI `rollback` 子命令 | `aether-cli/src/main.rs` | P0 |
| CLI `verify-tag` 子命令 | `aether-cli/src/main.rs` | P0 |
| CLI `status` 子命令 | `aether-cli/src/main.rs` | P1 |

### Phase 4：测试与文档（P1，2 天）

| 任务 | 文件 | 优先级 |
|------|------|--------|
| Agent 单元测试（Mock LLM） | `aether-core/tests/` | P0 |
| 工作流集成测试 | `aether-core/tests/workflows.rs` | P0 |
| CLI 端到端测试 | `aether-cli/tests/` | P1 |
| 更新 README | `README.md` | P1 |

---

## 9. 验收标准

### 9.1 LLM / Embedder 真实化

- [x] ~~`MockLLMClient::new()` 作为默认~~ → `aether init` 后自动从环境变量/配置文件获取真实 Provider
- [ ] 设置 `DEEPSEEK_API_KEY` 后，`aether search "认证模块"` 返回语义相关结果（而非随机匹配）
- [ ] 真实 LLM/Embedder 不可用时自动降级为 RuleBasedAnalyzer + MockEmbedder，并有明确日志
- [ ] `aether index` 使用真实 Embedding API 构建向量索引
- [ ] 支持 DeepSeek、OpenAI、OpenAI 兼容三种 Provider

### 9.2 3 大 Agent

- [ ] `aether do "检查 Agent 冲突"` 能正确识别热点模块并生成协调计划
- [ ] `aether verify-tag v1.0` 能 checkout Tag 并执行验证
- [ ] `aether rollback analyze abc1234` 能给出回滚建议
- [ ] `aether rollback execute abc1234 --method revert` 能成功执行 revert
- [ ] 6 大 Agent 全部可在 `AgentOrchestrator` 中通过任务类型路由到

### 9.3 多 Agent 端到端协调

- [ ] `aether workflow digest --since "2 hours ago"` 一条命令完成摘要 + 门控 + 验证全流程
- [ ] `aether workflow coordinate --since "2 hours ago"` 输出完整的冲突分析和合并建议
- [ ] `aether workflow verify-tags --keyword "RAG" --count 5` 按风险排序验证 5 个 Tag
- [ ] 所有工作流输出包含人类可读的 Markdown 和机器可读的 JSON 两种格式

### 9.4 对三个用户场景的最终覆盖

| 场景 | v0.2 状态 | v0.3 目标 |
|------|----------|----------|
| 场景1：commit1→2→3 恢复 | Mock 恢复，质量不可控 | 真实 LLM 驱动的语义恢复，准确率 > 85% |
| 场景2：大量 Tag 验证 | ❌ 不支持 | `aether workflow verify-tags` 完整支持 |
| 场景3：海量 PR 合并 + 多 Agent | Mock 分析 + 无协调 | `aether workflow coordinate` 真实协调 + 冲突检测 |

---

## 附录 A：文件变更清单

### 新增文件（~14 个）

```
aether-core/src/agents/coordinator.rs          # MultiAgentCoordinatorAgent
aether-core/src/agents/validation.rs           # ValidationRiskAgent
aether-core/src/agents/rollback.rs             # RollbackAgent
aether-core/src/semantic/embedder/openai_embedder.rs  # OpenAIEmbedder
aether-core/src/semantic/embedder/retry.rs      # RetryEmbedder 装饰器
aether-core/src/llm/client/retry.rs             # RetryLLMClient 装饰器
aether-core/src/workflow/mod.rs                 # WorkflowOrchestrator
aether-core/src/workflow/digest_flow.rs         # 工作流 1
aether-core/src/workflow/coordinate_flow.rs     # 工作流 2
aether-core/src/workflow/verify_flow.rs         # 工作流 3
aether-core/src/workflow/full_ci.rs             # 工作流 4
```

### 修改文件（~12 个）

```
aether-core/src/semantic/embedder.rs            # 追加 EmbedderFactory + OpenAIEmbedder re-export
aether-core/src/llm/factory.rs                  # 追加 from_config_or_env()
aether-core/src/llm/client.rs                   # 追加 RetryLLMClient
aether-core/src/llm/mod.rs                      # 模块声明更新
aether-core/src/config/types.rs                 # 追加 RollbackConfig, CoordinatorConfig
aether-core/src/domain/agent.rs                 # 追加 AgentIdentity, CoordinationPlan 等
aether-core/src/domain/tag.rs                   # 追加 TagValidationRequest 等
aether-core/src/agents/mod.rs                   # 注册 3 个新 Agent 模块
aether-core/src/agents/orchestrator.rs          # with_all_agents() 注册新 Agent
aether-core/src/storage/vector_db.rs            # 维度自适应
aether-core/src/lib.rs                          # pub mod workflow
aether-cli/src/main.rs                          # 新子命令 + LLM/Embedder 初始化改造
```

---

## 附录 B：核心 Prompt 模板（已在 v0.1 中定义，此处列出复用关系）

| 模板 ID | Agent | v0.3 使用位置 |
|---------|-------|-------------|
| `semantic_interpreter` | SemanticInterpreterAgent | 不变，继续使用 |
| `cross_commit_recovery` | CrossCommitRecoveryAgent | 不变，继续使用 |
| `merge_agent` | MergeAgent | 不变，继续使用 |
| `multi_agent_coordinator` | **MultiAgentCoordinatorAgent** | ← 第一次真正被调用 |
| `validation_risk` | **ValidationRiskAgent** | ← 第一次真正被调用 |
| `commit_intelligence` | AetherCI Pipeline | 不变，继续使用 |

---

## 附录 C：配置完整示例

```toml
# .aether/config.toml
# AetherVC v0.3 完整配置文件

[llm]
provider = "deepseek"
model = "deepseek-chat"
api_key = "${DEEPSEEK_API_KEY}"
api_base = "https://api.deepseek.com"
embedding_model = "text-embedding-3-small"
max_tokens = 4096
temperature = 0.3
max_retries = 3

[gate]
enabled = true
auto_pass = ["documentation", "test", "config_change"]
require_review = ["security_hardening", "feature_removal"]

[gate.modules]
high_risk = ["auth/", "database/", "payment/", "core/"]
low_risk = ["docs/", "tests/", "assets/", "examples/"]

[gate.thresholds]
max_files_changed = 10
max_lines_added = 500
max_lines_deleted = 200
max_commits_per_hour = 10

[gate.actions]
on_threshold_exceeded = "block"
on_critical_risk = "block"
on_high_risk = "queue"

[verify]
enabled = true

[verify.basic]
compile_check = true
lint_check = true
format_check = false

[verify.testing]
run_unit_tests = true
test_command = "cargo test"
run_affected_tests = true
coverage_threshold = 0.8

[verify.advanced]
static_analysis = true
security_scan = true
dependency_check = true

[rollback]
enabled = true
auto_rollback_on_compile_failure = true
auto_rollback_on_test_failure = false
test_failure_threshold = 0.1
auto_rollback_on_security_cve = true
require_human_approval = true
max_auto_rollbacks_per_hour = 3

[coordinator]
enabled = true
hotspot_threshold = 2
monitor_window_minutes = 120
auto_resolve_low_risk = true
```
