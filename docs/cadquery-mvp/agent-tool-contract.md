# Agent Tool Call 权限与 Schema 合同

本文固化 budn' CadQuery Agent MVP 的 LLM tool call 能力边界。`app server protocol command` 是客户端到服务端的产品命令；`Agent tool call` 是 LLM 在一次 Agent run 内可主动请求的能力。两者可以复用同一底层实现，但权限、记录和用户确认语义不能混用。

## 能力矩阵

| 能力 | 当前状态 | MVP tool call 目标 |
|---|---|---|
| workspace 文件读取 | 已有 `file.read` command；LLM 已有最小 `read_file` | `read_file` 输出大小、hash、截断和错误类型 |
| workspace 目录列举 | 已有 `workspace.list` command；LLM 已有最小 `list_directory` | `list_directory` 支持分页、过滤、受限递归和截断信息 |
| 文件搜索 | 无 LLM 工具 | `search_files` 只搜索安全文本范围，默认排除 `outputs/`、staging、二进制和过大文件 |
| 项目概览 | 无 LLM 工具 | `get_project_context` 汇总 components / parts / assemblies / plans / chats |
| Viewer selection | 已有 `selection.update` command | `get_selection` 读取当前 selection snapshot；`resolve_ref` / `cadquery_resolve_selection` 解析 Ref |
| Plan 持久化 | 旧链路曾从回复文本提取 Plan proposal | `save_cad_plan` 写 `plans/` 下的 Markdown CAD Plan 并返回 `plan_ref` |
| Chat summary | Chat JSONL 已存在 | `update_chat_summary` 通过 ChatStore API 更新 session meta，不允许直接写 `chats/*.jsonl` |
| 普通文件写入 | 已有 `file.write_text` command | `write_file` / `patch_file` / `copy_file` 只在 Execute + confirmation 范围内可用 |
| CadQuery preview | 已有 `cadquery.preview` command | 预览已有文件仍是只读产品动作；试运行拟议代码必须走 `cadquery_dry_run` staging 语义 |
| CadQuery execute | 已有 `cadquery.execute` command 与 staging commit | `cadquery_execute` 成为 Execute tool，受 confirmation、exact output scope 和单次成功 commit 约束 |
| CadQuery result | 已有 `cadquery.result.get` command | `cadquery_get_result` 只返回轻量结果摘要，不向 LLM 返回完整 mesh 数组 |

## Operation 权限表

| Tool | Inform | Plan | Execute | Auto 判定前 | Confirmation | 自动 LLM tool |
|---|---:|---:|---:|---:|---:|---:|
| `read_file` | 允许 | 允许 | 允许 | 允许 | 不需要 | 是 |
| `list_directory` | 允许 | 允许 | 允许 | 允许 | 不需要 | 是 |
| `search_files` | 允许 | 允许 | 允许 | 允许 | 不需要 | 是 |
| `get_project_context` | 允许 | 允许 | 允许 | 允许 | 不需要 | 是 |
| `get_selection` | 允许 | 允许 | 允许 | 允许 | 不需要 | 是 |
| `resolve_ref` | 允许 | 允许 | 允许 | 允许 | 不需要 | 是 |
| `cadquery_analyze_source` | 允许 | 允许 | 允许 | 允许 | 不需要 | 是 |
| `cadquery_get_result` | 允许 | 允许 | 允许 | 允许 | 不需要 | 是 |
| `cadquery_resolve_selection` | 允许 | 允许 | 允许 | 允许 | 不需要 | 是 |
| `update_chat_summary` | 允许 | 允许 | 允许 | 禁止 | 不需要 | 是 |
| `save_cad_plan` | 禁止 | 允许 | 禁止 | 禁止 | 不需要 | 是 |
| `cadquery_check_source` | 禁止 | 允许 | 允许 | 禁止 | 不需要 | 是 |
| `cadquery_dry_run` | 禁止 | 禁止 | 允许 | 禁止 | 不需要 | 是 |
| `write_file` | 禁止 | 禁止 | 允许 | 禁止 | 需要 | 是 |
| `patch_file` | 禁止 | 禁止 | 允许 | 禁止 | 需要 | 是 |
| `copy_file` | 禁止 | 禁止 | 允许 | 禁止 | 需要 | 是 |
| `cadquery_execute` | 禁止 | 禁止 | 允许 | 禁止 | 需要 | 是 |

`Auto` 不是独立权限级别。判定前只暴露只读上下文工具；判定为 Inform / Plan / Execute 后，当前 run 必须按判定结果刷新 tool definitions。自然语言“确认”不能直接把未确认 Auto turn 提升为 Execute；必须绑定已有 Plan proposal 与结构化 confirmation。

## 写入边界说明

`save_cad_plan` 不是普通 `write_file`：它写入的是产品语义对象，必须返回 `plan_ref`、展示路径、hash、目标 Ref、影响范围和 execution boundary。Plan 模式保存计划只能走该工具。

`agent.plan_proposed` 必须使用 `save_cad_plan` 的结构化结果生成确认数据：`plan_ref`、target、affected files、new files 和 export targets 都应来自同一次 tool result，不能由前端或回复文本重新猜测。

`AgentPlanConfirm` 必须由服务端校验同一 run 的 saved Plan：`plan_ref`、target、affected files、new files 和 export targets 必须与 `save_cad_plan` tool result 一致，否则不得进入 Execute。

`update_chat_summary` 不是普通 JSONL 文件写入：它只能通过 ChatStore API 更新 session summary、goal、related files 和 open questions 等 meta 数据，不能让 LLM 构造任意 `chats/*.jsonl` 内容。

`write_file` / `patch_file` 不能直接修改 CadQuery `.py` 模型：`.py` 模型生成和修改必须走 `cadquery_execute` 的 confirmation + staging + runner + exact output scope 边界。普通文本写入只服务说明文档、Ref Map、执行记录和确认范围内的非模型文件。

Plan 卡片“预览已有文件”和 `cadquery_dry_run` 必须分离：前者读取 workspace 现有 `.py` 并走只读预览产品动作；后者在 staging 中执行拟议完整源码，只能由 Execute tool loop 或用户显式产品动作触发，不提交真实 workspace。

`cadquery_execute` 成功后即消耗本次 Execute run 的单次成功 commit 额度。若配对 `.md` 执行记录在真实 commit 后追加失败，tool result 仍可返回 `status: ok`，并通过 `warnings` 暴露该失败；此时不得在同一 run 内把该 warning 当作 retryable build failure 继续调用 `cadquery_execute`。

CadQuery 失败结果会带 `diagnostics` object。当前 `diagnostics.traceback` 字段存在，但 runner traceback 仍可能主要包含在 `message` 中；调用方必须兼容 `diagnostics.traceback: null`，不能凭空生成 traceback 内容。

## 路径范围合同

Rust registry 中的 `AgentToolPathPolicy` 是运行时权限实现的 canonical source。Phase 0 固化如下路径范围，后续 executor 必须先按 registry 校验，再执行工具。

| Tool | 允许路径范围 | 禁止路径范围 | CadQuery `.py` 策略 | outputs 策略 |
|---|---|---|---|---|
| `read_file` / `search_files` | workspace 文本文件 | `.git`、`target`、`node_modules`、`outputs`、`.budn_staging` | 只读 | 只读摘要，不读大体量输出 |
| `list_directory` / `get_project_context` | workspace 安全目录 | `.git`、`target`、`node_modules`、`outputs`、`.budn_staging` | 只读 | 只列安全摘要 |
| `get_selection` | 不访问文件系统 | 不适用 | 禁止 | 禁止 |
| `resolve_ref` | owner `.py` / `.md` 及 Ref Map 文本 | workspace 外、内部构建目录 | 只读 | 禁止 |
| `save_cad_plan` | `plans/` | `chats/` | 禁止 | 仅声明当前 runner 会生成的 `outputs/{resolved_target 文件名 stem}.step/.stl/.3mf`，不写入 |
| `update_chat_summary` | ChatStore meta API | 直接写 `chats/*.jsonl` | 禁止 | 禁止 |
| `write_file` / `patch_file` | confirmation 范围内的 `components/`、`parts/`、`assemblies/`、`refs/`、`docs/` 文本文件 | `plans/`、`chats/`、`outputs/`、workspace 外 | 禁止普通写入 `.py` 模型 | 禁止 |
| `copy_file` | confirmation 范围内的同上路径 | `plans/`、`chats/`、`outputs/`、workspace 外 | 仅允许 byte-for-byte 复制到 confirmed `new_files` | 禁止 |
| `cadquery_analyze_source` / `cadquery_check_source` | `components/`、`parts/`、`assemblies/` | workspace 外、`outputs/` | 只读或静态检查 | 禁止 |
| `cadquery_dry_run` | staging 中的拟议 `.py` | 真实 workspace 回写、正式 `outputs/` | staging 临时代码 | 仅临时 result cache |
| `cadquery_execute` | confirmation 范围内的 `components/`、`parts/`、`assemblies/` target | confirmation 外、`chats/`、workspace 外 | 只能通过 CadQuery tool + staging commit | confirmed `outputs/` only |
| `cadquery_get_result` / `cadquery_resolve_selection` | result cache | 完整 mesh 大数组、workspace 任意文件 | 禁止 | 仅轻量摘要 |

普通文件工具的 confirmation 语义必须区分 `affected_files` 与 `new_files`：

- `write_file` 创建文件时目标必须在 `new_files`；覆盖既有文件时目标必须在 `affected_files`，并提供匹配的 `expected_hash`。
- `patch_file` 只允许修改 `affected_files` 中的既有文本文件。
- `copy_file` 的 `target_path` 只允许位于 `new_files`；`source_path` 不消耗 confirmation 范围，但仍受安全路径、文本文件、symlink 和 hard link alias 校验约束。

## Canonical Tool Result

完整 JSON Schema 以 `crates/app-server-core/src/agent/tools/registry/schemas.rs` 中每个 tool 的 `parameters`、`success_schema`、`error_schema` 为准。所有 tool result 必须是 JSON object，并包含：

```json
{
  "status": "ok | error",
  "tool_call_id": "call id from LLM provider",
  "tool": "tool_name",
  "message": "human-readable summary",
  "error_type": "permission_denied | invalid_arguments | not_found | file_conflict | cancelled | python_import_error | cadquery_build_error | topology_mapping_error | export_error | timeout",
  "retry_allowed": false
}
```

成功结果按工具扩展字段：

| Tool | 成功结果关键字段 |
|---|---|
| `read_file` | `path`、`text`、`offset`、`bytes_read`、`file_size`、`truncated`、`hash` |
| `list_directory` | `path`、`entries`、`entry_count`、`truncated` |
| `search_files` | `query`、`matches`、`truncated` |
| `get_project_context` | `objects`、`plans`、`chats`、`warnings` |
| `get_selection` | `selections`、`active_index`、`context_refs` |
| `resolve_ref` / `cadquery_resolve_selection` | `owner_ref_text`、`owner_path`、`candidate_feature_ref`、`stable_ref`、`ambiguous`、`risks` |
| `save_cad_plan` | `plan_ref`、`display_path`、`hash`、`summary`、`target_ref`、`target_path`、`affected_files`、`new_files`、`export_targets`、`execution_boundary`、`run_id` |
| `update_chat_summary` | `session_id`、`message_id`、`updated_fields` |
| `write_file` / `patch_file` / `copy_file` | `path`、`hash`、`created`、`conflict` |
| `cadquery_analyze_source` | `target_path`、`target_type`、`has_build_function`、`has_refs`、`paired_doc_path`、`local_dependencies`、`ref_keys`、`warnings` |
| `cadquery_check_source` | `contract.target_type_matches`、`contract.has_build_function`、`contract.has_refs`、`contract.unsafe_calls`、`contract.invalid_imports`、`warnings` |
| `cadquery_dry_run` | `result_id`、`build_id`、`root_object_kind`、`summary.part_count`、`summary.face_count`、`summary.edge_count`、`summary.vertex_count`、`summary.features`、`warnings` |
| `cadquery_execute` | `result_id`、`build_id`、`committed_files`、`exports`、`summary.part_count`、`summary.face_count`、`summary.edge_count`、`summary.vertex_count`、`summary.features`、`warnings` |
| `cadquery_get_result` | `result_id`、`build_id`、`root_ref_text`、`root_object_kind`、`parts`、`exports` |

错误结果必须保留 `tool_call_id` 的调用关联，并同时进入 `agent.tool_result` push event 与 Chat history。permission denied 也必须记录为 tool result，不能静默丢弃。CadQuery runtime error result 应包含 `diagnostics` object；当前至少包含可为 `null` 的 `diagnostics.traceback` 字段。

## Canonical Schema 摘要

| Tool | 输入必填字段 | 成功结果必填字段 |
|---|---|---|
| `read_file` | `path` | `status`、`tool`、`path`、`text`、`offset`、`bytes_read`、`file_size`、`truncated`、`hash` |
| `list_directory` | `path` | `status`、`tool`、`path`、`entries`、`entry_count`、`truncated` |
| `search_files` | `query` | `status`、`tool`、`query`、`matches`、`truncated` |
| `get_project_context` | 无 | `status`、`tool`、`objects`、`plans`、`chats`、`warnings` |
| `get_selection` | 无 | `status`、`tool`、`selections`、`active_index`、`context_refs` |
| `resolve_ref` | `ref_text` | `status`、`tool`、`owner_ref_text`、`owner_path`、`stable_ref`、`ambiguous`、`risks` |
| `save_cad_plan` | `title`、`target_ref`、`resolved_target`、`affected_files`、`export_targets`、`strategy`、`execution_boundary` | `status`、`tool`、`plan_ref`、`display_path`、`hash`、`summary`、`target_ref`、`target_path`、`affected_files`、`new_files`、`export_targets`、`execution_boundary`、`run_id` |
| `update_chat_summary` | `summary`、`goal`；可选 `related_files`、`open_questions` | `status`、`tool`、`session_id`、`message_id`、`updated_fields` |
| `write_file` | `path`、`contents` | `status`、`tool`、`path`、`hash`、`created`、`conflict` |
| `patch_file` | `path`、`expected_hash`、`search`、`replace` | `status`、`tool`、`path`、`hash`、`created`、`conflict` |
| `copy_file` | `source_path`、`target_path` | `status`、`tool`、`path`、`hash`、`created`、`conflict` |
| `cadquery_analyze_source` | `target_path` | `status`、`tool`、`target_path`、`target_type`、`has_build_function`、`has_refs`、`warnings` |
| `cadquery_check_source` | `target_path`、`target_type`、`code` | `status`、`tool`、`contract`、`warnings` |
| `cadquery_dry_run` | `target_path`、`target_type`、`code` | `status`、`tool`、`result_id`、`build_id`、`root_object_kind`、`summary`、`warnings` |
| `cadquery_execute` | `target_path`、`target_type`、`code` | `status`、`tool`、`result_id`、`build_id`、`committed_files`、`exports`、`summary`、`warnings` |
| `cadquery_get_result` | `result_id` | `status`、`tool`、`result_id`、`build_id`、`root_ref_text`、`root_object_kind`、`parts` |
| `cadquery_resolve_selection` | `result_id`、`selection_ref` | `status`、`tool`、`owner_ref_text`、`owner_path`、`stable_ref`、`ambiguous`、`risks` |

`cadquery_check_source.contract` 必须包含 `target_type_matches`、`has_build_function`、`has_refs`、`unsafe_calls`、`invalid_imports`。`cadquery_dry_run` 与 `cadquery_execute` 的 `summary` 必须包含 `part_count`、`face_count`、`edge_count`、`vertex_count`，可附加 `features`。
