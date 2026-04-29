# Agent / Plan 工作区计划流实施计划

## 背景

当前 CadQuery Agent MVP 使用四种 operation：`Inform`、`Plan`、`Execute`、`Auto`。Plan 阶段通过 `save_cad_plan` 写单个 `plans/*.md` 文件，再由 `agent.plan.confirm` 携带 `confirmed_cadquery` 进入 Execute。这个设计导致 Web UI 中 `/execute` 或 `execute` 模式如果没有结构化 confirmation，就会出现必然失败的权限错误。

新的产品方向是：只保留 `Agent` 和 `Plan` 两个模式。`Plan` 用于只读分析和生成计划档案；`Agent` 是可读写、可执行的正常工作状态。计划不再依赖单独确认流程，而是作为 workspace 内的任务包被 Agent 读取和执行。Markdown 预览打开 plan 时，应能直接触发执行该 plan。

## 用户强制约束识别

- 所有新 plan 必须存放在 workspace `plans/YYYYmmddnn-name/` 目录中。
- `YYYYmmdd` 使用创建当天日期，`nn` 是当天第 n 个 plan，建议从 `00` 开始，按已有同日期 plan 目录最大序号递增。
- 每个 plan 目录必须包含：
  - `request.md`
  - `plan.md`
  - `plan-result.md`
- 模式只保留 `Agent` 和 `Plan`。
- `Plan` 模式只读，`Agent` 模式读写。
- `Agent` 模式必须能直接读取已有 plan 并执行。
- 删除 `confirm plan` 产品流程。
- Markdown 预览打开 plan 时需要提供执行入口。

## 关键产品决策

### Plan 模式的“只读”定义

`Plan` 模式对 CAD 源文件、说明文件、refs、docs 和 outputs 只读；唯一允许的写入是创建或更新 `plans/YYYYmmddnn-name/` 计划档案。否则“Plan 模式只读”和“所有 plan 必须归档三个文件”无法同时成立。

### Agent 模式的写入边界

删除 confirmation 不等于删除安全边界。新的边界应改为：

- `Agent` 模式可以写文件和执行 CadQuery。
- CadQuery `.py` 模型源仍只能通过 CadQuery 专用工具和 staging 机制提交，不能用普通 `write_file` / `patch_file` 直接改写。
- 当 Agent 执行某个 plan 时，必须把 `plans/<id>/plan.md` 中的目标、影响文件、新文件和导出目标解析为本次 execution scope。
- 当 Agent 不带 plan 直接工作时，允许它按用户当前指令生成执行 scope，但仍必须受路径策略、staging、输出目录和单次成功 commit 约束。
- `plans/<id>/request.md` 和 `plans/<id>/plan.md` 一旦创建，应视为计划依据；Agent 执行时只更新 `plans/<id>/plan-result.md`。

### Plan package 格式

`plan.md` 必须同时服务人类阅读和机器解析。建议使用 YAML front matter 记录结构化执行范围，正文记录工程计划。

示例：

```markdown
---
plan_id: 2026050100-create-a-new-box
mode: plan
target_path: parts/box.py
target_type: part
affected_files:
  - parts/box.py
new_files: []
export_targets:
  - outputs/box.step
status: planned
created_at: 2026-05-01T09:12:00+08:00
source_chat_session: chat-1
---

# Create a New Box

## Goal

...
```

`plan-result.md` 初始内容应写入 `status: pending`，执行后追加 Agent run、提交文件、生成 outputs、失败诊断和剩余风险。

### 现有 Agent 流程文档对齐

这次不能只改 `agent-system-prompt.md` 和 `agent-tool-contract.md`。当前仓库里有多份文档定义 Agent 流程，且它们会互相影响后续实现判断：

- `docs/cadquery-mvp/init.md` 是 MVP 主 PRD，当前写的是 “Markdown CAD Plan → 用户确认执行 → CadQuery 生成 / 修改模型”，并要求 Agent 支持 Inform / Plan / Execute。
- `docs/cadquery-mvp/ref_components_parts_assemblies.md` 定义 Ref 到 Agent 的流程，当前原则中仍要求“用户只是讨论时不改文件；用户要方案时输出 Plan；用户确认后才执行”。
- `docs/2026042801-agent-chat-interaction-design/README.md` 是 Web Chat 产品交互约束，当前把 Plan 确认卡片、自然语言确认、`AgentPlanConfirm` 和 `AgentCadQueryConfirmation` 作为主安全模型。
- `docs/2026042801-agent-chat-interaction-design/competitive-analysis.md` 当前把 Agent Mode 描述成 “Agent 自动判断 + Plan 确认后执行”。
- `docs/known_issues.md` 中已有关于 CadQuery Execute confirmation、Plan 绑定和缺少结构化 edit intent 的历史记录，不能只改新计划而不更新这些状态。

Phase 1 必须先统一这些文档，再进入协议和代码改造。否则后续实现会同时面对旧 PRD、旧交互设计和新计划三套互相冲突的约束。

## Phase 1 — 文档与产品语义更新

### 输入

- `docs/cadquery-mvp/init.md`
- `docs/cadquery-mvp/agent-tool-contract.md`
- `docs/cadquery-mvp/agent-system-prompt.md`
- `docs/cadquery-mvp/ref_components_parts_assemblies.md`
- `docs/2026042801-agent-chat-interaction-design/README.md`
- `docs/2026042801-agent-chat-interaction-design/competitive-analysis.md`
- `docs/known_issues.md`

### 前序目标保护

- 保留 CadQuery Agent / staging / app server protocol 作为能力边界。
- 保留 OpenSCAD 现有能力不新增投入。
- 保留 Ref 五层模型：component / part / assembly、instance、feature、face / edge / vertex。

### 操作步骤

1. 新增或更新文档，明确 `Plan` 和 `Agent` 两个模式：
   - `Plan`：只读分析和计划档案创建。
   - `Agent`：读写执行，可直接执行已有 plan。
2. 更新 `docs/cadquery-mvp/init.md`：
   - 将 MVP 主链路改为 “Markdown CAD Plan package → Agent mode 执行 plan → CadQuery 生成 / 修改模型”。
   - 将 “Agent 支持 Inform / Plan / Execute” 改为 “Agent 支持 Agent / Plan 两个模式”。
   - 将用户关键流程中的“确认，生成这个滑盖版本”改为“在 Agent 模式运行该 plan”或“打开 plan 后运行”。
   - 将验收标准中的 Inform / Plan / Execute 改为 Agent / Plan，并明确 `plans/<id>/plan-result.md` 是执行记录。
3. 更新 `docs/cadquery-mvp/ref_components_parts_assemblies.md`：
   - 保留 Ref 定位、component / part / assembly 判断、face / edge / vertex 稳定性说明。
   - 将“用户确认后才执行”改为“只有 Agent mode 执行会修改文件；Plan mode 只创建计划档案”。
   - 将 `Confirmation Needed` 章节改为 `Execution Mode / Plan Run`，说明该 Ref 修改应由 Agent 直接执行还是先生成 plan package。
4. 更新 `docs/2026042801-agent-chat-interaction-design/README.md`：
   - 将交互模型从 Agent 自动判定 Inform / Plan / Execute 改为用户可见 `Agent / Plan` 双模式。
   - 删除 Plan 确认卡片状态机，替换为 Plan Package 卡片和 `Run Plan` 动作。
   - 删除 `/execute`、自然语言确认、轻量确认卡片和 `AgentCadQueryConfirmation` 作为主安全门禁的描述。
   - 将安全风险缓解改为 Agent mode path policy、CadQuery staging、`.py` 专用工具边界和 plan execution scope。
5. 更新 `docs/2026042801-agent-chat-interaction-design/competitive-analysis.md`：
   - 将 Agent Mode 从“自动判断 + Plan 确认后执行”改为“读写执行，可直接使用当前请求或已有 plan”。
   - 将 Plan Mode 从“确认卡片”改为“生成 workspace plan package”。
6. 将 `docs/cadquery-mvp/agent-system-prompt.md` 中 “Execution happens only after confirmation” 改为 “Execution happens only in Agent mode”，并删除 Execute operation 章节。
7. 将 `docs/cadquery-mvp/agent-tool-contract.md` 的 Operation 权限表改成 Agent / Plan mode 权限表；删除或标记废弃 `AgentPlanConfirm`、`AgentPlanReject`、`confirmed_cadquery` 相关说明。
8. 将 `save_cad_plan` 从单文件 `plans/*.md` 改为 plan package：`plans/YYYYmmddnn-name/{request.md,plan.md,plan-result.md}`。
9. 增加 plan package 的 machine-readable front matter 规范和 `plan-result.md` 更新规范。
10. 更新 `docs/known_issues.md`：记录旧 confirmation 流与新 Agent / Plan 模式冲突，并在实现完成后关闭。
11. 对上述文档做一次全文搜索，确保旧主流程关键词不再作为当前方案出现：
   - `Inform / Plan / Execute`
   - `确认执行`
   - `AgentPlanConfirm`
   - `AgentCadQueryConfirmation`
   - `confirmed_cadquery`
   - `Plan 确认卡片`

### 验收标准

- 文档不再把 confirmation 作为唯一执行入口。
- 文档清楚区分 `Plan` 只读边界和 `Agent` 读写边界。
- plan package 三文件结构和命名规则可由后端直接实现。
- 文档明确 legacy `plans/*.md` 的兼容策略。
- `init.md`、Ref PRD、Agent Chat 交互设计、system prompt 和 tool contract 对 Agent 流程的描述一致。
- 旧 confirmation 术语只允许出现在“历史设计 / deprecated / known issues”上下文中，不能作为当前产品主流程出现。

## Phase 2 — Protocol 与共享数据模型收敛

### 输入

- `crates/app-server-protocol/src/protocol.rs`
- `crates/app-server-protocol/tests/borsh_payload_roundtrip_tests.rs`
- `crates/studio-web-wasm/src/wasm_bridge/*`
- `packages/studio-web` protocol package 生成或桥接测试

### 前序目标保护

- 保持桌面 GUI 和 Web 走同一份 protocol。
- 保持 protocol 不绑定具体 transport。
- 保持 Borsh / serde roundtrip 测试覆盖协议变更。

### 操作步骤

1. 新增 `AgentMode`：
   - `Agent`
   - `Plan`
2. 将 `AgentInvokeRequest` 从 `operation` 语义迁移到 `mode` 语义，并新增可选 `plan_ref`：
   - `mode: AgentMode`
   - `plan_ref: Option<PathHandle>`
   - `context_refs: Vec<String>`
3. 引入 plan package 数据结构：
   - `AgentPlanPackageRef`
   - `AgentPlanSavedEvent`
   - `AgentPlanRunRequest` 可选；若不新增命令，则复用 `AgentInvokeRequest { mode: Agent, plan_ref }`。
4. 废弃 `AgentPlanConfirmRequest`、`AgentPlanRejectRequest` 和 `AgentCadQueryConfirmation`。
5. 提升 protocol version，并更新 host / wasm / web 协议测试。
6. 如需过渡兼容，保留旧字段反序列化但不在 Web 新 UI 中使用；旧 `agent.plan.confirm` 返回明确 deprecated error。

### 验收标准

- 协议层只暴露 `Agent` / `Plan` 两种产品模式。
- Web 和 host 对 `plan_ref` 的编码一致。
- 旧 confirmation 命令不再是主流程。
- 协议 roundtrip 测试覆盖 plan package ref 和 Agent mode 执行 plan。

## Phase 3 — 后端 Plan Package 存储与解析

### 输入

- `crates/app-server-core/src/agent/tools/semantic.rs`
- `crates/app-server-core/src/agent/tools/registry.rs`
- `crates/app-server-core/src/agent/tools/registry/schemas.rs`
- `crates/app-server-core/src/agent/tools/readonly/project.rs`
- `crates/app-server-host/src/plan_extraction.rs`
- `crates/app-server-core/tests/agent_tool_tests.rs`
- `crates/app-server-host/tests/plan_extraction_tests.rs`

### 前序目标保护

- 保持计划路径必须在 workspace `plans/` 下，拒绝 symlink 和 workspace escape。
- 保持普通文件工具不能直接改 CadQuery `.py` 模型源。
- 保持 outputs 只能由 CadQuery runner / export 流生成。

### 操作步骤

1. 将 `save_cad_plan` 改为创建 plan package：
   - 分配 `plans/YYYYmmddnn-name/`。
   - 写入 `request.md`。
   - 写入 `plan.md`，包含 YAML front matter。
   - 初始化 `plan-result.md`。
2. 新增 plan id 分配器：
   - 扫描 `plans/YYYYmmdd??-*` 目录。
   - 当天最大 `nn` + 1。
   - slug 只允许 ASCII 小写、数字和 `-`。
3. 新增 plan package parser：
   - 读取并校验 `plan.md` front matter。
   - 输出 execution scope：target、target type、affected files、new files、export targets。
   - 校验 plan 目录内三文件完整性。
4. 更新 `get_project_context`：
   - plans 从单个 `.md` 列表变为 plan package 列表。
   - 返回 plan id、title、status、target、updated time。
5. 更新 `save_cad_plan` tool schema：
   - 成功结果返回 `plan_id`、`plan_ref` 目录、`request_path`、`plan_path`、`result_path`。
6. 兼容 legacy `plans/*.md`：
   - 只读展示。
   - 不作为可直接执行 plan。
   - 可在后续单独做迁移工具。

### 验收标准

- 新 plan 按目录三文件创建。
- 当天序号稳定递增，不覆盖已有 plan。
- plan parser 能拒绝缺文件、越界路径、非法 target、非法 export target。
- `get_project_context` 能列出新 plan package。

## Phase 4 — 后端 Agent Mode 执行模型

### 输入

- `crates/app-server-host/src/dispatcher.rs`
- `crates/app-server-core/src/agent.rs`
- `crates/app-server-core/src/agent/tools.rs`
- `crates/app-server-core/src/agent/tools/registry.rs`
- `crates/app-server-core/src/agent/tools/file_write/path_policy.rs`
- `crates/app-server-core/src/agent/tools/cadquery/*`
- 相关 host / core tests

### 前序目标保护

- 保持所有文件 I/O、CadQuery 执行和 outputs 写入仍由 app server 承接。
- 保持 CadQuery 执行必须经过 staging，失败、超时或取消不得污染真实 workspace。
- 保持 `.py` 模型文件不得通过普通文档写入工具改写。
- 保持单次 Agent run 最多一次成功 CadQuery commit。

### 操作步骤

1. 删除 `operation_for_tool_loop` 的四模式判定，改为 `mode_for_tool_loop`：
   - `Plan`：只读工具 + `save_cad_plan`。
   - `Agent`：只读工具 + 写入工具 + CadQuery dry run / execute + plan result update。
2. 删除 `confirmed_cadquery` 作为执行前提，改为 `execution_scope`：
   - `plan_ref` 存在时，从 `plans/<id>/plan.md` 解析。
   - `plan_ref` 不存在时，由 Agent run 根据当前用户指令和工具参数形成 scope，但仍受 registry path policy 限制。
3. 将 `write_file` / `patch_file` 的 confirmation scope 校验替换为 Agent mode path policy：
   - 允许 Agent mode 写安全文本根。
   - 禁止普通工具写 `.py` 模型源。
   - 禁止直接写 `chats/` 和 `outputs/`。
   - 允许更新当前 plan 的 `plan-result.md`。
4. 将 `cadquery_execute` 的 confirmation scope 校验替换为 execution scope 校验：
   - 带 `plan_ref` 时 target / affected / new / exports 必须匹配 plan。
   - 不带 `plan_ref` 时 target 必须在 `components/`、`parts/`、`assemblies/`，exports 必须是 runner 允许的默认 outputs。
5. 增加 `update_plan_result` 语义工具或在 `cadquery_execute` 成功 / 失败后由 host 写入 `plan-result.md`。
6. 废弃 `agent.plan.confirm` / `agent.plan.reject` dispatcher 分支：
   - 移除主流程调用。
   - 如果保留 protocol 兼容，返回 deprecated error，并提示使用 Agent mode 执行 plan。
7. 更新错误文案：
   - 不再出现 “Ensure you have confirmed the plan”。
   - 改为 “Switch to Agent mode or run an existing plan.”。

### 验收标准

- `Agent` mode 能直接执行自由请求。
- `Agent` mode 带 `plan_ref` 时能执行已有 plan。
- `Plan` mode 不能改 CAD 源文件、不能生成 outputs、不能调用 runner。
- 删除 confirmation 后仍无法用普通文件工具改 `.py` 模型源。
- CadQuery staging、安全路径和单次成功 commit 约束仍有效。

## Phase 5 — Web Chat 模式简化

### 输入

- `packages/studio-web/src/workbench/chat-composer.tsx`
- `packages/studio-web/src/workbench/chat-actions.ts`
- `packages/studio-web/src/workbench/chat-zone.tsx`
- `packages/studio-web/src/workbench/chat-messages.tsx`
- `packages/studio-web/tests/unit/chat-zone.test.tsx`
- `packages/studio-web/tests/playwright/agent-chat-interaction.spec.ts`

### 前序目标保护

- 保持 Chat session、streaming token、context refs、selection pills 和 cancel 行为。
- 保持 Plan 生成后可以在 Chat 中被用户看见和打开。
- 保持 Agent run busy 状态下不能重复触发。

### 操作步骤

1. 将输入区下拉框从 `auto / inform / plan / execute` 改为 `Agent / Plan`。
2. 删除 `/inform`、`/execute` 产品命令；保留 `/plan` 可作为 Plan mode 快捷方式，新增 `/agent` 可作为 Agent mode 快捷方式。
3. `sendChatMessage()` 改为发送 `mode` 而不是旧 `operation`。
4. 删除 Plan Confirmation 卡片上的 `Confirm Execute` 和 `Cancel`。
5. 将 Plan 卡片改为 Plan Package 卡片：
   - 显示 plan id、target、affected files、exports。
   - 提供 `Open Plan`。
   - 提供 `Run Plan`，触发 `agent.invoke { mode: Agent, plan_ref }`。
6. 更新权限错误卡片文案，移除 confirmation 说明。
7. 更新 Web 单测和 Playwright：
   - 默认模式为 Agent。
   - 切换 Plan 后发送请求只生成 plan package。
   - Run Plan 发送 Agent mode + plan_ref。

### 验收标准

- UI 只出现 `Agent` 和 `Plan` 两个模式。
- `/execute` 不再作为可用命令出现。
- Plan 卡片不再展示 confirmation 概念。
- Run Plan 会走 Agent mode。
- selection context 仍随请求发送。

## Phase 6 — Markdown Plan Preview 执行入口

### 输入

- `packages/studio-web/src/viewers/markdown-viewer.tsx`
- `packages/studio-web/src/workbench/canvas-zone.tsx`
- `packages/studio-web/src/workbench/workbench-layout.tsx`
- `packages/studio-web/tests/unit/markdown-preview-security.test.ts`
- 新增或扩展 Markdown viewer 单测

### 前序目标保护

- 保持 Markdown 安全渲染和 rehype sanitize。
- 保持普通 Markdown 文件不出现 plan 专属执行入口。
- 保持执行入口只调用 app server protocol，不在前端绕过 Agent。

### 操作步骤

1. 增加 plan path 识别：
   - 当前打开路径匹配 `plans/YYYYmmddnn-name/plan.md`。
   - 可选支持打开 `request.md` 或 `plan-result.md` 时定位同目录 plan。
2. `MarkdownViewer` 增加可选 `onRunPlan(planRef)` 回调。
3. 在 plan Markdown 顶部或 viewer toolbar 展示 `Run Plan` 按钮。
4. 点击 `Run Plan`：
   - 确保有当前 Chat session，必要时创建。
   - 发送 `agent.invoke { mode: Agent, plan_ref }`。
   - UI 状态提示 “Running plan <id> in Agent mode”。
5. 执行完成后刷新 `plan-result.md` 和相关文件 tab。
6. 普通 `.md` 文件不渲染该按钮。

### 验收标准

- 打开 `plans/<id>/plan.md` 时可直接触发 Agent mode 执行。
- 普通 Markdown 预览不出现执行入口。
- 执行入口不破坏 Markdown sanitize。
- 执行完成后能看到 `plan-result.md` 更新。

## Phase 7 — 测试、迁移和文档收敛

### 输入

- Rust core / host / protocol tests
- Web unit / Playwright tests
- `docs/cadquery-mvp/*`
- `docs/known_issues.md`
- `prompt-archives` 对应结果存档

### 前序目标保护

- 不把 prompt-archives 的执行计划和 workspace `plans/` 产品计划混为一谈。
- 不引入 Python 辅助脚本。
- 不跳过 app server protocol 直接读写 workspace。

### 操作步骤

1. Rust 聚焦回归：
   - protocol roundtrip。
   - plan package 创建与解析。
   - Agent mode plan 执行。
   - Plan mode 写源文件拒绝。
   - `.py` 普通写入拒绝。
2. Web 聚焦回归：
   - Chat 模式。
   - Plan package card。
   - Markdown Plan preview Run Plan。
3. 端到端验证：
   - Plan mode 创建 `plans/YYYYmmddnn-name/`。
   - Markdown 打开 plan 并执行。
   - Agent mode 直接读取已有 plan 执行。
   - `plan-result.md` 记录成功和失败结果。
4. 更新 `docs/known_issues.md`：
   - 关闭 confirmation 流造成 `/execute` 必然失败的问题。
   - 记录 legacy `plans/*.md` 兼容范围。
5. 增加文档一致性检查：
   - `rg -n "Inform / Plan / Execute|确认执行|AgentPlanConfirm|AgentCadQueryConfirmation|confirmed_cadquery|Plan 确认卡片" docs`
   - 检查命中项是否只存在于历史设计、deprecated 说明或 known issues 中。
6. 更新 `plan-00-result.md`，逐 Phase 记录实现、review 和验证。

### 验收标准

- 旧 confirmation 流从产品主路径中移除。
- 新 Agent / Plan 双模式在文档、协议、后端和前端中一致。
- plan package 三文件结构可被创建、预览、执行和记录结果。
- 所有现有 Agent 流程文档与新产品流一致，不再留下旧流程作为当前实现依据。
- 回归测试通过。

## 风险与处理方式

| 风险 | 影响 | 处理方式 |
|---|---|---|
| 删除 confirmation 后写入边界变宽 | Agent 可能在自由请求中误改文件 | 通过 Agent mode path policy、CadQuery staging、`.py` 专用工具边界和 plan scope parser 控制 |
| `Plan` 只读与 plan 文件创建存在语义冲突 | 产品说明容易自相矛盾 | 明确 Plan 对 CAD 源文件只读，但允许写计划档案 |
| legacy `plans/*.md` 与新 plan package 并存 | UI 和 parser 判断复杂 | 新 plan package 可执行；legacy 单文件只读展示，迁移另开任务 |
| Protocol breaking change 影响 app / web / wasm | 多端不一致 | 提升 protocol version，集中更新 roundtrip 和 wasm bridge 测试 |
| Markdown 执行入口误出现在普通文档 | 用户可能误触执行 | 只对 `plans/<id>/plan.md` 结构化路径显示 Run Plan |
| Agent mode 自由执行缺少 plan scope | 权限模型比原 confirmation 更宽 | 保留 staging、路径根、`.py` 专用工具和 outputs 默认命名校验；复杂写入鼓励先生成 plan |

## 建议提交拆分

1. `docs: define agent plan package workflow`
2. `feat(protocol): replace agent operation with agent mode`
3. `feat(core): persist cad plans as workspace packages`
4. `feat(agent): execute plans through agent mode`
5. `feat(web): simplify agent modes and plan actions`
6. `feat(web): run plans from markdown preview`
7. `test: cover agent plan package workflow`

## 本计划不做的事

- 不迁移历史 prompt-archives。
- 不把 legacy `plans/*.md` 自动改写成新目录结构。
- 不新增 Python 脚本。
- 不改变 CadQuery runner 的 staging 原子性边界。
