# AetherVC 产品需求文档 (PRD) v0.2

> **AI-Native 语义版本控制系统 —— 让 AI 生成代码的变更速度对人类重新可控**

---

## 目录

1. [产品概述与问题定义](#1-产品概述与问题定义)
2. [目标用户与场景](#2-目标用户与场景)
3. [现状分析与差距评估](#3-现状分析与差距评估)
4. [核心功能需求](#4-核心功能需求)
5. [技术架构](#5-技术架构)
6. [核心数据模型](#6-核心数据模型)
7. [API 与 CLI 设计](#7-api-与-cli-设计)
8. [实现路线图](#8-实现路线图)
9. [成功指标](#9-成功指标)
10. [风险与缓解](#10-风险与缓解)

---

## 1. 产品概述与问题定义

### 1.1 产品愿景

AetherVC 是一个构建在 Git 之上的语义智能层，专为 **AI Coding 时代** 设计。它不仅记录"代码改了什么"，更理解"为什么改"、"影响有多大"、"是否需要人类介入"——让人类在 AI 大规模辅助编码的场景中保持对代码库的**理解能力和控制力**。

### 1.2 核心问题

```
┌─────────────────────────────────────────────────────────────────┐
│                     AI Coding 时代的版本管理困境                    │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   AI Agent 提交速度       人类理解速度                             │
│   ────────────────       ────────────────                        │
│   每小时 5-20 个 commit   每小时能认真 review 2-3 个 commit         │
│                                                                 │
│                    ↓  速度差 5-10 倍  ↓                            │
│                                                                 │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │  后果：                                                   │   │
│   │  1. 人类无法跟上 AI 的变更节奏，逐渐失去对代码库的理解       │   │
│   │  2. 大量低质量/冲突的代码被合并，债务累积                    │   │
│   │  3. 缺乏快速验证手段，回滚成本高                            │   │
│   │  4. AI 在不同分支的并行修改导致合并冲突爆炸                  │   │
│   │  5. 整体开发成本因重复劳动和修复而上升                       │   │
│   └─────────────────────────────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 1.3 产品定位

AetherVC 不是替代 Git，而是在 Git 之上增加三层能力：

| 层 | 能力 | 解决的问题 |
|----|------|-----------|
| **理解层** | 语义分析、意图推理、影响评估 | AI 改了太多，人类看不懂 |
| **管控层** | 审核门控、变更预算、风险阈值 | AI 改得太快，人类跟不上 |
| **验证层** | 自动测试编排、验证报告、回滚辅助 | 不敢信任 AI 的变更质量 |

---

## 2. 目标用户与场景

### 2.1 目标用户画像

| 角色 | 描述 | 核心诉求 |
|------|------|---------|
| **技术负责人/Tech Lead** | 管理多个 AI Agent 并行开发的团队 | 掌控全局变更态势，把控质量门禁 |
| **个人开发者 + AI** | 用 Cursor/Copilot/Claude Code 等快速迭代 | 理解 AI 改了什么，快速验证是否正确 |
| **QA/测试工程师** | 需要验证 AI 生成代码的质量 | 知道哪些变更需要重点测试，哪些可以跳过 |
| **DevOps/SRE** | 管理 CI/CD 流水线 | 将 AetherVC 分析集成到自动化流程中 |

### 2.2 核心使用场景

#### 场景 A：AI 批量提交后的"消化"

> 早上让 AI 修复了 3 个 bug，重构了用户模块，新增了一个 API 端点。
> AI 在 2 小时内产生了 15 个 commit。
> 
> **需求**：我需要一份**聚合摘要**告诉我 AI 今天做了什么，哪些是安全的，
> 哪些需要我亲自 review，而不是 15 份独立的 diff 分析报告。

#### 场景 B：多 Agent 并行开发的冲突感知

> 3 个 AI Agent 分别在 3 个分支上开发：用户画像、登录修复、数据库重构。
> 
> **需求**：在合并前自动检测它们之间的冲突（不只是 git conflict，
> 还包括语义冲突——改了同一个函数的不同部分），给出合并顺序建议。

#### 场景 C：变更质量门禁

> 团队规定：任何涉及 `auth/` 或 `database/` 模块的变更必须人工审核。
> AI 修改了 `src/api/` 下的接口，是否安全？
> 
> **需求**：根据预设策略自动判断，低风险变更自动放行，
> 高风险变更自动加入审核队列并发送通知。

#### 场景 D：快速验证与回滚

> AI 提交了 5 个 commit，声称"修复了登录 bug"。
> 但我不确定是否引入了新问题。
> 
> **需求**：自动对这批变更运行相关测试套件，生成验证报告。
> 如果失败，能快速定位到具体哪个 commit 引入的问题。

---

## 3. 现状分析与差距评估

### 3.1 AetherVC 当前已交付能力

| 模块 | 能力 | 状态 |
|------|------|------|
| `aether-core` | 领域模型（Commit, SemanticInfo, Recovery, Merge, Agent） | ✅ 已实现 |
| `aether-core` | Git 操作封装（基于 git2） | ✅ 已实现 |
| `aether-core` | 内存向量存储 + 语义搜索 | ✅ 已实现 |
| `aether-core` | 内存图存储（邻接表） | ✅ 已实现 |
| `aether-core` | 规则驱动语义分析器（RuleBasedAnalyzer） | ✅ 已实现 |
| `aether-core` | Mock 嵌入器 + Mock LLM 客户端 | ✅ 已实现 |
| `aether-core` | Prompt 模板管理器（5 个模板） | ✅ 已实现 |
| `aether-core` | 3 个 Agent：SemanticInterpreter, Recovery, Merge | ✅ 已实现 |
| `aether-core` | Agent 编排器 | ✅ 已实现 |
| `aether-core` | NL 指令解析器（规则驱动）+ 对话上下文管理 | ✅ 已实现 |
| `aetherci` | 五阶段语义 Diff 流水线（预处理器→分类→意图→影响→报告） | ✅ 已实现 |
| `aetherci` | Markdown 报告生成 | ✅ 已实现 |
| `aetherci` | 快速分析模式（跳过 LLM） | ✅ 已实现 |
| `aether-cli` | CLI 命令：init/do/search/index/recover/merge/analyze/auto/history | ✅ 已实现 |
| `aether-api` | REST API | ❌ 仅占位 |
| `aether-web` | Web Dashboard | ❌ 仅占位 |

### 3.2 差距矩阵

| 需求 | 当前能力 | 差距 |
|------|---------|------|
| 批量 commit 聚合摘要 | `aether analyze` 仅支持单 commit range | 缺少 **Digest Engine** |
| 按风险排序展示 | 单报告中有风险等级，但无跨 commit 排序 | 缺少批量风险排序 |
| 人工审核门控 | 无 | 缺少 **Review Gate** |
| 变更预算/阈值告警 | 无 | 缺少 **Diff Budget** |
| 自动验证集成 | 无 | 缺少 **Verification Hook** |
| AI 工具无缝集成 | 仅有 Prompt 模板文档 | 缺少 Git Hook / MCP Server |
| Web 可视化 | 无 | Web Dashboard 空白 |
| 真实 LLM 语义理解 | 全 Mock 实现 | 需接入真实 API |

---

## 4. 核心功能需求

### 4.1 变更摘要引擎 (Digest Engine) — 优先级 P0

#### 4.1.1 概述

将一段时间内的多个 commit **聚合为一份人类可读的摘要报告**，而非 N 份独立的 commit 分析。

#### 4.1.2 功能详述

**F-1.1 时间窗口聚合**

按时间范围聚合 commit 并生成摘要：

```
输入：时间范围（如 "last 2 hours", "since 2024-06-01", "HEAD~50..HEAD"）
输出：一份聚合摘要报告
```

摘要内容：

```
┌────────────────────────────────────────────┐
│  AI 变更摘要  |  2024-06-01 10:00 - 12:00  │
│  窗口内 commit 数：23                       │
│  涉及 Agent：3（Cline, Copilot, Claude）     │
├────────────────────────────────────────────┤
│                                            │
│  一句话总结：                                │
│  "主要完成了用户画像模块重构、修复了 3 个       │
│  登录相关 bug、新增了报表 API 端点。"          │
│                                            │
│  ┌─ 变更主题分布 ─────────────────────────┐ │
│  │ ████████████ 重构         (45%) 10c    │ │
│  │ ██████ Bug 修复           (26%)  6c    │ │
│  │ ███ 新功能                (13%)  3c    │ │
│  │ ██ 性能优化               (9%)   2c    │ │
│  │ █ 文档                    (4%)   1c    │ │
│  │ ▓ 测试                    (4%)   1c    │ │
│  └────────────────────────────────────────┘ │
│                                            │
│  ⚠ 需要关注的变更：                          │
│  • abc1234 (高风险) - 数据库 schema 变更      │
│  • def5678 (高风险) - 认证中间件重构           │
│  • ghi9012 (中风险) - 用户模型字段新增         │
│                                            │
│  ✅ 安全的变更（12个）：                      │
│  • 文档更新、格式化、测试补充、注释修正等        │
│                                            │
│  影响模块热力图：                            │
│  auth/    ████████ (8 次变更)               │
│  models/  ██████   (6 次变更)               │
│  api/     ████     (4 次变更)               │
│  tests/   ██       (2 次变更)               │
│                                            │
└────────────────────────────────────────────┘
```

**F-1.2 按 Agent/Author 分组**

```bash
aether digest --by-agent    # 按 AI Agent 分组：Cline 做了什么，Copilot 做了什么
aether digest --by-module   # 按代码模块分组
```

**F-1.3 变更主题自动聚类**

使用 LLM + 向量聚类，将语义相近的 commit 自动归为一个"主题"：

```
主题 1: "用户认证流程修复" — 涉及 commit: abc1, def2, ghi3
主题 2: "数据库迁移准备" — 涉及 commit: jkl4, mno5
主题 3: "API 文档补全" — 涉及 commit: pqr6
```

**F-1.4 对比摘要**

```bash
aether digest --compare v1.0..v1.1
```

输出两个版本之间的"变更简报"，适合 release note 生成。

#### 4.1.3 输入/输出

| 项目 | 说明 |
|------|------|
| 输入 | 时间窗口、commit 范围、分组维度、风险阈值 |
| 输出 | Markdown/JSON 聚合摘要报告 |
| 输出位置 | 终端输出 / `.aether/reports/` 目录 |

---

### 4.2 人工审核门控 (Review Gate) — 优先级 P0

#### 4.2.1 概述

根据团队配置的策略，对 AI 产生的 commit **自动分级**，高风险变更自动进入审核队列，低风险变更自动放行。

#### 4.2.2 功能详述

**F-2.1 审核策略配置**

在项目根目录的 `.aether/config.toml` 中配置：

```toml
[gate]
# 全局开关
enabled = true
# 自动通过的变更类型
auto_pass = ["documentation", "test", "formatting"]
# 必须人工审核的变更类型
require_review = ["breaking", "security", "database_schema"]

[gate.modules]
# 高风险模块：任何涉及这些模块的变更都需要审核
high_risk = ["auth/", "database/", "payment/", "core/"]
# 低风险模块：自动通过
low_risk = ["docs/", "tests/", "assets/", "examples/"]

[gate.thresholds]
# 变更规模阈值：超过则触发审核
max_files_changed = 10       # 单个 commit 变更文件数上限
max_lines_added = 500        # 新增行数上限
max_lines_deleted = 200      # 删除行数上限
# 时间窗口限制
max_commits_per_hour = 10    # 每小时 AI 提交上限

[gate.actions]
# 超阈值动作
on_threshold_exceeded = "block"   # block | warn | queue
on_critical_risk = "block"        # 严重风险：阻止
on_high_risk = "queue"            # 高风险：加入审核队列
```

**F-2.2 审核决策流程**

```
AI 提交 commit
     │
     ▼
┌─────────────────┐
│ AetherCI 分析    │ → 变更类型、风险等级、影响范围
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Review Gate 决策  │
│                  │
│ 检查清单：        │
│ □ 变更类型？     │
│ □ 涉及模块？     │
│ □ 变更规模？     │
│ □ 窗口频率？     │
│ □ 作者信任度？   │
└────────┬────────┘
         │
    ┌────┼────┐
    ▼    ▼    ▼
 自动通过  加入队列  阻止
 (绿标)   (黄标)   (红标)
```

**F-2.3 审核队列**

审核队列是一个可查询、可操作的结构化列表：

```bash
# 查看待审核队列
aether review queue

# 输出：
#   ID        Commit    风险    模块        作者      时间
#  ═══════════════════════════════════════════════════════
#  🔴 Q001  abc1234  严重要   auth/       Cline     10:23
#  🟡 Q002  def5678  高      database/   Claude    10:45
#  🟡 Q003  ghi9012  中      models/     Copilot   11:02

# 审核一个变更
aether review approve Q001 --comment "Auth 重构正确，但需要补充测试"

# 拒绝并要求修改
aether review reject Q002 --reason "schema 变更需要先跑 migration 测试"

# 批量操作
aether review approve --all-low-risk   # 批准所有低风险项
# 查看审核历史
aether review history --since "7 days ago"
```

**F-2.4 通知集成**

```bash
# 当审核队列有新项时：
# - 终端提示（默认）
# - Slack/Discord Webhook
# - GitHub PR 评论
# - 系统通知

aether gate notify --channel slack --webhook-url "https://..."
```

#### 4.2.3 数据模型

```rust
struct ReviewPolicy {
    enabled: bool,
    auto_pass_types: Vec<ChangeIntent>,     // 自动通过的类型
    require_review_types: Vec<ChangeIntent>, // 需要审核的类型
    high_risk_modules: Vec<String>,          // 高风险模块路径
    low_risk_modules: Vec<String>,           // 低风险模块路径
    thresholds: GateThresholds,
    on_threshold_exceeded: GateAction,
}

struct GateThresholds {
    max_files_changed: u32,
    max_lines_added: u32,
    max_lines_deleted: u32,
    max_commits_per_hour: u32,
}

enum GateAction { Block, Warn, Queue }

struct ReviewItem {
    id: String,
    commit_hash: String,
    risk_level: RiskLevel,
    reason: String,             // 触发审核的原因
    status: ReviewStatus,       // pending / approved / rejected
    reviewer: Option<String>,
    review_comment: Option<String>,
    created_at: DateTime<Utc>,
    resolved_at: Option<DateTime<Utc>>,
}

enum ReviewStatus { Pending, Approved, Rejected, Skipped }
```

---

### 4.3 快速验证集成 (Verification Hook) — 优先级 P0

#### 4.3.1 概述

当 AI 产生新 commit 时，自动运行预设的验证流水线，快速判断变更是否"安全"。

#### 4.3.2 功能详述

**F-3.1 验证配置**

```toml
# .aether/config.toml
[verify]
enabled = true

# 基础验证（变更后自动运行）
[verify.basic]
compile_check = true        # 编译检查
lint_check = true           # Lint 检查
format_check = false        # 格式检查（可选）

# 测试验证（变更后自动运行）
[verify.testing]
run_unit_tests = true       # 单元测试
test_command = "cargo test"
run_affected_tests = true   # 只跑受影响的测试（更快）
coverage_threshold = 0.8    # 覆盖率阈值

# 高级验证（仅高风险变更）
[verify.advanced]
static_analysis = true      # 静态分析
security_scan = true        # 安全扫描
dependency_check = true     # 依赖检查
```

**F-3.2 验证流程**

```
AI 提交 commit
     │
     ▼
┌──────────────┐
│ AetherCI 分析  │
└──────┬───────┘
       │
       ▼
┌──────────────────────────┐
│ 确定验证策略              │
│ 低风险 → 基础验证          │
│ 中风险 → 基础 + 测试       │
│ 高风险 → 完整验证流水线    │
└──────┬───────────────────┘
       │
       ▼
┌──────────────────────────┐
│ 执行验证                  │
│ 1. cargo build --check   │
│ 2. cargo clippy          │
│ 3. cargo test            │
│ 4. cargo audit           │
└──────┬───────────────────┘
       │
       ▼
┌──────────────────────────┐
│ 生成验证报告              │
│                          │
│ ✅ passed: 4/5 checks    │
│ ❌ failed: 1/5 checks    │
│    → test_user_login     │
│      预期：200           │
│      实际：500           │
│      文件：auth/login.rs  │
│      建议：检查 JWT 密钥配置│
└──────────────────────────┘
```

**F-3.3 智能测试选择**

不使用全量测试套件，而是基于变更影响范围**只运行相关测试**：

```bash
aether verify --smart     # 智能选择：分析 diff 影响的函数，
                           # 只运行引用这些函数的测试
aether verify --full      # 全量验证（用于高风险关键变更）
aether verify --quick     # 仅编译 + lint
```

**F-3.4 验证历史与趋势**

```bash
aether verify history --since "7 days ago"

# 输出：
#  日期        通过率   耗时    测试数
#  06-05      95%      45s     142
#  06-04      88%      52s     138
#  06-03      100%     38s     145
#  06-02      72% ⚠    63s     150   ← 异常日，建议回顾
```

#### 4.3.3 CLI 命令

```bash
aether verify HEAD                    # 验证最新 commit
aether verify HEAD~5..HEAD           # 验证范围
aether verify --watch                 # 监控模式，自动验证新 commit
aether verify --report               # 生成验证报告到 .aether/reports/
```

---

### 4.4 批量分析与风险排序 — 优先级 P1

#### 4.4.1 概述

扩展现有的 `aether analyze` 命令，支持批量 commit 分析和跨 commit 风险排序。

#### 4.4.2 CLI 设计

```bash
# 批量分析最近 20 个 commit
aether batch HEAD~20..HEAD

# 时间窗口批量分析
aether batch --since "2 hours ago"
aether batch --since "2024-06-01T00:00:00" --until "2024-06-01T12:00:00"

# 按风险排序输出
aether batch --risk-sort --limit 10

# 输出示例：
#  🔴 abc1234  CRITICAL  database/  schema migration     Cline      10:23
#  🔴 def5678  CRITICAL  auth/      认证中间件重构         Claude     10:45
#  🟡 ghi9012  HIGH      models/    User 字段新增         Cline      11:02
#  🟡 jkl3456  HIGH      api/       新增 /report 端点     Copilot    11:15
#  🟢 mno7890  LOW       tests/     补充登录测试          Cline      11:30
#  🟢 pqr1234  LOW       docs/      更新 README          Copilot    11:45

# 按模块过滤
aether batch --module "auth/" --risk-sort

# 仅显示需要关注的（中风险及以上）
aether batch --min-risk medium
```

#### 4.4.3 风险评分模型

每个 commit 的风险分数由以下维度加权计算：

```
RiskScore = W1*变更类型 + W2*影响模块敏感度 + W3*变更规模 + W4*作者信誉

其中：
- 变更类型：breaking(1.0) > security(0.9) > feature(0.5) > refactor(0.4) > bugfix(0.3) > docs(0.1)
- 模块敏感度：auth、database、payment(1.0) > api、models(0.6) > utils、tests(0.2)
- 变更规模：files>20(1.0) > files>10(0.7) > files>5(0.4) > files<=5(0.2)
- 作者信誉：基于历史验证通过率（0.0~1.0）
```

---

### 4.5 Web Dashboard — 优先级 P1

#### 4.5.1 概述

提供可视化的 AI 代码变更管控面板，一站式查看变更态势、审核队列、验证状态。

#### 4.5.2 页面功能

**页面 1：总览仪表盘 (Overview)**

```
┌──────────────────────────────────────────────────────────┐
│  AetherVC Dashboard                          [设置] [帮助] │
├──────────────────────────────────────────────────────────┤
│                                                          │
│  ┌─ 今日概览 ─────────────────────────────────────────┐  │
│  │  AI 提交：23    通过验证：18    待审核：4    被拒：1 │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌─ 变更趋势（7 天） ─────────────────────────────────┐  │
│  │  ▁▂▃▅▃▇▅  AI 每日提交量                           │  │
│  │  ▂▃▄▃▄▅▄  人工审核量                              │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
│  ┌─ 模块热力图 ───────────┐ ┌─ 风险分布 ──────────────┐ │
│  │  auth/     ████████  8 │ │ ● Critical  2           │ │
│  │  models/   ██████    6 │ │ ● High      4           │ │
│  │  api/      ████      4 │ │ ● Medium    8           │ │
│  │  tests/    ██        2 │ │ ● Low       9           │ │
│  └────────────────────────┘ └──────────────────────────┘ │
│                                                          │
│  ┌─ 待审核队列 ───────────────────────────────────────┐  │
│  │  优先级  Commit    风险     模块      等待时间      │  │
│  │  🔴     abc1234  CRITICAL  database/  2h 15m      │  │
│  │  🟡     def5678  HIGH      auth/      1h 30m      │  │
│  │  🟡     ghi9012  MEDIUM    models/    45m         │  │
│  └────────────────────────────────────────────────────┘  │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

**页面 2：变更时间线 (Timeline)**

- 按时间展示所有 commit，颜色标注风险等级
- 支持按 Agent/模块/风险等级过滤
- 点击展开单个 commit 的详细分析报告

**页面 3：审核中心 (Review Center)**

- 审核队列列表
- 一键批准/拒绝
- diff 对比视图
- 审核历史与统计

**页面 4：验证报告 (Verification)**

- 验证通过率趋势图
- 最近验证结果列表
- 失败的测试详情
- 验证耗时统计

**页面 5：项目配置 (Settings)**

- `.aether/config.toml` 可视化编辑
- 审核策略配置
- 通知渠道配置
- LLM API Key 配置

#### 4.5.3 技术选型

| 层 | 方案 |
|----|------|
| 后端框架 | axum (已选) |
| 前端框架 | 待定（HTMX + alpine.js 轻量方案 / React SPA） |
| 数据查询 | REST API |
| 实时更新 | Server-Sent Events (SSE) 简化实现 |
| 图表 | 前端轻量图表库 |

---

### 4.6 真实 LLM 集成 — 优先级 P1

#### 4.6.1 概述

当前所有语义能力基于 Mock 和规则驱动分析，需要接入真实 LLM 以提供真正有价值的语义理解。

#### 4.6.2 集成方案

```toml
# .aether/config.toml
[llm]
provider = "openai"           # openai | claude | local
api_key = "${AETHER_OPENAI_KEY}"  # 环境变量引用
model = "gpt-4o"             # 模型选择
max_tokens = 4000
temperature = 0.1            # 低温度保证分析一致性

[llm.fallback]
# LLM 不可用时的降级策略
strategy = "rules"           # rules | error | skip
cache_enabled = true         # 缓存分析结果
cache_ttl_hours = 24         # 缓存有效期
```

#### 4.6.3 需要接入的 LLM 调用点

| 调用点 | 当前实现 | 目标 |
|--------|---------|------|
| 语义分析 (SemanticInterpreterAgent) | MockLLMClient | OpenAI/Claude API |
| 意图推理 (IntentReasoner) | 规则降级 | LLM 增强推理 |
| 摘要生成 (Digest Engine - 新增) | - | LLM 聚合多个 commit 为摘要 |
| 冲突分析 (MergeAgent) | MockLLMClient | LLM 语义冲突分析 |
| NL 命令解析 (NaturalLanguageParser) | 纯规则 | LLM 增强解析 |

#### 4.6.4 Provider 实现

需要完整实现以下文件：

```
aether-core/src/llm/providers/
├── mod.rs
├── openai.rs          # OpenAI API 调用（当前仅骨架）
├── claude.rs          # 新增：Anthropic Claude API
└── local.rs           # 新增：本地模型（ollama/llama.cpp）
```

---

### 4.7 AI Coding 工具集成 — 优先级 P2

#### 4.7.1 Git Hook 集成

AI Coding 工具通常以命令行形式工作，最自然的集成方式是通过 Git hooks。

**post-commit hook** (`aether-core/src/hooks/post_commit.rs`):

```bash
#!/bin/sh
# .git/hooks/post-commit — 每次 AI 提交后自动触发

# 1. 快速分析（跳过 LLM，<1秒）
aether analyze HEAD --quick --json > .aether/last_analysis.json

# 2. 检查门控
aether gate check HEAD

# 3. 如果触发 block，阻止下一次自动提交
#    （通过写入 .aether/blocked 标记文件实现）
```

**pre-push hook**:

```bash
#!/bin/sh
# .git/hooks/pre-push — push 前拦截

# 批量分析待推送的 commit
aether batch --since "last push" --risk-sort

# 如果存在高风险未审核，阻止推送
aether gate check --block-if-unreviewed
```

#### 4.7.2 MCP Server 集成（远期）

为 Claude Desktop / Cursor 等支持 MCP (Model Context Protocol) 的工具提供 AetherVC 服务：

```
AI 工具 ──MCP──→ AetherVC MCP Server
                    ├── search_commits(q)
                    ├── analyze_recent(n)
                    ├── get_digest(since)
                    └── check_gate(commit_hash)
```

---

### 4.8 其他增强 — 优先级 P3

| 功能 | 描述 |
|------|------|
| **智能回滚 (RollbackAgent)** | 当你发现 AI 引入问题时，根据自然语言描述自动生成回滚 patch |
| **变更预算预警** | 当 AI 在短时间内修改过多核心模块时主动告警 |
| **知识图谱可视化** | 将代码模块间的依赖关系可视化，帮助理解变更影响 |
| **团队协作** | 多人 + 多 AI 的审核队列共享和任务分配 |
| **历史洞察** | "这个模块过去三个月被 AI 修改了 47 次，平均每月引入 2 个 bug" |

---

## 5. 技术架构

### 5.1 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                          应用层                                   │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌───────────────┐   │
│  │   CLI    │  │ REST API │  │ Web UI   │  │ MCP Server    │   │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └───────┬───────┘   │
│       └──────────────┴────────────┴───────────────┘              │
│                           │                                       │
├───────────────────────────┼───────────────────────────────────────┤
│                      服务层 (aetherci + new modules)               │
│                           │                                       │
│  ┌──────────┐ ┌──────────┐┌──────────┐┌──────────┐┌──────────┐ │
│  │  Digest  │ │  Review  ││  Verify  ││  Batch   ││  Gate    │ │
│  │  Engine  │ │  Queue   ││  Runner  ││ Analysis ││  Engine  │ │
│  └────┬─────┘ └────┬─────┘└────┬─────┘└────┬─────┘└────┬─────┘ │
│       └──────────────┴────────────┴────────────┴────────────┘    │
│                           │                                       │
├───────────────────────────┼───────────────────────────────────────┤
│                    核心层 (aether-core)                            │
│                           │                                       │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐              │
│  │  Agent 系统   │ │  语义分析层   │ │  LLM 集成层  │              │
│  │ orchestrator │ │  analyzer    │ │  openai      │              │
│  │ interpreter  │ │  embedder    │ │  claude      │              │
│  │ recovery     │ │  indexer     │ │  local       │              │
│  │ merge        │ │  search      │ │  prompts     │              │
│  └──────────────┘ └──────────────┘ └──────────────┘              │
│                           │                                       │
├───────────────────────────┼───────────────────────────────────────┤
│                       存储层                                       │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐     │
│  │   Git    │  │  Vector  │  │  Graph   │  │  Config      │     │
│  │  (git2)  │  │  Store   │  │  Store   │  │  (.aether/)  │     │
│  └──────────┘  └──────────┘  └──────────┘  └──────────────┘     │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

### 5.2 新增模块清单

| Crate | 模块 | 文件 | 描述 |
|-------|------|------|------|
| `aether-core` | `digest/` | `aggregator.rs`, `summarizer.rs` | 变更摘要引擎 |
| `aether-core` | `review/` | `gate.rs`, `queue.rs`, `policy.rs` | 审核门控 |
| `aether-core` | `verify/` | `runner.rs`, `check.rs`, `report.rs` | 验证集成 |
| `aether-core` | `batch/` | `analyzer.rs`, `sorter.rs` | 批量分析 |
| `aether-core` | `hooks/` | `mod.rs`, `post_commit.rs` | Git Hook 模板 |
| `aether-core` | `config/` | `mod.rs`, `loader.rs`, `types.rs` | 配置管理 |
| `aether-core` | `llm/providers/` | `claude.rs`, `local.rs` | LLM Provider 扩展 |
| `aetherci` | `pipeline/` | `digest.rs` | 摘要流水线 |
| `aether-web` | `src/` | 完整 Web 应用 | Dashboard |
| `aether-api` | `src/` | 完整 REST API | API 服务 |

### 5.3 新增依赖

```toml
[dependencies]
# Config
figment = "0.10"            # 分层配置加载

# Notifications
reqwest = { ... }           # HTTP 客户端（用于 webhook 通知）

# 验证相关
which = "6"                 # 查找系统命令（cargo, npm 等）

# Web Dashboard
# 前端通过静态资源嵌入或独立前端项目
```

---

## 6. 核心数据模型

### 6.1 Digest (变更摘要)

```rust
/// 变更摘要报告
struct DigestReport {
    id: String,
    window: TimeWindow,
    summary: String,                       // LLM 生成的一句话总结
    total_commits: u32,
    agents_involved: Vec<String>,
    topic_clusters: Vec<TopicCluster>,    // 变更主题聚类
    risk_distribution: RiskDistribution,
    module_heatmap: HashMap<String, u32>, // 模块 → 变更次数
    high_risk_items: Vec<DigestItem>,     // 需要关注的变更
    safe_items: Vec<DigestItem>,          // 安全的变更
    generated_at: DateTime<Utc>,
}

struct TimeWindow {
    from: DateTime<Utc>,
    to: DateTime<Utc>,
}

struct TopicCluster {
    label: String,                         // 主题标签，如"认证流程修复"
    summary: String,                       // 主题摘要
    commit_hashes: Vec<String>,
    change_type: ChangeIntent,
    risk_level: RiskLevel,
}

struct RiskDistribution {
    critical: u32,
    high: u32,
    medium: u32,
    low: u32,
}

struct DigestItem {
    commit_hash: String,
    message: String,
    risk_level: RiskLevel,
    affected_modules: Vec<String>,
    summary: String,
}
```

### 6.2 Review Queue (审核队列)

```rust
struct ReviewQueue {
    items: Vec<ReviewItem>,
    total: u32,
    pending: u32,
}

struct ReviewItem {
    id: String,
    commit_hash: String,
    commit_message: String,
    author: String,
    risk_level: RiskLevel,
    triggered_reason: String,             // 触发审核的原因
    affected_modules: Vec<String>,
    change_summary: String,
    status: ReviewStatus,
    reviewer: Option<String>,
    review_comment: Option<String>,
    created_at: DateTime<Utc>,
    resolved_at: Option<DateTime<Utc>>,
}

enum ReviewStatus {
    Pending,
    Approved,
    Rejected,
    Skipped,
}
```

### 6.3 Verification Report (验证报告)

```rust
struct VerificationReport {
    id: String,
    commit_hash: String,
    checks: Vec<VerificationCheck>,
    overall_status: VerificationStatus,
    duration_ms: u64,
    generated_at: DateTime<Utc>,
}

struct VerificationCheck {
    name: String,                          // "compile", "lint", "unit_tests"
    status: CheckStatus,
    output: Option<String>,
    duration_ms: u64,
    details: Option<String>,               // 失败的详细信息
}

enum VerificationStatus { Passed, Failed, Skipped, Error }

enum CheckStatus {
    Passed,
    Failed,
    Skipped,
    Error(String),
}
```

### 6.4 Gate Policy (门控策略)

```rust
struct GatePolicy {
    enabled: bool,
    auto_pass_types: Vec<ChangeIntent>,
    require_review_types: Vec<ChangeIntent>,
    high_risk_modules: Vec<String>,
    low_risk_modules: Vec<String>,
    thresholds: GateThresholds,
    actions: GateActions,
}

struct GateThresholds {
    max_files_changed: u32,
    max_lines_added: u32,
    max_lines_deleted: u32,
    max_commits_per_hour: u32,
}

struct GateActions {
    on_threshold_exceeded: GateAction,
    on_critical_risk: GateAction,
    on_high_risk: GateAction,
}

enum GateAction { Block, Warn, Queue }

struct GateDecision {
    commit_hash: String,
    action: GateAction,
    reason: String,
    risk_score: f32,
    timestamp: DateTime<Utc>,
}
```

---

## 7. API 与 CLI 设计

### 7.1 CLI 命令一览

```bash
# ─── 现有命令（保持不变） ───
aether init [--path <path>]
aether do <natural_language_command>
aether search <query> [--limit <n>]
aether index [--full]
aether recover <description>
aether merge --prs <pr1> <pr2> ...
aether analyze <commit_range> [--json] [--quick]
aether auto [--watch] [--output-dir <dir>]
aether history <function_name> [--limit <n>]

# ─── 新增命令 ───

# 变更摘要
aether digest [--since <time>] [--until <time>] [--range <range>]
              [--by-agent] [--by-module]
              [--compare <from>..<to>]
              [--json] [--output <file>]

# 批量分析
aether batch <range|--since <time>>
              [--risk-sort] [--limit <n>]
              [--module <module>] [--min-risk <level>]
              [--json]

# 审核门控
aether gate check [<commit_hash>] [--policy <file>]
aether gate status
aether review queue [--limit <n>]
aether review approve <queue_id> [--comment <text>]
aether review reject <queue_id> [--reason <text>]
aether review history [--since <time>]

# 验证
aether verify [<commit_hash>] [--smart|--full|--quick]
              [--watch] [--report]
aether verify history [--since <time>]

# 配置
aether config init                           # 生成默认配置
aether config show                           # 显示当前配置
aether config set <key> <value>              # 设置配置项
aether config validate                       # 验证配置合法性

# Hook 管理
aether hook install [--post-commit] [--pre-push]   # 安装 Git hooks
aether hook uninstall                              # 卸载 hooks
aether hook status                                 # 查看 hooks 状态
```

### 7.2 REST API 设计

```
Base URL: http://localhost:9786/api/v1

# ─── 分析 ───
POST   /analyze                    # 分析单个 commit
  Body: { diff, commit_message, author, ... }
  
POST   /batch/analyze              # 批量分析
  Body: { commits: [...], options: { risk_sort, limit } }

POST   /digest                     # 生成变更摘要
  Body: { window: { from, to }, group_by: "agent"|"module" }

# ─── 审核 ───
GET    /review/queue               # 获取审核队列
GET    /review/queue/:id           # 获取审核项详情
POST   /review/queue/:id/approve   # 批准
  Body: { comment }
POST   /review/queue/:id/reject    # 拒绝
  Body: { reason }
GET    /review/history             # 审核历史

# ─── 验证 ───
POST   /verify                     # 触发验证
  Body: { commit_hash, mode: "smart"|"full"|"quick" }
GET    /verify/:id                 # 获取验证结果
GET    /verify/history             # 验证历史

# ─── 门控 ───
POST   /gate/check                 # 检查门控
  Body: { commit_hash }
GET    /gate/policy                # 获取当前策略
PUT    /gate/policy                # 更新策略

# ─── 搜索 ───
GET    /search?q=<query>&limit=<n> # 语义搜索

# ─── 状态 ───
GET    /status                     # 系统状态
GET    /stats/dashboard            # Dashboard 统计数据
```

---

## 8. 实现路线图

### Phase 1：核心管控 (8-10 周, P0)

| 周 | 目标 | 交付物 |
|----|------|-------|
| 1-2 | 配置管理系统 | `.aether/config.toml` 加载/验证/CLI，[config/](file:///d:/ai/trae_projects/AetherVC/aether-core/src/config/) |
| 2-4 | **Digest Engine** | 时间窗口聚合、主题聚类、摘要生成、Markdown/JSON 输出 |
| 4-6 | **Review Gate** | 审核策略引擎、审核队列、`aether review` CLI |
| 6-8 | **Verification Hook** | 验证运行器、智能测试选择、验证报告 |
| 8-10 | 真实 LLM 集成 | OpenAI/Claude Provider 完整实现，LLM 缓存 |

### Phase 2：批量与可视化 (6-8 周, P1)

| 周 | 目标 | 交付物 |
|----|------|-------|
| 10-11 | 批量分析升级 | 风险排序、跨 commit 对比、模块过滤 |
| 11-14 | **Web Dashboard** | REST API 完整实现 + Web 前端（5 个页面） |
| 14-16 | 通知集成 | Slack/Discord Webhook，系统通知 |
| 16-18 | Git Hook 集成 | post-commit/pre-push hook 模板，`aether hook` CLI |

### Phase 3：生态与增强 (6-8 周, P2-P3)

| 周 | 目标 | 交付物 |
|----|------|-------|
| 18-20 | MCP Server | AI 工具通过 MCP 协议调用 AetherVC |
| 20-22 | RollbackAgent | 智能回滚功能实现 |
| 22-24 | 知识图谱可视化 | 模块依赖图渲染 |
| 24-26 | 团队协作 | 多用户审核队列共享 |
| 26-28 | 生产化 | Qdrant/Neo4j 存储后端、性能优化、文档完善 |

---

## 9. 成功指标

### 9.1 定量指标

| 指标 | 当前基准 | 目标值 | 衡量方式 |
|------|---------|-------|---------|
| AI 提交 → 人类理解的延迟 | 数小时~数天 | < 5 分钟（通过阅读 Digest） | 用户调研 |
| 高风险变更审核覆盖率 | 0% | 100% | Gate 拦截率 |
| 低风险变更自动通过率 | 0% | > 60% | Gate 自动通过率 |
| 验证发现问题的时间 | 数小时（手动测试） | < 3 分钟（自动验证） | 端到端计时 |
| AI 引入的 regression 发现时间 | 数天（用户反馈） | < 30 分钟（自动验证捕获） | Bug 跟踪系统 |
| 开发者代码库理解信心 | 低（"AI 改了太多我搞不清"） | 高（"每天看 Digest 就知道 AI 做了什么"） | 定期问卷调查 |

### 9.2 定性指标

- 开发者不需要逐条阅读 AI 的 diff 也能理解整体变更态势
- Tech Lead 可以通过 Dashboard 一目了然 AI 团队的工作质量
- 审核流程不会成为瓶颈（自动化程度 > 60%）
- AI 产出的代码信任度提升

---

## 10. 风险与缓解

| 风险 | 影响 | 概率 | 缓解措施 |
|------|------|------|---------|
| LLM 分析质量不稳定 | 摘要/意图分析不准确 | 中 | 提供规则降级+置信度标注+人工可纠正 |
| 验证执行时间过长 | 影响开发效率 | 中 | 智能测试选择 + 异步执行 + 缓存 |
| AI Coding 工具生态变化快 | 集成方式需频繁调整 | 高 | 以 Git Hook 为基础（通用性最强），MCP 作为可选 |
| 用户不习惯看 Dashboard | 功能白做 | 中 | CLI 优先，Dashboard 作为可选补充 |
| LLM API 成本过高 | 小团队负担不起 | 中 | 本地模型支持 + 分析结果缓存 + 按需调用策略 |
| 项目复杂度增长过快 | 维护困难 | 低 | Rust 模块化 + 完善的测试覆盖 + CI/CD |

---

## 附录 A：术语表

| 术语 | 定义 |
|------|------|
| **Digest** | 对一段时间内多个 commit 的聚合摘要报告 |
| **Review Gate** | 审核门控，根据策略自动决定变更是否放行 |
| **Verification Hook** | 验证钩子，自动对变更运行测试和检查 |
| **Gate Policy** | 门控策略，定义哪些情况触发审核 |
| **Risk Score** | 风险评分，综合考虑变更类型、模块、规模的加权分数 |
| **Topic Cluster** | 主题聚类，语义相近的 commit 归为一个主题 |
| **Smart Test Selection** | 智能测试选择，只运行受变更影响的相关测试 |
| **Diff Budget** | 变更预算，设定 AI 在一定时间内的变更上限 |

## 附录 B：参考文档

- [ARCHITECTURE.md](file:///d:/ai/trae_projects/AetherVC/docs/ARCHITECTURE.md) — 当前项目技术架构
- [README.md](file:///d:/ai/trae_projects/AetherVC/README.md) — 项目说明和命令参考
