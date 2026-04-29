# Agent / Plan 工作区计划流 Prompt 存档

## 用户输入

用户要求先提交当前工作区，然后基于以下方向思考并写出 plan，说明文档和产品本身如何修改，范围包括前端和后端：

1. 所有 plan 都需要在 workspace 的 `plans` 目录中创建对应文件夹：`YYYYmmddnn-name`，其中 `nn` 为当天第 n 个 plan，例如 `2026050100-create-a-new-box`。每个 plan 文件夹中存档三个文件：
   - `request.md`
   - `plan.md`
   - `plan-result.md`
2. 模式状态简化，只保留 `Agent` 和 `Plan` 两个模式。`Plan` 只读，`Agent` 读写，`Agent` 模式可以直接读取已有 plan 干活。
3. `Agent` mode 是可以正常干活的状态，`confirm plan` 流程无意义，需要删除。
4. Markdown 预览如果打开的是 plan，可以提供入口直接触发执行 plan。

## 已完成前置动作

- 已按用户要求提交当时工作区现有改动：
  - `fde4227 chore: checkpoint workspace changes`

## 当前代码与文档背景

- `docs/cadquery-mvp/agent-tool-contract.md` 当前把 `save_cad_plan` 定义为写入单个 `plans/*.md`，并把 `AgentPlanConfirm` / `confirmed_cadquery` 作为 Execute 写入边界。
- `docs/cadquery-mvp/agent-system-prompt.md` 当前强调：Planning does not execute，Execution happens only after confirmation。
- `crates/app-server-protocol/src/protocol.rs` 当前暴露 `AgentOperationLevel::{Inform, Plan, Execute, Auto}`、`AgentPlanConfirmRequest`、`AgentPlanRejectRequest`、`AgentCadQueryConfirmation`。
- `crates/app-server-host/src/dispatcher.rs` 当前 `agent.invoke` 禁止直接携带 `confirmed_cadquery`，`agent.plan.confirm` 会创建 Execute worker 并注入 confirmation scope。
- `crates/app-server-core/src/agent/tools/semantic.rs` 当前 `save_cad_plan` 会创建 `plans/<slug>.md` 单文件。
- `packages/studio-web/src/workbench/chat-composer.tsx` 当前有 `auto / inform / plan / execute` 下拉框。
- `packages/studio-web/src/workbench/chat-messages.tsx` 当前渲染 Plan Confirmation 卡片，包含 Preview / Confirm Execute / Cancel。
- `packages/studio-web/src/viewers/markdown-viewer.tsx` 当前只负责 Markdown 读取和安全预览，没有 plan 专属动作入口。

## 本计划目标

输出一个可执行的分 Phase 实施计划，覆盖文档、协议、后端工具与 dispatcher、前端 chat / Markdown preview，以及测试和迁移策略。
