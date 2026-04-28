# Agent Tool Call 能力补全计划 Prompt

本目录对应任务：**CadQuery Agent Tool Call 能力盘点、补全与权限模型重评估**。

## 原始用户要求

用户先要求阅读：

- `docs/cadquery-mvp`
- `prompt-archives/2026042700-cadquery-mvp-design`
- `prompt-archives/2026042801-agent-chat-redesign`

并基于“让 Agent 可以按照 MVP PRD 进行建模”的目标，检查当前已有 tool call 与缺失 tool call。

前一轮盘点结论：

- 当前真正暴露给 LLM 主动调用的 tool call 只有 `read_file` 与 `list_directory`。
- `cadquery.execute`、`cadquery.preview`、`cadquery.result.get` 等是 app server protocol command，不等同于 LLM 可主动调用的 Agent tool。
- Execute 阶段目前由后端在 confirmation 后调用 LLM 产出代码，再由 host 执行 CadQuery；这不是完整的多轮工具调用闭环。

用户随后补充缺失工具：

- `read_file()`
- `write_file()`
- `patch_file()`
- `copy_file()`
- `update_chat_summary()`
- `save_cad_plan()`

并要求新开一个 plan：

1. 现有 tool 也要检查能力范围，看看有没有需要补的东西。
2. 补全本轮对话中列出的缺失 tool。
3. 重新评估三个模式下 tool 的权限模型。

## 必读上下文

- `docs/cadquery-mvp/init.md`
- `docs/cadquery-mvp/ref_components_parts_assemblies.md`
- `docs/cadquery-mvp/agent-system-prompt.md`
- `docs/cadquery-mvp/decisions.md`
- `prompt-archives/2026042700-cadquery-mvp-design/plan-00.md`
- `prompt-archives/2026042700-cadquery-mvp-design/plan-00-result.md`
- `prompt-archives/2026042801-agent-chat-redesign/plan-00.md`
- `prompt-archives/2026042801-agent-chat-redesign/plan-00-result.md`

## 当前代码入口

- `crates/app-server-core/src/agent/tools.rs`
- `crates/app-server-core/src/agent.rs`
- `crates/app-server-core/src/llm/`
- `crates/app-server-host/src/dispatcher.rs`
- `crates/app-server-host/src/plan_extraction.rs`
- `crates/app-server-protocol/src/protocol.rs`
- `packages/studio-web/src/workbench/chat-actions.ts`
- `packages/studio-web/src/workbench/cadquery-agent-scope.ts`
- `packages/studio-web/src/workbench/chat-zone.tsx`

## 已确认约束

- 对外产品名为 `budn'`，代码标识符使用 `budn`。
- CadQuery Python runner 是 app server 外部工具豁免，不允许扩展为项目内任意 Python 辅助脚本。
- `.py` CadQuery 模型文件不得由普通文档写入工具直接改写；模型执行必须通过 app server / staging / CadQuery tool 边界完成。
- Agent Execute 必须经过结构化 confirmation，并只能修改已确认的 `affected_files` / `new_files`，只能生成已确认的 `export_targets`。
- `plans/` 下的 CAD Plan 需要可追溯；`AgentCadQueryConfirmation.plan_ref` 当前仍未持久绑定，是既有已知问题。
- `selector` 只能作为 Agent / runner 内部查找手段，不作为 MVP 用户可见 Ref 层。
- Plan 必须按 Phase 拆分，每个 Phase 写明输入、操作步骤、验收标准，以及实现当前 Phase 时要保护的前序目标。
- 每个 Phase 执行时必须遵循“干活 → review → 回归”循环，review 必须由独立 subagent 执行，Phase 完成后更新 `plan-00-result.md` 并自动推进。

## 本次 plan 目标

生成一个可直接执行的实施计划，覆盖：

1. 现有 LLM tool 与 app server command 的边界盘点和能力补齐。
2. `read_file()`、`write_file()`、`patch_file()`、`copy_file()`、`update_chat_summary()`、`save_cad_plan()` 的 tool schema、执行器、权限校验、测试与 UI / protocol 影响。
3. Inform / Plan / Execute 三种模式下工具权限模型重评估，尤其要区分只读工具、Plan 专用持久化工具、Execute confirmation 范围内写入工具和 CadQuery 建模工具。
