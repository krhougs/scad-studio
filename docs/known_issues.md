# 已知问题记录

## 2026-05-02 00:00:00: `scad-scene` 系统字体探测仍使用同步外部命令

- 来源：为 Agent 生命周期与 WebSocket 生命周期分离设计做 async/thread 现状检查时，检索生产代码中的线程与阻塞接口，确认 `crates/scad-scene/src/system_fonts.rs` 使用 `std::process::Command` 调用 `fc-match`。
- 原因：该路径属于渲染字体 fallback 探测逻辑，历史实现直接同步调用系统工具；它不在当前 Agent / WebSocket 主链路上，也不是本轮 Agent 生命周期设计的实现范围。
- 影响范围：
  - 当前 Agent / WebSocket 主链路未发现手写系统线程、`spawn_blocking` 或同步外部命令。
  - 若未来把系统字体探测放到 app server async 请求路径或高频 UI 状态刷新路径，可能阻塞当前执行线程，影响响应延迟。
- 可能的解法：
  - 将字体探测改为启动时一次性缓存，并避免在请求路径重复执行。
  - 若必须在 async 路径执行，改用 async 外部命令或把结果预计算到可复用状态中。
- 当前处理方式：本轮只记录问题；Agent 生命周期设计不依赖该字体探测路径。

## 2026-05-01 16:30:00: Anthropic web search citations 尚未映射到 protocol sources

- 来源：执行 `prompt-archives/2026050101-agent-provider-model-config/plan-00.md` Phase 6 文档与最终验证时，复核 Anthropic Messages provider 接入方式。当前 Anthropic web search 通过 `additional_params.tools` 注入 `web_search_20250305` server tool，但 app server 尚未把 Anthropic response citation 结构映射到 budn' protocol 的 `search_sources` 字段。
- 原因：
  - Anthropic API 可在响应内容中提供 citation / search result 相关结构。
  - 当前 Rig Anthropic provider 路径把 additional tools 传递到 provider request，但 app server 侧没有稳定消费 Anthropic citation 的结构化适配层。
- 影响范围：
  - Anthropic native web search 可按 provider server tool 执行，但 Web Chat 暂时不能稳定展示 Anthropic citation URL 列表。
  - `search_sources` protocol 字段仍保持可选；在结构化 citation 映射完成前，前端不得从文本或文件名推断来源。
- 可能的解法：
  - 升级或扩展 Rig Anthropic provider，使其暴露稳定 citation / search result 结构。
  - 在保持 Rig-only Agent 边界的前提下，在 app server 内增加受控的 Anthropic response citation 适配层。
  - 为 OpenAI 与 Anthropic 的 `search_sources` 统一补充 provider-specific contract tests。
- 当前处理方式：保留 protocol 可选 `search_sources` 与 capability 状态；Web 只展示 provider/model web search 能力与实际返回的 sources，不伪造 Anthropic citations。

## 2026-05-01 07:45:00: Rig 0.35.0 暂未暴露 OpenAI web search sources / annotations

- 来源：执行 `prompt-archives/2026050100-async-rig-web-search/plan-00.md` Phase 5 时，核对 OpenAI 官方 web search 文档与本地 `rig-core-0.35.0` 源码。OpenAI Responses API 会在 `web_search_call.action.sources` 与 message annotations 中提供来源信息；但 Rig 当前 `AdditionalParameters::Include` 未包含 `web_search_call.action.sources`，流式 `MultiTurnStreamItem` 也没有把 URL citation / sources 暴露为可直接消费的结构。
- 原因：
  - OpenAI Responses API 的 web search 来源字段存在于 provider 原始响应结构中。
  - 当前 Rig 版本会把 hosted `web_search` 合并进 Responses request tools，但没有给 app server 侧提供稳定的 structured sources / annotations 输出。
- 影响范围：
  - Phase 5 只能在 Chat history 记录 provider capability record 与最终 assistant 文本，不能把来源 URL 作为 protocol 字段或可点击引用稳定输出。
  - Phase 6 设计 Web 来源展示时，不能假定 core 已经能拿到结构化 citation；需要先升级 Rig、向 Rig 补适配，或在 app server 内增加受控的 provider response 解析层。
- 可能的解法：
  - 升级到暴露 OpenAI Responses web search sources / annotations 的 Rig 版本。
  - 若上游暂不支持，在 `app-server-core` 内增加最小 OpenAI Responses stream 适配，但仍必须保持 Rig-only Agent 边界与 hosted tool 语义，不得回退到自建互联网搜索工具。
  - protocol 侧先设计可选 sources 字段，并允许当前 provider capability record 作为降级路径。
- 当前处理方式：native web search 在 `agents.toml` 中默认开启；当前 OpenAI 路径仅注册 hosted `web_search`，记录 `agent_run_capabilities.native_web_search_enabled`，并保留最终文本。Phase 6 已在 protocol、Chat history 和 Web UI 中加入可选来源字段与展示路径；在 Rig 暴露结构化 sources / annotations 前，该字段保持空列表，Web 端显示 capability 状态但不会伪造来源。

## 2026-05-01 00:00:00: WebSocket 连接处理 future 曾不满足 `Send`

- 状态：已处理，`prompt-archives/2026050100-async-rig-web-search/plan-00.md` Phase 3 已恢复普通 `tokio::spawn`。
- 来源：执行 Phase 3 时，将 app server core / host 的文件系统、预览、导出、ChatStore、CadQuery runner 与 staging 路径改为 async 后，`app-server-host` websocket smoke 编译暴露 `tokio::spawn` 要求连接处理 future 满足 `Send`。
- 原因：
  - dispatcher 中部分 async 路径曾通过借用参数进入同一个连接处理 future，例如 ChatStore、CadQuery staging、preview / config / workspace helper 和若干路径参数。
  - `tokio::spawn` 会把升级后的 websocket 连接 future 放入多线程 runtime；这些借用 future 不能跨 worker 线程移动。
- 影响范围：
  - 影响 `app-server-host/src/websocket.rs` 的 websocket upgrade 后连接处理。
  - 若保留 blocking runtime 包装，会为连接处理占用 blocking 线程，不适合作为长期 WebSocket 运行模型。
- 可能的解法：
  - 将 dispatcher 生产路径经过的 async helper 改为 owned 参数或在 await 前完成借用转换。
  - 为 `handle_connection` 内的 request dispatch future 保留编译期 `Send` 断言。
  - 恢复普通 `tokio::spawn` 后补跑 websocket smoke 与 host 全量测试。
- 当前处理方式：
  - 已按上述方案处理。WebSocket upgrade 后直接使用普通 `tokio::spawn`，request dispatch 通过 `require_send(...)` 编译期断言。
  - `cargo test -p app-server-host` 已通过，包含 `websocket_smoke_roundtrip` 相关用例。

## 2026-04-30 22:24:00: 重载后 Chat history 没有恢复 CadQuery artifact relation

- 状态：已处理，`prompt-archives/2026043002-cadquery-web-polish-replan/plan-00.md` Phase 5 已让 CadQuery tool result history 携带 `mesh_result`，并在 Studio common 读取 chat history 时恢复 CadQuery result 缓存。
- 来源：执行 `prompt-archives/2026043002-cadquery-web-polish-replan/plan-00.md` Phase 5 独立 review 后的真实 Web 补充验证。
- 原因：
  - live `agent.mesh_ready` push 会更新前端 `cadquery_results`，因此同一会话内 `.step` 可以通过显式 artifact relation 找到 `.py` preview target。
  - 但 Host 持久化 Chat tool result 时 `mesh_result` 为 `None`；页面重载后只剩 chat history，没有 live push 中的 artifact relation。
  - Studio common 读取 `chat.history` 时也没有把历史消息里的 `mesh_result` 合并回 `cadquery_results`。
- 影响范围：
  - Agent 生成的 `.step` 在同一 live session 中可打开；重载页面或重新连接后，文件列表点击 `.step` 可能无法通过显式 artifact relation 进入 CadQuery 预览。
  - Phase 5 早期后置验证只证明没有打开临时 result tab，不能充分证明 `.step` 点击本身不是空操作。
- 可能的解法：
  - Host 在记录 CadQuery tool result 时，从同一份 CadQuery result cache 找到对应 `result_id`，把完整 `CadQueryResultReady` 写入 ChatStore `mesh_result`。
  - Studio common 在 `chat.history` response 中读取每条消息的 `mesh_result`，并合并到 `cadquery_results`。
  - Web 文件列表继续只查询 `artifact_relation.exports`，不恢复任何基于路径或文件名的源文件推断。
- 当前处理方式：已采用上述方案。`chat_history_response_restores_cadquery_results_from_mesh_records` 覆盖 history 恢复路径；真实网页脚本 `/tmp/budn_phase5_gap_verify_web.ts` 在重载后先打开 `.step`，得到 `AIRPODS-PRO2-CHARGING-TRAY.STEP` tab，并完成 CadQuery 预览、mode 切换和 Ref 多选验证。

## 2026-04-30 21:55:00: CadQuery `MODEL_DETAILS` 嵌套值会被执行契约拒绝

- 状态：已处理，`prompt-archives/2026043002-cadquery-web-polish-replan/plan-00.md` Phase 5 已允许非空 dict / list 字段值通过 CadQuery model contract 校验。
- 来源：执行 `prompt-archives/2026043002-cadquery-web-polish-replan/plan-00.md` Phase 5 真实 Web Playwright 验收。
- 原因：
  - Agent 初始生成的 CadQuery source 包含 module-level `MODEL_DESCRIPTION`，并且 `MODEL_DETAILS` 中包含 `purpose`、`key_dimensions`、`intended_use`、`assumptions`、`interaction_notes`、`manufacturing_or_placement_constraints`。
  - 其中 `key_dimensions` 是 dict，`assumptions` 和 `manufacturing_or_placement_constraints` 是 list。
  - `cadquery_check_source` 对该源码返回 `missing MODEL_DESCRIPTION / MODEL_DETAILS` warning，`cadquery_execute` 返回 `CadQuery model source must include MODEL_DESCRIPTION and MODEL_DETAILS fields ...`，说明当前契约校验实际只接受更窄的字段值形态。
- 影响范围：
  - 真实 LLM 很自然会把尺寸、假设和制造约束生成为结构化 dict / list；这会导致前几次 `cadquery_execute` 被拒绝，增加真实 Agent run 的失败率和耗时。
  - 当前 Phase 5 中 Agent 通过把字段值改成字符串后成功执行，因此不阻塞本轮验收，但会影响后续 CadQuery Agent 质量判断。
- 可能的解法：
  - 扩展 `MODEL_DETAILS` 契约校验，允许非空 string、dict、list 等 JSON-like Python 字面量，只要求字段存在且非空。
  - 或者在 system prompt / tool schema 中明确要求 `MODEL_DETAILS` 每个字段值必须是非空字符串；该方案会降低模型说明结构化程度。
  - 无论选择哪种方式，都需要补充 `cadquery_execute` 成功与拒绝路径测试，并同步 `cadquery_check_source` warning 语义。
- 当前处理方式：已选择扩展契约校验。`workspace_tool_executor_cadquery_execute_accepts_python_model_contract_variants` 覆盖非空 dict / list 成功路径，`workspace_tool_executor_cadquery_execute_rejects_non_module_or_empty_model_details` 继续覆盖空 dict / list 拒绝路径。

## 2026-04-30 21:55:00: `update_chat_summary.related_files` 不能关联 `outputs/` 导出物

- 状态：已处理，`prompt-archives/2026043002-cadquery-web-polish-replan/plan-00.md` Phase 5 已允许 Chat summary 把 `outputs/` 下导出物作为相关文件记录。
- 来源：执行 `prompt-archives/2026043002-cadquery-web-polish-replan/plan-00.md` Phase 5 selection 修改验收。
- 原因：
  - Agent 在完成 `parts/airpods-pro2-charging-tray.py` 与 `outputs/airpods-pro2-charging-tray.step` 同步更新后，调用 `update_chat_summary` 并把 `.py` 与 `.step` 都放入 `related_files`。
  - tool 返回 `path root 'outputs' is denied for this tool`；Agent 移除 `outputs/...step` 后重试成功。
  - 当前 summary 相关文件策略允许模型源文件，但不允许导出物路径。
- 影响范围：
  - Chat summary 无法把一次 Agent run 的导出物作为相关文件保存，用户后续从聊天摘要恢复上下文时只能看到 `.py`，看不到 `.step`。
  - 这不影响 `.step` artifact relation 预览，因为预览仍由 CadQuery result relation 承接；影响的是聊天语义摘要和后续 Agent 上下文。
- 可能的解法：
  - 调整 `update_chat_summary` path policy，允许 `outputs/` 下由 app-server 已知 artifact relation 证明来源的导出物作为只读 related file。
  - 或把导出物放入 summary 的独立 `related_outputs` 字段，避免和可编辑源文件共用同一权限策略。
- 当前处理方式：已调整 `update_chat_summary` 的 related file 根目录策略，允许 `components`、`parts`、`assemblies`、`plans`、`refs`、`docs` 和 `outputs`，继续拒绝 `chats`、`.git`、`target`、`node_modules` 与 `.budn_staging`。`workspace_tool_executor_update_chat_summary_appends_chatstore_meta` 覆盖 `.py` 与 `.step` 同时写入 ChatStore summary 的路径。

## 2026-04-29 17:55:58: Web 文件列表缺少手动刷新入口

- 状态：已处理，`prompt-archives/2026042903-web-agent-chat-ui-fixes/plan-00.md` Phase 3 已增加 Files panel 刷新按钮。
- 来源：用户反馈“文件列表缺一个刷新按钮”。
- 原因：
  - Web 工作台已有 watch event 自动刷新目录树，也已有 `refreshRootListing` 与 `refreshExpandedDirectories` 两条刷新函数。
  - Files panel 没有暴露手动触发入口，用户在 watch 事件延迟、外部工具写入或需要主动重拉目录时只能等待自动刷新。
- 影响范围：
  - Web Files panel 的 root 文件列表和已展开目录列表缺少手动刷新能力。
  - 不影响 app server protocol；刷新仍应通过现有 `workspace.list` 请求完成。
- 可能的解法：
  - 在 Files panel header 增加刷新按钮。
  - 点击后复用 Workbench 现有 root listing 与 expanded directories 刷新函数，不直接读取本地文件系统。
- 当前处理方式：
  - Files panel header 已增加可访问名称为 `refresh files` 的图标按钮。
  - 点击按钮会通过 Workbench 现有 `workspace.list` 链路重新请求 root 与已展开目录。
  - 已增加单元测试覆盖按钮渲染与点击回调。

## 2026-04-29 13:20:30: 旧 confirmation 主流程与 Agent / Plan 双模式冲突

- 状态：已处理，`prompt-archives/2026042902-agent-plan-workspace-flow/plan-00.md` 已覆盖文档、protocol、后端、Web Chat 和 Markdown preview 的迁移。
- 来源：执行 `prompt-archives/2026042902-agent-plan-workspace-flow/plan-00.md` Phase 1。
- 原因：
  - 旧文档和运行时 prompt 把 `Inform / Plan / Execute / Auto`、Plan 确认卡片、`AgentPlanConfirm`、`AgentCadQueryConfirmation` 和 `confirmed_cadquery` 作为主执行流程。
  - 新产品方向只保留 `Agent` 和 `Plan` 两个模式；Plan mode 只创建 workspace plan package，Agent mode 可直接执行当前请求或已有 plan。
  - 如果旧语义继续作为当前约束存在，后续 Web `/execute` 或 execute 模式仍会依赖结构化确认数据，导致没有 confirmation payload 时必然失败。
- 影响范围：
  - `docs/cadquery-mvp/init.md`、Ref PRD、Agent Chat 交互设计、竞品分析、system prompt 和 tool contract 的当前流程说明。
  - `crates/app-server-protocol`、`app-server-core`、`app-server-host`、Web Chat composer、Plan 卡片和 Markdown preview 执行入口。
- 可能的解法：
  - 将运行时和文档主流程统一为 `Agent` / `Plan` 双模式。
  - 将 plan 持久化改为 `plans/YYYYmmddnn-name/{request.md,plan.md,plan-result.md}`。
  - 废弃旧 confirmation command；新入口使用 `agent.invoke { mode: Agent, plan_ref }`。
  - 用 Agent mode path policy、CadQuery staging、`.py` 专用工具边界和 execution scope 替代旧 confirmation 安全边界。
- 当前处理方式：
  - 当前产品主路径只保留 `Agent` / `Plan` 双模式；Web Chat 和 Markdown preview 的执行入口均使用 `agent.invoke { mode: Agent, plan_ref }`。
  - `plans/YYYYmmddnn-name/{request.md,plan.md,plan-result.md}` 是可执行 workspace plan package；app server 解析该目录生成 execution scope，并在执行后更新同目录 `plan-result.md`。
  - legacy `plans/*.md` 只作为历史计划文件只读展示，不生成可执行 `plan_ref`，也不会触发 Markdown preview 的 `Run Plan` 入口。
  - `agent.plan.confirm` 和 `agent.plan.reject` 保留为 deprecated 兼容 command，返回 deprecated error，不再作为当前 UI 或后端主执行流程。

## 2026-04-29 23:20:00: CadQuery runner traceback 仍需结构化拆分

- 来源：执行 `prompt-archives/2026042900-agent-tool-calls/plan-00.md` Phase 5 独立 review。
- 原因：
  - `cadquery_execute` 在真实 commit 后追加配对 `.md` 执行记录；若追加失败，当前返回 `status: ok` 并在 warnings 中提示，而不是返回 `status: error`。这是为了避免真实 commit 已发生后同一 Execute run 继续重试并破坏“单次成功 commit”边界。
  - `CadQueryToolRuntimeError` 当前仍主要通过 `message` 携带 runner 错误文本；tool error result 已提供 `diagnostics.traceback` 字段，但 traceback 还未从 runner stderr/message 中结构化拆分。
- 影响范围：
  - LLM 和 UI 可以看到 warnings；工具合同和 system prompt 已明确“post-commit 文档追加失败以 warning 呈现”这一策略，避免后续实现误认为必须返回 error。
  - 后续若要让 LLM 更稳定地基于 Python traceback 修复 CadQuery build error，需要把 runner traceback / diagnostics 从 message 中结构化拆出来。
- 可能的解法：
  - 保持 `docs/cadquery-mvp/agent-tool-contract.md` 和 system prompt 中的 warning 语义与运行时一致。
  - 扩展 `CadQueryToolRuntimeError`，增加 `traceback` 与 `diagnostics` 字段，并在 `cadquery_tool_error()` 中从 runner stderr / message 填充。
- 当前处理方式：Phase 7 已同步文档语义；运行时继续保证安全边界：commit 前完成 topology 与 doc update preflight，commit 后文档追加失败进入 warnings，`cadquery_execute` 成功后仍会设置单次 commit guard。剩余问题只保留 runner traceback / diagnostics 的结构化拆分。

## 2026-04-28 09:22:00: CadQuery edit intent 尚未进入结构化 Agent 输出

- 状态：当前问题；旧本地 plan 草稿和 prompt 关键词判断已删除，剩余问题是 Rig Agent 仍没有专用结构化 edit intent 字段。
- 来源：用户明确要求不允许存在 move / replace 等硬编码判断，这些编辑意图应由模型自行决定。
- 原因：
  - 当前 protocol 和 Web 执行范围还没有表达模型输出的结构化 edit intent 字段，例如 `InstanceMove`、`InstanceReplacement`、`ComponentReplacement`。
  - 本轮已删除 `crates/app-server-core/src/agent/selection.rs` 与 `packages/studio-web/src/workbench/cadquery-agent-scope.ts` 中的 prompt 关键词判断。
  - `docs/cadquery-mvp/agent-system-prompt.md` 记录模型应承担的结构化判断责任，并通过 `cadquery_agent_system_prompt()` 作为运行时 system prompt 加载。
  - 现阶段 Web 只能使用显式 target path、plan package 或 selection 的结构化 owner/ref 信息，不能替模型猜测用户要移动、替换还是修改本体。
- 影响范围：
  - prompt 中出现 move / replace / 移动 / 替换 等词不会再改变确认范围，也不会生成几何修改。
  - 在结构化 edit intent 接入前，instance move / replacement 这类语义需要通过显式 target、plan package 或后续结构化 tool call 才能准确表达。
  - 未配置可用 Rig provider 时，Agent 会返回 `LlmError` 并记录错误消息；不会执行本地固定 CadQuery 几何模板。
  - 这不扩大写入权限；Agent mode 仍受 `target_path`、`affected_files` / `new_files`、`export_targets` 和 staging exact output scope 限制。
- 可能的解法：
  - 在 protocol 中增加模型 tool output 专用的结构化 edit intent enum，并把它作为 Agent mode execution scope 的一部分展示给用户。
  - Rig Agent 输出 target path、target type、affected files、export targets 和 edit intent，由 app server 校验结构化字段，不从 prompt 文本推断。
  - Web UI 只展示模型输出的结构化 execution scope；如果后续需要人工修正，应通过显式控件修改结构化字段，再由 Agent mode 执行，而不是恢复关键词词表。
- 当前处理方式：已删除 prompt 关键词推断、本地 plan 草稿和本地 CadQuery 几何 codegen；生产 Agent 入口走 Rig provider 路径，当前执行范围来自显式 `plan_ref` / plan package 和结构化 selection context。

## 2026-04-28 06:01:20: CadQuery Execute confirmation 尚未持久绑定 CAD Plan 文件

- 状态：历史记录；曾在 `prompt-archives/2026042900-agent-tool-calls/plan-00.md` Phase 3 中按旧 confirmation 流处理，现已被 `prompt-archives/2026042902-agent-plan-workspace-flow/plan-00.md` 的 Agent / Plan 双模式迁移取代。
- 来源：执行 `prompt-archives/2026042700-cadquery-mvp-design/plan-00.md` Phase 3 第二轮独立 review。
- 原因：
  - 原记录中，协议已有 `AgentCadQueryConfirmation.plan_ref` 字段，但 Web Chat 侧尚未实现 CAD Plan 文件持久化、计划选择和确认绑定流程。
  - 原记录中，Execute 前只校验 `target_path`、`affected_files` / `new_files` 与 `export_targets`，但 `plan_ref` 仍为 `null`。
- 历史影响范围：
  - 已新增 `save_cad_plan`，将 CAD Plan 作为 Markdown 文件持久化到 `plans/`，并通过 Chat tool result 记录 `plan_ref`、target、affected files、new files 和 export targets。
  - `agent.plan_proposed` 已携带 `plan_ref` 和 `new_files`，服务端在 `AgentPlanConfirm` 时会读取同一 run 的 saved Plan 并校验 confirmation scope 是否一致。
  - 协议 payload 因 `AgentPlanProposedEvent` 新增字段升级到 protocol version 3，避免旧 Borsh 客户端继续以 2.2 协商后解码失败。
- 历史处理方式：
  - 已采用产品语义工具 `save_cad_plan`，而不是普通 `write_file`，保存计划。
  - 已采用服务端校验同一 run 的 saved Plan 与 confirmation 的 `plan_ref`、target、affected files、new files 和 export targets。
  - `agent.invoke` 不再接受直接携带的 CadQuery confirmation，确认执行必须走 `agent.plan.confirm`。
  - 后续如果引入多 Plan 版本，仍需要在 Chat UI 中明确用户当前选择的是哪一版计划。
- 当前处理方式：本条仅作为历史记录保留。新主路径不再使用 `agent.plan.confirm`，改为 `agent.invoke { mode: Agent, plan_ref }`；剩余风险并入“旧 confirmation 主流程与 Agent / Plan 双模式冲突”记录跟踪。

## 2026-04-28 05:38:28: CadQuery edge / vertex pick tolerance 仍需真实模型校准

- 来源：执行 `prompt-archives/2026042700-cadquery-mvp-design/plan-00.md` Phase 2，按计划实现 Viewer face / edge / vertex 选择与浏览器验证。
- 原因：
  - 当前 Web Viewer 使用 Three.js `Raycaster`，edge picking 配置为 `Line.threshold = 2`，vertex picking 配置为 `Points.threshold = 4`。
  - 这两个阈值已能覆盖 Phase 2 浏览器测试中的基础选择路径，但还没有基于真实复杂 CadQuery 模型、不同缩放比例、不同投影模式和高 DPI 设备做系统校准。
- 影响范围：
  - MVP 中 edge / vertex 选择可用，但在特别小的模型、密集边线、重叠顶点或极端缩放视角下，仍可能出现误选或较难选中的情况。
  - 后续若把 edge / vertex 选择作为 Agent 精细修改的主要入口，需要补充真实模型样本与误选率验证，不能只依赖当前最小浏览器用例。
- 可能的解法：
  - 增加包含小倒角、密集孔位、重复实例和不同单位尺度的 CadQuery fixture，用 Playwright 覆盖 edge / vertex 命中稳定性。
  - 根据 mesh bounds、camera distance 和 device pixel ratio 动态计算 Raycaster tolerance，而不是固定阈值。
  - 在 UI 中为 edge / vertex hover 提供更明确的预览反馈，降低误选风险。
- 当前处理方式：Phase 2 先保留固定阈值并记录本条；基础浏览器验证已覆盖 face / edge / vertex / part / assembly / repeated instance / hover，真实复杂模型校准留到后续 viewer 精度专项处理。

## 2026-04-28 04:57:31: 全仓库 `cargo fmt --check` 受既有无关格式差异阻塞

- 来源：执行 `prompt-archives/2026042700-cadquery-mvp-design/plan-00.md` Phase 1 验证时，额外运行 `cargo fmt --check`。
- 原因：
  - `cargo fmt --check` 报告格式差异位于 `crates/scad-scene/tests/mesh_tests.rs`、`crates/scad-scene/tests/three_mf_tests.rs`、`crates/studio-common/tests/params_tests.rs`、`crates/studio-web-wasm/src/wasm_bridge/params.rs` 和 `crates/studio-web-wasm/src/wasm_bridge/renderer.rs`。
  - 这些文件不是 Phase 1 本轮 Chat / Agent / CadQuery protocol 变更范围，按精准手术原则不在本轮格式化无关源码。
- 影响范围：
  - 在这些既有格式差异修复前，不能把全仓库 `cargo fmt --check` 作为 Phase 1 本轮通过证据。
  - 后续若 CI 增加全仓库 fmt 检查，当前分支会被这些无关差异阻塞。
- 可能的解法：
  - 单独提交 formatting-only 变更，限定为 rustfmt 报告的既有文件。
  - 若后续任务正好触及这些文件，可在对应任务中同步格式化并纳入 review。
  - 在 CI 中固定全仓库 fmt 检查，避免格式差异继续累积。
- 当前处理方式：本轮不修改无关源码；已对 Phase 1 触及的 Rust 文件执行 `rustfmt --edition 2024 --check` 并通过，后续仍需单独处理全仓库 fmt 差异。

## 2026-04-28 04:18:42: Agent 后端真实 provider 配置缺口

- 状态：历史记录；`prompt-archives/2026050100-async-rig-web-search/plan-00.md` Phase 4 已接入 Rig OpenAI Responses API，旧 `AgentBackend` / 本地文本草稿 / OpenAI-compatible Chat Completions 路径已删除。
- 来源：执行 `prompt-archives/2026042700-cadquery-mvp-design/plan-00.md` Phase 1，按计划评估 Rig 后实现 Agent / Chat / CadQuery tool 主链路；后续由 `prompt-archives/2026050100-async-rig-web-search/plan-00.md` Phase 4 完成架构替换。
- 原因：
  - Phase 1 已确认 `rig-core` 当前评估版本为 `0.35.0`，其 provider 抽象、tool calling、stream API 和自定义 agent 控制 hook 方向符合后续接入需求。
  - 当时实现已读取 `BUDN_AGENT_CONFIG`、`BUDN_AGENT_OPENAI_API_KEY` / `OPENAI_API_KEY`、旧模型 env fallback、timeout、max tokens、temperature 和 reasoning effort。
  - 当时仍缺少可复现的 provider mock 测试夹具，以及模型原生联网搜索 capability / 来源记录字段；这些后续已由 `prompt-archives/2026050101-agent-provider-model-config/plan-00.md` 扩展到 `agents.toml` 多 provider / 多模型配置与 protocol registry。
- 影响范围：
  - 当时未配置 Rig OpenAI Responses provider 时，Agent run 会通过 `agent.error` 返回 `LlmError`，并在 Chat history 中记录失败消息。
  - 当前生产 Agent 不再有本地文本草稿或固定几何模板后备路径；复杂 CAD 修改必须由 Rig multi-turn tool loop 完成。
  - native web search capability 和可选来源字段已接入 protocol / Web；真实来源列表仍受 Rig 结构化输出能力限制，见本文件 2026-05-01 记录。
- 可能的解法：
  - 为 Rig streaming、tool call、cancel 和 provider error mapping 建立更细的 provider mock / test support。
  - 将 provider 的认证失败、限流和 hosted tool 不可用错误映射为更具体的协议错误与 Chat 事件。
- 当前处理方式：生产 Agent 入口走 Rig provider 路径，当前支持 OpenAI Responses 与 Anthropic Messages。推荐配置入口为本机私有 `agents.toml`，由 `BUDN_AGENT_CONFIG` 指向；缺少 provider 配置时返回清晰错误并保持 workspace 不变。模型原生联网搜索通过 provider hosted / server tool 接入，Web 只展示 app server protocol 暴露的 capability 与来源字段。

## 2026-04-28 03:18:00: CadQuery output 回写仍有本地并发 TOCTOU 残余风险

- 来源：执行 `prompt-archives/2026042700-cadquery-mvp-design/plan-00.md` Phase 0c 独立 review。review 确认 `outputs -> /outside` 符号链接逃逸已被修复，但指出 `commit_files()` 在 prepare 阶段完成路径校验后，最终 `atomic_copy_file()` 仍按普通路径执行写入。
- 原因：
  - 当前实现会逐级检查 CadQuery output 目标父目录，拒绝符号链接目录，并确认真实路径仍在 canonical workspace root 内。
  - 但检查与最终 `copy + rename` 之间仍存在很短的时间窗口；本机其它进程如果在该窗口内把已检查目录替换成符号链接，理论上仍可能影响写入目标。
  - 要彻底消除这类 TOCTOU，需要改为基于目录文件描述符、no-follow 语义和更细粒度原子操作的写入流程；这超出 Phase 0c MVP 本地信任模型。
- 影响范围：
  - 在当前 MVP 假设下，workspace 属于本地可信项目目录，且同一时间只允许一个 running agent session，因此该风险不阻断 Phase 0c。
  - 如果后续把 workspace 当作不可信输入，或支持多 agent / 外部同步工具高并发写入，不能继续依赖当前普通路径写入模型作为强安全边界。
- 可能的解法：
  - 为 workspace 写入实现基于目录句柄的安全写入 API，打开父目录时禁止跟随符号链接，后续文件创建和 rename 均相对该目录句柄执行。
  - 把 CadQuery staging commit、普通文件写入、导出回写统一迁移到同一套 no-follow 写入 API。
  - 在支持多 running agent session 前，重新评估 staging commit 的锁、事务和回滚语义。
- 当前处理方式：Phase 0c 保留当前实现，测试覆盖 `outputs` 符号链接逃逸并确认不会写出 workspace；本条作为后续安全边界升级前必须复查的已知问题。

## 2026-04-26 22:24:20: Web `.scad` 外部刷新用例等不到第二次 preview_ready

- 来源：执行 `bun --cwd packages/studio-web test:e2e tests/playwright/preview-request-dedup.spec.ts -g "scad refresh emits one equivalent preview request"`，连续两次失败；在包含 `canvas-interaction`、`parameters-presets`、`preview-request-dedup` 的组合 Playwright 回归中同样失败。
- 原因：
  - 用例在打开 `cube.scad` 并收到初始 `preview_ready` 后，清空协议录制帧，再向 `examples/cube.scad` 追加一行注释，随后等待新的 `preview_ready`。
  - 当前失败点是第二次等待超时，说明本轮环境中没有观察到文件变更后对应的 `preview_ready` response。
  - 本轮点光源强度改动只触及 web preview appearance、Three.js 渲染选项和相关测试，没有修改 `.scad` 文件 watch、preview request dispatch 或 app server 刷新路径；该失败暂不能直接归因为本轮功能改动。
- 影响范围：
  - 不能把完整 `preview-request-dedup.spec.ts` 作为本轮通过证据。
  - 会影响后续判断 Web 文件监听刷新链路是否可靠，尤其是外部修改已打开 `.scad` 后是否能自动触发预览。
  - 本轮新增的点光源强度持久化、appearance 不触发 OpenSCAD 重新渲染、参数预设持久化相关目标仍由更聚焦的测试覆盖。
- 可能的解法：
  - 单独调查 `WorkbenchLayout` 的 watch event 到 `ScadWorkbench` refresh signal，再到 preview request 的完整链路。
  - 在该用例中增加对 watch event、outgoing `preview.request`、incoming response 类型的诊断输出，确认是未发请求、请求未完成，还是 response 录制遗漏。
  - 若根因是目录级 watch 去抖或事件丢失，应回到 Web 文件监听刷新计划中统一修复，而不是在点光源配置任务中扩大改动范围。
- 当前处理方式：本轮先记录为已知问题；点光源强度任务只使用相关通过用例作为验收证据，不把该刷新用例计入本轮功能完成条件。

## 2026-04-25 06:18:49: wasm-pack Chrome smoke 在本机 ChromeDriver 版本不匹配时失败

- 来源：执行 `wasm-pack test --headless --chrome crates/studio-web-wasm --test wasm_bridge_smoke`，以及带 `-- --nocapture` 的复现命令。
- 原因：
  - 本机 Google Chrome 版本为 `147.0.7727.103`。
  - `wasm-pack` 本轮下载并启动的 ChromeDriver 版本为 `148.0.7778.56`。
  - ChromeDriver 启动后，`wasm-bindgen-test-runner` 报 `http status: 404`，driver 进程状态为 `signal: 9 (SIGKILL)`。
- 影响范围：
  - 直接运行 `wasm-pack test --headless --chrome crates/studio-web-wasm --test wasm_bridge_smoke` 在当前机器上无法作为通过证据。
  - 默认 `bun run web:smoke` 已改为 Playwright S1b，不再依赖 wasm-pack 下载的 ChromeDriver。
  - Rust native 目标的 `cargo test -p studio-web-wasm --tests` 仍可执行，但 `wasm_bridge_smoke.rs` 在 native target 下没有实际用例，不能替代浏览器 wasm 验证。
- 可能的解法：
  - 安装与本机 Chrome 147 匹配的 ChromeDriver，并通过 `CHROMEDRIVER` 指向该二进制。
  - 或升级本机 Chrome 到与 `wasm-pack` 下载的 ChromeDriver 148 匹配的版本。
  - 若 CI 固定浏览器版本，需要同步固定 ChromeDriver 来源，避免 wasm smoke 因环境漂移失败。
- 当前处理方式：默认 smoke 链路改用 `packages/studio-web/tests/playwright/wasm-bridge-smoke.spec.ts`，由 Playwright 启动浏览器并验证 browser wasm bridge 的 Borsh frame。`wasm-pack` 直接命令的问题保留为环境记录。

## 2026-04-24 14:40:28: plan-01 已修复多项旧 Web parity 记录

- 来源：执行 `prompt-archives/2026042400-studio-web-parity-audit-plan/plan-01.md` 后，对照当前源码、验证命令和本文件内旧条目。
- 原因：
  - 本轮已把 Markdown 预览改为 `@uiw/react-markdown-preview`，并补充 Mermaid 与安全处理。
  - 本轮已把 `.scad` 预览接入真实 mesh viewer，把参数 / presets 移到右侧 inspector，把 Settings / Files / Log 移到左侧 panel。
  - 本轮已接通设置配置到预览、导出、切片器请求，并兼容读取桌面端 `.scad.json` 预设格式。
  - 本轮已补齐类型化参数控件、切片器动作、打开中文档刷新和相关 Playwright / unit 回归覆盖。
- 影响范围：
  - 下方 2026-04-24 03:20:00、01:24:53、02:08:00 的多条 Web parity 记录现在属于历史审计记录，不再代表当前源码状态。
  - 后续判断当前 Web 缺口时，应优先参考 `prompt-archives/2026042400-studio-web-parity-audit-plan/plan-01-result.md` 和最新源码。
- 可能的解法：
  - 后续单独整理本文件，把已解决条目迁入 resolved 归档，保留发现来源和修复计划编号。
- 当前处理方式：本轮先在文件顶部声明当前状态，避免旧记录误导后续开发判断。

## 2026-04-24 14:40:28: Studio Web 生产构建存在大 chunk warning

- 来源：执行 `bun run --cwd packages/studio-web build`，Vite 构建成功后提示部分 chunk 超过 500 kB。
- 原因：
  - Web 端引入 `@uiw/react-markdown-preview` 和 Mermaid 后，Mermaid 相关异步 chunk 体积较大。
  - 主应用包、WASM 产物、KaTeX / Mermaid 图表模块等资源总量较高。
- 影响范围：
  - 当前不影响构建产物生成，也不阻断 plan-01 功能验收。
  - 如果后续需要优化首屏加载或 PWA 离线缓存体积，需要单独评估代码分割、手动 chunk 和按需加载策略。
- 可能的解法：
  - 对 Mermaid、Markdown、viewer 和 WASM 初始化路径继续拆分异步边界。
  - 在 Vite `rollupOptions.output.manualChunks` 中显式拆分大型第三方依赖。
  - 为 PWA precache 制定资源体积预算，并把超预算构建纳入 CI 检查。
- 当前处理方式：记录为非阻塞已知问题；plan-01 先保留现有功能完整性和安全策略。

## 2026-04-24 03:20:00: 历史记录：Web Markdown 曾未达到桌面端 CommonMark 能力

- 来源：执行 `prompt-archives/2026042400-studio-web-parity-audit-plan/plan-00.md` Phase 7 时，对照 Web Markdown 预览与 `旧 Rust GUI crate 的 Markdown 预览实现`。桌面端使用 `egui_commonmark::CommonMarkCache`，当时 Web 端仍是项目内简化解析器。
- 原因：
  - plan-00 为避免引入新的前端 Markdown 依赖和额外安全审查，只保留当时已有的简化解析器。
  - 当时覆盖标题、无序列表、围栏代码块、段落、行内代码与链接，但不覆盖完整 CommonMark / GFM。
- 影响范围：
  - 这是历史阶段的能力差距；plan-01 已改为 `@uiw/react-markdown-preview`，并补充 Mermaid 与安全处理。
  - 后续若继续提高 Markdown 兼容性，应以当前 Web Markdown 依赖和用户文档样例为准核对 CommonMark / GFM 细节，而不是回到旧解析器方案。
- 可能的解法：
  - 继续围绕当前 `@uiw/react-markdown-preview` 实现补充 CommonMark / GFM 差异用例。
  - 如确实需要跨环境一致输出，可把 Markdown 解析能力放到共享 wasm / server 能力层，让 Web 和未来其它客户端消费同一份解析结果。
- 当前处理方式：plan-01 已删除旧解析器代码和对应测试；本条保留为历史审计记录。

## 2026-04-24 03:20:00: 旧 Web parity 已知问题条目有多项已被新计划修复

- 来源：继续执行 `prompt-archives/2026042400-studio-web-parity-audit-plan/plan-00.md` 后，对照当前源码、回归用例与旧记录。本文件早前的多条 Web parity 记录来自 2026-04-24 01:24:53 / 02:08:00 审计。
- 原因：
  - 后续实现已经修复 `.scad` 真实 viewer、配置透传、共享预设、类型化参数、切片器动作与打开中文档刷新。
  - 旧条目的“当前处理方式”仍描述为未修复，若不说明状态，会误导后续计划。
- 影响范围：
  - 继续追踪 Web parity 时，应以 `prompt-archives/2026042400-studio-web-parity-audit-plan/plan-00-result.md` 和当前源码为准，不应把旧条目里的未修复描述当成当前事实。
- 可能的解法：
  - 后续整理 `docs/known_issues.md` 时，把已解决条目移入单独的 resolved 归档。
  - 或逐条重写旧记录的当前状态，保留历史原因与修复版本。
- 当前处理方式：本轮先增加本状态说明；旧条目仍保留历史审计信息，但不再作为当前 Web parity 缺口判断依据。当前仍未完成的 Web 侧差距是上一个条目记录的 Markdown CommonMark / GFM 能力。

## 2026-04-24 01:24:53: 历史记录：Web `.scad` 文档曾未接回真实 3D 预览、导出与切片器流程

- 来源：对照 `旧 Rust GUI crate 的 viewer tab 实现`、`旧 Rust viewer crate 的 side panel 实现` 与当时的 Web workbench 实现。桌面端把 `.scad`、`.stl`、`.3mf` 都作为 `ViewerTab` 处理；当时 Web 端 `.scad` 仍走源码 / 预览分屏路径，只显示源码、错误文本和 preview 状态文字。
- 原因：
  - 当时 `CanvasZone` 对 `.scad` 返回的是源码 / 预览分屏，不是真实 `MeshViewer`。
  - 当时 `ExportPanel` / `SlicerPanel` 只在 `activeTab.kind === "mesh"` 时显示，`.scad` tab 走不到这条路径。
  - 当时顶部 view pills 对 `.scad` 也会显示，但 `.scad` 路径没有消费相机 preset。
- 影响范围：
  - 这是历史阶段的能力差距；plan-01 已让 `.scad` 通过 `ScadPreviewViewer` 渲染真实 mesh viewer，并开放导出与切片器动作。
  - 参数与 presets 已进入右侧 inspector，不再放在中间预览区。
- 可能的解法：
  - 已在 plan-01 中完成真实 mesh viewer、导出、切片器和右侧 inspector 接线。
- 当前处理方式：保留为历史审计记录；当前判断以 plan-01 结果和最新源码为准。

## 2026-04-24 01:24:53: Web 设置页保存的 OpenSCAD / slicer 配置没有接入预览、导出与切片器请求

- 来源：对照 `旧 Rust GUI crate 的 protocol client 实现`、`旧 Rust viewer crate 的 settings dialog 实现` 与当时的 Web 设置、预览、导出和切片器实现。桌面端请求 `PreviewRequest` / `ExportRun` / `SlicerList` 时会把 `AppConfig` 中的 `openscad_path` 和 `slicers` 一并传给 server；当时 Web 端设置页虽然能 `ConfigLoad` / `ConfigSave`，但工作台请求里仍写死空值。
- 原因：
  - 当时 `.scad` 预览请求把 `configured_openscad_path` 固定为 `null`。
  - `ExportPanel` 把 `configured_openscad_path` 固定为 `null`、`configured_slicers` 固定为空数组、`slicer_name` 固定为 `null`。
  - `SlicerPanel` 调 `dispatchSlicerList` 时固定传 `{ configured: [] }`。
  - `/settings` 路由维护的是局部 `useState`；当前工作台没有共享配置快照，也没有任何桥接把保存后的配置注入预览、导出或切片器请求。
- 影响范围：
  - 用户在 `/settings` 填写的 OpenSCAD 路径和切片器路径不会影响工作台里的预览、导出和切片器行为；设置页目前更像“可保存但未接线”的孤岛。
  - 如果 server 机器没有默认 PATH 下的 OpenSCAD，Web `.scad` 预览和导出仍可能失败，即使用户已经在设置页填了路径。
  - 切片器列表大概率长期为空，且“发送到切片器”功能在 Web 端没有真正成立的基础。
- 可能的解法：
  - 在 Web 端维护一份与 workbench 共享的已加载配置快照，并在 `PreviewRequest` / `SlicerList` / `ExportRun` 时显式透传。
  - 或让 workbench 进入页面时先 `ConfigLoad`，把必要字段缓存到 bridge 层，再由相关面板消费。
- 当前处理方式：本轮仅记录为已确认缺口；在修复前，不能把 Web 设置 / 导出 / 切片器判定为“功能完成”。

## 2026-04-24 01:24:53: Web 预设文件路径与共享预设模型不一致

- 来源：对照 `crates/studio-common/src/{document,presets}.rs`、`crates/app-server-protocol/src/presets.rs` 与 `packages/studio-web/src/workbench/{preset-io,scad-workbench}.tsx`。共享层提供 `PresetFile` 与 `preset_path_for_source`；Web 端仍定义了另一套路径和 JSON 结构。
- 原因：
  - 共享预设路径是 `preset_path_for_source(source) -> source.with_extension("scad.json")`，例如 `cube.scad -> cube.scad.json`。
  - Web 端路径是 `<source>.presets.json`，例如 `cube.scad -> cube.scad.presets.json`。
  - 共享 `PresetFile` 是 `BTreeMap<String, BTreeMap<String, ParameterValue>>`；Web 端持久化的是 `{ version: 1, presets: [{ name, defines: string[] }] }`。
- 影响范围：
  - 同一份 `.scad` 对应两套预设文件格式，后续迁移到共享预设模型时需要处理兼容读取。
  - Web 预设无法保留 `ParameterValue` 的类型语义，只剩 `name=value` 字符串，后续要做“恢复默认值”“按类型控件编辑”时会继续受阻。
  - 不能把 Web 预设能力判定为已经复用共享文档状态模型。
- 可能的解法：
  - 把 Web 端预设路径收敛到 `studio-common::preset_path_for_source` 的同一语义。
  - Web 端预设读写改为复用 `app-server-protocol::PresetFile` 结构，而不是自造 `version + defines[]` 文件格式。
- 当前处理方式：本轮仅记录问题，不改现有文件格式；后续若修复，必须同时考虑历史 `.presets.json` 的迁移或兼容读取策略。

## 2026-04-24 02:08:00: Web 参数面板仍停留在手工 `name=value` 覆写，缺少类型化 Customizer 参数模型

- 来源：对照 `crates/studio-common/src/{document,params}.rs` 与 `packages/studio-web/src/workbench/{parameters-panel,scad-workbench}.tsx`。共享层已有参数值类型；Web 端现在只允许用户手工维护一组 `name=value` 字符串。
- 原因：
  - Web 端没有消费 `ParameterEntry` / `ParameterValue` 这套共享模型，也没有参数解析后的类型信息输入。
  - `ParametersPanel` 只是字符串输入表单；`restore defaults` 语义是清空整张 override 列表，不是像桌面端那样按参数恢复默认值。
  - `ScadWorkbench` 最终只是把字符串数组直接塞进 `PreviewRequest.defines`，没有参数分组、类型校验或默认值比对。
- 影响范围：
  - Web 端不能自动发现 `.scad` 可编辑参数，也没有数值滑块、布尔勾选、枚举下拉这类类型化控件。
  - 用户不知道可用参数名时几乎无法使用该面板；即使知道名字，也缺少默认值、取值范围和分组选项。
  - 预设系统继续只能保存字符串 defines，无法与共享参数状态模型形成同一语义。
- 可能的解法：
  - 先补齐可供 Web 消费的参数定义来源，再把 Web 参数状态改为 `ParameterEntry` / `ParameterValue` 语义。
  - 参数面板改为按类型渲染，并补单项恢复默认值、自动触发重渲染和与预设文件的一致序列化。
- 当前处理方式：本轮只记录为已确认差距；在补齐参数定义来源和类型化 UI 之前，不能把 Web 参数编辑判定为共享参数模型的完整实现。

## 2026-04-24 02:08:00: Web 切片器面板只有列表，没有“发送到切片器”动作

- 来源：检查 `packages/studio-web/src/workbench/{export-panel,slicer-panel}.tsx` 与 `ExportRun.slicer_name` 的请求链路。Web 端目前只展示切片器列表，不提供任何触发动作。
- 原因：
  - `SlicerPanel` 只是把 `SlicerListResponse` 渲染成只读列表，没有动作按钮。
  - `ExportPanel` 发 `ExportRun` 时把 `slicer_name` 固定为 `null`，因此即使服务端已经返回切片器列表，Web 端也不会走“发送到切片器”路径。
- 影响范围：
  - 即使后续把设置页配置接入工作台，Web 端仍只能“看见切片器”，不能“发送到切片器”。
  - 旧 result 把切片器能力写成“已接回”会误导后续计划，因为当前实现离真实工作流还差最后一跳。
- 可能的解法：
  - 把切片器列表与导出动作打通，为每个切片器提供单独按钮，或提供可选目标切片器的导出表单。
  - `ExportRun` 请求必须传入 `slicer_name`，并与配置快照联动。
- 当前处理方式：本轮只记录审计结论；在动作入口补齐前，Web 切片器功能只能视为只读信息展示。

## 2026-04-24 01:24:53: Web 文件监听刷新只覆盖目录树与激活 `.scad`，其它打开中的文档不会自动更新

- 来源：检查 `packages/studio-web/src/{workbench/workbench-layout,viewers/markdown-viewer,viewers/image-viewer,viewers/mesh-viewer,workbench/scad-workbench}.tsx`。Web 端 watch 事件目前主要做目录树重拉和激活文档刷新。
- 原因：
  - `WorkbenchLayout.onWatchEvent` 只会 `refreshRootListing`、`refreshExpandedDirectories`，以及在激活 tab 为 `.scad` 时递增 `scadRefreshSignal`。
  - `MarkdownViewer`、`ImageViewer`、`MeshViewer` 的 effect 依赖都只有 `path`，没有任何 watch invalidation 输入。
  - `ScadWorkbench` 的 `loadPresets()` 也只在 `path` 变化时触发，外部修改预设文件不会自动刷新。
- 影响范围：
  - 已打开的 Markdown、图片、`.stl`、`.3mf` 文档在源文件变化后不会自动重载；目录树会变，但内容面板保持旧数据。
  - `.scad` 预设文件若被外部工具或其它客户端改写，Web 参数/预设面板不会同步更新。
  - 这类问题非常容易让 smoke 通过，因为目录树确实刷新了，但用户真正盯着看的活动文档没有更新。
- 可能的解法：
  - 给各 viewer / preset loader 增加 watch invalidation 输入，至少在 path 命中或目录级事件发生时重拉当前内容。
  - 更彻底的方案是把文档级 watch 生命周期收敛到共享状态层，而不是只在 React 顶层做一次目录级广播。
- 当前处理方式：先记录为 review finding；在补齐所有文档类型的 invalidation 前，Web 端不能宣称文件监听已覆盖全部打开文档。

## 2026-04-23 20:10:00: `WatchChangedEvent.changed_paths` 只给目录级路径，Web 端无法精确匹配文件

- 来源：执行 Phase 7 步骤 E（`.scad` 自动重渲染）时，Playwright smoke 观察 `client_drain_events` 产出的 `WatchEvent` payload：`changed_paths` 往往只包含目录级 `PathHandle`（`path_segments: []`），没有被修改的具体文件 handle。
- 原因：`app-server-host::watch` 聚合 notify 事件后目前只回传监听的目录 handle；文件级变更事件未投递到 `WatchChangedEvent`。
- 影响范围：
  - 当时 Web 端 WorkbenchLayout 不能仅凭 `changed_paths` 判断"当前激活的 scad 文件是否被修改"。当时退让方案是：凡是 scad tab 激活且有任何 watch 事件，均触发 refreshSignal，smoke 写入 "auto rerender triggered by {path} (directory change)"。
  - 若多个文件同时变更，Web 端会做一次粗粒度重渲染而不是按文件去抖；对 preview 成本可控但理论上浪费。
- 可能的解法：
  - 服务端把 notify 事件里的文件 handle 透传到 `WatchChangedEvent.changed_paths` 而不是只回目录；需要在 `app-server-host::watch` 中按事件类型填充 `changed_paths`。
  - 或在协议层新增 `WatchChangedEvent.reason`（`DirectoryChanged` vs `FileChanged { paths }`）让 client 显式知道粒度。
- 当前处理方式：已在 `prompt-archives/2026050100-async-rig-web-search/plan-00.md` Phase 1 执行中修复。`WatchChangedEvent.changed_paths` 现在按实际变更路径映射为 `PathHandle`，Web 端按源文件与设置文件分别刷新。本条保留为历史记录，不再阻塞后续开发判断。

## 2026-04-23 20:05:00: `PreviewRequest` 链路未返回 `ParsedParameters`，Web 参数面板无法自动解析 .scad 参数

- 来源：执行 Phase 7（`prompt-archives/2026042300-studio-web-feature-parity/plan-00.md` §Phase 7 步骤 B）时，按计划应"优先走协议层 `ParsedParameters`，无法做到时回退到源码字符串解析"。审查 `crates/app-server-core/src/preview.rs` 与 `crates/app-server-host/src/dispatcher.rs` 后确认，`CommandSuccess::PreviewReady` 当前只带回 `PreviewReadyResponse { requested_kind, artifact }`，不包含 `ParsedParameters`；`studio-common` 的 re-export 也没有把解析能力接到 `ManagedClient` 的正常流程。
- 原因：协议层 `ParameterDefinition` / `ParsedParameters` 类型虽已存在，但没有命令能把它们投递给客户端；web 端想拿到参数定义就必须新增协议命令或给 `PreviewRequest` 加返回字段——都超出 Phase 7 范围（Phase 7 明确禁止改协议/server-core）。
- 影响范围：
  - Web 参数面板只能依赖"用户手工输入 `name=value` 后由 `PreviewRequest.defines` 透传"这一退化路径，没法像桌面端一样先解析 `.scad` 顶部的变量默认值再渲染成带类型控件。
  - `docs/web-platform-limits.md §9` 明确声明这条限制；但从协议层看，这属于"能力缺口"而非"平台差异"，放已知问题而不是平台限制页。
  - 未来若 Phase 8+ 想做参数 UI 的"恢复默认值"语义，必须先补协议层支持。
- 可能的解法：
  - 给 `PreviewRequest` / `PreviewReadyResponse` 增加可选 `ParsedParameters` 字段；或单独添加 `ParametersInspect` 命令返回 `ParsedParameters`。任一都要同步 `app-server-core` 的解析实现与 `studio-common::ManagedClient` 的回执处理。
  - 另起 plan，在其中定义协议扩展、兼容策略（`#[serde(default)]`）和两端同步改动。
- 当前处理方式：Phase 7 web 端参数面板走手工 `name=value` 回退路径；`docs/web-platform-limits.md §9` 记录用户可见约束；本条记录解释为什么不是"平台限制"。

## 2026-04-23 20:05:00: `ExportRun.output_path` 要求 server 侧绝对 `PathBuf`，web 端无法决定目标目录

- 来源：执行 Phase 7 步骤 C（Export 流）时，检查 `app-server-protocol::ExportRunRequest.output_path: PathBuf` 与 `crates/app-server-core/src/export.rs::export_model`。Web 端发的是浏览器侧 UTF-8 字符串，该字符串被 server 作为 CLI `-o` 参数传给 OpenSCAD，按 server 进程 cwd 或绝对路径解析。
- 原因：协议在定义 `ExportRunRequest` 时假定客户端知道 server 机器真实文件系统；这在桌面端成立，在 web 端不成立。没有 `PathHandle`-化的导出接口，也没有"写到 workspace 下某个相对路径"的语义。
- 影响范围：
  - Web 端 Phase 7 导出 UI 只接受用户输入文件名（默认 `<stem>.stl`），实际写入 server 进程 cwd，不保证在 workspace 根目录下，也不保证对用户可见。
  - smoke（`@export-slicer`）只能断言 `export done|export error`，不能验证导出文件位置。
- 可能的解法：
  - 扩展协议：`ExportRunRequest.output` 改为 `PathHandleWritable`（新增路径类型），由 server 解析为 workspace 根下的相对路径；或复用现有 `PathHandle` 作为目录 + 文件名两字段。
  - 需求上若只要求"导出到 workspace 某目录"，可先约定 server 端默认写到 `workspace_root/exports/<filename>`。
- 当前处理方式：已在 `prompt-archives/2026042500-borsh-protocol-wasm/plan-00.md` Phase 1 和 Phase 4 中解决。`ExportRun.output_path` 已改为 workspace 内 portable path，web 端默认把导出文件名解析为当前源文件同目录下的 portable `PathHandle`；`docs/web-platform-limits.md §10` 已同步为当前行为。本条保留为历史问题记录，不再阻塞后续开发判断。

## 2026-04-22 00:00:00: 历史记录：旧 Rust GUI 应用缺少交互式桌面回归能力

- 来源：执行 `prompt-archives/2026042200-studio-app-server-unification/plan-00.md` 的验收过程中，已能通过 workspace 构建/测试和桌面二进制编译确认旧 Rust GUI 应用可进入运行路径，但当时会话没有桌面自动化能力，无法在同一条执行链中继续点击菜单、打开工作区、切换文档标签并观察真实窗口渲染。
- 原因：当前环境具备编译、测试和进程级启动能力，但不具备桌面 GUI 级别的交互自动化工具；已有自动化测试主要覆盖状态机和纯逻辑，不能等价替代完整的人机交互回归。
- 影响范围：
  - 该记录只解释当时桌面 GUI 回归覆盖不足，不再作为当前产品端风险跟踪。
  - Phase 1 已删除旧 Rust GUI 应用、旧 Rust egui UI 层和旧 Rust viewer crate；后续验收不再要求补交互式桌面回归。
- 可能的解法：
  - 无需继续为旧 Rust GUI 应用补自动化；如未来重新引入原生桌面端，应另起计划定义新的产品边界、测试 harness 和验收标准。
- 当前处理方式：本条转为历史记录。当前生产 GUI 端为 Web，回归以 WebSocket host、studio-web、studio-web-wasm 和浏览器 smoke 为准。

## 2026-04-07 21:39:25: 历史记录：旧 Rust GUI DocumentWorkspace 曾保留 `DocumentKey` 与 `TabId` 双身份体系

- 来源：对 `旧 Rust GUI crate 的 app 实现`、`旧 Rust GUI crate 的入口实现`、`旧 Rust GUI crate 的文档实现`、`旧 Rust GUI crate 的 viewer tab 目录`、`旧 Rust GUI crate 的 Markdown 预览实现` 的迁移代码审查。
- 原因：文档工作区已经以 `DocumentKey` 作为主身份，但运行时消息分发仍依赖 `legacy_tab_id()`，`ViewerTab`/`MarkdownTab` 继续实现 `WorkTab`，`main.rs` 仍通过 `document_by_legacy_tab_id_mut()` 查找会话。
- 影响范围：
  - 该记录只解释旧 Rust GUI 应用迁移时的结构风险，不再作为当前产品端风险跟踪。
  - Phase 1 已删除旧 Rust GUI 应用、旧 Rust egui UI 层和旧 Rust viewer crate；后续计划不再要求整理旧 `tab_system`。
- 可能的解法：
  - 无需继续清理旧 Rust GUI 应用内部身份体系；如未来重新引入原生桌面端，应重新定义文档身份模型。
- 当前处理方式：本条转为历史记录。当前生产 GUI 端为 Web，文档身份与运行时接线以后续 Web / protocol 边界为准。

## 2026-04-07 21:39:25: 历史记录：旧 Rust GUI DocumentWorkspace 真实运行时分支缺少自动化测试

- 来源：对 `旧 Rust GUI crate 的 app 实现`、`旧 Rust GUI crate 的入口实现`、`旧 Rust GUI crate 的工作区实现` 的 DocumentWorkspace 迁移代码审查。
- 原因：当前 `旧 Rust GUI 测试` 只验证通用状态与欢迎态，未覆盖真实文档会话下的打开文件、watch 回调、Viewer/Markdown 分发与工作区轨道交互；生产代码中的真实会话分支仍主要依赖 `cargo build` 做编译级回归。
- 影响范围：
  - 该记录只解释旧 Rust GUI 应用当时的测试缺口，不再作为当前产品端风险跟踪。
  - Phase 1 已删除旧 Rust GUI 应用、旧 Rust egui UI 层和旧 Rust viewer crate；后续验收不再要求补旧 GUI 运行时分支测试。
- 可能的解法：
  - 无需继续为旧 Rust GUI 应用补测试；如未来重新引入原生桌面端，应另起计划定义运行时测试 harness。
- 当前处理方式：本条转为历史记录。当前生产 GUI 端为 Web，回归以 WebSocket host、studio-web、studio-web-wasm 和浏览器 smoke 为准。

## 2026-04-02 16:47:56: 本地环境缺少可验证 3MF 彩色预览的 OpenSCAD CLI / Nightly

- 来源：为 3MF 彩色预览计划检查本机 OpenSCAD 环境时，执行 `command -v openscad` 与读取 `OPENSCAD_PATH`，结果均为空。
- 原因：当前工作机未安装可直接调用的 OpenSCAD CLI，因此无法确认是否具备支持彩色 3MF 预览的 Nightly 能力。
- 影响范围：
  - 无法在本机完成 `scad -> OpenSCAD 3MF -> 彩色预览` 的端到端验证。
  - 后续实现阶段只能先依赖 3MF fixture、单元测试和用户环境联调来验证颜色解析与渲染。
- 可能的解法：
  - 在执行阶段安装 OpenSCAD Nightly，并通过 `OPENSCAD_PATH` 或设置窗口显式指向该版本。
  - 在仓库中加入最小彩色 3MF fixture，用于脱离 OpenSCAD 环境验证解析与渲染链路。
  - 将“Nightly 环境下的人工联调”列为独立验收项，而不是与纯单元测试混在一起。
- 当前处理方式：已补 `tests/three_mf_tests.rs`、`tests/mesh_tests.rs`、`tests/pipeline_tests.rs` 等回归测试，自动化验证覆盖 3MF 解析与颜色渲染协议；在具备 Nightly 的环境前，不宣称完成 `scad -> OpenSCAD 3MF -> 彩色预览` 的端到端人工验收。

## 2026-04-01 13:20: feature-roadmap 与现行 plan 在 3MF 解析范围上不一致

- 来源：对照 `docs/feature-roadmap.md` 与 `prompt-archives/2026033101-full-features/plan-00.md`。
- 原因：roadmap 仍包含“3MF 文件解析（支持颜色信息）”，但当前 plan 仅覆盖 3MF 导出，不包含 3MF 导入解析。
- 影响范围：即使按现行 plan 完成所有 Phase，也无法直接把 roadmap 全部未完成项勾选为已完成。
- 可能的解法：
  - 单独补一轮 3MF 解析计划，明确是否需要颜色贴图、零件层级和 ZIP 容器读取。
  - 或者回写 roadmap/plan，明确当前版本仅支持 3MF 导出，不支持导入解析。
- 当前处理方式：本轮已实现 3MF 预览解析并同步更新 `docs/feature-roadmap.md`，该问题不再阻塞后续开发判断。
