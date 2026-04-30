# Async 后端 / Rig Agent / 模型原生联网搜索实施计划

> 执行者要求：执行本计划前必须通读 `plan-prompt.md` 与本文件。每个 Phase 必须按“实现 -> 独立 subagent review -> 修复 -> 再次验证”的循环推进；只有当前 Phase 的验收标准、前序目标保护和 review 阻塞项全部满足后，才能进入下一个 Phase。

## 背景

budn' 当前 Agent 后端已经具备 workspace 工具、路径权限、CadQuery staging、Chat history、Agent 事件和 protocol 契约，但 LLM / Agent 执行路径仍包含同步 HTTP/SSE、自研多轮 tool loop、同步 dispatcher 和线程式 worker。由于产品尚未发布，本次改造以一次架构切换为目标，不保留生产级双路径。

用户已补充：Rust 桌面 app 可以完全删除，并且删除桌面 app 必须放在最前面，避免后续计划继续把桌面端当作需要保护的目标。

## 用户强制约束识别

- Phase 1 必须先删除 Rust 桌面端及其桌面专属生产路径。
- Web 是唯一生产 GUI 端；后续计划不得继续保护 `studio-app`。
- 删除 `crates/studio-app`；删除或迁移只服务桌面端的 `scad-ui`、`scad-viewer`。
- 删除桌面 in-process / mpsc host 生产路径；WebSocket 是 app server 的生产 transport。
- 后端最终形态必须是 async 服务边界；生产请求路径不保留同步 dispatcher。
- Rig 是唯一生产 Agent 执行引擎；旧 `LlmProvider`、`openai_compat`、`ureq` SSE parser 和自研多轮 tool loop 必须删除或退出生产编译路径。
- Agent 联网搜索必须使用模型 provider 自己的 hosted tool；不新增自建互联网搜索工具。
- 不为历史兼容保留 OpenAI-compatible Chat Completions 生产路径；MVP 生产路径以 Rig + OpenAI Responses API 为准。
- 保留 budn' 的工具注册、路径权限、CadQuery staging、Chat history、Agent run 管理、取消语义和 protocol 事件。
- Web 仍不得绕过 app server protocol 直接触碰 I/O、外部调用或 provider。
- 项目内不新增 Python 调用；CadQuery Python runner 例外边界不扩大。
- 每个 Phase 完成后必须更新 `plan-00-result.md`。

## 无开放待决项声明

本计划没有开放选择项。执行时按本文件定义的目标、范围、强制约束和验收标准推进；遇到编译、测试或设计细节问题时，依据当前源码、文档和本计划自行修正，不暂停等待新的需求判断。

## 参考资料

- `Cargo.toml`
- `crates/studio-app/*`
- `crates/scad-ui/*`
- `crates/scad-viewer/*`
- `crates/app-server-host/src/dispatcher.rs`
- `crates/app-server-host/src/websocket.rs`
- `crates/app-server-host/src/in_process.rs`
- `crates/app-server-host/src/mpsc_transport.rs`
- `crates/app-server-host/src/runtime.rs`
- `crates/app-server-core/src/llm/mod.rs`
- `crates/app-server-core/src/llm/openai_compat.rs`
- `crates/app-server-core/src/agent.rs`
- `crates/app-server-core/src/agent/tools.rs`
- `docs/cadquery-mvp/decisions.md`
- `docs/cadquery-mvp/agent-tool-contract.md`
- `docs/known_issues.md`
- Rig docs.rs：`https://docs.rs/rig-core/latest/rig/`
- OpenAI Responses API：`https://platform.openai.com/docs/api-reference/responses/create`
- OpenAI web search guide：`https://platform.openai.com/docs/guides/tools-web-search`

## Phase 1 — 删除 Rust 桌面端与桌面专属生产路径

### 输入

- `Cargo.toml`
- `crates/studio-app/*`
- `crates/scad-ui/*`
- `crates/scad-viewer/*`
- `crates/app-server-host/src/in_process.rs`
- `crates/app-server-host/src/mpsc_transport.rs`
- `crates/app-server-host/src/runtime.rs`
- `crates/app-server-host/src/lib.rs`
- `crates/app-server-host/examples/gui_shutdown_abort_smoke.rs`
- `crates/app-server-host/tests/mpsc_transport_tests.rs`
- `crates/app-server-host/tests/in_process_roundtrip_tests.rs`
- `crates/app-server-host/tests/session_lifecycle_tests.rs`
- `crates/app-server-transport/*`
- `crates/app-server-protocol/src/protocol.rs`
- `crates/app-server-protocol-wasm/*`
- `crates/studio-common/*`
- `crates/studio-web-wasm/*`
- `packages/app-server-protocol/src/index.ts`
- `packages/studio-web/src/wasm-bridge/client.ts`
- `scripts/run_smoke.ts`
- `packages/studio-web/tests/playwright/browser-smoke.spec.ts`
- `docs/architecture.md`
- `docs/getting-started.md`
- `docs/web-platform-limits.md`
- `docs/2026042500-cross-platform-path-policy/README.md`
- `docs/design-system/*`
- `docs/known_issues.md`

### 前序目标保护

- 保护 app server core、protocol、WebSocket host、studio-web、studio-web-wasm、studio-common 中仍被 Web 生产路径使用的能力。
- 保护 `scad-scene` 的纯渲染数据结构与 Web mesh 解码能力。
- 保护 Web 通过 app server protocol 访问后端能力，不引入端侧 I/O 或 provider 调用。

### 操作步骤

1. 从 workspace 移除 `crates/studio-app`，删除 crate 文件、桌面 binary、bundle metadata 和相关测试。
2. 删除 `crates/scad-ui` 与 `crates/scad-viewer`；如果其中有 Web 仍需要的纯逻辑，先迁移到 `scad-scene`、`studio-common` 或 Web 包，再删除桌面 UI crate。
3. 清理根 workspace 依赖中只服务被删除桌面 UI crate 的依赖，例如 `egui_commonmark`；若 `egui`、`winit`、`muda`、`rfd` 等只由被删除 crate 使用，也一并移除。
4. 删除 `app-server-host` 中的 in-process / mpsc 生产入口、生产导出、桌面启动辅助、GUI shutdown example 和对应 host 测试，包括 `InProcessHost`、`MpscTransportAdapter`、`spawn_in_process_mpsc_host`、`AbortDecision`、`JoinThenAbort`、`evaluate_shutdown`、`GUI_SHUTDOWN_TIMEOUT`。
5. 调整 `app-server-transport`，保留 WebSocket wire / wasm client / 测试 harness 中仍被使用的部分，删除仅服务桌面 in-process transport 的生产 API。
6. 清理 `studio-common` 中只服务桌面端的状态、平台分支和公开 API；保留 Web wasm 与 app server 仍使用的共享状态和纯函数。
7. 清理 protocol、生成包和 Web bridge 中的桌面 platform 值；如果字段仍需要表达非 Web client，用 `other` 或更通用命名，不保留 `desktop` 作为产品端。
8. 重命名 Web smoke 与 Playwright 标签中容易和 Rust `scad-viewer` crate 混淆的 `scad-viewer` / `scad_viewer` 标识，改为 Web viewer 或 SCAD preview 语义。
9. 处理 `session_lifecycle_tests.rs` 中既有 `python3` 调用：如果测试随桌面 in-process 路径删除则一并删除；如果保留等价生命周期测试，必须改用 Rust 测试辅助或 repo-local bun 脚本，不得继续使用 Python。
10. 更新架构、getting started、Web 平台差异、跨平台路径策略和设计系统文档，移除“桌面端仍是目标产品端”的表述。
11. 更新 `docs/known_issues.md`，将桌面 parity、桌面自动化、桌面 smoke 缺口改为历史记录或关闭，不作为后续目标继续保留。

### 验收标准

- workspace members 不包含 `crates/studio-app`、`crates/scad-ui`、`crates/scad-viewer`。
- 根 workspace dependency 不保留只服务已删除桌面 UI crate 的依赖。
- 生产源码不再导出 `InProcessHost`、`MpscTransportAdapter`、`spawn_in_process_mpsc_host`。
- 生产源码不再导出 `AbortDecision`、`JoinThenAbort`、`evaluate_shutdown`、`GUI_SHUTDOWN_TIMEOUT` 等桌面 GUI shutdown API。
- app-server-host 测试不再包含项目内 `python` / `python3` 调用。
- 文档不再指导用户运行 `cargo run -p studio-app`。
- `studio-common` 不再包含只服务桌面 GUI 的公开 API。
- protocol、生成包和 Web bridge 不再包含 `ClientPlatform::Desktop`、`"desktop"` product platform 或等价桌面产品端类型。
- Web smoke 与 Playwright 标签不再使用 `scad-viewer` / `scad_viewer` 指代 Web viewer 用例。
- Web 仍能通过 app server protocol 与 WebSocket host 工作。
- `cargo test --workspace` 通过。
- `bun run protocol:build` 通过。
- `bun run protocol:check-generated` 通过。
- `bun run --cwd packages/studio-web typecheck` 通过。
- `bun run web:smoke` 通过。
- `bun run web:smoke:browser` 通过。

## Phase 2 — 建立 WebSocket-only async 后端服务边界

### 输入

- `crates/app-server-host/src/dispatcher.rs`
- `crates/app-server-host/src/websocket.rs`
- `crates/app-server-host/src/lib.rs`
- `crates/app-server-transport/src/lib.rs`
- 现有 host roundtrip、WebSocket、protocol 相关测试

### 前序目标保护

- 保护 Phase 1 删除桌面端后的边界：不得重新引入 in-process / mpsc 生产 host。
- 保护 app server / protocol / transport 分离：protocol 不绑定 Tokio、WebSocket 或平台私有类型。
- 保护 WebSocket host 作为唯一生产 transport。
- 保护现有 Agent、workspace、file、preview、CadQuery 命令的外部 protocol 行为，除非本计划后续 Phase 明确替换。

### 操作步骤

1. 将 host 请求处理边界改为 async service，使 WebSocket host 与测试 harness 都调用同一份 async 请求处理入口。
2. 将 `dispatch_envelope`、`dispatch_command` 和内部 command handler 迁移为 async 路径。
3. 调整 WebSocket host，使收到 client request 后不再在 async task 内直接执行同步 dispatcher。
4. 保留 transport trait 与 protocol 的分离；测试 harness 可以保留同步辅助方法，但不得作为生产 host 入口。
5. 删除或停止导出生产用同步 dispatcher API。
6. 更新 host roundtrip 测试，覆盖 WebSocket host 经过 async service。

### 验收标准

- 生产 host 请求入口只有 async service。
- WebSocket 请求处理不会直接调用同步 dispatcher。
- 生产源码没有 in-process / mpsc host 回归。
- protocol 与 transport 仍保持分离。
- `cargo test -p app-server-host` 通过。
- `cargo test -p app-server-transport` 通过。

## Phase 3 — 核心 I/O、ChatStore、预览与子进程路径 async 化

### 输入

- `crates/app-server-core/src/workspace.rs`
- `crates/app-server-core/src/file.rs`
- `crates/app-server-core/src/chat.rs`
- `crates/app-server-core/src/preview.rs`
- `crates/app-server-core/src/child_terminator.rs`
- `crates/app-server-core/src/config.rs`
- `crates/app-server-core/src/presets.rs`
- `crates/app-server-core/src/export.rs`
- `crates/app-server-core/src/watch.rs`
- `crates/app-server-core/src/agent/plan_package.rs`
- `crates/app-server-core/src/agent/tools/file_write.rs`
- `crates/app-server-core/src/agent/tools/readonly.rs`
- `crates/app-server-core/src/agent/tools/readonly/*`
- `crates/app-server-core/src/agent/tools/cadquery.rs`
- `crates/app-server-core/src/llm/config.rs`
- `crates/app-server-core/src/cadquery/*`
- `crates/app-server-host/src/dispatcher.rs`
- `crates/app-server-host/src/cadquery_env.rs`
- 现有 workspace、file、chat、CadQuery、preview/export 测试

### 前序目标保护

- 保护 Phase 1 删除桌面端后的 Web-only 生产 GUI 边界。
- 保护 Phase 2 建立的 async service 是唯一后端请求处理入口。
- 保护 CadQuery staging 原子提交语义：失败、超时或取消不得污染真实 workspace。
- 保护 app server 是唯一 I/O 与外部调用能力层。
- 保护 Chat history、Agent event 和 preview/export protocol 行为。

### 操作步骤

1. 将 workspace、file、config、presets、plan package 和 Agent 工具文件访问 API 改为 async，并让 host command handler 直接 await。
2. 将 ChatStore 的读取、追加、会话列举和 JSONL 文件操作改为 async。
3. 将 preview/export 相关文件读取、外部命令和缓存更新接入 async service。
4. 将 preview、export 与 CadQuery runner host 边界改为 async 子进程与 async 取消控制；同步移除 `child_terminator` 中基于 `std::process::Child` 的生产终止路径，保持 staging、contract validation、dry run、execute、mesh/topology 输出顺序不变。
5. 将 file watcher 到 host push event 的桥接改为 async channel 驱动，避免在请求处理路径中阻塞 runtime。
6. 删除非 Agent 后端任务中的 `std::thread::spawn`、`thread::sleep` 生产使用；Agent worker 的删除放入 Phase 4，与 Rig-only 路径一起完成。
7. 对仍然必须同步执行的纯 CPU 计算保持普通函数；它们不得执行文件系统、网络、子进程或跨线程等待。
8. 更新测试，使取消、超时、失败和成功写回都覆盖 async 路径。

### 验收标准

- 后端生产请求路径不再使用同步文件系统、同步子进程或线程式等待执行 I/O。
- 除旧 Agent worker 将在 Phase 4 删除外，app-server-core 与 app-server-host 的生产 I/O 路径不再依赖 `std::fs`、`std::process::Command`、`std::thread::spawn` 或 `thread::sleep`。
- CadQuery 成功写回、失败回滚、取消回滚和超时回滚测试通过。
- Chat history 读写测试通过，并验证 tool result 与 assistant event 顺序不变。
- Preview/export roundtrip 测试通过。
- `cargo test -p app-server-core` 通过。
- `cargo test -p app-server-host` 通过。

## Phase 4 — Rig 成为唯一生产 Agent 执行引擎

### 输入

- `crates/app-server-core/Cargo.toml`
- `crates/app-server-core/src/lib.rs`
- `crates/app-server-core/src/agent.rs`
- `crates/app-server-core/src/llm/mod.rs`
- `crates/app-server-core/src/llm/openai_compat.rs`
- `crates/app-server-core/src/llm/config.rs`
- `crates/app-server-core/src/agent/tools.rs`
- `crates/app-server-core/src/agent/tools/*`
- `crates/app-server-host/src/dispatcher.rs`
- `crates/app-server-core/tests/agent_*`
- `crates/app-server-core/tests/llm_tests.rs`
- `crates/app-server-host/tests/shared_dispatcher_roundtrip_tests.rs`
- `crates/app-server-host/tests/dispatcher_pure_fn_tests.rs`
- `crates/app-server-host/tests/plan_extraction_tests.rs`
- `packages/studio-web/src/workbench/chat-messages.tsx`

### 前序目标保护

- 保护 Phase 1 删除桌面端后的 Web-only 生产 GUI 边界。
- 保护 Phase 2/3 的 async service 与 async I/O 边界。
- 保护工具注册、路径权限、CadQuery staging、Chat history、Agent run 管理和 protocol event。
- 保护 system prompt 中的 budn' 产品契约，不把测试 fixture、验收 prompt 或具体 demo 对象写入通用 Agent 代码。

### 操作步骤

1. 按 docs.rs 与 Context7 核对当前 `rig-core` 版本的 Agent、tool、streaming、Responses API provider 和 hosted tool 能力。
2. 将 `rig-core` 加入 Rust workspace 依赖，并让 app-server-core 使用统一版本。
3. 建立 Rig provider 配置读取路径，生产 CAD Agent 使用 OpenAI Responses API provider。
4. 删除或改写旧 OpenAI-compatible 配置语义和用户可见提示，尤其是 `base_url` / `BUDN_LLM_BASE_URL` / Chat Completions 文案；如果保留通用 API key、model 或 config env，必须明确指向 Rig + OpenAI Responses API provider。
5. 将现有工具 registry 与 path policy 包装为 Rig tool adapter；adapter 只负责把 Rig tool call 转入现有工具执行器，不复制权限逻辑。
6. 将 Rig multi-turn streaming event 映射为当前 protocol 事件：token、reasoning、tool start、tool result、done、error。
7. 将 Agent 取消语义接入 Rig 请求和工具执行，取消后不得继续写 workspace 或追加成功状态。
8. 将 Agent worker 改为 async task，删除旧 `std::thread::spawn(move || run_agent_worker(worker))` 与 `thread::sleep` 生产路径。
9. 删除生产路径中的 `LlmProvider` trait、`openai_compat`、`ureq` SSE parser、`run_tool_loop_with_registry_and_reasoning` 和本地假 Agent 回答。
10. 更新 `crates/app-server-core/src/lib.rs`，移除旧 Agent / LLM 类型、旧生成函数和旧 tool loop 的生产 re-export。
11. 保留测试专用 mock backend；mock 只能存在于测试模块、测试 fixture 或明确的 test support 路径。
12. 更新 Agent / LLM 测试，覆盖 Rig tool call、multi-turn、stream token、reasoning、tool error、取消和 Chat history 写入。

### 验收标准

- 生产 Agent 入口只通过 Rig 执行。
- `ureq` 不再是 app-server-core 的 LLM 依赖。
- 生产代码中不存在旧 OpenAI-compatible SSE parser。
- 生产代码和用户可见提示不再要求配置 Chat Completions `base_url`。
- 生产代码中不存在自研多轮 tool loop 入口。
- `crates/app-server-core/src/lib.rs` 不再导出旧 Agent / LLM 生产 API。
- Agent worker 不再使用 `std::thread::spawn` 或 `thread::sleep` 运行生产请求。
- 工具权限、路径策略和 CadQuery staging 测试仍通过。
- Rig mock 测试覆盖 token、reasoning、tool call、tool result、error 和 cancel。
- `cargo test -p app-server-core agent` 通过。
- `cargo test -p app-server-host agent` 通过。
- `bun run --cwd packages/studio-web typecheck` 通过。
- 涉及 Chat 消息呈现的 Web 单元测试通过；若当前没有对应测试，必须补充最小覆盖后运行 `bun run --cwd packages/studio-web test:unit`。

## Phase 5 — 接入模型原生联网搜索

### 输入

- Rig OpenAI Responses API provider 文档
- OpenAI Responses API 与 web search 官方文档
- `crates/app-server-core/src/agent.rs`
- `crates/app-server-core/src/agent/tools/*`
- `crates/app-server-protocol/src/protocol.rs`
- `docs/cadquery-mvp/agent-system-prompt.md`
- Agent 配置读取路径

### 前序目标保护

- 保护 Phase 1 删除桌面端后的 Web-only 生产 GUI 边界。
- 保护 Phase 4 的 Rig-only Agent 路径。
- 保护 workspace 状态只能通过 workspace 工具读取，模型联网搜索不能替代本地项目状态判断。
- 保护路径权限和 CadQuery staging；hosted web search 不获得文件系统能力。

### 操作步骤

1. 在 Agent provider 配置中增加模型原生联网搜索能力开关，默认关闭，生产启用时必须显式配置。
2. 通过 Rig OpenAI Responses API provider 注册 provider-native `web_search` hosted tool。
3. 禁止新增自建互联网搜索工具；现有 `search_files` 继续只表示 workspace 文件搜索。
4. 在 system prompt 中明确：联网搜索只用于外部事实、标准、材料、API 或背景资料；workspace 文件、selection、Ref、CadQuery 输出必须通过 app server 工具读取。
5. 将模型返回的搜索来源、引用或 annotations 映射到 protocol 中的可持久化字段；如果 provider 未返回结构化来源，则只保存最终回答文本与 provider capability 记录。
6. 在 Chat history 中记录本轮是否启用模型原生联网搜索，不记录 provider 密钥或敏感配置。
7. 处理 provider 不支持、认证失败、限流、搜索工具不可用和请求取消错误，并映射为现有 Agent error event。
8. 补充测试，覆盖搜索关闭、搜索开启、provider 不支持、搜索错误和带来源回答。

### 验收标准

- 模型原生联网搜索只通过 provider hosted tool 接入。
- 关闭搜索时，Rig request 不包含 hosted `web_search`。
- 开启搜索时，Rig request 包含 hosted `web_search`，且 workspace 工具权限不变化。
- Chat history 能区分本轮是否启用模型原生搜索。
- provider 错误被映射到 Agent error event。
- `search_files` 的语义仍是本地 workspace 文件搜索。
- `cargo test -p app-server-core agent` 通过。

## Phase 6 — Protocol、Web 端侧与配置接入

### 输入

- `crates/app-server-protocol/src/protocol.rs`
- `crates/app-server-protocol-wasm/*`
- `crates/studio-common/*`
- `crates/studio-web-wasm/*`
- `packages/studio-web/src/*`
- `package.json`
- `packages/studio-web/package.json`

### 前序目标保护

- 保护 Phase 1 删除桌面端后的 Web-only 生产 GUI 边界。
- 保护 Phase 2/3/4/5 的 async service、Rig-only Agent 和 provider-native web search。
- 保护 Web 走同一份 app server protocol，不引入端侧直连 I/O 或 provider 调用。

### 操作步骤

1. 在 protocol 中表达 Agent provider capability、模型原生搜索启用状态和搜索来源字段。
2. 更新 Borsh / serde roundtrip 测试，覆盖新增 capability 与搜索来源字段。
3. 更新 `studio-common` 的 Agent 状态模型，使 Web wasm 与 Web UI 使用同一套 capability 与搜索来源解释；删除桌面专属状态。
4. 更新 Web wasm bridge 和 protocol package 生成结果。
5. 更新 Studio Web Chat 呈现：当回答包含搜索来源时显示来源；当 provider 不支持搜索时显示后端返回的错误或 capability 状态。
6. 删除 Web 端对旧 LLM provider、旧确认流、旧 Agent operation 或桌面 platform 的依赖引用。

### 验收标准

- protocol roundtrip 测试覆盖 Agent capability 与搜索来源。
- Web 只通过 app server protocol 获得 Agent 状态。
- Web UI 不直接调用联网搜索 API。
- 端侧没有旧 Agent operation / confirmation 生产入口。
- `cargo test -p app-server-protocol` 通过。
- `cargo test -p app-server-protocol-wasm` 通过。
- `cargo test -p studio-common` 通过。
- `cargo test -p studio-web-wasm` 通过。
- `bun run protocol:build` 通过。
- `bun run protocol:check-generated` 通过。
- `bun run --cwd packages/studio-web typecheck` 通过。
- `bun run --cwd packages/studio-web test:unit` 通过。

## Phase 7 — 文档、已知问题、最终验证与独立 review

### 输入

- `docs/cadquery-mvp/decisions.md`
- `docs/cadquery-mvp/agent-tool-contract.md`
- `docs/cadquery-mvp/agent-system-prompt.md`
- `docs/architecture.md`
- `docs/getting-started.md`
- `docs/web-platform-limits.md`
- `docs/2026042500-cross-platform-path-policy/README.md`
- `docs/known_issues.md`
- 本计划所有 Phase 的 diff 与测试结果

### 前序目标保护

- 保护前六个 Phase 已完成的桌面删除、async service、async I/O、Rig-only Agent、provider-native web search 和 Web-only protocol 接入目标。
- 保护不保留生产双路径的约束。
- 保护文档与实际代码一致。

### 操作步骤

1. 更新 CadQuery Agent 相关文档，说明 Rig 是生产 Agent 引擎，模型原生联网搜索通过 provider hosted tool 接入。
2. 更新架构文档和 getting started，说明 Web 是唯一生产 GUI 端，Rust 桌面 app 已删除。
3. 更新 `docs/known_issues.md`，关闭“Agent 后端尚未接入真实 LLM provider 配置”或改写为新的具体问题；关闭或改写桌面 parity / 桌面自动化相关旧记录，不得保留与已实现状态冲突的记录。
4. 全仓库搜索已删除桌面路径关键词并删除生产引用：
   - `studio-app`
   - `studio_app`
   - `scad-ui`
   - `scad_ui`
   - `scad-viewer`
   - `scad_viewer`
   - `ClientPlatform::Desktop`
   - `Desktop =`
   - `pub enum ClientPlatform`
   - `"desktop" | "web" | "other"`
   - `platform: "desktop"`
   - `spawn_in_process_mpsc_host`
   - `InProcessHost`
   - `MpscTransportAdapter`
   - `mpsc_transport`
   - `AbortDecision`
   - `JoinThenAbort`
   - `evaluate_shutdown`
   - `GUI_SHUTDOWN_TIMEOUT`
   - `cargo run -p studio-app`
5. 全仓库搜索旧 Agent / LLM 路径关键词并删除生产引用：
   - `ureq`
   - `openai_compat`
   - `LlmProvider`
   - `AgentBackend`
   - `LocalAgentBackend`
   - `llm_generate_cadquery_code`
   - `create_provider`
   - `run_tool_loop`
   - `Chat Completions`
   - `chat/completions`
   - `stream_chat`
   - `stream_chat_with_reasoning`
   - `BUDN_LLM_BASE_URL`
   - `BUDN_LLM_CONFIG`
   - `base_url`
   - `OpenAiCompatible`
   - `OpenAI-compatible`
   - `OpenAI compatible`
   - `anthropic-sdk-rust`
   - `async-openai`
   - `自建薄 provider trait`
   - `no vendor lock-in`
   - `退回 SDK 客户端`
6. 在 `crates/app-server-core/src` 与 `crates/app-server-host/src` 的生产源码范围内搜索阻塞式后端路径并删除或改为 async；测试代码、非 app server crate 不属于该清理范围，但不得包含后端 I/O、provider 调用或同步 dispatcher。以下是阻塞式候选关键词，必须结合 import 与类型来源判断；`tokio::fs`、`tokio::process::Command`、`tokio::sync::mpsc`、`tokio::task::JoinHandle` 属于允许的 async 目标，不得因同名词误删：
   - `std::fs::`
   - `use std::fs`
   - `use std::{fs`
   - `fs::`
   - `fs::{`
   - `fs::read`
   - `fs::write`
   - `fs::read_dir`
   - `fs::read_to_string`
   - `fs::rename`
   - `fs::canonicalize`
   - `fs::symlink_metadata`
   - `fs::metadata`
   - `fs::create_dir`
   - `fs::create_dir_all`
   - `OpenOptions`
   - `File::open`
   - `File::create`
   - `std::process::Command`
   - `std::process::Child`
   - `use std::process::Command`
   - `Command::new`
   - `child.kill`
   - `terminate_child`
   - `std::thread::spawn`
   - `std::thread::Builder`
   - `std::thread::JoinHandle`
   - `use std::thread`
   - `use std::thread::{`
   - `thread::{self, JoinHandle}`
   - `thread::spawn`
   - `thread::Builder`
   - `thread::sleep`
   - `std::sync::mpsc`
   - `use std::sync::mpsc`
   - `use std::sync::{mpsc`
   - `std::sync::mpsc::channel`
   - `sync::mpsc::{self`
   - `recv_timeout`
   - `RecvTimeoutError`
7. 运行完整验证：
   - `cargo test --workspace`
   - `bun run protocol:build`
   - `bun run protocol:check-generated`
   - `bun run --cwd packages/studio-web typecheck`
   - `bun run --cwd packages/studio-web test:unit`
   - `bun run web:build`
   - `bun run web:smoke`
   - `bun run web:smoke:browser`
8. 启动独立 subagent 做 Plan 级 review，覆盖所有 Phase 是否满足本计划验收标准、是否存在旧生产路径残留、是否有行为冲突、测试是否覆盖完整交付标准。
9. 根据 Plan 级 review 结果修复问题并重新运行相关验证；只有 review 无阻塞项后，才记录计划完成。

### 验收标准

- 文档与代码一致，不再描述 Rust 桌面 app 作为当前产品端。
- 文档与代码一致，不再描述旧生产 Agent 路径。
- 已知问题记录准确反映改造后的实际状态。
- 全仓库不存在 Rust 桌面 app 生产入口。
- 全仓库不存在旧 LLM / Agent 生产入口。
- app-server-core 与 app-server-host 的生产源码不存在同步后端 I/O、同步子进程、同步 dispatcher 或线程式等待入口。
- Web 端不承担后端 I/O、provider 调用或同步 dispatcher 职责。
- 完整验证命令全部通过。
- Plan 级独立 review 无阻塞项。
- `plan-00-result.md` 完整记录每个 Phase 的完成情况、变更摘要、验证结果和遗留风险。
