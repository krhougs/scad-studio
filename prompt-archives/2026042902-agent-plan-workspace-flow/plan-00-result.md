# Agent / Plan 工作区计划流执行结果

## 当前状态

正在执行计划。Phase 1 已完成文档与产品语义更新，Phase 2 已完成 protocol 与共享数据模型收敛，并通过独立 subagent review；后续 Phase 自动继续执行。

## 前置提交

- `fde4227 chore: checkpoint workspace changes`

## Phase 结果记录

| Phase | 状态 | 结果 |
|---|---|---|
| Phase 1 — 文档与产品语义更新 | 已完成 | 已统一 Agent / Plan 双模式文档、运行时 system prompt、tool contract、Ref PRD、Chat 交互设计和 known issues；旧 confirmation 术语仅保留在历史 / deprecated / known issues 语境 |
| Phase 2 — Protocol 与共享数据模型收敛 | 已完成 | 已将 Agent 请求协议收敛为 Agent / Plan 双模式、加入 plan package ref / saved event、提升 protocol version 到 v4，并让旧 confirmation 命令仅保留 deprecated 兼容路径 |
| Phase 3 — 后端 Plan Package 存储与解析 | 未开始 | 待执行 |
| Phase 4 — 后端 Agent Mode 执行模型 | 未开始 | 待执行 |
| Phase 5 — Web Chat 模式简化 | 未开始 | 待执行 |
| Phase 6 — Markdown Plan Preview 执行入口 | 未开始 | 待执行 |
| Phase 7 — 测试、迁移和文档收敛 | 未开始 | 待执行 |

## 执行记录

- 已创建 `plan-prompt.md`，记录用户输入、前置提交和当前代码 / 文档背景。
- 已创建 `plan-00.md`，覆盖文档、协议、后端、前端、Markdown preview、测试和迁移策略。
- 已根据反馈补充文档对齐范围：
  - `docs/cadquery-mvp/init.md`
  - `docs/cadquery-mvp/ref_components_parts_assemblies.md`
  - `docs/2026042801-agent-chat-interaction-design/README.md`
  - `docs/2026042801-agent-chat-interaction-design/competitive-analysis.md`
  - `docs/cadquery-mvp/agent-system-prompt.md`
  - `docs/cadquery-mvp/agent-tool-contract.md`
  - `docs/known_issues.md`
- 已根据反馈补充 system prompt 改造范围：
  - 将运行时 prompt 从 Inform / Plan / Execute / Auto 重写为 Agent / Plan 两模式。
  - 将 confirmation 规则改为 Agent mode execution scope、plan package 和 CadQuery staging 约束。
  - 要求同步更新 `build_turn_context()`、CadQuery generation context、本地 fallback 文案和 prompt / LLM 单测。

### Phase 1 — 文档与产品语义更新

- 完成情况：
  - 更新 `docs/cadquery-mvp/init.md`，将 MVP 主链路改为 “Markdown CAD Plan package → Agent mode 执行 plan → CadQuery 生成 / 修改模型”，并补充 `plans/YYYYmmddnn-name/{request.md,plan.md,plan-result.md}`、front matter、legacy `plans/*.md` 只读兼容和 `plan-result.md` 执行记录语义。
  - 更新 `docs/cadquery-mvp/ref_components_parts_assemblies.md`，保留 Ref 五层模型，将旧确认执行表述改为 Plan mode 创建计划档案、Agent mode 执行修改。
  - 重写 `docs/cadquery-mvp/agent-system-prompt.md`，把运行时契约改为 Agent / Plan 双模式，明确 Plan mode 唯一写入计划档案、Agent mode 才写入和执行，保留 CadQuery staging、`.py` 专用工具和 outputs 派生文件边界。
  - 重写 `docs/cadquery-mvp/agent-tool-contract.md`，将 Operation 权限表改为 Mode 权限表，定义 plan package tool result、`plan_status`、legacy plan 只读和 deprecated confirmation command。
  - 更新 `docs/2026042801-agent-chat-interaction-design/README.md` 和 `competitive-analysis.md`，将 Plan 确认卡片替换为 Plan Package 卡片和 `Run Plan` 动作，删除 `/execute` 作为产品主路径。
  - 更新 `docs/known_issues.md`，新增旧 confirmation 主流程与 Agent / Plan 双模式冲突记录，并把旧 edit intent / plan binding 记录改为历史语境。
- Review：
  - 第一轮 review 发现 front matter 示例、`save_cad_plan` 状态字段、`plan-result.md` 更新条件问题，已修复。
  - 第二轮 review 发现 Plan mode 仍允许 `update_chat_summary`，已改为禁止。
  - 第三至第六轮 review 未发现阻塞项；后续指出的 Ref 描述和 known issues 历史措辞风险已修正。
- 验证：
  - `rg -n 'Inform / Plan / Execute|Operation level|Operation: Execute|确认执行|AgentPlanConfirm|AgentCadQueryConfirmation|confirmed_cadquery|Confirmed target|confirmation scope|Plan 确认卡片|Confirmation Needed|Web UI 只展示和确认|作为 confirmation' ...`：仅命中 deprecated / known issues 历史语境。
  - `git diff --check`：通过。
- 遗留问题：
  - Phase 1 仅完成文档和运行时 prompt 契约更新；protocol、后端、Web Chat 和 Markdown preview 实现继续由 Phase 2 至 Phase 6 完成。

### Phase 2 — Protocol 与共享数据模型收敛

- 完成情况：
  - 更新 `crates/app-server-protocol/src/protocol.rs` 和 TypeScript protocol package，将 `AgentOperationLevel` / `operation` 替换为 `AgentMode` / `mode`，并在 `AgentInvokeRequest` 中加入 `plan_ref` 和 `context_refs`。
  - 新增 `AgentPlanPackageRef`、`AgentPlanSavedEvent` 和 `agent.plan_saved` push event；保留 `AgentPlanConfirmRequest`、`AgentPlanRejectRequest` 和 `AgentCadQueryConfirmation` 作为 deprecated 兼容类型。
  - 将 protocol version 提升到 v4，并更新 Rust roundtrip、host、wasm bridge、Web wiring 和 protocol smoke 覆盖。
  - 更新 app server host dispatcher，使 `agent.plan.confirm` 和 `agent.plan.reject` 返回明确 deprecated error，不再作为执行成功路径。
  - 为了保持协议变更后的编译和行为一致，同步将 core agent turn、tool registry 和 Web Chat 主请求路径迁移到 `Agent` / `Plan` mode；Web 新主路径不再暴露 `/execute`、`/inform`、Plan Confirmation 或 Confirm Execute。
- Review：
  - 第一轮 Phase 2 review 发现 host / core 仍依赖旧 `AgentOperationLevel`、Web 仍展示 Plan Confirmation、Web 单测仍固定旧确认流，已修复。
  - 第二轮 Phase 2 review 发现 host 测试仍使用旧 `AgentInvokeRequest` 字段、旧 confirm 成功测试仍存在、host / wasm 协议样例仍为 v3，已修复。
  - 第三轮 Phase 2 review 未发现阻塞项；review 明确指出 core 中残留的 `confirmation_scope` / `requires_confirmation` 属于 Phase 4 执行模型重构范围，未重新成为 Web、host 或 protocol 的当前主流程入口。
- 验证：
  - `cargo fmt --check -p app-server-protocol -p app-server-core -p app-server-host -p studio-web-wasm`：通过。
  - `cargo test -p app-server-protocol --tests`：通过。
  - `cargo test -p app-server-core --tests`：通过，仅有既有 watch dead_code warning。
  - `cargo test -p app-server-host --tests`：通过，仅有既有 watch dead_code warning。
  - `cargo test -p studio-web-wasm --tests`：通过。
  - `bun run protocol:smoke`：通过。
  - `cd packages/studio-web && bun run typecheck`：通过。
  - `cd packages/studio-web && bun run test:unit -- tests/unit/chat-actions.test.ts tests/unit/chat-zone.test.tsx tests/unit/protocol-package-import.test.ts tests/unit/workbench-wiring.test.ts`：4 个测试文件、20 个测试通过。
  - `rg -n "AgentOperationLevel|operation_for_tool_loop|agent_tool_definitions_for_operation|confirmed_cadquery|Confirmed target|Operation level|Operation: Execute|Confirm Execute|Plan Confirmation|/execute|/inform" ...`：仅命中 deprecated 兼容字段、deprecated confirm 测试 helper 和 prompt 反向断言。
  - `git diff --check`：通过。
- 遗留问题：
  - Phase 2 只完成协议与共享数据模型收敛；plan package 的实际创建 / 解析由 Phase 3 完成。
  - core 内部 `confirmation_scope` / `requires_confirmation` 仍会在 Phase 4 中替换为 Agent mode execution scope 和 path policy。
