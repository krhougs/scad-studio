# 竞品交互分析

## Codex App

### 关键启发

- **任务导向而非选择导向**：用户描述意图，Agent 自行探索和规划，不需要用户预选目标
- **确认是内联的**：Agent 提出变更方案，用户在对话中逐步审批，不需要预选操作模式
- **并行独立上下文**：每个任务在独立线程中，可同时进行多项工作
- **记忆和持久化**：Agent 跨会话保留上下文，长期任务可以自动唤醒继续

### 不适用于 budn' 的部分

- **通用性过高**：Codex 是通用 Agent 平台，budn' 是聚焦 CAD 的工具，不需要这种广度
- **Computer Use 模式**：屏幕操作、鼠标点击等能力不适用于结构化 CAD 交互
- **插件生态**：MVP 阶段不需要插件系统

---

## Cursor

### 关键启发

- **@-mention 上下文注入**：`@file`、`@codebase`、`@docs` 作为可选的精确上下文附加到对话中。用户主动添加，系统不强制要求
- **Agent Mode 直接工作**：描述高级目标，Agent 自动规划并执行跨文件变更
- **自动上下文增强**：系统在用户无感知的情况下自动附加相关上下文（当前文件、最近编辑、活跃错误）
- **模式是策略而非门禁**：Agent/Plan/Debug/Ask 改变 Agent 的行为策略，不限制用户可以做什么
- **上下文用量可见**：显示当前使用了多少上下文，超限时自动摘要

### 对 budn' 的映射

| Cursor 概念 | budn' 等价物 |
|-------------|-------------|
| @file | @part[name]、@component[name] |
| @codebase | 项目结构自动上下文（components/parts/assemblies 目录） |
| 文件编辑器选择 | Viewer 3D 选择（面/边/顶点/部件） |
| Agent Mode | 读写执行，可直接使用当前请求或已有 plan package |
| Plan Mode | 生成 workspace plan package |

### 不适用于 budn' 的部分

- **代码中心假设**：Cursor 围绕文本编辑器设计，budn' 的主要交互是 3D 视觉 + 对话
- **技术用户假设**：Cursor 假设用户是开发者，budn' 目标用户包括 CAD 新手
- **符号级索引**：代码符号搜索不直接适用于 CAD 项目结构

---

## budn' 的独特定位

budn' 不是代码编辑器，也不是通用 Agent 平台。它是 **3D CAD 设计的 AI 协作工具**。

独特优势：
1. **空间 @-mention**——在 3D Viewer 中点击一个面就是最自然的"引用"方式，比任何文本编辑器的 @-mention 都更直观
2. **Ref 系统**——MVP 用户可见 Ref 覆盖 component / part / assembly、instance、feature、face / edge / vertex，结构化程度高于自由文本搜索
3. **Plan package before run**——CAD 复杂修改需要可审阅、可复用、可记录结果的任务包
4. **物理世界约束**——尺寸、制造工艺、材料属性是 CAD 对话中的一等公民，代码编辑器不涉及

设计时应发挥这些独特优势，而不是简单复制代码编辑器的交互模式。
