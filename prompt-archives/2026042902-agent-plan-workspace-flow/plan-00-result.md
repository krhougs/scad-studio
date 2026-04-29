# Agent / Plan 工作区计划流执行结果

## 当前状态

计划执行完成。Phase 1 已完成文档与产品语义更新，Phase 2 已完成 protocol 与共享数据模型收敛，Phase 3 已完成后端 Plan Package 存储与解析，Phase 4 已完成后端 Agent Mode 执行模型，Phase 5 已完成 Web Chat 模式简化，Phase 6 已完成 Markdown Plan Preview 执行入口，Phase 7 已完成测试、迁移和文档收敛；所有 Phase 均已通过独立 subagent review，Plan 级独立 review 未发现阻塞问题或高风险问题。

## 前置提交

- `fde4227 chore: checkpoint workspace changes`

## Phase 结果记录

| Phase | 状态 | 结果 |
|---|---|---|
| Phase 1 — 文档与产品语义更新 | 已完成 | 已统一 Agent / Plan 双模式文档、运行时 system prompt、tool contract、Ref PRD、Chat 交互设计和 known issues；旧 confirmation 术语仅保留在历史 / deprecated / known issues 语境 |
| Phase 2 — Protocol 与共享数据模型收敛 | 已完成 | 已将 Agent 请求协议收敛为 Agent / Plan 双模式、加入 plan package ref / saved event、提升 protocol version 到 v4，并让旧 confirmation 命令仅保留 deprecated 兼容路径 |
| Phase 3 — 后端 Plan Package 存储与解析 | 已完成 | 已将 `save_cad_plan` 改为 workspace plan package 三文件结构，新增 plan package parser、执行范围解析、legacy plan 只读展示和 `get_project_context` plan package 列表 |
| Phase 4 — 后端 Agent Mode 执行模型 | 已完成 | 已将后端执行模型从 confirmation precondition 切换为 Agent / Plan mode，Agent mode 支持自由请求和 plan_ref execution scope 执行，Plan mode 保持只读加 `save_cad_plan`，并在 `cadquery_execute` 成功 / 失败时安全追加 `plan-result.md` |
| Phase 5 — Web Chat 模式简化 | 已完成 | 已将 Web Chat 输入、快捷命令、Plan Package 卡片和 Run Plan 动作切换到 Agent / Plan 双模式；Run Plan 通过 Agent mode + plan_ref 触发，并保留 selection context 与 busy 防重复触发 |
| Phase 6 — Markdown Plan Preview 执行入口 | 已完成 | 已在 Markdown preview 中为 plan package Markdown 增加 Run Plan 入口，普通 Markdown 不显示执行入口，执行仍通过 app server protocol 的 Agent mode + plan_ref 触发 |
| Phase 7 — 测试、迁移和文档收敛 | 已完成 | 已完成 Rust / Web / protocol / wasm / 文档一致性回归，已关闭旧 confirmation known issue 并记录 legacy `plans/*.md` 只读兼容范围；第二轮独立 review 未发现阻塞问题 |

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

### Phase 3 — 后端 Plan Package 存储与解析

- 完成情况：
  - 新增 `app_server_core::agent::plan_package`，将 `save_cad_plan` 从单文件 `plans/*.md` 改为创建 `plans/YYYYmmddnn-slug/{request.md,plan.md,plan-result.md}`，并返回 `plan_id`、`plan_ref`、`request_path`、`plan_path` 和 `result_path`。
  - 新增当天 plan id 分配和 slug 规范化逻辑：扫描同日期已有 plan package，按最大序号递增，slug 只保留 ASCII 小写字母、数字和连字符，无法生成时使用 `cad-plan`。
  - 为 `plan.md` 增加机器可解析 front matter，记录 `target_path`、`target_type`、`affected_files`、`new_files`、`export_targets`、`status` 和 `created_at`，并初始化 `plan-result.md` 为 `status: pending`。
  - 新增 `parse_plan_package()`，校验 plan package 三文件完整性、拒绝 symlink `plans/`、拒绝 workspace escape、校验 target / affected / new / export 路径，并返回规范化后的执行范围。
  - 更新 `get_project_context`，返回 `kind: plan_package` 的 plan 列表和 `kind: legacy_plan` 的根目录旧版单文件计划；legacy plan 只读展示，不作为可执行 plan package。
  - 更新 `save_cad_plan` tool schema、registry 测试和语义导出，使 Plan mode 的唯一写入为创建 plan package；普通文件工具继续禁止直接改写 CadQuery `.py` 模型源。
- Review：
  - 第一轮 Phase 3 review 发现三个阻塞问题：parser / project context 未拒绝 symlink `plans/` 父目录、parser 返回未规范化 front matter 路径、`get_project_context` 缺少 `updated_ms`。已补充实现和测试覆盖。
  - 第二轮 Phase 3 review 未发现阻塞项；review 确认 symlink 拒绝、legacy plan 只读展示、执行范围规范化返回、`updated_ms` 输出和 `.py` 普通写入边界符合 Phase 3 验收标准。
- 验证：
  - `cargo fmt --check -p app-server-core -p app-server-host`：通过。
  - `cargo test -p app-server-host --test plan_extraction_tests parse_plan_package`：通过，覆盖 5 个 parser 测试。
  - `cargo test -p app-server-core --test agent_tool_tests get_project_context`：通过，覆盖 4 个 project context 测试。
  - `cargo test -p app-server-core --tests`：通过，仅有既有 `watch` dead_code warning。
  - `cargo test -p app-server-host --tests`：通过，仅有既有 `watch` dead_code warning。
  - `git diff --check`：通过。
- 遗留问题：
  - Phase 3 仅完成 plan package 创建、展示和解析；Phase 4 将使用 parser 输出替换旧 `confirmation_scope` / `requires_confirmation`，并在 Agent mode 执行 plan 后更新 `plan-result.md`。

### Phase 4 — 后端 Agent Mode 执行模型

- 完成情况：
  - 将 core 内部执行范围从 `confirmation_scope` / `AgentToolConfirmationScope` 替换为 `execution_scope` / `AgentExecutionScope`，并支持从 Phase 3 的 plan package parser 生成 plan 执行范围。
  - 更新 tool registry 和 path policy：`Agent` mode 可使用安全文本写入工具和 `cadquery_execute`；`Plan` mode 仍只允许只读工具与 `save_cad_plan`；普通文件工具继续拒绝直接改写 CadQuery `.py` 模型源、`chats/`、`outputs/` 和 staging 路径。
  - 更新 `cadquery_execute`：无 `plan_ref` 时允许 Agent mode 自由请求执行，但 target 必须在 `components/`、`parts/`、`assemblies/`，导出目标必须符合 runner 默认 outputs 规则；有 `plan_ref` 时 target、target type、affected / new files 和 export targets 必须匹配 plan package execution scope。
  - 为 `cadquery_execute` 成功和失败路径增加 `plan-result.md` 追加记录；更新失败时将 `plan_result_update_warning` 写入工具结果，不静默丢弃。
  - 为 `plan-result.md` 更新增加路径形态、symlink component、regular file 和 hard link 防护；Unix 使用 `nlink()`，Windows 使用 `number_of_links()`，其他无法可靠判断的平台保守拒绝写入并返回 warning。
  - 更新 host `agent.invoke`：`Agent` mode 带 `plan_ref` 时解析 plan package 并把 execution scope 同时传给 LLM turn input 和 tool executor；解析失败时返回 Agent error，不执行工具。
  - 更新 runtime prompt 上下文、本地 fallback 和 CadQuery generation context：输出 `Mode`、`Plan ref`、`Execution scope`、context refs 和 selection，不再把 `Operation: Execute`、`Confirmed target`、`confirmed_cadquery` 作为当前执行规则。
  - 将 `studio-common` 共享客户端测试从旧 `AgentOperationLevel` / `operation` / `confirmed_cadquery` 构造方式迁移到 `AgentMode` / `plan_ref`。
- Review：
  - 第一轮 Phase 4 review 发现 `cadquery_execute` 的早期参数、路径、scope、contract 和无 runtime 失败没有记录 `plan-result.md`，且 LLM CadQuery generation request 仍使用旧上下文；已修复并补充失败记录与 LLM context 测试。
  - 第二轮 Phase 4 review 发现 `studio-common` 测试仍使用旧 protocol 字段，以及 `plan-result.md` 缺少 hard link 防护；已修复并补充 `studio-common` 回归和 hard link 测试。
  - 第三轮 Phase 4 review 发现 hard link 防护只覆盖 Unix；已补充 Windows `number_of_links()` 路径和其他平台保守拒绝策略。
  - 第四轮 Phase 4 review 未发现阻塞项或高风险问题。
- 验证：
  - `cargo test -p app-server-core --tests`：通过，仅有既有 `watch` dead_code warning。
  - `cargo test -p app-server-host --tests`：通过，仅有既有 `watch` dead_code warning。
  - `cargo test -p studio-common --tests`：通过。
  - `cargo test -p app-server-protocol --tests`：通过。
  - `cargo fmt --check -p app-server-core -p app-server-host -p studio-common`：通过。
  - `git diff --check`：通过。
  - `rg -n "confirmation scope|Operation: Execute|Confirmed target|Ensure you have confirmed|confirmed_cadquery|AgentOperationLevel|operation_for_tool_loop" ...`：仅命中 deprecated 文档、protocol deprecated 字段、host deprecated compatibility tests / helper 和 prompt 反向断言。
- 遗留问题：
  - Phase 4 未处理 Web Chat UI 和 Markdown Plan Preview；Phase 5 将继续简化 Web Chat 模式与 Plan Package / Run Plan 入口，Phase 6 将实现 Markdown plan preview 执行入口。

### Phase 5 — Web Chat 模式简化

- 完成情况：
  - 保持 Chat 输入区只暴露 `Agent` 和 `Plan` 两个模式；`/plan` 和 `/agent` 作为显式快捷命令，`/execute` 不再作为产品命令处理。
  - Web Chat 主请求继续发送 `mode`、`plan_ref` 和 `context_refs`；普通消息不带 plan 时发送 `plan_ref: null`，selection context 仍随请求发送。
  - 删除 Web Chat 当前主流程中的 Plan Confirmation 控件语义；legacy `agent.plan_proposed` 仅作为普通 agent event 显示，不渲染 Confirm Execute / Cancel 动作。
  - 新增 Plan Package 卡片，显示 plan id、target、affected files、new files 和 exports，并提供 `Open Plan` 与 `Run Plan`。
  - `Open Plan` 只在 `plan_ref` 为 `plans/<plan_id>` 且 `plan_path` 为同一 workspace、同一目录下 `plan.md` 时渲染；不满足条件时不展示 Plan Package 操作。
  - `Run Plan` 通过 `agent.invoke { mode: "agent", plan_ref, context_refs }` 触发，Agent run busy 或本地 busy 时不重复触发。
  - 为 Phase 2 的 protocol 字段变更重新生成 `packages/studio-web-wasm/generated` 产物，修复浏览器端旧 wasm 仍要求 `operation` 字段的问题。
- Review：
  - 第一轮 Phase 5 review 发现 `Open Plan` 直接信任 `agent.plan_saved.package.plan_path`，不能证明只打开同一 plan package 下的 `plan.md`。已补充 `plan_ref` / `plan_path` 结构和 workspace 一致性校验，并增加反例测试。
  - 第二轮 Phase 5 review 未发现阻塞项或高风险问题。
- 验证：
  - `cd packages/studio-web && bun run typecheck`：通过。
  - `cd packages/studio-web && bun run test:unit -- tests/unit/chat-zone.test.tsx tests/unit/chat-actions.test.ts tests/unit/workbench-wiring.test.ts tests/unit/protocol-package-import.test.ts`：4 个测试文件、22 个测试通过。
  - `cd packages/studio-web && bun run test:e2e -- tests/playwright/agent-chat-interaction.spec.ts`：10 个测试通过。
  - `cd packages/studio-web && bun run test:e2e -- tests/playwright/wasm-bridge-smoke.spec.ts`：1 个测试通过。
  - `bun run check:wasm-bindgen`：通过，`wasm-bindgen` 版本为 0.2.117。
  - `git diff --check`：通过。
  - `rg -n "Confirm Execute|Plan Confirmation|agent\\.plan\\.confirm|agent\\.plan\\.reject|AgentOperationLevel|operation_for_tool_loop|confirmed_cadquery|Confirmed target|Operation level|Operation: Execute|/inform" packages/studio-web/src packages/studio-web/tests packages/studio-web-wasm/generated -S`：无命中。
- 遗留问题：
  - Phase 5 未处理 Markdown Plan Preview 顶部执行入口；Phase 6 将实现打开 `plans/<id>/plan.md` 后直接运行 plan。

### Phase 6 — Markdown Plan Preview 执行入口

- 完成情况：
  - 新增 plan preview 路径识别逻辑，只将 `plans/YYYYmmddnn-name/{plan.md,request.md,plan-result.md}` 识别为可执行 plan package，并生成目录级 `plan_ref`。
  - `MarkdownViewer` 在识别到 plan package Markdown 时显示 `Run Plan` 操作；普通 Markdown 不显示执行入口，原有 `rehypeSanitize` 渲染链路保持不变。
  - Workbench 将 Markdown preview 的 `Run Plan` 接入既有 `runSavedPlan()`，确保创建或复用当前 Chat session 后发送 `agent.invoke { mode: "agent", plan_ref }`，并通过 busy / agent run 状态避免重复触发。
  - 保持已有 watch refresh 机制，plan 执行完成并更新 `plan-result.md` 后，当前 active Markdown tab 可通过 refresh signal 重新读取内容；未激活 tab 在切换时重新读取。
  - 补充 unit 和 Playwright 覆盖：plan path 识别、普通 Markdown 不显示按钮、Markdown 安全渲染不受影响、从 `plans/<id>/plan.md` 点击 `Run Plan` 会发送 Agent mode 请求和正确的 `plan_ref`。
- Review：
  - Phase 6 独立 review 未发现阻塞问题或高风险问题；review 确认路径识别、Markdown 安全渲染、执行入口复用 `runSavedPlan()`、watch refresh 和测试覆盖符合 Phase 6 验收标准。
- 验证：
  - `cd packages/studio-web && bun run test:unit -- tests/unit/plan-preview-path.test.ts tests/unit/markdown-viewer.test.tsx`：2 个测试文件、5 个测试通过。
  - `cd packages/studio-web && bun run typecheck`：通过。
  - `cd packages/studio-web && bun run test:e2e -- tests/playwright/markdown-preview.spec.ts`：2 个测试通过。
  - `cd packages/studio-web && bun run test:unit -- tests/unit/plan-preview-path.test.ts tests/unit/markdown-viewer.test.tsx tests/unit/markdown-preview-security.test.ts tests/unit/chat-zone.test.tsx tests/unit/chat-actions.test.ts tests/unit/workbench-wiring.test.ts tests/unit/protocol-package-import.test.ts`：7 个测试文件、32 个测试通过。
  - `cd packages/studio-web && bun run test:e2e -- tests/playwright/agent-chat-interaction.spec.ts`：10 个测试通过。
  - `git diff --check`：通过。
  - `rg -n "Confirm Execute|Plan Confirmation|agent\\.plan\\.confirm|agent\\.plan\\.reject|AgentOperationLevel|operation_for_tool_loop|confirmed_cadquery|Confirmed target|Operation level|Operation: Execute|/inform" packages/studio-web/src packages/studio-web/tests -S`：无命中。
- 遗留问题：
  - Phase 6 未处理最终跨 Phase 回归和整体交付 review；Phase 7 将补充端到端验收与清理。

### Phase 7 — 测试、迁移和文档收敛

- 完成情况：
  - 完成 Rust 侧聚焦回归，覆盖 protocol roundtrip、plan package 创建与解析、Agent mode plan 执行、Plan mode 写源文件拒绝、普通文件工具拒绝直接改写 `.py` 模型源、system prompt 和 LLM turn context 的 Agent / Plan 双模式契约。
  - 完成 Web 侧聚焦回归，覆盖 Chat 模式、Plan Package card、Markdown Plan preview `Run Plan` 和 browser wasm bridge。
  - 更新 `docs/known_issues.md`，将旧 confirmation 主流程与 Agent / Plan 双模式冲突记录标记为已处理，并记录 legacy `plans/*.md` 仅只读展示、不生成可执行 `plan_ref`、不触发 Markdown preview `Run Plan`。
  - 完成文档一致性扫描，旧 confirmation 关键词仅命中 deprecated 说明、known issues 历史记录和反向断言测试。
  - 清理 Playwright 回归生成的临时 chat JSONL 文件，保留用户既有未跟踪文件 `AGENTS.new.md` 不动。
- Review：
  - 第一轮 Phase 7 review 发现结果存档顶部仍显示 Phase 6 / Phase 7 未开始，且未记录 Phase 7 执行结果；本次已修复顶部状态表并补充 Phase 7 执行记录。
  - 第二轮 Phase 7 review 未发现阻塞问题或高风险问题。
- 验证：
  - `cargo test -p app-server-protocol --tests`：通过。
  - `cargo test -p app-server-core --tests`：通过，仅有既有 `watch` dead_code warning。
  - `cargo test -p app-server-host --tests`：通过，仅有既有 `watch` dead_code warning。
  - `cargo test -p studio-common --tests`：通过。
  - `cargo test -p studio-web-wasm --tests`：通过。
  - `cargo fmt --check -p app-server-protocol -p app-server-core -p app-server-host -p studio-common -p studio-web-wasm`：通过。
  - `cd packages/studio-web && bun run typecheck`：通过。
  - `cd packages/studio-web && bun run test:unit -- tests/unit/plan-preview-path.test.ts tests/unit/markdown-viewer.test.tsx tests/unit/markdown-preview-security.test.ts tests/unit/chat-zone.test.tsx tests/unit/chat-actions.test.ts tests/unit/workbench-wiring.test.ts tests/unit/protocol-package-import.test.ts`：7 个测试文件、32 个测试通过。
  - `cd packages/studio-web && bun run test:e2e -- tests/playwright/agent-chat-interaction.spec.ts tests/playwright/markdown-preview.spec.ts tests/playwright/wasm-bridge-smoke.spec.ts`：13 个测试通过。
  - `bun run protocol:smoke`：通过。
  - `bun run check:wasm-bindgen`：通过，`wasm-bindgen` 版本为 0.2.117。
  - `git diff --check`：通过。
  - `rg -n "Inform / Plan / Execute|Operation level|Operation: Execute|确认执行|AgentPlanConfirm|AgentCadQueryConfirmation|confirmed_cadquery|Confirmed target|confirmation scope|Plan 确认卡片" docs crates/app-server-core/src crates/app-server-core/tests`：仅命中 deprecated 说明、known issues 历史记录和反向断言测试。
- 遗留问题：
  - Phase 7 当前无新增代码遗留问题；已进入并通过 Plan 级独立 review。

## Plan 级独立 Review

- 完成情况：
  - 已启动独立 subagent 对完整 plan 进行最终 review，覆盖每个 Phase 是否满足原计划验收标准、Phase 之间是否存在行为冲突或重复实现、前序目标是否被后续 Phase 破坏、测试 / 编译验证是否覆盖整体交付标准、结果文档是否准确记录执行情况。
- Review 结论：
  - 未发现阻塞问题或高风险问题。
- Review 证据摘要：
  - 结果存档已记录 Phase 1 至 Phase 7 均已完成，Phase 7 已通过第二轮独立 review，并列出完整验证命令。
  - 协议主路径已收敛为 `AgentMode`、`AgentInvokeRequest { mode, plan_ref, context_refs }`，旧 confirmation 类型只保留 deprecated 说明。
  - 后端保留 plan package 创建 / 解析、execution scope 和 `plan-result.md` 安全更新边界；普通 `.py` 写入、staging、outputs 边界未被后续 Phase 放宽。
  - Web Chat 和 Markdown preview 的 `Run Plan` 均走 `agent.invoke` + `mode: "agent"` + `plan_ref`，Markdown 仍使用 `rehypeSanitize`。
  - `docs/known_issues.md` 已关闭旧 confirmation 主流程问题，并明确 legacy `plans/*.md` 只读展示、不生成可执行 `plan_ref`。
  - 旧流程关键词扫描只命中 deprecated 说明、known issues 历史记录和反向断言测试；最终工作树只剩用户既有未跟踪文件 `AGENTS.new.md`。
