# Agent Tool Call 权限与 Schema 合同

本文固化 budn' CadQuery Agent MVP 的 LLM tool call 能力边界。`app server protocol command` 是客户端到服务端的产品命令；`Agent tool call` 是 LLM 在一次 Agent run 内可主动请求的能力。两者可以复用同一底层实现，但权限、记录和执行语义不能混用。

## 能力矩阵

| 能力 | 当前状态 | MVP tool call 目标 |
|---|---|---|
| workspace 文件读取 | 已有 `file.read` command；LLM 已有最小 `read_file` | `read_file` 输出大小、hash、截断和错误类型 |
| workspace 目录列举 | 已有 `workspace.list` command；LLM 已有最小 `list_directory` | `list_directory` 支持分页、过滤、受限递归和截断信息 |
| 文件搜索 | 无 LLM 工具 | `search_files` 只搜索安全文本范围，默认排除 `outputs/`、staging、二进制和过大文件 |
| 项目概览 | 无 LLM 工具 | `get_project_context` 汇总 components / parts / assemblies / plan packages / chats |
| Viewer selection | 已有 `selection.update` command | `get_selection` 读取当前 selection snapshot；`resolve_ref` / `cadquery_resolve_selection` 解析 Ref |
| Plan package 持久化 | 已有 `save_cad_plan` 单文件实现 | `save_cad_plan` 写 `plans/YYYYmmddnn-name/{request.md,plan.md,plan-result.md}` 并返回 `plan_ref` |
| Chat summary | Chat JSONL 已存在 | `update_chat_summary` 通过 ChatStore API 更新 session meta，不允许直接写 `chats/*.jsonl` |
| 普通文件写入 | 已有 `file.write_text` command | `write_file` / `patch_file` / `copy_file` 只在 Agent mode 的安全文本路径策略内可用 |
| CadQuery preview | 已有 `cadquery.preview` command | 预览已有文件仍是只读产品动作；试运行拟议代码必须走 `cadquery_dry_run` staging 语义 |
| CadQuery execute | 已有 `cadquery.execute` command 与 staging commit | `cadquery_execute` 成为 Agent mode tool，受 execution scope、exact output scope 和单次成功 commit 约束 |
| CadQuery result | 已有 `cadquery.result.get` command | `cadquery_get_result` 只返回轻量结果摘要，不向 LLM 返回完整 mesh 数组 |

## Mode 权限表

| Tool | Plan mode | Agent mode | 执行范围来源 | 自动 LLM tool |
|---|---:|---:|---|---:|
| `read_file` | 允许 | 允许 | 不需要 | 是 |
| `list_directory` | 允许 | 允许 | 不需要 | 是 |
| `search_files` | 允许 | 允许 | 不需要 | 是 |
| `get_project_context` | 允许 | 允许 | 不需要 | 是 |
| `get_selection` | 允许 | 允许 | 不需要 | 是 |
| `resolve_ref` | 允许 | 允许 | 不需要 | 是 |
| `cadquery_analyze_source` | 允许 | 允许 | 不需要 | 是 |
| `cadquery_get_result` | 允许 | 允许 | 不需要 | 是 |
| `cadquery_resolve_selection` | 允许 | 允许 | 不需要 | 是 |
| `update_chat_summary` | 禁止 | 允许 | ChatStore semantic API | 是 |
| `save_cad_plan` | 允许 | 禁止执行中使用 | Plan package path policy | 是 |
| `cadquery_check_source` | 允许 | 允许 | Source contract check | 是 |
| `cadquery_dry_run` | 禁止 | 允许 | Execution scope + staging | 是 |
| `write_file` | 禁止 | 允许 | Agent mode safe text path policy | 是 |
| `patch_file` | 禁止 | 允许 | Agent mode safe text path policy | 是 |
| `copy_file` | 禁止 | 允许 | Agent mode safe text path policy | 是 |
| `cadquery_execute` | 禁止 | 允许 | Execution scope + staging | 是 |

`Plan` 和 `Agent` 是产品模式，不再拆分 Inform / Execute / Auto。对话、解释、读取上下文都可以在两个模式中发生；差异只在写入和执行权限。

## Plan Package 合同

`save_cad_plan` 不是普通 `write_file`：它写入的是产品语义对象。新 plan 必须使用目录：

```text
plans/YYYYmmddnn-name/
├── request.md
├── plan.md
└── plan-result.md
```

约束：

- `YYYYmmdd` 使用创建当天日期。
- `nn` 是当天第 n 个 plan，从 `00` 开始，按已有同日期 plan 目录最大序号递增。
- `name` 只能包含 ASCII 小写字母、数字和连字符。
- `request.md` 保存用户原始请求和必要上下文。
- `plan.md` 必须包含 YAML front matter，记录 `plan_id`、`mode`、`target_path`、`target_type`、`affected_files`、`new_files`、`export_targets`、`status`、`created_at`、`source_chat_session`。
- `plan-result.md` 初始写入 `status: pending`；Agent mode 执行后追加 run、提交文件、生成 outputs、失败诊断和剩余风险。
- Legacy `plans/*.md` 是只读历史计划，不作为可直接执行 plan。

`save_cad_plan` 成功结果必须返回 `plan_id`、`plan_ref`、`request_path`、`plan_path`、`result_path`、`target_path`、`target_type`、`affected_files`、`new_files`、`export_targets`、`plan_status`、`run_id`。

## 写入边界说明

`Plan` mode 对 CAD 源文件、说明文件、refs、docs 和 outputs 只读；唯一允许写入是 `save_cad_plan` 创建或更新 workspace plan package。

`Agent` mode 可以直接根据当前请求工作，也可以读取 `plan_ref` 指向的 plan package。带 `plan_ref` 时，host 必须读取 `plans/<id>/request.md` 和 `plans/<id>/plan.md`，解析 front matter，并把其中的 target、affected files、new files 和 export targets 作为 execution scope。

`update_chat_summary` 不是普通 JSONL 文件写入：它只能通过 ChatStore API 更新 session summary、goal、related files 和 open questions 等 meta 数据，不能让 LLM 构造任意 `chats/*.jsonl` 内容。

`write_file` / `patch_file` 不能直接修改 CadQuery `.py` 模型：`.py` 模型生成和修改必须走 `cadquery_execute` 的 staging、runner 和 exact output scope 边界。普通文本写入只服务说明文档、Ref Map、计划结果和安全文本范围内的非模型文件。

Plan package 卡片“Open Plan”和 Markdown preview “Run Plan”必须分离：前者读取 workspace 现有 Markdown 并安全预览；后者触发 `agent.invoke { mode: Agent, plan_ref }`，由 app server 按 execution scope 执行。

`cadquery_execute` 成功后即消耗本次 Agent run 的单次成功 commit 额度。若配对 `.md` 执行记录在真实 commit 后追加失败，tool result 仍可返回 `status: ok`，并通过 `warnings` 暴露该失败；此时不得在同一 run 内把该 warning 当作 retryable build failure 继续调用 `cadquery_execute`。

CadQuery 失败结果会带 `diagnostics` object。当前 `diagnostics.traceback` 字段存在，但 runner traceback 仍可能主要包含在 `message` 中；调用方必须兼容 `diagnostics.traceback: null`，不能凭空生成 traceback 内容。

## 路径范围合同

Rust registry 中的 `AgentToolPathPolicy` 是运行时权限实现的 canonical source。目标路径必须先按 registry 校验，再执行工具。

| Tool | 允许路径范围 | 禁止路径范围 | CadQuery `.py` 策略 | outputs 策略 |
|---|---|---|---|---|
| `read_file` / `search_files` | workspace 文本文件 | `.git`、`target`、`node_modules`、`outputs`、`.budn_staging` | 只读 | 只读摘要，不读大体量输出 |
| `list_directory` / `get_project_context` | workspace 安全目录 | `.git`、`target`、`node_modules`、`outputs`、`.budn_staging` | 只读 | 只列安全摘要 |
| `get_selection` | 不访问文件系统 | 不适用 | 禁止 | 禁止 |
| `resolve_ref` | owner `.py` / `.md` 及 Ref Map 文本 | workspace 外、内部构建目录 | 只读 | 禁止 |
| `save_cad_plan` | `plans/YYYYmmddnn-name/` | `chats/`、workspace 外、legacy `plans/*.md` 覆盖 | 禁止 | 仅声明 runner 会生成的 `outputs/{target stem}.step/.stl/.3mf`，不写入 |
| `update_chat_summary` | ChatStore meta API | 直接写 `chats/*.jsonl` | 禁止 | 禁止 |
| `write_file` / `patch_file` | Agent mode 安全文本范围：`components/`、`parts/`、`assemblies/` 的 `.md`，`refs/`，`docs/`，当前 plan 的 `plan-result.md` | `chats/`、`outputs/`、workspace 外、非当前 plan 的 `request.md` / `plan.md` | 禁止普通写入 `.py` 模型 | 禁止 |
| `copy_file` | Agent mode 安全文本范围内的新文件 | `chats/`、`outputs/`、workspace 外 | 不允许复制后直接改写 `.py` 模型；模型变体仍走 CadQuery tool | 禁止 |
| `cadquery_analyze_source` / `cadquery_check_source` | `components/`、`parts/`、`assemblies/` | workspace 外、`outputs/` | 只读或静态检查 | 禁止 |
| `cadquery_dry_run` | staging 中的拟议 `.py` | 真实 workspace 回写、正式 `outputs/` | staging 临时代码 | 仅临时 result cache |
| `cadquery_execute` | execution scope 内的 `components/`、`parts/`、`assemblies/` target | scope 外、`chats/`、workspace 外 | 只能通过 CadQuery tool + staging commit | scoped `outputs/` only |
| `cadquery_get_result` / `cadquery_resolve_selection` | result cache | 完整 mesh 大数组、workspace 任意文件 | 禁止 | 仅轻量摘要 |

普通文件工具的 Agent mode 语义必须区分既有文件与新文件：

- `write_file` 创建文件时目标必须位于安全文本范围；覆盖既有文件时必须提供匹配的 `expected_hash`。
- `patch_file` 只允许修改安全文本范围内的既有文本文件。
- `copy_file` 的 `target_path` 只允许位于安全文本范围内的新文件；`source_path` 仍受安全路径、文本文件、symlink 和 hard link alias 校验约束。

## Deprecated Protocol Commands

`AgentPlanConfirm`、`AgentPlanReject` 和 `AgentCadQueryConfirmation` 属于历史 confirmation 流。过渡期可以保留旧字段反序列化和旧 command handler，但新 UI 不再使用它们；旧 `agent.plan.confirm` 和 `agent.plan.reject` 应返回 deprecated error，并提示使用 `agent.invoke { mode: Agent, plan_ref }`。

`confirmed_cadquery` 属于历史请求字段。新 `agent.invoke` 不接受它作为执行前提；执行范围来自 `plan_ref` 解析出的 execution scope，或 Agent mode 当前请求生成的 execution scope。

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
| `get_selection` | `selections`、`active_index` |
| `resolve_ref` / `cadquery_resolve_selection` | `owner_ref_text`、`owner_path`、`candidate_feature_ref`、`stable_ref`、`ambiguous`、`risks` |
| `save_cad_plan` | `plan_id`、`plan_ref`、`request_path`、`plan_path`、`result_path`、`target_path`、`target_type`、`affected_files`、`new_files`、`export_targets`、`plan_status`、`run_id` |
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
| `get_selection` | 无 | `status`、`tool`、`selections`、`active_index` |
| `resolve_ref` | `ref_text` | `status`、`tool`、`owner_ref_text`、`owner_path`、`stable_ref`、`ambiguous`、`risks` |
| `save_cad_plan` | `title`、`request`、`target_ref`、`target_path`、`target_type`、`affected_files`、`new_files`、`export_targets`、`strategy`、`execution_scope` | `status`、`tool`、`plan_id`、`plan_ref`、`request_path`、`plan_path`、`result_path`、`target_path`、`target_type`、`affected_files`、`new_files`、`export_targets`、`run_id`、`plan_status` |
| `update_chat_summary` | `summary`、`goal`；可选 `related_files`、`open_questions` | `status`、`tool`、`session_id`、`message_id`、`updated_fields` |
| `write_file` | `path`、`contents` | `status`、`tool`、`path`、`hash`、`created`、`conflict` |
| `patch_file` | `path`、`expected_hash`、`search`、`replace` | `status`、`tool`、`path`、`hash`、`created`、`conflict` |
| `copy_file` | `source_path`、`target_path` | `status`、`tool`、`path`、`hash`、`created`、`conflict` |
| `cadquery_analyze_source` | `target_path` | `status`、`tool`、`target_path`、`target_type`、`has_build_function`、`has_refs`、`warnings` |
| `cadquery_check_source` | `target_path`、`target_type`、`code` | `status`、`tool`、`contract`、`warnings` |
| `cadquery_dry_run` | `target_path`、`target_type`、`code`、可选 `execution_scope` | `status`、`tool`、`result_id`、`build_id`、`root_object_kind`、`summary`、`warnings` |
| `cadquery_execute` | `target_path`、`target_type`、`code`、可选 `execution_scope` | `status`、`tool`、`result_id`、`build_id`、`committed_files`、`exports`、`summary`、`warnings` |
| `cadquery_get_result` | `result_id` | `status`、`tool`、`result_id`、`build_id`、`root_ref_text`、`root_object_kind`、`parts` |
| `cadquery_resolve_selection` | `result_id`、`selection_ref` | `status`、`tool`、`owner_ref_text`、`owner_path`、`stable_ref`、`ambiguous`、`risks` |

`cadquery_check_source.contract` 必须包含 `target_type_matches`、`has_build_function`、`has_refs`、`unsafe_calls`、`invalid_imports`。`cadquery_dry_run` 与 `cadquery_execute` 的 `summary` 必须包含 `part_count`、`face_count`、`edge_count`、`vertex_count`，可附加 `features`。
