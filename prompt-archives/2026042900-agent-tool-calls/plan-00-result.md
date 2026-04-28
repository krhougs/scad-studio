# Agent Tool Call 能力补全执行结果

## 当前状态

计划已创建，并已根据 plan review 修订 Auto 权限、canonical tool schema、CadQuery 单次成功提交、Execute 后 `.md` / Ref Map 更新和 `copy_file()` 复制边界。随后补充了 CadQuery 专用工具完整行为合同，并将 Phase 5 改为 `cadquery_analyze_source()`、`cadquery_check_source()`、`cadquery_dry_run()`、`cadquery_execute()`、`cadquery_get_result()`、`cadquery_resolve_selection()` 六个专用工具。

Phase 0 已完成实现、聚焦验证和最终独立复审。Phase 0 固化了 Agent tool registry、Operation 权限表、路径范围合同和 canonical schema，并同步了 Agent system prompt 与工具合同文档。

## Phase 结果记录

| Phase | 状态 | 结果 |
|---|---|---|
| Phase 0 — Tool 能力盘点与权限合同 | 已完成 | 已新增 registry/schema/path policy，补充 `docs/cadquery-mvp/agent-tool-contract.md`，同步 system prompt；聚焦测试与独立复审通过 |
| Phase 1 — Tool Registry 与统一执行入口 | 未开始 | 等待执行 |
| Phase 2 — 只读上下文工具补齐 | 未开始 | 等待执行 |
| Phase 3 — CAD Plan 与 Chat 语义持久化工具 | 未开始 | 等待执行 |
| Phase 4 — 受限文件写入工具 | 未开始 | 等待执行 |
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
