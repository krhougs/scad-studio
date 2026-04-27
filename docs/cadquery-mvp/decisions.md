# CAD Agent Harness MVP — 方向决策记录

日期：2026-04-27

## 已确认决策

### 1. Python 约束处理

CadQuery 子进程豁免。视为外部工具（类似 OpenSCAD CLI），不算项目内 Python 代码。需更新 AGENTS.md 记录此豁免。

### 2. 产品定位

CadQuery 替代 OpenSCAD。CadQuery Agent 是产品新方向。MVP 期间不删 OpenSCAD，但不再投入新功能。

### 3. Agent 运行时

Rust 自建 LLM 抽象层。全部跑在后端，no vendor lock-in。Phase 1 开始时按 crates.io / docs.rs 当前版本评估 Rig 的 tool use、streaming 和自定义 Agent loop 能力；贴合需求就用，否则退回 SDK 客户端（anthropic-sdk-rust + async-openai）+ 自建薄 provider trait。不固定旧版本号。

### 4. B-rep 拓扑方案

CadQuery Python 端直接输出 topology metadata + mesh + feature mapping。我们控制所有 Python 模型代码，可以自定义输出格式。前端按 face group 渲染和选择。拓扑稳定性通过 CadQuery `.tag()` + selector 组合实现。

### 5. MVP 范围

必须包含 face/edge/vertex 精细选择。这是核心差异化能力，接受相应的开发周期和技术风险。

### 6. Chat 存储格式

JSONL（`chats/*.jsonl`）。每行一条消息记录，支持 tool calls 和 function results 的结构化存储。

### 7. Project 概念

Project = workspace，同一个东西。不引入新概念，现有 workspace 机制直接承载 CAD project。

### 8. CadQuery 架构

复用现有外部工具子进程模式概念（与 OpenSCAD CLI 同一类），但在 `app-server-core/src/cadquery/` 新建子进程调用模块（现有 preview.rs 是 OpenSCAD 专用，不直接复用代码路径）。CadQuery 本身是 tool call，系统原子完成写入+执行+返回。

### 9. Ref 层级

MVP 实现 5 层：component/part/assembly、instance、feature、raw face/edge/vertex。砍掉 selector 和 subshape 层，后续按需加回。

### 10. mesh wire format

基于现有 Borsh 协议扩展，不另起炉灶。Python runner 输出 JSON，Rust 端转 Borsh 传前端。

### 11. 前端架构

基于现有框架增量改造，保持当前 UI，不大改架构。

### 12. 并发模型

限制同时只有一个 running agent session。

### 13. Python 环境

MVP 手动安装，分发策略留到产品化阶段。

### 14. face label 和 CadQuery Selector 的映射规则

Python 端 ref_mapper 负责建立 feature→face 映射。REFS dict 中声明的 features 通过 tag 或 selector 找到对应的 face_indices。自动推导 candidate selectors + 歧义检测。

## 已关闭的开放问题

以上所有决策覆盖了原开放问题列表中的所有项目。
