产品需求文档（PRD）
产品名称：AetherVC（以太版本控制）—— AI-Native 语义版本管理系统
版本：v0.1（MVP）
文档日期：2026年6月
目标用户：AI 重度开发者团队、多 Agent 开发团队、需要与 AI 深度协作的工程师
1. 产品概述
AetherVC 是一个下一代版本控制系统，专为 AI 大规模代码生成时代设计。它在传统 Git 之上构建了一层语义智能层，让人类可以通过自然语言与版本控制系统进行深度交互，实现跨 commit 恢复、自动合并、海量 Tag 智能验证、多 Agent 冲突协调等能力。
核心价值主张：

人类从“管理 commit” 升级为 “管理目标和验收标准”
AI 提交爆炸时，系统依然可控、可理解、可验证
真正实现“和 AI 一起编程”的版本管理闭环

2. 业务目标

支持单人每天与 AI 产生 50+ commits 的工作流
支持 10+ AI Agent 同时提交代码
极大降低人类 review 和 merge 负担（目标：80% PR 自动处理）
实现语义级代码恢复，而非单纯文件级

**3. 核心用户场景（User Stories）
场景1 - 语义恢复
用户：“把上周 agent 实现的用户画像分析模块恢复回来，它在昨天的重构中被删掉了”
→ 系统自动定位、生成 patch、处理冲突、创建新 commit
场景2 - 大量 Tag 验证
用户：“把最近 15 个含 ‘RAG’ 关键词的 tag 部署到测试环境，按风险从低到高验证”
→ 系统自动分发、影子部署、生成验证报告
场景3 - 海量 PR 合并
系统自动分类、排序、解决低风险冲突、高风险的给出清晰建议
场景4 - 多 Agent 协作
多个 Agent 同时修改同一模块时，系统自动协调或组织讨论
4. 功能需求（Functional Requirements）
4.1 自然语言接口层

支持中英文自然语言指令
指令类型：恢复、查询、合并、部署、分析、回滚等
支持多轮对话上下文记忆

4.2 智能 Agent 系统（核心）
必须实现的 6 大核心 Agent（使用 LangGraph 或 CrewAI 编排）：

Semantic Interpreter Agent - 理解每一次变更的语义
Cross-Commit Recovery Agent - 跨版本智能恢复
Merge Agent - 智能 PR 合并
Multi-Agent Coordinator - 多 Agent 冲突协调
Validation & Risk Agent - Tag/Commit 验证与风险评估
Rollback Agent - 智能监控与自动回滚

（详细 Prompt 模板见本文档附录）
4.3 语义与知识层

向量数据库：存储所有 commit/tag 的语义向量
图数据库：维护代码依赖关系图、变更影响图
RAG 知识库：历史最佳实践 + 项目上下文

4.4 存储层

底层必须兼容 Git（可直接 clone/push）
额外维护语义索引

4.5 环境与部署层

支持多环境（dev/staging/prod）
支持影子部署（shadow deployment）
支持自动验证流水线

5. 非功能需求

性能：支持 10,000+ commits 的语义搜索 < 2秒
兼容性：完整 Git 兼容，可与 GitHub/GitLab 联动
可扩展性：支持 50+ 并行 AI Agent
安全性：所有 AI 操作需人类最终确认（可配置信任等级）
可观测性：完整操作日志 + 可视化仪表盘

6. 技术架构要求
（参考之前提供的架构图）

后端：Python + FastAPI
Agent 框架：LangGraph（推荐）或 CrewAI
向量数据库：Qdrant 或 Chroma
图数据库：Neo4j（可选，先用 NetworkX）
LLM：支持 Claude 3.5 Sonnet / GPT-4o / Grok / DeepSeek 等，通过 LiteLLM 统一调用
存储：本地 Git + 对象存储（语义索引）
前端：可选 Streamlit / Gradio / Next.js（MVP 可先用 CLI + Web UI）

7. MVP 范围（第一阶段交付）
MVP 必须完成：

Git 仓库语义索引系统（commit 自动向量化）
自然语言 → Git 操作翻译器
Cross-Commit Recovery Agent（重点）
Merge Agent（基础版）
语义搜索功能
命令行 + 简单 Web 交互界面
完整的 Prompt 模板库 + Agent 编排

非 MVP（二期）：

多 Agent 实时协调
自动部署流水线
可视化依赖图


开发任务分解（可直接交给 Claude Code / Cursor / Windsurf）
主任务：实现 AetherVC v0.1 MVP
Phase 1：项目初始化（1天）

创建项目结构（推荐 monorepo）
集成 LiteLLM + LangGraph
配置本地 Git 测试仓库

Phase 2：语义索引层（2-3天）

实现 commit 解析 + diff 提取
生成语义 embedding（使用 voyage-code 或 text-embedding-3-large）
构建向量数据库索引

Phase 3：核心 Agent 实现（4-5天）

实现 Semantic Interpreter Agent
实现 Cross-Commit Recovery Agent（最高优先级）
实现 Merge Agent
实现基础的 Agent 编排工作流

Phase 4：自然语言接口（2天）

CLI 工具（类似 aether vc recover ...）
简单聊天界面（Gradio 或 Streamlit）

Phase 5：测试与集成（2天）

使用真实多 commit 测试用例
验证跨 commit 恢复准确率


附录：核心 Prompt 模板
（请直接使用我之前提供的 5 个 Agent Prompt 模板）

验收标准：

能通过自然语言成功恢复被删除的历史功能
语义搜索准确率 > 85%
Merge Agent 能正确处理简单冲突
系统可稳定运行在本地 Git 仓库上
代码结构清晰、可扩展