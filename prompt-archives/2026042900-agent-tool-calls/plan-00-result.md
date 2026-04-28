# Agent Tool Call 能力补全执行结果

## 当前状态

计划已创建，并已根据 plan review 修订 Auto 权限、canonical tool schema、CadQuery 单次成功提交、Execute 后 `.md` / Ref Map 更新和 `copy_file()` 复制边界。随后补充了 CadQuery 专用工具完整行为合同，并将 Phase 5 改为 `cadquery_analyze_source()`、`cadquery_check_source()`、`cadquery_dry_run()`、`cadquery_execute()`、`cadquery_get_result()`、`cadquery_resolve_selection()` 六个专用工具。

Phase 0 已完成实现、聚焦验证和最终独立复审。Phase 0 固化了 Agent tool registry、Operation 权限表、路径范围合同和 canonical schema，并同步了 Agent system prompt 与工具合同文档。

Phase 1 已完成实现、两轮独立 review 收敛和聚焦回归。Phase 1 将 LLM tool loop 切换到 registry 驱动的统一执行入口，并补齐 operation 过滤、执行前权限校验、confirmation scope 校验、通用 tool push event 与 Chat JSONL 记录。

Phase 2 已完成实现、多轮独立 review 收敛和完整聚焦回归。Phase 2 将只读上下文工具接入统一执行入口，补齐结构化读取、目录列举、文本搜索、项目上下文、当前选择和 Ref 解析能力，并保护 Phase 1 已建立的 registry 权限边界。

Phase 3 已完成实现、多轮独立 review 收敛和完整聚焦回归。Phase 3 新增 CAD Plan 与 Chat summary 语义工具，将 Plan proposal、Plan confirmation、protocol version 和 Web 确认流绑定到同一份 saved Plan 结构化结果。

Phase 4 已完成实现、独立 review 和完整聚焦回归。Phase 4 新增受限普通文件写入工具，按 confirmation 范围区分 `affected_files` 与 `new_files`，并保持 CAD Plan、Chat JSONL、CadQuery `.py` 模型源的专用工具边界。

## Phase 结果记录

| Phase | 状态 | 结果 |
|---|---|---|
| Phase 0 — Tool 能力盘点与权限合同 | 已完成 | 已新增 registry/schema/path policy，补充 `docs/cadquery-mvp/agent-tool-contract.md`，同步 system prompt；聚焦测试与独立复审通过 |
| Phase 1 — Tool Registry 与统一执行入口 | 已完成 | 已接入 registry tool loop、统一运行上下文、执行前权限与路径校验、通用 tool 事件和 Chat 记录；两轮独立 review 与聚焦回归通过 |
| Phase 2 — 只读上下文工具补齐 | 已完成 | 已接入 `read_file`、`list_directory`、`search_files`、`get_project_context`、`get_selection`、`resolve_ref`；多轮独立 review 与完整聚焦回归通过 |
| Phase 3 — CAD Plan 与 Chat 语义持久化工具 | 已完成 | 已接入 `save_cad_plan`、`update_chat_summary`、同 run `plan_ref` 确认校验、protocol v3 和 Web Plan 确认范围；多轮独立 review 与完整聚焦回归通过 |
| Phase 4 — 受限文件写入工具 | 已完成 | 已接入 `write_file`、`patch_file`、`copy_file`，补齐 registry 与 executor 双层防护、普通写入范围、hash 冲突检测、symlink / hard link alias 拒绝和 CadQuery `.py` copy 边界；独立 review 与完整聚焦回归通过 |
| Phase 5 — CadQuery 专用工具与执行边界 | 未开始 | 等待执行 |
| Phase 6 — 前端确认流与协议补强 | 未开始 | 等待执行 |
| Phase 7 — 权限模型回归、文档同步与端到端验证 | 未开始 | 等待执行 |

## 执行记录

本文件将在每个 Phase 完成后实时更新，记录完成情况、变更摘要、验证命令、review 结论和遗留问题。

### Phase 0 — Tool 能力盘点与权限合同

完成情况：

- 新增 `AgentToolSpec` registry，覆盖 17 个目标工具：`read_file`、`list_directory`、`search_files`、`get_project_context`、`get_selection`、`resolve_ref`、`save_cad_plan`、`update_chat_summary`、`write_file`、`patch_file`、`copy_file`、`cadquery_analyze_source`、`cadquery_check_source`、`cadquery_dry_run`、`cadquery_execute`、`cadquery_get_result`、`cadquery_resolve_selection`。
- 固化 Operation 权限：Auto 判定前仅只读上下文；Plan 可用 `save_cad_plan` 与 `cadquery_check_source`；Execute 才允许受确认范围限制的普通写入、`cadquery_dry_run` 和 `cadquery_execute`。
- 固化路径范围：普通写入拒绝 `chats/`、`outputs/` 和 CadQuery `.py` 模型源；`copy_file` 仅允许 `.py` byte-for-byte 复制；CadQuery staging 目录按现有实现拒绝 `.budn_staging`；`cadquery_execute` 只允许 confirmed outputs。
- 固化 canonical schema：逐工具定义输入 schema、成功 schema、错误 schema；错误 schema 包含 `tool_call_id`、`python_import_error`、permission denied / conflict / cancelled / timeout 等通用错误字段。
- 新增文档 `docs/cadquery-mvp/agent-tool-contract.md`，同步更新 `docs/cadquery-mvp/agent-system-prompt.md` 的工具权限规则。

最终权限表摘要：

| Operation | 自动 LLM tool |
|---|---|
| Inform | 只读上下文工具 + `update_chat_summary` |
| Plan | Inform 工具 + `save_cad_plan` + `cadquery_check_source` |
| Execute | 只读上下文工具 + `update_chat_summary` + `cadquery_check_source` + `cadquery_dry_run` + `cadquery_execute` + confirmation 范围内的 `write_file` / `patch_file` / `copy_file` |
| Auto 判定前 | `read_file`、`list_directory`、`search_files`、`get_project_context`、`get_selection`、`resolve_ref`、`cadquery_analyze_source`、`cadquery_get_result`、`cadquery_resolve_selection` |

验证命令：

- `cargo test -p app-server-core --test agent_tool_tests --test agent_tool_registry_tests`
  - 结果：`agent_tool_registry_tests` 5 passed；`agent_tool_tests` 12 passed。
  - 备注：仍有既有 `watch.rs` dead_code warning，未在本 Phase 处理。

独立 review：

- 第一次 review 指出 canonical schema、路径范围、测试覆盖和结果存档缺口，已修正。
- 第二次 review 指出 `.budn_staging`、错误 schema、CadQuery summary/contract schema 和结果存档缺口，已修正。
- 最终 review 未发现 block / important 问题，确认 Phase 0 验收满足。

### Phase 1 — Tool Registry 与统一执行入口

完成情况：

- 将文本 Agent LLM tool loop 改为 `run_tool_loop_with_registry()`，由 Phase 0 registry 按 operation 生成可见工具集合。
- 新增 `AgentToolRunContext`，统一携带 workspace root、session id、run id、operation、selection snapshot、active selection、context refs 和 confirmation scope，后续工具实现不再依赖临时参数扩展。
- 新增 `AgentToolConfirmationScope`，在工具执行前校验 `affected_files`、`new_files` 和 `export_targets`。
- 执行 tool 前统一做权限判定；未授权工具返回结构化错误结果并跳过 executor，仍记录 tool start/result。
- 执行 tool 前统一校验 path policy：
  - 拒绝 denied roots，包括 `outputs/`、`.budn_staging` 等。
  - 普通 `write_file()` / `patch_file()` 拒绝修改 `components/`、`parts/`、`assemblies/` 下的 CadQuery `.py` 模型源。
  - `cadquery_execute()` 的 `export_targets` 必须是字符串数组，并且全部在 confirmed `outputs/` scope 中。
  - `cadquery_execute()` 带 `export_formats` 时必须提供有效 `export_targets`。
- 删除旧的公开绕行入口 re-export：`agent_tool_definitions()` 与 `run_tool_loop()` 不再从 crate root 暴露，避免绕过 registry 权限入口。
- `WorkspaceToolExecutor` 暂未实现的已注册工具返回结构化 `unsupported_tool`，避免非结构化 `Unknown tool` 进入 Chat 记录。
- `AgentToolEventRecorder` 将通用 LLM tool start/result 同步写入 Agent push event 与 Chat JSONL，不再只有 CadQuery Execute tool 被记录。
- Auto 进入 tool loop 前会收敛到具体 operation：有确认时为 Execute；明确 `/plan`、`plan`、`方案`、`计划` 时为 Plan；其余默认 Inform，避免未确认解释型请求直接暴露 Plan/Execute 工具。

验证命令：

- `cargo test -p app-server-core --test agent_tool_tests --test agent_tool_registry_tests --test agent_tests --test chat_tests`
  - 结果：`agent_tests` 13 passed；`agent_tool_registry_tests` 5 passed；`agent_tool_tests` 23 passed；`chat_tests` 8 passed。
  - 备注：仍有既有 `watch.rs` dead_code warning，未在本 Phase 处理。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests --test dispatcher_pure_fn_tests`
  - 结果：`dispatcher_pure_fn_tests` 13 passed；`shared_dispatcher_roundtrip_tests` 10 passed。
  - 备注：`shared_dispatcher_roundtrip_tests` 中存在既有未使用 helper warning，未在本 Phase 处理。

独立 review：

- 第一轮 review 发现 3 个 block：普通文件工具未强制 CadQuery `.py` 边界、`cadquery_execute` 未强制 confirmed export scope、`ToolExecutor` 缺少统一运行上下文；已通过测试与实现修复。
- 第一轮 review 还指出 Auto 判定过宽、旧公开 API 可绕过 registry、未实现工具返回非结构化结果；已收紧 Auto 判定、移除旧公开 re-export，并统一返回 `unsupported_tool`。
- 第二轮 review 未发现 block；指出 `export_targets` 类型校验不严，已新增非字符串 `export_targets` 红绿用例并修复。
- 最终回归通过，Phase 1 无遗留 block。

### Phase 2 — 只读上下文工具补齐

前序目标保护：

- 保持 Phase 1 的 registry 驱动入口，所有只读工具仍通过 `WorkspaceToolExecutor`、operation 权限表、path policy 和统一 tool 事件执行。
- 保持 Auto 判定前只能访问只读上下文工具的边界，不向 Plan / Execute 写入能力扩散。
- 保持 `chats/`、`outputs/`、`.budn_staging` 和 denied roots 的读取边界，避免只读工具通过符号链接或路径变形暴露受保护内容。

完成情况：

- 新增只读工具实现并接入统一执行入口：`read_file`、`list_directory`、`search_files`、`get_project_context`、`get_selection`、`resolve_ref`。
- `read_file()` 返回结构化 JSON，包含 `status`、`tool`、`path`、`text`、`offset`、`bytes_read`、`file_size`、`truncated`、`hash`；支持 `offset` / `max_bytes`，并将单次读取限制在 64 KiB。
- `read_file()` 拒绝 workspace escape、denied roots、指向 denied roots 的符号链接、无效 UTF-8 和二进制特征内容。
- `list_directory()` 支持递归、substring pattern、kind filter 和 `max_entries`，并在截断前完成过滤；目录列举会跳过或拒绝不安全符号链接。
- `search_files()` 在安全文本文件内递归搜索，跳过 denied roots、二进制文件、过大文件和不安全符号链接，`max_results` 限制为 50。
- `get_project_context()` 汇总 components、parts、assemblies、plans、chats，并拒绝把受保护目录符号链接当作有效项目内容。
- `get_selection()` 返回当前选择快照、激活索引和上下文 Ref。
- `resolve_ref()` 支持对象 Ref、真实 `REFS.features` feature Ref、raw selection 和 selection candidate；对 path-like ref、符号链接逃逸、没有稳定 feature 定义的几何选择返回结构化不稳定结果。
- `resolve_ref()` 的 REFS 解析会跳过字符串、注释和非 dict `REFS` 赋值，避免把普通文本误判为稳定 feature map。
- 更新 canonical schema：为只读工具补齐上限字段、substring 语义说明，以及 `resolve_ref` 成功结果中的 `owner_doc_path`、`raw_ref_text` 字段。

验证命令：

- `cargo test -p app-server-core --test agent_tool_tests --test agent_tool_registry_tests --test agent_tests --test chat_tests`
  - 结果：`agent_tests` 13 passed；`agent_tool_registry_tests` 5 passed；`agent_tool_tests` 55 passed；`chat_tests` 8 passed。
  - 备注：仍有既有 `watch.rs` dead_code warning，未在本 Phase 处理。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests`
  - 结果：`shared_dispatcher_roundtrip_tests` 10 passed。
  - 备注：仍有既有未使用 helper warning，未在本 Phase 处理。
- `git diff --check`
  - 结果：通过。
- 新增文件与函数规模检查
  - 结果：通过；新增只读工具文件均小于 500 行，新增函数均小于 50 行。

独立 review：

- 第一轮 review 指出：`resolve_ref` 存在字符串 / 注释误判、selection candidate 未校验 feature 是否真实存在、若干只读工具对符号链接和 denied roots 覆盖不足；已补充红绿用例并修复。
- 第二轮 review 指出：`get_project_context` 对 paired `.md` 符号链接、目录列举中符号链接子项、Ref owner source 符号链接逃逸仍需强化；已补充测试并修复。
- 最终 review 未发现 block / important 问题。
- 遗留 minor：`source_has_refs_feature` 当前 lexical scanner 不区分 top-level `REFS` 与函数局部或属性赋值形式的 `REFS`。该问题不影响本 Phase 已定义的验收口径，后续若需要严格限定 top-level `REFS`，应单独补充 parser 规则和测试。

### Phase 3 — CAD Plan 与 Chat 语义持久化工具

前序目标保护：

- 保持 Phase 1 的 registry 驱动入口，`save_cad_plan` 仅在 Plan operation 可用，`update_chat_summary` 仅在 Inform / Plan / Execute 可用，Auto 直接调用会被拒绝。
- 保持 Phase 2 的只读工具无副作用边界，Plan 持久化只写 `plans/`，不允许借由 Plan 工具修改 `components/`、`parts/`、`assemblies/` 下的 `.py` 或对象说明 `.md`。
- 保持 CadQuery 执行必须经过 confirmation + staging 的边界，普通 Agent invoke 不再接受直接携带的 CadQuery confirmation。

完成情况：

- 新增 `save_cad_plan()` 语义工具：
  - 只在 workspace `plans/` 下创建 Markdown CAD Plan。
  - 输入包含 `title`、`target_ref`、`resolved_target`、`affected_files`、`new_files`、`export_targets`、`strategy`、`risks`、`acceptance`、`execution_boundary`。
  - 输出包含 `plan_ref`、`display_path`、`hash`、`summary`、`target_ref`、`target_path`、`affected_files`、`new_files`、`export_targets`、`execution_boundary`、`run_id`。
  - 校验 `resolved_target` 必须在 `affected_files` 或 `new_files` 中，避免 saved Plan 与后续 confirmation 范围脱节。
  - 校验 `export_targets` 必须位于 `outputs/`，扩展名只允许当前 runner 会生成的 `.step`、`.stl`、`.3mf`，文件名必须匹配 `outputs/{resolved_target 文件名 stem}.{extension}`。
  - 对 `plans/` 目录和目标文件使用 symlink 安全检查，避免已存在 symlink 文件被写穿。
- 新增 `update_chat_summary()` 语义工具：
  - 通过 `ChatStore::update_summary()` 写入 Chat meta 记录，不暴露 `chats/*.jsonl` 任意写入能力。
  - `related_files` 仅允许安全 workspace path root。
  - 显式空 `related_files` 会覆盖旧 session 摘要中的相关文件列表。
  - 成功结果包含 `message_id`，便于 Chat history 追溯。
- Plan proposal 与 confirmation 绑定：
  - `agent.plan_proposed` 只从同一 run 的 `save_cad_plan` tool result 产生 confirmable proposal，不再把文本 JSON fallback 作为可确认计划。
  - `AgentPlanConfirm` 读取同一 session 内同一 plan run 保存的 CAD Plan，并校验 `plan_ref`、target、affected files、new files、export targets 与 confirmation 一致。
  - `agent.invoke` 若直接携带 `confirmed_cadquery` 会返回 `InvalidCommand`，确认执行必须走 `agent.plan.confirm`。
  - `validate_cadquery_confirmation()` 增加 export target 扩展名、格式一致性和 runner 默认输出文件名校验。
- Protocol / transport / client 同步：
  - `AgentPlanProposedEvent` 新增 `plan_ref` 和 `new_files` 字段，并用 serde default 保持反序列化兼容。
  - `CURRENT_PROTOCOL_VERSION` 升级到 3，server capability 调整为 3..3，host handshake 使用协议协商并在失败时输出 transport error。
  - studio-app、studio-common、studio-web-wasm 和 Web protocol package 测试同步 protocol version 3。
- Web 确认流同步：
  - `ChatZone` 只对当前 Chat session 的 Agent event 生成 pending Plan，避免跨 session 误确认。
  - 无 `plan_ref` 的 `agent.plan_proposed` 不再展示确认按钮。
  - `agent.done` 不会清除刚生成的 Plan 卡片。
  - `confirmPlan()` 使用后端 proposal 的 `plan_ref`、affected files、new files、export targets，不再从 prompt 或 selection 重新构造范围。
  - `export_formats` 从 proposal 的 export target 扩展名推导，当前与 runner 默认输出格式保持一致。
- 文档同步：
  - 更新 `docs/cadquery-mvp/agent-system-prompt.md`，明确 Plan 阶段需要保存 CAD Plan。
  - 更新 `docs/cadquery-mvp/agent-tool-contract.md`，记录 `save_cad_plan`、`update_chat_summary`、protocol v3、Plan confirmation 和 export target 约束。
  - 更新 `docs/known_issues.md`，记录 `plan_ref` 持久绑定已处理，并补充 direct `agent.invoke` confirmation route 已被拒绝。

验证命令：

- `cargo test -p app-server-core --test agent_tool_tests --test agent_tool_registry_tests --test agent_tests --test chat_tests`
  - 结果：`agent_tests` 13 passed；`agent_tool_registry_tests` 5 passed；`agent_tool_tests` 68 passed；`chat_tests` 8 passed。
  - 备注：仍有既有 `watch.rs` dead_code warning，未在本 Phase 处理。
- `cargo test -p app-server-host --test plan_extraction_tests --test shared_dispatcher_roundtrip_tests --test dispatcher_pure_fn_tests --test mpsc_transport_tests --test in_process_roundtrip_tests --test websocket_smoke_roundtrip --test session_tests --test session_lifecycle_tests`
  - 结果：`dispatcher_pure_fn_tests` 17 passed；`in_process_roundtrip_tests` 1 passed；`mpsc_transport_tests` 5 passed；`plan_extraction_tests` 19 passed；`session_lifecycle_tests` 5 passed；`session_tests` 3 passed；`shared_dispatcher_roundtrip_tests` 13 passed；`websocket_smoke_roundtrip` 6 passed。
  - 备注：`shared_dispatcher_roundtrip_tests` 中仍有既有未使用 helper warning，未在本 Phase 处理。
- `cargo test -p app-server-protocol --test borsh_payload_roundtrip_tests --test borsh_frame_tests`
  - 结果：`borsh_frame_tests` 7 passed；`borsh_payload_roundtrip_tests` 15 passed。
- `cargo test -p studio-common --test managed_client_tests --test app_server_client_tests`
  - 结果：`app_server_client_tests` 2 passed；`managed_client_tests` 20 passed。
- `cargo test -p app-server-transport --test websocket_wire_tests`
  - 结果：4 passed。
- `cargo check -p studio-app`
  - 结果：通过。
- `cargo test -p studio-web-wasm --test wasm_bridge_smoke`
  - 结果：0 tests，命令通过。
- `bun --filter @budn/studio-web test:unit -- chat-zone.test.tsx chat-actions.test.ts workbench-wiring.test.ts protocol-package-import.test.ts`
  - 结果：22 passed。
- `bun --filter @budn/studio-web typecheck`
  - 结果：通过。
- `git diff --check`
  - 结果：通过。
- 新增文件规模检查：
  - `semantic.rs` 464 行，`semantic_chat.rs` 86 行，`semantic_export.rs` 56 行；均小于 500 行。

独立 review：

- 第一轮 review 指出 `save_cad_plan.export_targets` 未被 registry 允许、direct executor 未做 operation 权限校验、saved Plan 查找范围过窄、`update_chat_summary` schema 与实现不一致；已通过测试与实现修复。
- 第二轮 review 指出 protocol version 兼容、confirmation 未绑定同 run saved Plan、`message_id` 成功 schema 缺口；已升级 protocol v3 并修复。
- 第三轮 review 指出 Web 仍使用旧协议和旧确认范围、host handshake 未协商、fallback 文本 Plan 仍可能产生不可确认提案、文档 schema 不一致；已修复。
- 第四轮 review 指出 Web 会在 `agent.done` 后清除 Plan 卡片、`save_cad_plan` export targets 语义与 confirmation 不一致、Web 固定 `step` 格式；已修复。
- 第五轮 review 指出 `resolved_target` 未要求进入确认范围、export target 扩展名和 export format 不一致、Web 仍展示 `plan_ref: null` proposal；已修复。
- 第六轮 review 指出 direct `agent.invoke` 可绕过 `AgentPlanConfirm`、Chat summary 显式空相关文件无法清空、文档 outputs 策略不准确；已修复。
- 第七轮 review 指出 Web pending Plan 未按 session 过滤、export target 可保存 runner 不会生成的文件名；已修复。
- 最终 review 未发现 block / important 问题。

遗留 minor：

- `useStreamAccumulator()` 仍消费未按当前 Chat session 过滤的 raw events，可能短暂显示其他 session token，但不会导致跨 session Plan 被确认。
- `recentNonTokenEvents()` 先取全局最近 10 条再过滤 session；如果其他 session event 很多，当前 session 较早 Plan 卡片可能消失。该问题影响展示可靠性，不影响后端确认安全。
- `save_cad_plan` 在 `symlink_metadata` 判定未占用后写入文件，已避开既有 symlink 文件，但仍存在极小并发替换窗口。当前本地 workspace 威胁模型下未作为 Important 处理，后续若要强化不可信 workspace，可改成原子创建且不跟随 symlink 的写入方式。

### Phase 4 — 受限文件写入工具

前序目标保护：

- 保持 Phase 3 的 Plan / Chat 语义工具边界：普通 `write_file`、`patch_file`、`copy_file` 不能写 `plans/`，不能直接写 `chats/*.jsonl`，不能替代 `save_cad_plan()` 或 `update_chat_summary()`。
- 保持 CadQuery `.py` 模型源只能通过 CadQuery tool 修改的边界：`write_file` 与 `patch_file` 均拒绝 CadQuery `.py`；`copy_file` 只允许将 CadQuery `.py` 源 byte-for-byte 复制到 confirmed `new_files` 目标。
- 保持 Phase 1 的 registry 执行入口与 direct executor 双层防御：LLM tool loop 在执行前拒绝越权调用，直接调用 executor 时仍执行同等路径和权限检查。

完成情况：

- 新增 `write_file()`：
  - 只允许 Execute + confirmation。
  - 只写 UTF-8 文本，拒绝 NUL / binary 内容，允许空文本。
  - 新建文件必须位于 confirmed `new_files`，且不得带 `expected_hash`。
  - 覆盖既有文件必须位于 confirmed `affected_files`，且 `expected_hash` 必须匹配当前内容。
  - 拒绝 `plans/`、`chats/`、`outputs/`、`.budn_staging`、workspace escape、CadQuery `.py`、symlink 和 Unix hard link alias。
- 新增 `patch_file()`：
  - 只允许 Execute + confirmation。
  - 只修改 confirmed `affected_files` 中的既有文本文件。
  - 使用 `expected_hash` 和唯一 `search` 匹配做冲突检测，允许空 `replace`。
  - 拒绝 CadQuery `.py`、`plans/`、`chats/`、`outputs/`、workspace escape、symlink 和 Unix hard link alias。
- 新增 `copy_file()`：
  - 只允许 Execute + confirmation。
  - `target_path` 必须位于 confirmed `new_files` 且目标不存在。
  - 源文件必须是安全 workspace 文本文件，拒绝 symlink 和 Unix hard link alias。
  - CadQuery `.py` 目标必须从 CadQuery `.py` 源复制，禁止从普通文本源复制到模型目标。
  - 复制只做 byte-for-byte 写入，不提供内容修改能力。
- 将 registry 路径策略拆到 `tool_path_policy.rs`，把普通文件工具的 path policy 与 registry intent 校验分开：
  - path policy 负责 roots、CadQuery 模型文件、confirmed scope 和 export target。
  - registry intent 负责 `write_file` 创建 / 覆盖意图，避免 LLM tool loop 把 `new_files` 与 `affected_files` 混用。
- 将普通文件写入 executor 拆到 `file_write.rs` 与 `file_write/path_policy.rs`，集中处理文本校验、hash、symlink、hard link alias、scope 和 copy model 边界。
- 更新 `docs/cadquery-mvp/agent-tool-contract.md`，移除普通写入对 `plans/` 的许可，并明确 `affected_files` / `new_files` 语义。

验证命令：

- `cargo test -p app-server-core --test agent_tool_tests workspace_tool_executor_write_file_overwrites_with_matching_hash -- --nocapture`
  - 结果：1 passed。
- `cargo test -p app-server-core --test agent_tool_tests workspace_tool_executor_copy_file_rejects_symlink_target -- --nocapture`
  - 结果：1 passed。
- `cargo test -p app-server-core --test agent_tool_tests workspace_tool_executor_copy_file_rejects_hard_link_alias_target -- --nocapture`
  - 结果：1 passed。
- `cargo test -p app-server-core --test agent_tool_tests --test agent_tool_registry_tests --test chat_tests`
  - 结果：`agent_tool_registry_tests` 5 passed；`agent_tool_tests` 103 passed；`chat_tests` 8 passed。
  - 备注：仍有既有 `watch.rs` dead_code warning，未在本 Phase 处理。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests --test dispatcher_pure_fn_tests`
  - 结果：`dispatcher_pure_fn_tests` 17 passed；`shared_dispatcher_roundtrip_tests` 13 passed。
  - 备注：`shared_dispatcher_roundtrip_tests` 中仍有既有未使用 helper warning，未在本 Phase 处理。
- `git diff --check`
  - 结果：通过。
- 新增文件规模检查：
  - `tools.rs` 257 行，`tool_path_policy.rs` 369 行，`file_write.rs` 382 行，`file_write/path_policy.rs` 344 行；均小于 500 行。

独立 review：

- 第一轮 review 指出 `patch_file` symlink 写穿风险、`new_files` / `affected_files` 混用风险；已补充测试并修复 executor 与 registry 校验。
- 第二轮 review 指出 `plans/` 仍可被普通写入工具写入、hard link alias 风险和空内容 / 空 replace 覆盖不足；已移除普通写入 `plans/` root，并补充测试。
- 第三轮 review 指出 registry 对 `patch_file` / `copy_file` 的确认范围语义仍不够精确，文档中普通写入 root 描述未同步；已修复。
- 第四轮 review 指出 `copy_file` 可从普通文本复制到 CadQuery `.py` 新目标，以及 registry 未提前校验 `write_file` 的创建 / 覆盖意图；已补充红绿用例并修复。
- 最终独立 review 未发现 blocker 或 important 问题；建议补充 `write_file` 覆盖成功、copy 目标 symlink 和 copy 目标 hard link alias 测试，已补齐并通过回归。

遗留问题：

- 未发现需要写入 `docs/known_issues.md` 的新问题。

### Phase 5 — CadQuery 专用工具与执行边界

前序目标保护：

- 保持 Phase 4 的普通文件写入边界：CadQuery `.py` 模型源仍不能通过 `write_file()` / `patch_file()` 改写；`copy_file()` 只能复制已有 CadQuery `.py` 到 confirmed `new_files`。
- 保持 Phase 1 的统一 tool loop 入口：LLM Execute 现在通过 registry 暴露并执行 CadQuery tools，旧的 direct `ClientCommand::CadQueryExecute` 写入入口已禁用。
- 保持 CadQuery staging 原子边界：dry run 只写 staging，不写真实 workspace；execute 在 topology 校验通过后才 commit 真实 target / outputs。

完成情况：

- 新增 CadQuery tool executor：
  - `cadquery_analyze_source()`：只读分析现有 `.py`，返回 target type、`build`、`REFS`、配对 `.md`、本地依赖和 ref keys；拒绝 symlink alias。
  - `cadquery_check_source()`：静态检查拟议完整源码，输出 `target_type_matches`、`has_build_function`、`has_refs`、`unsafe_calls`、`invalid_imports` 和 warnings。
  - `cadquery_dry_run()`：通过 `CadQueryToolRuntime` 在 staging 中执行拟议源码，不回写真实 workspace，不写正式 outputs；无效 `params_json` 会提前拒绝。
  - `cadquery_execute()`：只允许 Execute + confirmation，校验目标文件、export targets、source contract、危险调用、不允许的 project-local import、单次成功 commit guard 和配对 `.md` 更新范围。
  - `cadquery_get_result()`：从 result cache 返回轻量结果摘要，不返回完整 mesh 大数组。
  - `cadquery_resolve_selection()`：把 face / feature ref 映射到稳定 feature candidate；拒绝 `@selector[...]` 与 `@subshape[...]` 用户可见输出。
- 新增 host runtime：
  - `HostCadQueryToolRuntime` 复用 `stage_cadquery_project()`、`run_cadquery_runner_with_cancel()`、exact output commit scope 和 result cache。
  - Execute 路径先 runner、再 `root_object_kind` 校验、再真实 commit，避免 topology mismatch 污染 workspace。
  - Execute 成功后如果存在配对 `.md`，要求该 `.md` 在 confirmed scope 内；host preflight 拒绝 symlink / hard link，commit 后追加 budn' CadQuery 执行记录。追加失败会进入 tool result warnings，message 改为 `CadQuery execution completed with warnings`。
  - 旧 `ClientCommand::CadQueryExecute` 直接协议写入入口返回 `InvalidCommand`，避免绕过 Agent Execute tool loop 和 confirmation。
- 扩展静态安全检查：
  - 拒绝 `open`、`io.open`、`Path`、`write_text`、`write_bytes`、`unlink`、`subprocess`、`os.system/remove/rename/replace`、`shutil.rmtree/move` 等明显文件系统或外部进程调用。
  - 拒绝 `docs`、`chats`、`plans`、`outputs`、`.budn_staging`、`target`、`node_modules` 等不允许的 project-local import，并覆盖 `import docs as d, chats.session` 这类 alias / 逗号语法。
- 同步 canonical schema：
  - `cadquery_check_source` contract 增加 `invalid_imports`。
  - `cadquery_execute` input schema 不再强制 `export_targets`，与无导出执行路径一致。
  - runtime error result 增加 `diagnostics.traceback` 字段，当前无结构化 traceback 时为 `null`。

验证命令：

- `cargo test -p app-server-core --test agent_tool_tests workspace_tool_executor_cadquery -- --nocapture`
  - 结果：14 passed。
- `cargo test -p app-server-core --test agent_tool_tests workspace_tool_executor_cadquery_execute_rejects_invalid_project_import -- --nocapture`
  - 结果：1 passed。
- `cargo test -p app-server-core --test agent_tool_registry_tests --test agent_tool_tests --test cadquery_staging_tests`
  - 结果：`agent_tool_registry_tests` 5 passed；`agent_tool_tests` 118 passed；`cadquery_staging_tests` 12 passed。
- `cargo test -p app-server-core --test agent_tool_tests --test agent_tool_registry_tests --test chat_tests --test agent_tests --test llm_tests --test cadquery_tests --test cadquery_staging_tests`
  - 结果：`agent_tests` 13 passed；`agent_tool_registry_tests` 5 passed；`agent_tool_tests` 116 passed；`cadquery_staging_tests` 12 passed；`cadquery_tests` 10 passed；`chat_tests` 8 passed；`llm_tests` 34 passed。
  - 备注：后续补充测试后，最新 `agent_tool_tests` 为 118 passed。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests --test dispatcher_pure_fn_tests`
  - 结果：`dispatcher_pure_fn_tests` 17 passed；`shared_dispatcher_roundtrip_tests` 13 passed。
- `git diff --check`
  - 结果：通过。
- 新增文件规模检查：
  - `cadquery.rs` 201 行，`cadquery/args.rs` 478 行，`cadquery/support.rs` 392 行；均小于 500 行。

独立 review：

- 第一轮 Phase 5 review 指出静态合同未拒绝危险调用、host runtime 在 commit 后才校验 root type、execute 成功后缺少 `.md` / Ref Map 更新机制、旧 direct `CadQueryExecute` 入口仍可绕过 tool loop、`analyze_source` symlink alias、selector ref 用户可见输出和 dry-run params 校验缺口；已修复。
- 第二轮 review 指出旧 direct `CadQueryExecute` 仍保留、`.md` 更新缺 hard link alias 防护、post-commit `.md` 更新失败会导致已 commit 后仍可重试；已禁用 direct 写入入口，增加 hard link 防护，并把 post-commit 文档追加失败降级为 warnings，保证单次 commit guard 生效。
- 第三轮 review 未发现 blocker；指出 import alias 绕过、warnings 语义和 diagnostics 字段缺口；已补充 import alias 解析、warnings message 和 `diagnostics.traceback`。
- 最终短 review 确认没有 Blocker。

遗留问题：

- `diagnostics.traceback` 当前仅提供字段占位；runner traceback 仍主要在 error message 中。后续 Phase 7 文档同步或 runner 错误结构化时可继续拆分。
- `.md` 执行记录追加失败在真实 commit 之后以 warnings 呈现，不再返回 `status: error`，避免已提交后同一 Execute run 继续重试。该策略已在本 Phase 结果中记录，Phase 7 文档同步时需要写入最终工具合同。
