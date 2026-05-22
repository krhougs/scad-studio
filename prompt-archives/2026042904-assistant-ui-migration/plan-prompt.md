# Prompt 存档

## 原始请求

1. "先commit当前工作区，然后研究一下用 https://github.com/assistant-ui/assistant-ui 完全替代当前的Agent Chat实现，但是要保留当前的样式"
2. "出个plan我们再讨论一下"
3. 用户决策：合成消息方案 + 先单会话
4. "调用环境里的codex对plan进行review"（多轮 codex review 收敛）
5. 用户修正 CSS 约束：不限制实现方式，但最终成品必须符合当前设计系统
6. 5 轮 codex review 收敛，通过全部 7 条审查标准

## 用户强制约束

- 使用 `@assistant-ui/react`（不安装 `@assistant-ui/react-ui`）
- **不新增** `@assistant-ui/react-markdown`，继续复用现有 Markdown 渲染器
- Agent events 采用合成消息方案
- 先单会话，保留现有 `<select>` 切换器
- 引入新库不改变功能的样式，最终成品必须符合当前设计系统
- protocol 补 `run_id`
- 安装依赖使用 `bun`，遵循项目工具链约束

## 背景

当前分支 `plan/2026042902-agent-plan-workspace-flow` 包含 agent plan workspace flow 的完整改动（12 commits, 88 files changed）。assistant-ui 迁移基于此分支进行。

## 相关源码

- `packages/studio-web/src/workbench/chat-zone.tsx` — 主编排组件
- `packages/studio-web/src/workbench/chat-messages.tsx` — 消息渲染、timeline 构建
- `packages/studio-web/src/workbench/chat-composer.tsx` — Composer 组件
- `packages/studio-web/src/workbench/chat-actions.ts` — 提交动作函数
- `packages/studio-web/src/styles/workbench-zones.css` — BEM CSS
- `packages/studio-web/src/wasm-bridge/client.ts` — WasmClient
- `crates/app-server-protocol/src/protocol.rs` — Protocol 定义
