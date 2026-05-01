# Async 后端 / Rig Agent / 模型原生联网搜索执行结果

## 当前状态

- 计划已创建。
- 独立 reviewer 已完成只读审查，未发现阻塞项。
- reviewer 第一轮提出的高风险与普通问题已修订进计划：补齐同步 I/O / 子进程 / 线程模块清单，调整 Phase 2 与 Phase 3 的 Agent worker 顺序，补齐旧路径搜索关键词，补齐 protocol wasm 与 Rust crate 验证。
- reviewer 第二轮提出的范围冲突已修订进计划：旧 Agent / LLM 生产入口按全仓库搜索，后端同步 I/O / 子进程 / 线程关键词只检查 app-server-core 与 app-server-host 生产源码；同时补充 Web smoke 与 browser smoke 验证。
- reviewer 第三轮未发现阻塞项或高风险问题；唯一普通问题已修订：Phase 3 host 侧 Agent 测试输入从不存在的通配路径改为实际测试文件。
- 用户追加要求 Rust 桌面 app 可以完全删除，并要求把删除桌面 app 调整到最前面；计划已重写为 7 个 Phase，Phase 1 先删除 Rust 桌面端与桌面专属生产路径。
- reviewer 第四轮指出 protocol / 生成包桌面 platform 残留风险与 Web smoke `scad-viewer` 标签歧义；已修订 Phase 1 与 Phase 7，纳入 `ClientPlatform::Desktop`、`"desktop"` product platform 和 Web smoke 标签重命名检查。
- reviewer 第五轮未发现阻塞项或高风险问题；普通问题已修订：补充 host 侧 in-process / mpsc 测试输入，补充 `Desktop =` / `ClientPlatform` 搜索关键词，补充删除桌面 UI 后的 workspace 依赖清理。
- reviewer 第六轮指出 `child_terminator`、GUI shutdown example、旧 Chat Completions 精确关键词、跨平台路径文档与 `python3` 测试调用遗漏；已修订 Phase 1、Phase 3 和 Phase 7 输入、操作步骤与最终搜索关键词。
- reviewer 第七轮指出同步文件系统关键词和旧 LLM 配置 / UI 提示检查不足；已扩展 Phase 4 的配置替换要求，并扩展 Phase 7 的 `fs::`、`OpenOptions`、`File::open`、`BUDN_LLM_BASE_URL`、`base_url`、OpenAI-compatible 等搜索关键词。
- reviewer 第八轮指出同步 mpsc / `recv_timeout` 关键词、Web UI 提示输入和历史 fallback 文档关键词遗漏；已补充 Phase 4 输入与 Phase 7 搜索关键词。
- 用户指出同步路径搜索与 Tokio 同名类型存在歧义；已修订 Phase 7，明确候选项必须结合 import 与类型来源判断，`tokio::fs`、`tokio::process::Command`、`tokio::sync::mpsc`、`tokio::task::JoinHandle` 是允许的 async 目标。
- reviewer 第九轮指出 `thread::Builder` / `std::thread::JoinHandle`、不存在的 host 文件输入、Phase 4 Web 验证和 Phase 1 smoke 验证问题；已补充阻塞式线程关键词，移除不存在文件输入，并补充 Web typecheck / unit 与 smoke 验收。
- reviewer 第十轮复审未发现阻塞项、高风险或普通问题。
- Phase 1 已完成实现、验证和独立复审；第四轮 Phase 1 复审未发现阻塞项或高风险问题，准备提交 Phase 1。

## Phase 记录

### Phase 1 — 删除 Rust 桌面端与桌面专属生产路径

- 状态：已完成，准备提交。
- 变更摘要：
  - 删除 `crates/studio-app`、`crates/scad-ui`、`crates/scad-viewer`，并从 workspace 成员与锁文件中移除相关依赖。
  - 删除 `app-server-host` 的 in-process / mpsc 生产入口、导出、runtime、GUI shutdown example 和对应 host 测试。
  - 删除 protocol / TypeScript bridge 中的桌面 product platform，保留 `ClientPlatform::Web = 1` 与 `Other = 2` 的 wire discriminant。
  - 将 Web smoke 中 `scad_viewer` / `@scad-viewer` 重命名为 `scad_preview` / `@scad-preview`。
  - 修正 watcher 事件，使服务端向 Web 发送实际变更路径；Web 端按源文件与设置文件分别刷新，避免设置文件变化触发无关预览重跑。
  - 调整 Playwright smoke harness，使用隔离 host 环境和临时 workspace，避免测试污染共享 fixture。
  - 更新架构、getting started、Web 平台限制、跨平台路径策略、设计系统与已知问题文档，明确当前生产 GUI 端为 Web，旧 Rust GUI / viewer 相关问题转为历史记录或 Web 当前能力问题。
- 验证结果：
  - `bun run protocol:build && bun run protocol:check-generated` 通过。
  - `bun run --cwd packages/studio-web typecheck` 通过。
  - `cargo test --workspace` 通过；仍有既有 dead_code warning，未在本 Phase 扩大处理范围。
  - `bun run --cwd packages/studio-web test:unit` 通过；仍有既有 React `act(...)` warning，未在本 Phase 扩大处理范围。
  - `bun run web:smoke` 通过；构建阶段仍有既有 Vite 大 chunk warning，未在本 Phase 扩大处理范围。
  - `bun run web:smoke:browser` 通过，75 个 Playwright 用例通过。
- 独立复审记录：
  - 第一轮 Phase 1 复审发现两个代码 / 文档阻塞项：`ClientPlatform::Web` discriminant 不得改变，`docs/known_issues.md` 不得伪造归档路径或继续把旧桌面 GUI 回归作为当前风险。已修正。
  - 第二轮 Phase 1 复审发现三个归档阻塞项：`docs/known_issues.md` 仍有不可验证的外部 worktree 路径，部分旧桌面 / viewer 差距仍作为当前目标描述，`plan-00-result.md` 仍写 Phase 1 未执行。已修正。
  - 第三轮 Phase 1 复审发现 `docs/known_issues.md` 的 Markdown 历史记录、`docs/architecture.md` 的硬约束回顾和 `docs/feature-roadmap.md` 的平台菜单说明仍有旧桌面当前目标口吻。已改为 Web / 共享模型 / 历史记录口径。
  - 第四轮 Phase 1 复审未发现阻塞项或高风险问题；普通问题指出 `docs/architecture.md` 对 `scad-scene` 的描述比当前 crate 状态更激进。已改为 Web 生产路径只消费其 mesh / STL / 3MF 纯数据能力，并记录旧 renderer / pipeline / gizmo / 窗口模块仍待后续整理。
- 遗留问题：
  - Phase 1 未处理后续 async service、Rig-only Agent 和模型原生联网搜索目标；这些仍按 Phase 2 到 Phase 6 自动推进。

### Phase 2 — 建立 WebSocket-only async 后端服务边界

- 状态：已完成，准备提交。
- 前序目标保护：
  - 未重新引入 in-process / mpsc host；Phase 1 删除的桌面生产路径保持删除状态。
  - WebSocket 仍是唯一生产 transport；protocol 与 transport 分离未改变。
  - 现有 workspace、file、preview、CadQuery、Agent 与 session 行为继续通过原有 host roundtrip 和 WebSocket 测试覆盖。
- 变更摘要：
  - 将 `HostRequestDispatcher::handshake`、`HostRequestDispatcher::dispatch_envelope` 和内部 `dispatch_command` 改为 async 方法。
  - WebSocket host 在握手和 request frame 处理中 await 同一 async dispatcher 入口，不再调用同步 dispatcher 方法。
  - host roundtrip 测试改为覆盖 async dispatcher 入口；测试 harness 保留同步辅助函数，但其内部通过 Tokio runtime 调用 async dispatcher。
- TDD 记录：
  - 先将 `shared_dispatcher_roundtrips_handshake_workspace_file_and_preview` 改为 async 调用并运行 `cargo test -p app-server-host shared_dispatcher_roundtrips_handshake_workspace_file_and_preview`，确认因 `handshake` / `dispatch_envelope` 不是 future 而编译失败。
  - 将生产入口改为 async 后，重新运行同一测试，测试通过。
- 验证结果：
  - `cargo test -p app-server-host` 通过；仍有既有 app-server-core dead_code warning，未在本 Phase 扩大处理范围。
  - `cargo test -p app-server-transport` 通过。
  - 针对同步入口的源码搜索未发现 `pub fn dispatch_envelope` 或同步 `fn dispatch_command`；仅剩 WebSocket 对 `dispatcher.dispatch_envelope(envelope).await` 的调用和 `async fn dispatch_command` 定义。
- 独立复审记录：
  - Phase 2 独立复审未发现阻塞项、高风险问题或普通问题；确认 WebSocket host 已 await async dispatcher，测试 harness 的 `Runtime::new().block_on(...)` 只存在于测试范围内，Phase 1 删除桌面端 / in-process / mpsc 的边界未回归。

### Phase 3 — 核心 I/O、ChatStore、预览与子进程路径 async 化

- 状态：已完成，准备提交。
- 前序目标保护：
  - 未重新引入 Rust 桌面端、`studio-app`、`scad-ui`、`scad-viewer` 或 in-process / mpsc host 生产路径。
  - 保持 Phase 2 的 async dispatcher 入口；WebSocket host 继续通过 async dispatcher 处理 protocol request。
  - CadQuery staging 仍保持先 staging、成功后提交、失败 / 取消 / 超时不污染真实 workspace 的边界。
- 变更摘要：
  - 将 workspace、file、config、presets、plan package、Agent readonly / file_write / semantic 工具、ChatStore 等文件系统路径改为 async。
  - 将 preview / export / CadQuery runner / CadQuery contract / CadQuery staging / host CadQuery env check 改为 `tokio::fs` 与 `tokio::process`。
  - 删除不再使用的 `child_terminator` 同步 `std::process::Child` 终止路径。
  - 将 watcher 从 `std::sync::mpsc + recv_timeout + thread::spawn` 改为 Tokio channel 与 interval 驱动；公开匹配函数保留 canonical 路径语义并改为 async。
  - 将 ChatStore JSONL 会话创建、追加、history、archive、summary 更新改为 async，并同步调整 host dispatcher 与 Agent tool observer 的调用。
  - 将 WebSocket 连接处理恢复为普通 `tokio::spawn`，并对 request dispatch future 增加编译期 `Send` 断言。
  - CadQuery runner 取消 / 超时路径已显式 kill 并等待子进程退出，再返回错误；contract runner 出错和超时路径会先删除临时 contract 文件。
- 第一轮独立复审记录：
  - 发现 CadQuery contract 临时文件在 spawn / wait 错误路径可能泄漏；已改为所有返回前执行 `contract_file.remove().await`。
  - 发现 runner 取消 / 超时仅依赖 `kill_on_drop(true)`，staging cleanup 可能早于子进程完全退出；已改为显式 `start_kill`、`wait` 并等待 stdout / stderr 读取任务结束。
  - 发现超时与 cleanup 测试覆盖不足；已补充 runner timeout marker、contract timeout 临时文件清理、staging timeout cleanup 测试，并增强 cancel 后 staging 根目录清理断言。
  - 发现 Phase 3 结果记录仍为未执行；本节已更新。
- 第二轮独立复审记录：
  - 发现 WebSocket 连接处理使用 `spawn_blocking + current_thread runtime` 包装，违反 Phase 3 对生产 async WebSocket 路径的验收；已改为普通 `tokio::spawn`，并将 dispatcher 生产路径经过的 ChatStore、staging、preview、config、workspace helper 改为 owned 参数或 await 前完成借用转换。
  - 发现 `StagedCadQueryProject` 公共 API 缺少误用提示；已增加 `#[must_use]`，并保持 commit / cleanup 消费 `self`，降低 staging 目录被遗留的风险。
  - 发现结果记录仍缺少最终验证证据；本节已补充 `git diff --check` 与完整 core / host 测试结果。
- 第三轮独立复审记录：
  - 发现 CadQuery staging 成功后、runner 启动前的取消检查会直接返回，导致 `.budn_staging` 遗留；已在该取消分支显式 cleanup，并补充 `cadquery_staging_cleans_up_when_cancelled_after_stage_before_runner` 回归测试。
  - 发现 Chat JSONL owned append 路径使用 read-modify-write 覆盖文件，可能丢失并发追加；已恢复 `OpenOptions::append(true)` 语义，同时保留 WebSocket dispatch future 的 `Send` 编译断言。
  - 发现 CadQuery 配对说明文档追加记录使用 read-modify-write 覆盖文件；已恢复 append 写入语义。
- 第四轮独立复审记录：
  - 未发现阻塞项、高风险或普通问题。
  - 确认 staging 取消清理、Chat JSONL append 语义、CadQuery 说明文档 append 语义、WebSocket `tokio::spawn` / dispatch future `Send` 约束和结果文档记录均满足 Phase 3 要求。
- 验证结果：
  - `cargo test -p app-server-core cadquery_staging_cleans_up_when_cancelled_after_stage_before_runner` 通过。
  - `cargo test -p app-server-core cadquery_runner` 通过，覆盖 runner 成功、取消、超时、Python import error 与大 stdout。
  - `cargo test -p app-server-core cadquery_contract_removes_temp_file_on_timeout` 通过。
  - `cargo test -p app-server-core cadquery_staging_cleans_up_after_runner_timeout` 通过。
  - `cargo test -p app-server-core` 通过。
  - `cargo test -p app-server-host` 通过，包含 WebSocket smoke roundtrip、压缩与大 frame 用例。
  - `git diff --check` 通过。
- 遗留问题：
  - 旧 Agent worker 的 `std::thread` / `thread::sleep` 路径按计划留到 Phase 4 与 Rig-only 迁移一起删除。

### Phase 4 — Rig 成为唯一生产 Agent 执行引擎

- 状态：已完成，准备提交。
- 前序目标保护：
  - 未重新引入 Rust 桌面端、`studio-app`、`scad-ui`、`scad-viewer` 或 in-process / mpsc host 生产路径。
  - 保持 Phase 2/3 的 async dispatcher、async I/O、async CadQuery runner / staging 和 WebSocket-only 生产 host 边界。
  - 工具 registry、路径权限、CadQuery staging、Chat history、Agent run 管理、取消语义和 protocol event 继续由 app server 承接。
- 变更摘要：
  - 将 `rig-core` 加入 workspace 依赖，`app-server-core` 使用 Rig OpenAI Responses API 作为唯一生产 Agent 执行引擎。
  - 删除旧 `openai_compat`、`LlmProvider`、OpenAI-compatible Chat Completions 配置语义、`ureq` SSE parser、自研多轮 tool loop 和本地假 Agent 回答。
  - 新增 Rig Agent 配置读取：`BUDN_AGENT_CONFIG`、`BUDN_AGENT_OPENAI_API_KEY` / `OPENAI_API_KEY`、`BUDN_AGENT_MODEL`、`BUDN_AGENT_TIMEOUT_SECS`、`BUDN_AGENT_MAX_TOKENS`、`BUDN_AGENT_TEMPERATURE`、`BUDN_AGENT_REASONING_EFFORT`。
  - 将现有 Agent tool registry 包装为 Rig dynamic tools，继续复用既有 path policy、CadQuery staging、semantic tool 和 readonly/file_write 工具实现。
  - 将 Rig streaming event 映射到现有 protocol 事件：token、reasoning、tool start、tool result、done 和 error。
  - Agent worker 改为 `tokio::spawn` async task；删除旧 `std::thread::spawn`、`thread::sleep` 和同步 helper。
  - Rig 请求创建与 stream drain 使用同一 timeout，并在 provider stream pending 时通过 cancel tick 响应 Agent cancel。
  - Agent tool history 改为按事件顺序收集并串行追加到 ChatStore，保留 `run_id` 与 CadQuery `mesh_result`。
  - Web Chat 空状态和错误提示改为 Rig OpenAI Responses 配置说明。
  - 更新 `docs/known_issues.md`，关闭旧 AgentBackend / 本地文本草稿 / OpenAI-compatible 生产路径记录，并保留结构化 edit intent 作为当前后续问题。
- TDD / 回归记录：
  - 补充 Rig stream 映射测试，覆盖 token、reasoning、tool call、tool result。
  - 补充 `drain_rig_stream` 测试，覆盖 provider stream pending 时 cancel 返回、配置 timeout 返回和 provider error 映射。
  - 补充 host recorder 测试，覆盖 tool call/result 按顺序写入 Chat history，并持久化 `run_id` 与 CadQuery `mesh_result`。
  - 调整 host Agent 测试，使 Agent async task 与测试 dispatch 运行在同一 Tokio runtime 内，避免测试 helper 临时 runtime 结束时中止 worker。
- 第一轮独立复审记录：
  - 发现 Rig stream cancel 只在 `stream.next().await` 返回后检查，provider pending 时会阻塞 Agent run；已改为 `tokio::select!` 同时监听 cancel tick、timeout 和 stream item。
  - 发现 `BUDN_AGENT_TIMEOUT_SECS` 已读取但未用于 Rig 请求；已将同一 timeout 覆盖 stream 创建和 drain。
  - 发现 tool call/result 使用独立 `tokio::spawn` 写 Chat history，等待完成不能保证 JSONL 顺序；已改为内存队列收集并串行写入。
  - 发现测试缺口：缺少 cancel / timeout / provider error 与 tool history 顺序覆盖；已补充上述回归测试。
- 第二轮独立复审记录：
  - 未发现阻塞项或高风险问题。
  - 确认 `BUDN_AGENT_TIMEOUT_SECS` 已被请求创建和 stream drain 使用，`drain_rig_stream` 可在 stream pending 时响应 cancel / timeout。
  - 确认 `AgentToolEventRecorder` 已按事件顺序串行写入 ChatStore，并保留 `run_id` 与 CadQuery `mesh_result`。
  - 普通问题指出本结果文档仍写 Phase 4 未执行；本节已更新。
- 验证结果：
  - `cargo test -p app-server-core drain_rig_stream` 通过。
  - `cargo test -p app-server-host agent_tool_recorder_flushes_history_in_event_order` 通过。
  - `cargo test -p app-server-core` 通过。
  - `cargo test -p app-server-host` 通过。
  - `bun run --cwd packages/studio-web typecheck` 通过。
  - `bun run --cwd packages/studio-web test:unit` 通过；仍有既有 React `act(...)` warning，未在本 Phase 扩大处理范围。
  - `git diff --check` 通过。
  - Phase 4 旧路径搜索无命中：`ureq`、`openai_compat`、`LlmProvider`、`LlmMessage`、`LlmResponse`、`LlmConfig`、`LlmTool`、`AgentBackend`、`LocalAgentBackend`、`llm_generate_cadquery_code`、`create_provider`、`run_tool_loop`、`Chat Completions`、`chat/completions`、`BUDN_LLM_BASE_URL`、`BUDN_LLM_CONFIG`、`BUDN_LLM`、`base_url`、`OpenAiCompatible`、`OpenAI-compatible`、`OpenAI compatible`、`std::thread::spawn`、`thread::sleep`、`block_on_old_agent_future`。
- 遗留问题：
  - 模型原生联网搜索尚未接入；按计划留到 Phase 5。
  - protocol / Web 端 capability 与搜索来源字段尚未接入；按计划留到 Phase 6。
  - 更完整的 provider mock / hosted tool capability 集成测试适合随 Phase 5/6 一起补齐；本 Phase 已覆盖 Rig stream 映射、cancel、timeout、provider error 和 host Chat history 顺序。

### Phase 5 — 接入模型原生联网搜索

- 状态：已完成，准备提交。
- 前序目标保护：
  - 未重新引入 Rust 桌面端、`studio-app`、`scad-ui`、`scad-viewer` 或 in-process / mpsc host 生产路径。
  - 保持 WebSocket async service、async I/O、Rig-only Agent、工具 registry、路径权限和 CadQuery staging 边界。
  - 没有新增本地互联网搜索工具；联网搜索只通过 OpenAI Responses hosted tool 暴露给模型。
- 变更摘要：
  - 为 Rig Agent 配置增加 `native_web_search`，支持 `BUDN_AGENT_NATIVE_WEB_SEARCH=true` 与 `BUDN_AGENT_CONFIG` TOML 字段，默认关闭。
  - Rig 请求参数在开启配置时加入 `tools: [{ "type": "web_search" }]`，并保留 reasoning 参数；本地源码审查确认 Rig 0.35.0 会把该 hosted tool 追加到 OpenAI Responses request tools，不会覆盖已有 workspace function tools。
  - Host 在每次 Agent run 开始时读取同一份 Rig 配置，向 Chat history 写入 `agent_run_capabilities` meta record，记录 provider 与 native web search 是否开启，且不记录 API key。
  - Agent turn context 增加 native web search 状态，system prompt 明确 native web search 只能用于外部事实、标准、API / 背景资料，本地 workspace 必须走 app server 工具。
  - Web Chat 空状态补充 `BUDN_AGENT_NATIVE_WEB_SEARCH=true` 可选配置提示。
  - `docs/known_issues.md` 新增记录：Rig 0.35.0 暂未暴露 OpenAI web search `sources / annotations`，本 Phase 只能保存最终文本与 provider capability record，来源展示留到 Phase 6 处理。
- TDD / 回归记录：
  - 补充 Rig additional params 测试，覆盖默认不注册 hosted web search、开启后注册 hosted `web_search`。
  - 补充 Rig Agent 配置测试，覆盖 env 与 TOML 配置中的 enabled / disabled 状态，并确认 Debug 不泄漏 API key。
  - 补充 host Chat history meta 测试，覆盖 `agent_run_capabilities.native_web_search_enabled` 与 `run_id` 持久化。
  - 补充 host error mapping 测试，确认 timeout 映射为 `AgentErrorType::Timeout`，unsupported / auth / rate / search error 仍归入 `LlmError`。
- 独立复审记录：
  - Phase 5 独立复审未发现阻塞项或高风险问题。
  - reviewer 确认当前实现没有新增本地互联网搜索工具，`web_search` 通过 Rig OpenAI Responses `additional_params.tools` 注册为 provider hosted tool；本地 `rig-core-0.35.0` 源码显示该字段会追加到已有 function tools 后面，不会覆盖 workspace 工具。
  - reviewer 提出的普通问题中，配置文件路径测试已补齐；结果文档已更新；hosted tool 与 function tools 共存风险已通过 Rig 源码审查确认；provider search error 更细测试受当前协议错误类型和 Rig 流式抽象限制，保留为非阻塞测试缺口。
- 验证结果：
  - `cargo test -p app-server-core rig_agent_additional_params` 通过。
  - `cargo test -p app-server-core rig_agent_config` 通过。
  - `cargo test -p app-server-host agent_capability_meta_records_native_web_search_state` 通过。
  - `cargo test -p app-server-host rig_agent_errors_map_timeout_separately_from_provider_errors` 通过。
  - `cargo test -p app-server-core` 通过。
  - `cargo test -p app-server-host` 通过。
  - `bun run --cwd packages/studio-web typecheck` 通过。
  - `bun run --cwd packages/studio-web test:unit` 通过；仍有既有 React `act(...)` warning，未在本 Phase 扩大处理范围。
  - `git diff --check` 通过。
- 遗留问题：
  - OpenAI web search 的结构化 sources / annotations 暂未被当前 Rig 版本暴露，已记录到 `docs/known_issues.md`；Phase 6 需要在 protocol / Web 展示设计中处理该降级路径。

### Phase 6 — Protocol、Web 端侧与配置接入

- 状态：已完成，准备提交。
- 前序目标保护：
  - 未重新引入 Rust 桌面端、`studio-app`、`scad-ui`、`scad-viewer` 或 in-process / mpsc host 生产路径。
  - 保持 WebSocket async service、async I/O、Rig-only Agent 和 provider-native web search 边界。
  - Web 端只消费 app server protocol snapshot 与 Chat history，不直接调用 provider 或联网搜索 API。
- 变更摘要：
  - protocol 版本提升到 7，新增 `AgentProviderCapabilities` 与 `AgentSearchSource`，并在 `ServerCapabilities.agent_provider`、`ChatMessageRecord.search_sources` 中持久化能力状态和搜索来源。
  - ChatStore JSONL 读写兼容 `search_sources`，旧记录默认空来源。
  - host handshake capability 统一从 Rig Agent 配置生成，向 Web 暴露 provider、model、native web search enabled 和当前 sources 支持状态。
  - `studio-common` snapshot 保存 `agent_provider`，Web wasm snapshot 通过同一条 protocol / managed client 路径暴露该字段。
  - TypeScript protocol package、Web Chat snapshot、Zustand store 和 Chat runtime 接入新增字段；历史 assistant 消息包含搜索来源时渲染 sources data part。
  - Web Chat 增加搜索来源列表呈现，仅允许 `http://` / `https://` 链接；Chat header 在 native web search 开启时显示 capability 状态。
  - 补充 Web store、Chat runtime、Chat message、protocol package、studio-common handshake 与 wasm snapshot 的测试覆盖。
- 独立复审记录：
  - 第一轮 Phase 6 复审发现三个问题：`protocol:check-generated` 需要在生成物 staged 后通过；Web `chatHistoryEqual` 未比较 `search_sources`；`studio-common` / `studio-web-wasm` 缺少非空 `agent_provider` 传递测试。均已修复。
  - 第二轮 Phase 6 复审未发现剩余阻塞项或高风险问题；唯一普通问题为结果归档未更新，本节已补齐。
- 验证结果：
  - `cargo test -p app-server-protocol` 通过。
  - `cargo test -p app-server-protocol-wasm` 通过。
  - `cargo test -p studio-common` 通过。
  - `cargo test -p studio-web-wasm` 通过。
  - `cargo test -p app-server-host` 通过。
  - `cargo test -p app-server-core` 通过；首次与其他编译并发运行时 CadQuery staging 出现两个超时 / 取消不稳定失败，单独重跑对应测试文件通过，随后单独完整重跑 `app-server-core` 通过。
  - `bun run protocol:build` 通过。
  - `bun run protocol:check-generated` 通过。
  - `bun run --cwd packages/studio-web typecheck` 通过。
  - `bun run --cwd packages/studio-web test:unit` 通过；仍有既有 React `act(...)` warning，未在本 Phase 扩大处理范围。
  - `git diff --check` 通过。
- 遗留问题：
  - Rig 0.35.0 仍未暴露 OpenAI web search 的结构化 sources / annotations；protocol 与 Web 已具备字段和展示路径，真实来源填充需要等待 provider / Rig 暴露结构化数据，已在 `docs/known_issues.md` 记录。

### Phase 7 — 文档、已知问题、最终验证与独立 review

- 状态：未执行。
