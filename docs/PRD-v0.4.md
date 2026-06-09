# AetherVC 产品需求文档 (PRD) v0.4

> **主题：从"功能完备"到"生产可用" —— 稳定性硬化、端到端验证、性能优化与 DX 提升**
>
> 基于 v0.3 PRD 交付后代码审计，聚焦当前真实实现状态下的剩余工作。

---

## 目录

1. [背景：v0.3 实现状态审计](#1-背景v03-实现状态审计)
2. [Stage 1：核心路径稳定性硬化（P0）](#2-stage-1核心路径稳定性硬化p0)
3. [Stage 2：端到端工作流验证（P0）](#3-stage-2端到端工作流验证p0)
4. [Stage 3：性能与成本优化（P1）](#4-stage-3性能与成本优化p1)
5. [Stage 4：开发者体验与集成（P1）](#5-stage-4开发者体验与集成p1)
6. [Stage 5：AetherCI 真实 LLM 接入（P1）](#6-stage-5aetherci-真实-llm-接入p1)
7. [实现路线图](#7-实现路线图)
8. [验收标准](#8-验收标准)

---

## 1. 背景：v0.3 实现状态审计

### 1.1 代码审计结论

**v0.3 PRD 中提出的绝大多数目标已经实现**。以下是逐项审计结果：

#### Part A：真实 LLM / Embedder 接入 → ✅ 已完成

| PRD 需求 | 实现状态 | 文件 |
|----------|:---:|------|
| `LLMFactory::from_config_or_env()` | ✅ 已实现 | [aether-core/src/llm/factory.rs](file:///d:/trae_projects/AetherVC/aether-core/src/llm/factory.rs) |
| `DeepSeekClient` 真实 API 调用 | ✅ 已实现 | [aether-core/src/llm/providers/deepseek.rs](file:///d:/trae_projects/AetherVC/aether-core/src/llm/providers/deepseek.rs) |
| `OpenAICompatibleClient` 真实 API | ✅ 已实现 | [aether-core/src/llm/providers/openai.rs](file:///d:/trae_projects/AetherVC/aether-core/src/llm/providers/openai.rs) |
| `OpenAIEmbedder` (支持 1536/3072 维度) | ✅ 已实现 | [aether-core/src/semantic/embedder/openai.rs](file:///d:/trae_projects/AetherVC/aether-core/src/semantic/embedder/openai.rs) |
| `EmbedderFactory` | ✅ 已实现 | [aether-core/src/semantic/embedder/factory.rs](file:///d:/trae_projects/AetherVC/aether-core/src/semantic/embedder/factory.rs) |
| `RetryLLMClient` 装饰器 | ✅ 已实现 | [aether-core/src/llm/client/retry.rs](file:///d:/trae_projects/AetherVC/aether-core/src/llm/client/retry.rs) |
| `RetryEmbedder` 装饰器 | ✅ 已实现 | [aether-core/src/semantic/embedder/retry.rs](file:///d:/trae_projects/AetherVC/aether-core/src/semantic/embedder/retry.rs) |
| CLI 入口替换 Mock 为真实工厂 | ✅ 已替换 | [aether-cli/src/main.rs#L472-L477](file:///d:/trae_projects/AetherVC/aether-cli/src/main.rs) `create_app_context()` |
| 配置系统 `.aether/config.toml` | ✅ 支持 | [aether-core/src/config/types.rs](file:///d:/trae_projects/AetherVC/aether-core/src/config/types.rs) (全部 6 个配置段) |
| 环境变量回退 + Mock 降级 | ✅ 已实现 | `from_config_or_env()` 内建三级回退 |
| `embedding_model` 配置字段 | ✅ 已添加 | `LLMConfig.embedding_model` |
| 向量存储动态维度 | ✅ 已支持 | `InMemoryVectorStore` 无硬编码维度，`OpenAIEmbedder.dimension()` 运行时决定 |

#### Part B/C/D：3 大 Agent 补齐 → ✅ 已完成

| PRD 需求 | 实现状态 | 文件 |
|----------|:---:|------|
| `MultiAgentCoordinatorAgent` + `execute()` | ✅ 已实现 | [aether-core/src/agents/coordinator.rs](file:///d:/trae_projects/AetherVC/aether-core/src/agents/coordinator.rs) |
| 冲突矩阵构建逻辑 | ✅ 已实现 | `build_conflict_matrix()` |
| LLM 驱动的协调计划生成 | ✅ 已实现 | `generate_coordination_plan()` + `parse_llm_coordination_plan()` |
| `ValidationRiskAgent` + `execute()` | ✅ 已实现 | [aether-core/src/agents/validation.rs](file:///d:/trae_projects/AetherVC/aether-core/src/agents/validation.rs) |
| Tag 风险评分模型 | ✅ 已实现 | `calculate_risk_score()` |
| `RollbackAgent` + `execute()` | ✅ 已实现 | [aether-core/src/agents/rollback.rs](file:///d:/trae_projects/AetherVC/aether-core/src/agents/rollback.rs) |
| 回滚执行 + 历史记录 | ✅ 已实现 | `execute_rollback()` + `perform_git_revert()` + `perform_git_reset()` |
| 领域类型扩展 | ✅ 已实现 | [aether-core/src/domain/agent.rs](file:///d:/trae_projects/AetherVC/aether-core/src/domain/agent.rs) (全部类型已定义) |
| 配置类型扩展 | ✅ 已实现 | `RollbackConfig`, `CoordinatorConfig`, `LLMFallbackConfig`, `StorageConfig` |
| 在 orchestrator 中注册 3 个新 Agent | ✅ 已实现 | [aether-core/src/agents/orchestrator.rs#L29-L31](file:///d:/trae_projects/AetherVC/aether-core/src/agents/orchestrator.rs) |
| CLI `rollback` 子命令 | ✅ 已定义 | `Commands::Rollback` |
| CLI `verify-tag` 子命令 | ✅ 已定义 | `Commands::VerifyTag` |
| CLI `workflow` 子命令 | ✅ 已定义 | `Commands::Workflow` |
| CLI `config` 子命令 | ✅ 已定义 | `Commands::Config` |

#### Part E/F：工作流 / 配置 / 用户体验 → ✅ 已完成

| PRD 需求 | 实现状态 | 文件 |
|----------|:---:|------|
| `WorkflowOrchestrator` | ✅ 已实现 | [aether-core/src/workflow/mod.rs](file:///d:/trae_projects/AetherVC/aether-core/src/workflow/mod.rs) |
| Digest 流程 | ✅ 已实现 | `execute_digest_flow()` |
| Coordinate 流程 | ✅ 已实现 | `execute_coordinate_flow()` |
| VerifyTags 流程 | ✅ 已实现 | `execute_verify_tags_flow()` |
| FullCI 流程 | ✅ 已实现 | `execute_full_ci_flow()` |
| 持久化向量存储 | ✅ 已实现 | `PersistentVectorStore` in [aether-core/src/storage/vector_db.rs](file:///d:/trae_projects/AetherVC/aether-core/src/storage/vector_db.rs) |
| 持久化图存储 | ✅ 已实现 | `PersistentGraphStore` in [aether-core/src/storage/graph_db.rs](file:///d:/trae_projects/AetherVC/aether-core/src/storage/graph_db.rs) |
| CLI `status` 子命令 | ✅ 已定义 | `Commands::Status` |

### 1.2 当前系统架构全景图

```
aether-cli (main.rs)
  │
  ├── create_app_context()
  │     ├── ConfigLoader::load()          → 读取 .aether/config.toml
  │     ├── LLMFactory::from_config_or_env() → DeepSeekClient / OpenAICompatibleClient / Mock
  │     ├── EmbedderFactory::create()     → OpenAIEmbedder / MockEmbedder
  │     ├── PersistentVectorStore / InMemoryVectorStore  (按配置选择)
  │     ├── PersistentGraphStore / InMemoryGraphStore    (按配置选择)
  │     └── AgentOrchestrator::with_all_agents() → 全部 6 个 Agent 注册
  │
  ├── 15+ CLI 子命令
  │     ├── init / do / search / index / recover / merge
  │     ├── analyze / auto / history / status
  │     ├── digest / batch / gate / review / verify
  │     ├── config / hook / graph
  │     ├── workflow (digest / coordinate / verify-tags / full-ci)
  │     ├── rollback (analyze / execute / history / reputation)
  │     └── verify-tag
  │
  └── aetherci::SemanticDiffPipeline (AetherCI)
        ├── IntentReasoner → LLM 推理 (带降级)
        ├── ImpactAnalyzer  → 图依赖分析
        └── ReportGenerator → Markdown + JSON 双格式
```

### 1.3 v0.4 要解决的真实问题

v0.3 实现了功能的"广度"，v0.4 需要解决功能的"深度"：

```
┌─────────────────────────────────────────────────────────────┐
│                      v0.4 核心命题                           │
│                                                             │
│   v0.3 交付了全部功能代码，但大量代码在"骨架"状态下工作。         │
│                                                             │
│   v0.4 要让每一条链路真正跑通、每一个 Agent 给出可靠输出、       │
│   每一个 LLM 调用都有缓存和降级保护、每一个测试都覆盖真实路径。   │
│                                                             │
│   四大方向：                                                  │
│   1. 稳定性硬化 —— 所有核心路径有集成测试                      │
│   2. 端到端验证 —— 6 Agent 协调工作流全链路跑通               │
│   3. 性能优化   —— 批量 + 缓存放量级大仓库可承受              │
│   4. DX 提升    —— 真实 LLM 接入手册 + CI secrets + AetherCI  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 1.4 v0.3 → v0.4 差距矩阵

| 维度 | v0.3 状态 | v0.4 目标 |
|------|----------|----------|
| 核心 Agent 测试 | 3 个核心 Agent 有单元测试，3 个新 Agent 测试缺失 | 6 个 Agent 全部有单元测试 + 集成测试 |
| LLM/Embedder 真实测试 | 仅 Mock 测试 | 真实 API Key 下通过 integration tests |
| 多 Agent 协调流 | `WorkflowOrchestrator` 存在但未 E2E 验证 | 多 Agent 冲突检测 + 协调建议真实可跑通 |
| AetherCI + LLM | `SemanticDiffPipeline` 未在 CLI 中注入真实 LLM | AetherCI 在 CLI analyze 中接入真实 LLM |
| Embedding 批量处理 | 仅 `embed_batch()` 单次调用 | 批量 + 并发 + 增量更新 |
| LLM 调用缓存 | `LLMFallbackConfig.cache_enabled` 已定义，未实现 | Embedding + Completion 双缓存 |
| Token 优化 | 无 | 长 diff 自动 truncate + summarization |
| Rollback Agent | 基本逻辑存在，`perform_git_revert` 为占位实现 | 真实 git revert 执行 + snapshot 保护 |
| Config 命令实现 | `ConfigAction` 枚举已定义 | `init/show/set/validate` 子命令完整实现 |
| README/文档 | 描述 Mock 状态 | 更新为真实 LLM 接入手册 |
| CI/CD secrets | 无 LLM 测试 | GitHub Actions 注入 secrets，跑真实 LLM integration tests |

---

## 2. Stage 1：核心路径稳定性硬化（P0）

> **目标**：让每一条核心链路有"可信赖的"测试覆盖，而非仅靠"代码看起来对"。

### 2.1 核心 Agent 测试补全

#### F-S1-1：MultiAgentCoordinatorAgent 单元测试

**优先级**：P0

**当前状态**：[agents/coordinator.rs](file:///d:/trae_projects/AetherVC/aether-core/src/agents/coordinator.rs) 只有实现代码，无 `#[cfg(test)]` 测试模块。

**需求**：新增以下测试用例：

```rust
// aether-core/src/agents/coordinator.rs 底部追加

#[cfg(test)]
mod tests {
    use super::*;
    // 测试用例：

    // 1. collect_agents: 模拟 Git log 返回多 Agent 提交，验证身份识别
    // 2. build_conflict_matrix: 两个 Agent 修改同一模块 → 生成 1 个热点
    // 3. build_conflict_matrix: 四个 Agent 修改同一模块 → Critical 严重程度
    // 4. build_conflict_matrix: 零冲突 → 空热点列表
    // 5. rule_based_coordination_plan: 规则驱动协调计划生成（无 LLM）
    // 6. parse_llm_coordination_plan: 解析带合并顺序的 LLM 输出
    // 7. execute (Agent trait): task_type = "coordinate_agents" → success
}
```

**验收标准**：
- `cargo test -p aether-core --lib agents::coordinator` 全部通过
- 覆盖率 >= 80%

#### F-S1-2：ValidationRiskAgent 单元测试

**优先级**：P0

**当前状态**：[agents/validation.rs](file:///d:/trae_projects/AetherVC/aether-core/src/agents/validation.rs) 只有实现代码，无测试模块。

**需求**：新增以下测试用例：

```rust
// aether-core/src/agents/validation.rs 底部追加

#[cfg(test)]
mod tests {
    // 1. calculate_risk_score: breaking + auth → 接近 1.0
    // 2. calculate_risk_score: documentation + docs/ → 接近 0.0
    // 3. sort_tags RiskAsc: 低风险在前
    // 4. sort_tags RiskDesc: 高风险在前
    // 5. resolve_tag_to_commit: 完整 hash → 直接返回
    // 6. resolve_tag_to_commit: 短 hash → 从 Git log 匹配
    // 7. execute (Agent trait): task_type = "validate_tag" → success
    // 8. execute: keyword filter → 仅返回匹配 Tag
}
```

**验收标准**：
- `cargo test -p aether-core --lib agents::validation` 全部通过
- 覆盖率 >= 80%

#### F-S1-3：RollbackAgent 单元测试

**优先级**：P0

**当前状态**：[agents/rollback.rs](file:///d:/trae_projects/AetherVC/aether-core/src/agents/rollback.rs) 只有实现代码，无测试模块。

**需求**：新增以下测试用例：

```rust
// aether-core/src/agents/rollback.rs 底部追加

#[cfg(test)]
mod tests {
    // 1. analyze_need_rollback: 编译失败 + auto_rollback_on_compile_failure=true → Revert
    // 2. analyze_need_rollback: 安全漏洞 + auto_rollback_on_security_cve=true → Revert
    // 3. analyze_need_rollback: 测试失败率 50% > 阈值 10% → Revert
    // 4. analyze_need_rollback: 测试失败率 5% < 阈值 10% → None
    // 5. analyze_need_rollback: config.enabled=false → None
    // 6. execute_rollback: require_approval=true + require_human_approval=true → PendingApproval
    // 7. update_reputation: Agent 信誉分更新
    // 8. execute (Agent trait): task_type = "rollback" → success
}
```

**验收标准**：
- `cargo test -p aether-core --lib agents::rollback` 全部通过
- 覆盖率 >= 80%

#### F-S1-4：WorkflowOrchestrator 单元测试

**优先级**：P0

**当前状态**：[workflow/mod.rs](file:///d:/trae_projects/AetherVC/aether-core/src/workflow/mod.rs) 只有实现代码，无测试模块。

**需求**：新增以下测试用例：

```rust
// aether-core/src/workflow/mod.rs 底部追加

#[cfg(test)]
mod tests {
    // 1. execute_digest_flow: 返回包含 markdown 和 risk_distribution 的结果
    // 2. execute_coordinate_flow: task_type = "coordinate_agents" 被正确路由
    // 3. execute_verify_tags_flow: task_type = "validate_tag" 被正确路由
    // 4. execute_full_ci_flow: 空 repo → "No commits to analyze"
}
```

### 2.2 LLM / Embedder 真实路径集成测试

#### F-S1-5：真实 LLM 集成测试（条件执行）

**优先级**：P0

**当前状态**：所有 LLM 测试都使用 Mock。没有测试验证真实 API 调用路径。

**需求**：创建条件性集成测试文件 `aether-core/tests/real_llm_tests.rs`：

```rust
// 通过环境变量控制是否运行真实 API 测试
// 未设置 DEEPSEEK_API_KEY → skip with warning
// 设置了 DEEPSEEK_API_KEY → 真实调用验证

#[tokio::test]
#[ignore = "requires DEEPSEEK_API_KEY"]
async fn test_real_deepseek_completion() {
    // 1. 验证 DeepSeekClient 可以完成一次 completion
    // 2. 验证返回内容非空
    // 3. 验证 token 消耗在合理范围
}

#[tokio::test]
#[ignore = "requires DEEPSEEK_API_KEY"]
async fn test_real_deepseek_chat() {
    // 1. chat() 方法正常工作
    // 2. 返回 content 非空
}

#[tokio::test]
#[ignore = "requires DEEPSEEK_API_KEY or OPENAI_API_KEY"]
async fn test_real_embedding() {
    // 1. OpenAIEmbedder 单条 embedding 成功
    // 2. 验证维度正确（text-embedding-3-small → 1536）
    // 3. embed_batch 批量 embedding 成功
}

#[tokio::test]
#[ignore = "requires DEEPSEEK_API_KEY or OPENAI_API_KEY"]
async fn test_llm_factory_from_config_or_env() {
    // 1. LLMConfig 填充真实 api_key → 创建非 Mock 客户端
    // 2. 验证 LLMFactory 三级回退逻辑
}

#[tokio::test]
async fn test_llm_factory_fallback_to_mock() {
    // 无环境变量 → 输出 MockLLMClient
    // 验证日志级别包含 "回退"
}

#[tokio::test]
#[ignore = "requires DEEPSEEK_API_KEY or OPENAI_API_KEY"]
async fn test_retry_on_failure() {
    // 1. 使用临时无效 API Key 触发失败
    // 2. 验证 RetryLLMClient 重试指定次数
    // 3. 验证最终返回错误
}
```

**设计原则**：
- 默认 `cargo test` 不运行真实 API 测试（不带 `--ignored` 时跳过）
- CI 中通过 `cargo test -- --ignored` 启用
- 每个测试开头检查环境变量，未设置则 skip 而非 fail
- 设置 timeout（30s）防止网络卡住

**CI 集成**：
```yaml
# .github/workflows/ci.yml 追加
- name: Real LLM Integration Tests
  if: github.event_name == 'push' && github.ref == 'refs/heads/main'
  env:
    DEEPSEEK_API_KEY: ${{ secrets.DEEPSEEK_API_KEY }}
  run: cargo test -- --ignored
```

#### F-S1-6：RetryLLMClient + RetryEmbedder 单元测试

**优先级**：P0

**当前状态**：`RetryLLMClient` 和 `RetryEmbedder` 有实现，但测试不足。

**需求**：

```rust
// aether-core/src/llm/client/retry.rs 测试补充
// 1. 第一次失败，第二次成功 → 最终成功
// 2. 全部失败 → 返回错误
// 3. 重试次数符合配置

// aether-core/src/semantic/embedder/retry.rs 测试补充
// 1. 同上逻辑
// 2. 验证维度信息在重试后保持正确
```

### 2.3 向量存储持久化测试

#### F-S1-7：PersistentVectorStore 正确性测试

**优先级**：P0

**当前状态**：`PersistentVectorStore` 已实现但无独立测试。

**需求**：

```rust
// aether-core/tests/persistent_storage_tests.rs

#[tokio::test]
async fn test_persistent_vector_store_crud() {
    // 1. store_commit → 文件写入
    // 2. search_similar → 正确返回（维度任意）
    // 3. delete → 文件删除
    // 4. 重启后数据仍存在（二次加载）
}

#[tokio::test]
async fn test_persistent_vector_store_dynamic_dimension() {
    // 1. 存储 1536 维向量 → 正确
    // 2. 存储 3072 维向量 → 正确
    // 3. 不同维度向量互不干扰
}

#[tokio::test]
async fn test_persistent_graph_store_crud() {
    // 1. 添加节点和边
    // 2. 查询关系
    // 3. 重启后数据仍存在
}
```

### 2.4 ConfigLoader 测试补全

#### F-S1-8：LLMConfig + RollbackConfig + CoordinatorConfig 测试

**优先级**：P0

**当前状态**：[config/loader.rs](file:///d:/trae_projects/AetherVC/aether-core/src/config/loader.rs) 的 `set_value` 方法中没有覆盖新配置项。

**需求**：在 `set_value` 中新增以下 key 支持：

```
"rollback.enabled"
"rollback.auto_rollback_on_compile_failure"
"rollback.auto_rollback_on_test_failure"
"rollback.test_failure_threshold"
"rollback.require_human_approval"
"coordinator.enabled"
"coordinator.hotspot_threshold"
"coordinator.monitor_window_minutes"
"llm.embedding_model"
"llm.max_retries"
```

**测试用例**：

```rust
// config/loader.rs tests 追加
// 1. set_value "rollback.enabled" → 正确保存
// 2. set_value "llm.embedding_model" → 正确保存
// 3. set_value "llm.max_retries" → 正确保存
// 4. set_value 未知 key → 返回错误
// 5. 设置后重新 load → 值正确
// 6. validate 检测高风险模块配置
```

---

## 3. Stage 2：端到端工作流验证（P0）

> **目标**：让"一个命令完成多 Agent 协调"真正可跑通，而非停在代码层面。

### 3.1 多 Agent 冲突检测 E2E 验证

#### F-S2-1：MultiAgentCoordinator 端到端场景

**优先级**：P0

**当前状态**：`MultiAgentCoordinatorAgent` 有完整的 `execute()`，但从未在真实多 Agent 场景下验证过。

**测试场景设计**：

```
场景：模拟 3 个 AI Agent 同时修改 auth/ 模块

准备：
  - 创建临时 Git 仓库
  - Agent-A (Cline): 在 auth/login.rs 修改 JWT 验证逻辑
  - Agent-B (Copilot): 在 auth/middleware.rs 重构认证中间件
  - Agent-C (Claude): 在 models/user.rs 新增字段
  - 触发生成语义索引

验证：
  1. collect_agents() 识别出 3 个 Agent
  2. build_conflict_matrix() 检测到 auth/ 是热点模块
  3. generate_coordination_plan() 生成合并顺序建议
  4. 输出包含 "auth/" 和 "Cline" "Copilot" 协调建议
```

**实现文件**：`aether-core/tests/e2e_multi_agent.rs`

#### F-S2-2：完整工作流串联测试

**优先级**：P0

**需求**：验证 WorkflowOrchestrator 的 4 种工作流在真实 Git 仓库上完整跑通。

```rust
// aether-core/tests/e2e_workflows.rs

#[tokio::test]
async fn test_workflow_digest_e2e() {
    // 1. 创建一个有 5 个 commit 的临时仓库
    // 2. 索引所有 commit
    // 3. 执行 execute_digest_flow()
    // 4. 验证返回 markdown 包含风险分布
}

#[tokio::test]
async fn test_workflow_coordinate_e2e() {
    // 1. 创建多个 Agent 同时修改同一模块的场景
    // 2. 执行 execute_coordinate_flow()
    // 3. 验证返回冲突矩阵和协调计划
}

#[tokio::test]
async fn test_workflow_verify_tags_e2e() {
    // 1. 创建带 Tag 的仓库
    // 2. 执行 execute_verify_tags_flow()
    // 3. 验证返回 Tag 风险评估列表
}

#[tokio::test]
async fn test_workflow_full_ci_e2e() {
    // 1. 创建仓库 + 1 个 commit
    // 2. 执行 execute_full_ci_flow()
    // 3. 验证分析 + 门控 + 验证 全部执行
}
```

### 3.2 ValidationRiskAgent E2E 验证

#### F-S2-3：Tag 验证端到端场景

**优先级**：P0

**当前状态**：`ValidationRiskAgent` 有 `execute()` 实现，但 `run_local_verification()` 返回的是 mock 结果。

**需求**：让 `run_local_verification()` 真正调用 `VerificationRunner`：

```rust
// agents/validation.rs 修改
async fn run_local_verification(
    &self,
    tag: &str,
    commit_hash: &str,
) -> Result<serde_json::Value> {
    // 实际调用 VerificationRunner
    let runner = VerificationRunner::new(self.context.verify_config.clone());
    let mode = VerifyMode::Smart;  // 智能模式：编译 + 受影响测试
    
    // checkout 到指定 commit
    // self.context.git_repo.checkout(commit_hash)?;
    
    let report = runner.run_all(None, mode)?;
    
    // 返回结构化结果
    Ok(serde_json::to_value(&report)?)
}
```

### 3.3 RollbackAgent 真实 Git 操作

#### F-S2-4：真实 git revert 执行

**优先级**：P0

**当前状态**：[agents/rollback.rs#L163-L169](file:///d:/trae_projects/AetherVC/aether-core/src/agents/rollback.rs) 的 `perform_git_revert()` 只返回模拟 hash，未做真实 git 操作。

**需求**：实现真实的 git revert：

```rust
async fn perform_git_revert(&self, commit_hash: &str) -> Result<Option<String>> {
    use git2::{Repository, RevertOptions};
    
    let repo = Repository::open(&self.context.git_repo.path())?;
    
    // 1. 解析 commit
    let oid = git2::Oid::from_str(commit_hash)?;
    let commit = repo.find_commit(oid)?;
    
    // 2. 创建 revert
    let mut opts = RevertOptions::new();
    opts.mainline(1);
    let reverted_index = repo.revert_commit(&commit, &repo.head()?, Some(&mut opts))?;
    
    // 3. 检查冲突
    if reverted_index.has_conflicts() {
        return Err(crate::utils::AetherError::AgentError(
            "Revert 存在冲突，需要手动解决".to_string()
        ));
    }
    
    // 4. 写入 tree
    let tree_id = reverted_index.write_tree_to(&repo)?;
    let tree = repo.find_tree(tree_id)?;
    
    // 5. 创建 revert commit
    let signature = repo.signature()?;
    let message = format!("Revert \"{}\"\n\nThis reverts commit {}.\n[Auto-reverted by AetherVC RollbackAgent]", 
        commit.message().unwrap_or(""), commit_hash);
    
    let new_commit = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &tree,
        &[&commit],
    )?;
    
    tracing::info!("[RollbackAgent] 已创建 revert commit: {}", new_commit);
    Ok(Some(new_commit.to_string()))
}
```

**具体任务**：
1. 在 `aether-core/Cargo.toml` 中确认 `git2` dependency 已包含（确认：已有）
2. 实现 `perform_git_revert()` 的真实 git2 逻辑
3. 实现 `perform_git_reset()` 的真实 git2 逻辑（带保护：先创建 stash/branch snapshot）
4. 头文件创建 snapshot 保护机制
5. 测试：在临时仓库中 commit → revert → 验证文件恢复

---

## 4. Stage 3：性能与成本优化（P1）

> **目标**：让 AetherVC 在大仓库（1000+ commits）上运行不超预算、不超时。

### 4.1 Embedding 批量处理与增量更新

#### F-S3-1：增量索引

**优先级**：P1

**当前状态**：`aether index` 每次全量重建索引，大仓库开销巨大。

**需求**：

```rust
// SemanticIndexer 新增方法
impl SemanticIndexer {
    /// 增量索引：仅索引新 commit
    pub async fn incremental_index(&self) -> Result<IndexReport> {
        // 1. 获取已索引的 commit hash 集合
        let indexed_hashes: HashSet<String> = self.vector_store
            .get_all_commit_hashes().await?;
        
        // 2. 获取所有远程 commit
        let all_commits = self.git_repo.list_commits().await?;
        
        // 3. 过滤出新 commit
        let new_commits: Vec<_> = all_commits.into_iter()
            .filter(|c| !indexed_hashes.contains(&c.id.0))
            .collect();
        
        // 4. 批量 embedding + 存储
        if new_commits.is_empty() {
            return Ok(IndexReport { indexed: 0, skipped: indexed_hashes.len() as u64, failed: 0 });
        }
        
        let texts: Vec<String> = new_commits.iter()
            .map(|c| format!("{}: {}", c.message, c.diff_summary))
            .collect();
        
        let embeddings = self.embedder.embed_batch(&texts).await?;
        
        let batch: Vec<_> = new_commits.iter().zip(embeddings.iter())
            .map(|(c, e)| (c.id.0.clone(), e.clone(), CommitMetadata::from(c)))
            .collect();
        
        self.vector_store.store_batch(batch).await?;
        
        Ok(IndexReport {
            indexed: new_commits.len() as u64,
            skipped: indexed_hashes.len() as u64,
            failed: 0,
        })
    }
}
```

**CLI 行为变更**：

```
# aether index       → 增量索引（默认行为）
# aether index --full → 全量重建（当维度/模型变更时使用）
```

#### F-S3-2：批量 Embedding 并发控制

**优先级**：P1

**需求**：大型仓库一次 embed 数千条文本会超 API 速率限制。

```rust
// 批量处理的并发控制
pub async fn embed_batch_with_throttle(
    &self,
    texts: &[String],
    batch_size: usize,       // 每批发送数量（默认 20）
    concurrency: usize,      // 并发批次数（默认 3）
) -> Result<Vec<Vec<f32>>> {
    use futures::stream::{self, StreamExt};
    
    let chunks: Vec<&[String]> = texts.chunks(batch_size).collect();
    
    let results = stream::iter(chunks)
        .map(|chunk| async {
            // 小批量调用 API
            self.embed_batch(&chunk.to_vec()).await
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;
    
    // 合并结果
    let mut embeddings = Vec::new();
    for result in results {
        embeddings.extend(result?);
    }
    Ok(embeddings)
}
```

### 4.2 LLM 调用缓存

#### F-S3-3：Completion 缓存

**优先级**：P1

**当前状态**：`LLMFallbackConfig.cache_enabled` 字段已定义，但缓存逻辑未实现。

**需求**：对 LLM completion 结果做语义相似度缓存。

```rust
// aether-core/src/llm/cache.rs（新文件）

pub struct LLMCache {
    store: PersistentVectorStore,   // 复用向量存储做语义匹配
    entries: Vec<CacheEntry>,
    ttl_hours: u32,
}

struct CacheEntry {
    prompt_hash: u64,
    prompt_embedding: Vec<f32>,
    response: String,
    created_at: DateTime<Utc>,
}

impl LLMCache {
    /// 查找语义相似的缓存结果
    /// 如果存在相似度 > 0.9 的缓存 → 直接返回
    /// 否则 → 标记为 miss
    pub async fn lookup(&self, prompt: &str) -> Option<String> {
        let prompt_embedding = self.embedder.embed(prompt).await.ok()?;
        
        let similar = self.store.search_similar(&prompt_embedding, 1, None).await.ok()?;
        
        if let Some(best) = similar.first() {
            if best.score > 0.9 {
                // TTL 检查
                if let Some(entry) = self.entries.iter().find(|e| e.prompt_hash == best.commit_hash.parse::<u64>().unwrap_or(0)) {
                    let age = Utc::now() - entry.created_at;
                    if age.num_hours() < self.ttl_hours as i64 {
                        return Some(entry.response.clone());
                    }
                }
            }
        }
        
        None
    }
    
    /// 存储缓存条目
    pub async fn store(&mut self, prompt: &str, response: &str) -> Result<()> {
        // ...
    }
}
```

#### F-S3-4：Embedding 缓存

**优先级**：P1

**需求**：同一段代码文本的 embedding 不应重复调用 API。简单 key-value 缓存：

```rust
// HashMap<text_hash, Vec<f32>>
// text_hash = blake3(text)
// 持久化到 .aether/cache/embeddings.json
```

### 4.3 Token 优化

#### F-S3-5：长 Diff 自动摘要

**优先级**：P1

**当前状态**：AetherCI intent.rs 中 diff 超过 4000 字符直接截断，可能丢失关键信息。

**需求**：对超长 diff 先做结构化摘要再发给 LLM：

```rust
// aetherci/src/pipeline/intent.rs

fn prepare_llm_input(diff: &str, max_tokens: usize) -> String {
    if diff.len() < max_tokens * 4 {  // 近似：1 token ≈ 4 chars
        return diff.to_string();
    }
    
    // Step 1: 提取文件列表 + 变更类型摘要
    let mut summary = String::new();
    summary.push_str(&format!("# 变更文件: {}个\n\n", count_files(diff)));
    
    // Step 2: 提取函数签名变更（关键信息密度最高）
    summary.push_str("## 函数签名变更:\n");
    for sig in extract_function_signatures(diff) {
        summary.push_str(&format!("- {}\n", sig));
    }
    
    // Step 3: 保留前 N 个文件的完整 diff，其余仅保留统计
    // ...
    
    summary
}
```

---

## 5. Stage 4：开发者体验与集成（P1）

> **目标**：让新用户 5 分钟内从 0 配置到真实 LLM 可用。

### 5.1 CLI config 子命令完整实现

#### F-S4-1：`aether config init`

**优先级**：P1

**当前状态**：`ConfigAction` 枚举已定义在 [main.rs](file:///d:/trae_projects/AetherVC/aether-cli/src/main.rs)，但 `cmd_config` 未完整实现。

**需求**：实现交互式初始化：

```
$ aether config init

? LLM Provider [deepseek]: deepseek
? API Key [${DEEPSEEK_API_KEY}]: ${DEEPSEEK_API_KEY}
? Model [deepseek-chat]: deepseek-chat
? Embedding Model [text-embedding-3-small]: 
? Storage Backend (memory/persistent) [persistent]: 
? Max Retries [3]: 

✓ 配置文件已生成: .aether/config.toml
✓ 配置验证通过
```

#### F-S4-2：`aether config show`

```
$ aether config show

[AetherVC Configuration]

LLM:
  Provider:          deepseek
  Model:             deepseek-chat
  API Key:           ${DEEPSEEK_API_KEY}  (set)
  Embedding Model:   text-embedding-3-small (1536维)
  Temperature:       0.3
  Max Retries:       3
  Fallback:          rules

Gate:
  Enabled:           true
  High Risk Modules: auth/, database/, payment/, core/
  Max Files/Commit:  10

Verify:
  Enabled:           true
  Compile Check:     true
  Lint Check:        true
  Unit Tests:        true

Rollback:
  Enabled:           true
  Compile Failure:   auto-rollback
  Test Failure:      notify-only

Coordinator:
  Enabled:           true
  Hotspot Threshold: 2 agents

Storage:
  Backend:           persistent
  Data Directory:    .aether
```

#### F-S4-3：`aether config set <key> <value>`

**优先级**：P0

**当前状态**：`set_value` 只支持部分 key，需要扩展到全部配置段。

**需求**：支持所有配置项的 set：

```bash
aether config set llm.provider        deepseek
aether config set llm.model           deepseek-chat
aether config set llm.api_key         sk-xxx
aether config set llm.temperature     0.3
aether config set llm.embedding_model text-embedding-3-small
aether config set llm.max_retries     3
aether config set gate.enabled        true
aether config set verify.enabled      true
aether config set rollback.enabled    true
aether config set rollback.auto_rollback_on_compile_failure true
aether config set rollback.test_failure_threshold 0.15
aether config set coordinator.hotspot_threshold 3
aether config set coordinator.monitor_window_minutes 60
aether config set storage.backend     persistent
aether config set storage.data_dir    .aether
```

#### F-S4-4：`aether config validate`

```
$ aether config validate

✓ LLM provider: deepseek
✓ Embedding model: text-embedding-3-small (1536 维)
⚠ API Key 未设置 —— 将使用 Mock 模式
  设置方法: aether config set llm.api_key ${DEEPSEEK_API_KEY}
✓ Gate thresholds: 合理
✓ Rollback config: 合理
⚠ 风险: auth/ 模块被标记为高风险，建议确认
```

### 5.2 CLI status 命令增强

#### F-S4-5：`aether status` 完善

**优先级**：P1

**当前状态**：`Commands::Status` 已定义，`cmd_status` 需完整实现。

**需求输出**：

```
$ aether status

AetherVC v0.4.0

Repository: /home/user/myproject
Branch: main
HEAD: abc1234  feat: add user profile API

───────────────────────────────────────────
Semantic Index
  Total Commits: 1,234
  Last Indexed:  2024-06-09 14:30:00
  Storage:       persistent (.aether/vectors/)
  Status:        ✅ healthy

LLM Integration
  Provider:      DeepSeek (deepseek-chat)
  Embedder:      OpenAI-compatible (text-embedding-3-small, 1536d)
  Status:        ✅ connected (last response: 1.2s ago)

Agents (6/6 ready)
  ✅ SemanticInterpreter     ✅ CrossCommitRecovery
  ✅ Merge                   ✅ MultiAgentCoordinator
  ✅ ValidationRisk          ✅ Rollback

Gate & Verify
  Gate Engine:   ✅ enabled (auto-pass: docs, tests, config)
  Verification:  ✅ enabled (compile + lint + test)

Workflows
  digest:        ✅ ready
  coordinate:    ✅ ready
  verify-tags:   ✅ ready
  full-ci:       ✅ ready

───────────────────────────────────────────
Health: 🟢 All systems operational
```

### 5.3 README 更新：真实 LLM 接入手册

#### F-S4-6：README 补充真实 LLM 接入章节

**优先级**：P1

**当前状态**：README 仍然描述 Mock 状态，未提及 LLMFactory 已接入。

**需求**：在 README 中新增"真实 LLM 配置"章节：

```markdown
## X. 真实 LLM 接入

AetherVC v0.4 已完整支持真实 LLM 和 Embedding API。
只需设置环境变量或配置文件即可从 Mock 模式切换到真实 AI 语义理解。

### 快速开始（3 步）

#### 1. 设置 API Key

```bash
# Windows PowerShell
$env:DEEPSEEK_API_KEY="sk-your-key-here"

# Linux/macOS
export DEEPSEEK_API_KEY="sk-your-key-here"
```

#### 2. 初始化配置

```bash
aether config init
```

#### 3. 验证

```bash
aether index          # 用真实 Embedding API 构建语义索引
aether search "认证"   # 语义搜索（非关键词匹配）
aether status         # 检查 LLM 连接状态
```

### 支持的 Provider

| Provider | LLM | Embedding | 配置 |
|----------|:---:|:---:|------|
| DeepSeek | ✅ | ✅ | 设置 `DEEPSEEK_API_KEY` |
| OpenAI   | ✅ | ✅ | 设置 `OPENAI_API_KEY` |
| OpenAI-Compatible | ✅ | ✅ | 设置 `OPENAI_API_KEY` + `api_base` |
| Mock     | ✅ | ✅ | 无需配置（回退模式） |

### 降级策略

当 LLM/Embedder 不可用时，AetherVC 自动降级：

1. API 调用 → 3 次重试（指数退避）
2. 仍然失败 → RuleBasedAnalyzer（规则驱动语义分析）
3. 所有输出标记 `⚠ LLM unavailable, using rules`
4. 语义搜索降级为关键词匹配
```

### 5.4 GitHub Actions CI 真实 LLM 测试

#### F-S4-7：CI secrets 注入

**优先级**：P1

**需求**：在 `.github/workflows/ci.yml` 中新增 job：

```yaml
# .github/workflows/ci.yml

jobs:
  real-llm-integration:
    name: Real LLM Integration Tests
    runs-on: ubuntu-latest
    # 仅在 main 分支 push / PR 合并时运行
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    env:
      DEEPSEEK_API_KEY: ${{ secrets.DEEPSEEK_API_KEY }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run real LLM tests
        run: cargo test -p aether-core --test real_llm_tests -- --ignored
        timeout-minutes: 10
```

---

## 6. Stage 5：AetherCI 真实 LLM 接入（P1）

> **目标**：`aether analyze` 命令当前创建 Pipeline 时未注入 LLM 客户端，导致意图推理全部走规则降级。需要让 AetherCI 也能使用真实 LLM。

### 6.1 问题分析

**当前调用链**（`aether analyze` → `cmd_analyze`）：

```
aether analyze HEAD~1..HEAD
  → cmd_analyze(commit_range, json, quick, repo_path)
  → SemanticDiffPipeline::default_pipeline()  ← 无 LLM！
  → intent_reasoner.reason() → 全部走 rule_based 降级
```

**问题**：[main.rs](file:///d:/trae_projects/AetherVC/aether-cli/src/main.rs) 中 `cmd_analyze` 创建 Pipeline 时未传入 LLM 客户端。

### 6.2 需求

#### F-S5-1：AetherCI Pipeline 接入真实 LLM

**优先级**：P1

**修改**：`cmd_analyze` 函数中创建 Pipeline 时传入 LLM 客户端：

```rust
// aether-cli/src/main.rs cmd_analyze 函数修改

async fn cmd_analyze(
    commit_range: &str,
    json: bool,
    quick: bool,
    repo_path: &str,
) -> anyhow::Result<()> {
    let (orchestrator, indexer, context_manager, config) = create_app_context(repo_path)?;

    // 从 AgentContext 中获取 LLM 客户端
    let agent_context = orchestrator.get_context();  // 需要添加 getter

    // 创建 Pipeline（传入真实 LLM）
    let pipeline = if quick {
        SemanticDiffPipeline::default_pipeline()  // 快速模式：跳过 LLM
    } else {
        SemanticDiffPipeline::new(
            agent_context.llm_client.clone(),   // ← 注入真实 LLM！
            agent_context.graph_store.clone(),
        )
    };

    // ... 解析 commit_range, 执行 pipeline ...
}
```

**附带修改**：
1. `AgentOrchestrator` 新增 `get_context()` getter 方法
2. `cmd_analyze` 重构为接收已初始化的 context

#### F-S5-2：AetherCI Pipeline 输出增强

**优先级**：P1

**需求**：在 AetherCI 输出中标记是否使用了真实 LLM：

```markdown
# Commit Intelligence Report
...
**分析模式**: deepseek-chat (真实 LLM)
**置信度**: 0.87 (LLM 推理)
...
```

vs

```markdown
...
**分析模式**: ⚠ rules (降级推理)
**置信度**: 0.45 (规则推理)
...
```

---

## 7. 实现路线图

### Stage 1：核心路径稳定性硬化（P0，3-4 天）

| 任务 | 文件 | 优先级 | 工作量 |
|------|------|--------|--------|
| F-S1-1: MultiAgentCoordinator 单元测试 | `agents/coordinator.rs` | P0 | 中 |
| F-S1-2: ValidationRisk 单元测试 | `agents/validation.rs` | P0 | 中 |
| F-S1-3: Rollback 单元测试 | `agents/rollback.rs` | P0 | 中 |
| F-S1-4: WorkflowOrchestrator 单元测试 | `workflow/mod.rs` | P0 | 小 |
| F-S1-5: 真实 LLM 集成测试（条件执行） | `tests/real_llm_tests.rs`（新增） | P0 | 大 |
| F-S1-6: RetryLLMClient/RetryEmbedder 测试 | `client/retry.rs`, `embedder/retry.rs` | P0 | 小 |
| F-S1-7: PersistentVectorStore 正确性测试 | `tests/persistent_storage_tests.rs`（新增） | P0 | 中 |
| F-S1-8: ConfigLoader set_value 补全 + 测试 | `config/loader.rs` | P0 | 小 |

### Stage 2：端到端工作流验证（P0，3-4 天）

| 任务 | 文件 | 优先级 | 工作量 |
|------|------|--------|--------|
| F-S2-1: 多 Agent 冲突检测 E2E | `tests/e2e_multi_agent.rs`（新增） | P0 | 大 |
| F-S2-2: 4 种工作流 E2E 测试 | `tests/e2e_workflows.rs`（新增） | P0 | 大 |
| F-S2-3: run_local_verification 真实化 | `agents/validation.rs` | P0 | 中 |
| F-S2-4: perform_git_revert 真实 git2 操作 | `agents/rollback.rs` | P0 | 中 |

### Stage 3：性能与成本优化（P1，2-3 天）

| 任务 | 文件 | 优先级 | 工作量 |
|------|------|--------|--------|
| F-S3-1: 增量索引 + `--full` 标志 | `semantic/indexer.rs`, `cli/main.rs` | P1 | 中 |
| F-S3-2: 批量 Embedding 并发控制 | `embedder/openai.rs` | P1 | 小 |
| F-S3-3: Completion 语义缓存 | `llm/cache.rs`（新增） | P1 | 大 |
| F-S3-4: Embedding hash 缓存 | `embedder/cache.rs`（新增） | P1 | 中 |
| F-S3-5: 长 diff 自动摘要 | `aetherci/pipeline/intent.rs` | P1 | 中 |

### Stage 4：开发者体验与集成（P1，2-3 天）

| 任务 | 文件 | 优先级 | 工作量 |
|------|------|--------|--------|
| F-S4-1: `aether config init` 交互式 | `cli/main.rs` | P1 | 中 |
| F-S4-2: `aether config show` 格式化输出 | `cli/main.rs` | P1 | 小 |
| F-S4-3: `aether config set` 全部 key | `config/loader.rs` | P0 | 小 |
| F-S4-4: `aether config validate` 完善 | `config/loader.rs` | P1 | 小 |
| F-S4-5: `aether status` 完整实现 | `cli/main.rs` | P1 | 中 |
| F-S4-6: README 真实 LLM 接入手册 | `README.md` | P1 | 小 |
| F-S4-7: CI secrets 注入 | `.github/workflows/ci.yml` | P1 | 小 |

### Stage 5：AetherCI 真实 LLM 接入（P1，1-2 天）

| 任务 | 文件 | 优先级 | 工作量 |
|------|------|--------|--------|
| F-S5-1: cmd_analyze 注入真实 LLM | `cli/main.rs` | P1 | 小 |
| F-S5-2: AetherCI 输出标记分析模式 | `aetherci/pipeline/report.rs` | P1 | 小 |

---

## 8. 验收标准

### 8.1 Stage 1 验收

- [ ] `cargo test -p aether-core --lib` 全部通过，6 个 Agent 模块覆盖率 >= 80%
- [ ] `cargo test -p aether-core`（含集成测试）全部通过
- [ ] 设置 `DEEPSEEK_API_KEY` 后 `cargo test -p aether-core -- --ignored` 全部通过
- [ ] `PersistentVectorStore` 支持任意维度向量的 CRUD 操作
- [ ] `ConfigLoader::set_value` 支持所有 20+ 配置键

### 8.2 Stage 2 验收

- [ ] 在包含 3 个 Agent 并行修改的模拟仓库中，`aether workflow coordinate` 输出完整冲突矩阵和协调建议
- [ ] `aether workflow digest` 输出包含风险分布、高风险列表的 Markdown 报告
- [ ] `aether verify-tag` 能 checkout tag 并运行真实验证
- [ ] `aether rollback execute <commit> --method revert` 能创建 revert commit（真实 git 操作）

### 8.3 Stage 3 验收

- [ ] 1000+ commit 仓库增量索引 < 全量索引的 10%
- [ ] API 调用频率在速率限制内（通过并发控制验证）
- [ ] 相同/相似 prompt 第二次调用命中缓存（响应时间 < 50ms）
- [ ] 超长 diff（> 10000 chars）被正确摘要后发给 LLM

### 8.4 Stage 4 验收

- [ ] `aether config init` 可交互式生成完整配置文件
- [ ] `aether config show` 输出所有配置项的当前值
- [ ] `aether status` 输出 6 Agent 状态、LLM 连接状态、存储状态
- [ ] README 包含真实 LLM 快速配置 3 步指南
- [ ] GitHub Actions main 分支 push 后自动跑真实 LLM integration tests

### 8.5 Stage 5 验收

- [ ] `aether analyze HEAD~1..HEAD`（非 `--quick`）使用真实 LLM 推理意图
- [ ] AetherCI 报告顶部显示分析模式和置信度来源
- [ ] `aether analyze --quick` 仍走规则推理（无 LLM 调用）

### 8.6 v0.4 最终能力矩阵

| 能力 | v0.3 状态 | v0.4 目标 |
|------|----------|----------|
| 真实 LLM 接入 | 代码就绪，CLI 已连接 | CLI + AetherCI 双路径接通 |
| 6 大 Agent | 全部有实现 | 全部有实现 + 测试（覆盖率 >= 80%） |
| 多 Agent 协调 | WorkflowOrchestrator 存在 | E2E 验证通过 |
| 语义搜索 | 可用 | 可用（真实 Embedder 默认） |
| Tag 验证 | 基本逻辑 | checkout + 真实编译/测试 |
| 智能回滚 | 占位 git 操作 | 真实 git revert + snapshot 保护 |
| 存储 | 持久化实现 | 持久化实现 + CRUD + 任意维度测试 |
| 缓存 | 配置字段已定义 | Completion + Embedding 双缓存 |
| 增量索引 | 无 | 支持 `aether index` 默认增量 |
| CI/CD | 仅 build + test (Mock) | + secrets 注入，真实 LLM 测试 |
| 文档 | Mock 状态描述 | 真实 LLM 接入手册 |

---

## 附录 A：文件变更清单（估计）

### 修改文件（~15 个）

```
aether-core/src/agents/coordinator.rs       # + 单元测试模块
aether-core/src/agents/validation.rs        # + 单元测试模块 + run_local_verification 真实化
aether-core/src/agents/rollback.rs          # + 单元测试模块 + perform_git_revert 真实化
aether-core/src/agents/orchestrator.rs      # + get_context() getter
aether-core/src/config/loader.rs            # + set_value 补全所有新 key
aether-core/src/semantic/indexer.rs         # + incremental_index
aether-core/src/semantic/embedder/openai.rs # + embed_batch_with_throttle
aether-core/src/workflow/mod.rs             # + 单元测试模块
aether-core/src/llm/client/retry.rs         # + 单元测试补全
aether-core/src/semantic/embedder/retry.rs  # + 单元测试补全
aether-cli/src/main.rs                      # cmd_analyze LLM 注入, cmd_config/show/status 完善
aetherci/src/pipeline/intent.rs             # + prepare_llm_input (长 diff 摘要)
aetherci/src/pipeline/report.rs             # + 分析模式标记
README.md                                    # + 真实 LLM 接入章节
.github/workflows/ci.yml                     # + 真实 LLM integration test job
```

### 新增文件（~6 个）

```
aether-core/tests/real_llm_tests.rs         # 条件执行的真实 API 集成测试
aether-core/tests/persistent_storage_tests.rs # PersistentVectorStore/GraphStore CRUD 测试
aether-core/tests/e2e_multi_agent.rs         # 多 Agent 冲突检测 E2E
aether-core/tests/e2e_workflows.rs           # 4 种工作流 E2E
aether-core/src/llm/cache.rs                 # Completion 语义缓存
aether-core/src/semantic/embedder/cache.rs   # Embedding hash 缓存
```

---

## 附录 B：资源配置参考

### LLM API 成本预估（以 DeepSeek 为例）

| 操作 | 单次 Token | 频率（中型仓库） | 日成本估算 |
|------|-----------|------------------|-----------|
| 意图推理 | ~2000 tokens | 20 commits/天 | ¥0.20 |
| 协调计划 | ~1500 tokens | 5 次/天 | ¥0.04 |
| 语义搜索 | ~100 tokens | 50 次/天 | ¥0.03 |
| Embedding (1536d) | ~0.5K tokens/条 | 500 条/天 | ¥0.50 |

**总计**：约 ¥0.77/天（启用缓存后降低 40-60%）

### 存储空间

| 存储类型 | 条目大小 | 1000 commits | 10000 commits |
|----------|---------|-------------|--------------|
| 向量存储 (1536d) | ~6KB/条 | ~6MB | ~60MB |
| 图存储 | ~0.5KB/条 | ~0.5MB | ~5MB |
| 配置文件 | ~1KB | ~1KB | ~1KB |
| 缓存 | ~2KB/条 | ~2MB | ~20MB |

---

## 附录 C：v0.3 vs v0.4 对比总结

| 维度 | v0.3（蓝图） | v0.4（硬化） |
|------|-------------|-------------|
| **阶段定位** | 功能实现 | 质量 + 可靠性 |
| **LLM 接入** | 写代码 | 写测试 + 验证 |
| **3 Agent** | 写代码 | 写测试 + E2E 验证 + 真实 git 操作 |
| **工作流** | 写代码 | E2E 验证 |
| **缓存** | 配置占位字段 | 实现 + 测试 |
| **AetherCI** | 独立模块 | 接入 CLI 真实 LLM 通路 |
| **文档** | PRD 文档 | README 用户手册 |
| **CI/CD** | build + test (Mock) | + 真实 LLM integration test |
| **验收** | 代码编译通过 | 全链路可跑通 |
