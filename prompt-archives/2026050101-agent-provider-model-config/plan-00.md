# Agent Provider / Model 配置与 Web 切换实施计划

> 执行要求：执行前通读 `plan-prompt.md`、本计划和根 `AGENTS.md`。每个 Phase 按“实现 -> 独立 subagent review -> 修复 -> 验证 -> 更新 `plan-00-result.md` -> 阶段提交”推进。执行过程中不得新增用户选择题；计划与现有源码不一致时，以当前源码和现有行为为准修正实现。

## 背景

当前 budn' Agent 后端已基于 Rig 0.35.0 和 OpenAI Responses API 单 provider 路径运行，并通过 provider hosted `web_search` 支持模型原生搜索。当前配置和 Web capability 仍是单 provider / 单模型形态。本计划把配置、provider registry、protocol、Web UI 和 `bun run web` 启动链路统一迁移到 `agents.toml`。

## 锁定决策

- 配置文件为 `agents.toml`，必须被 ignore；仓库只提交不含密钥的 `agents.example.toml`。
- `BUDN_AGENT_CONFIG=agents.toml` 是当前推荐入口；本机 `.env` 和 `agents.toml` 不进入仓库 diff。
- OpenAI 第一版使用 Responses API provider；Anthropic 第一版使用 Messages API provider。
- `anthropic_version` 可选；未配置时使用 `2023-06-01`。
- provider 支持多模型；模型发现默认开启，发现模型和手动模型同时合并，手动同 id 只覆盖显式字段。
- 配置文件、protocol 和 Web UI 都支持 `reasoning_effort` 与 `service_label`；Anthropic `reasoning_effort` 映射到 thinking / budget。
- Web UI 必须真实提供 provider/model 列表、模型切换、reasoning effort 和 service label 控件。
- wire protocol、WASM bridge、TypeScript package 和前端接口必须支持读取 provider/model registry。
- `agent.invoke` / 发消息 API 必须携带 provider、model、reasoning effort 和 service label，本次 run 使用请求携带的参数快照。
- native web search 默认开启；`web_search_supported` 是布尔值。provider 实际调用失败必须按 Agent error 暴露，不用未知状态掩盖问题。
- Web 端不得读取密钥、配置文件或直接调用 provider；provider/model 列表和切换必须走 app server protocol。
- 结构化搜索来源若当前 Rig provider 未暴露，则保留 protocol 的可选 sources 字段和 capability，不伪造来源。

## 保护边界

- 保持 Rig-only Agent 生产路径，不恢复旧 OpenAI-compatible Chat Completions、自研 provider trait 或旧 HTTP/SSE parser。
- 保护 OpenAI Responses 现有能力：token streaming、reasoning streaming、tool call/result、timeout、cancel、错误映射和 native web search。
- 保护 Chat history、Agent event、Agent run、tool call/result 顺序和 `run_id` 持久化。
- 保护 CadQuery staging 原子性、path policy、workspace tools、readonly / file_write / semantic / CadQuery tool registry。
- 保护 WebSocket host、async dispatcher、studio-common managed client、studio-web-wasm bridge、TypeScript protocol package 和 Web snapshot。
- 保护 Web Chat、workspace tree、files panel、SCAD preview、CadQuery viewer、mesh viewer、selection / Ref layer、watch refresh 和 `bun run web`。
- 当前 workspace 已不包含 `crates/studio-app`；不得为了桌面兼容重新引入已删除生产目标。
- `.env`、`agents.toml`、`llm.toml`、API key 和本机模型偏好不得进入仓库 diff。

## 主要入口

- Core / host / protocol：`crates/app-server-core/src/llm/config.rs`、`crates/app-server-core/src/agent.rs`、`crates/app-server-core/src/llm/model_discovery.rs`、`crates/app-server-host/src/dispatcher.rs`、`crates/app-server-protocol/src/protocol.rs`
- Shared / WASM / TypeScript：`crates/studio-common/src/managed_client/*`、`crates/studio-web-wasm/src/wasm_bridge/*`、`packages/app-server-protocol/src/index.ts`
- Web：`packages/studio-web/src/state/protocol-store.ts`、`packages/studio-web/src/workbench/chat-zone.tsx`
- 启动与配置：`scripts/run_studio_web_dev.ts`、`scripts/run_websocket_host.ts`、`.gitignore`、`.env`、`llm.toml`
- 外部资料：OpenAI Responses / web search 官方文档、Anthropic Messages / web search / thinking 官方文档、Rig 0.35.0 OpenAI Responses 与 Anthropic provider 源码

## Phase 1 — 配置格式与 ignore 基线

### 输入

- `.gitignore`
- `.env`
- `llm.toml`
- `crates/app-server-core/src/llm/config.rs`
- `crates/app-server-core/tests/llm_tests.rs`
- `docs/getting-started.md`
- `docs/cadquery-mvp/python-runner.md`
- `docs/known_issues.md`

### 前序目标保护

- 保护现有 Rig-only Agent 生产路径，不恢复旧 `BUDN_LLM_CONFIG` / OpenAI-compatible Chat Completions 语义。
- 保护本机密钥不进入 Git diff。
- 保护 `bun run web` 仍由 Bun 自动读取仓库根 `.env`。

### 操作步骤

1. 在 `.gitignore` 增加 `agents.toml`，保持 `.env`、`.env.local`、`llm.toml` 继续忽略。
2. 新增 `agents.example.toml`，只包含占位 env 变量名、provider/model 示例和安全默认值，不包含真实 API key。
3. 将配置读取设计为优先读取 `BUDN_AGENT_CONFIG` 指向的 TOML 文件；没有该 env 时保留现有 env fallback，便于 CI 与最小本地调试。
4. 定义配置语义：
   - 顶层 `active_provider`、`active_model`。
   - 顶层 `[defaults]`：`timeout_secs`、`max_tokens`、`temperature`、`native_web_search`、`discover_models`，其中 `native_web_search` 和 `discover_models` 默认值为 `true`。
   - `[[providers]]`：`id`、`kind`、`api_key_env`，可选 `anthropic_version`、`discover_models`。
   - `[[providers.models]]`：`id`、`label`、可选 `max_tokens`、`temperature`、`reasoning_effort`、`service_label`、`native_web_search`、`web_search_supported`、`web_search_unsupported_reason`。
   - `native_web_search` 表达“用户希望开启搜索”，默认 `true`；`web_search_supported` 表达该 provider/model 是否声明支持 provider-native 搜索，默认按 provider kind 推导为 `true`，允许模型级显式覆盖为 `false`。
   - `discover_models` 表达是否读取 provider 模型列表，默认 `true`；关闭后该 provider 只使用手动配置模型。
   - 手动配置模型与发现模型按同一 provider 下的 model id 合并：发现模型提供基础 `id` / `label`，手动配置同 id 只覆盖显式配置字段；手动配置未被发现的 id 作为额外模型保留。
   - `reasoning_effort` 表示模型 reasoning 参数，例如 `minimal`、`low`、`medium`、`high`、`xhigh`；`service_label` 表示用户可见服务档位或路由标签，例如 `fast`、`default`、`flex`、`high`。
5. 配置校验必须覆盖：
   - provider id 唯一。
   - 同一 provider 下 model id 唯一。
   - active provider/model 必须存在。
   - `anthropic_version` 只允许 Anthropic provider 使用，且空字符串报错。
   - provider API key 从 env 名读取，配置文件不得要求直接写 key。
   - `discover_models` 只能是布尔值，默认开启。
   - 同 id 手动模型 override 只覆盖显式字段，不得清空发现模型已有 label 或 capability。
   - `web_search_supported = false` 时必须提供或生成可展示的不支持原因。
6. 将当前本机 `.env` 迁移为 `BUDN_AGENT_CONFIG=agents.toml` 和现有 `CADQUERY_RUNNER_PYTHON=/opt/homebrew/bin/python3.11`。
7. 根据当前本机旧 `llm.toml` 的模型偏好创建本机 `agents.toml`，但不提交该文件；若旧配置是 OpenAI-compatible base URL，不把该旧 base URL 迁移为生产 provider。
8. 更新文档，说明 `agents.toml` 是本机私有文件，仓库只提交 `agents.example.toml`。
9. 若发现旧 `BUDN_LLM_CONFIG` 文档或提示仍作为当前配置方式存在，改为历史说明或删除。

### 验收标准

- `.gitignore` 包含 `agents.toml`。
- 仓库包含 `agents.example.toml`，且不包含真实密钥。
- 当前本机 `.env` 使用 `BUDN_AGENT_CONFIG=agents.toml`。
- 当前本机 `agents.toml` 存在且不在 Git diff 中。
- `llm.toml` 不再作为当前推荐配置格式。
- 配置测试覆盖默认值、active model 校验、Anthropic 可选版本、重复 id、缺失 API key env。
- 配置测试覆盖 `native_web_search = true` 且 `web_search_supported = false` 时解析为“请求搜索但不可注入 hosted/server tool”的 capability。
- 配置测试覆盖未显式设置 `web_search_supported` 时按 provider kind 使用默认布尔能力。
- 配置测试覆盖 `discover_models` 默认开启、provider 级关闭，以及发现模型与手动 override 同时生效。
- 配置测试覆盖同 id 手动模型只覆盖 `label`、`reasoning_effort`、`service_label` 等显式字段，不会删除发现模型。
- `cargo test -p app-server-core rig_agent_config` 通过。
- `bun -e 'console.log(process.env.BUDN_AGENT_CONFIG)'` 输出 `agents.toml`。
- `git status --short` 不显示 `.env`、`agents.toml` 或 `llm.toml`。

## Phase 2 — Provider registry 与 Agent 执行分发

### 输入

- `crates/app-server-core/src/llm/config.rs`
- `crates/app-server-core/src/llm/model_discovery.rs`
- `crates/app-server-core/src/llm/mod.rs`
- `crates/app-server-core/src/agent.rs`
- `crates/app-server-core/src/lib.rs`
- `crates/app-server-core/tests/llm_tests.rs`
- `crates/app-server-core/tests/*agent*`
- `docs/known_issues.md`

### 前序目标保护

- 保护 Phase 1 的新配置格式和私有配置边界。
- 保护 Rig-only Agent 执行路径，不引入新的自研 provider trait 或旧 HTTP/SSE parser。
- 保护 workspace 工具、路径权限、CadQuery staging、Chat history、Agent run 管理、取消语义和 protocol event。

### 操作步骤

1. 将单个 `RigAgentConfig` 扩展为解析后的 active model 配置与 provider/model registry。
2. 增加 provider kind：
   - `openai_responses`
   - `anthropic_messages`
3. 增加 provider model discovery：
   - OpenAI provider 调用 Models API list endpoint，生成发现模型列表。
   - Anthropic provider 调用 Models API list endpoint，生成发现模型列表。
   - discovery 超时、认证失败、限流或 provider 错误必须记录为 provider capability 状态；不得阻止手动配置模型继续进入 registry，也不得阻止 Web 工作台启动。
4. 合并发现模型与手动模型配置，生成最终 model registry；手动同 id 配置作为字段级 override，手动未发现 id 作为额外模型。
5. Agent run 根据 active provider/model 构造对应 Rig client 与 Agent builder。
6. OpenAI 分支保留 Responses API provider-native hosted tool 语义。
7. Anthropic 分支使用 Rig Anthropic provider，未配置 `anthropic_version` 时使用默认 `2023-06-01`。
8. 对两个 provider 统一复用当前工具 registry、tool observer、stream drain、timeout、cancel tick 和 Chat history 写入路径。
9. 将 active model 的 `reasoning_effort` 和 `service_label` 映射到 provider 请求参数：
   - OpenAI `reasoning_effort` 进入 Responses `reasoning.effort`。
   - OpenAI `service_label` 按当前 provider API 支持映射到 service tier 或等价 additional params；若 provider 当前不支持该标签，必须作为 capability 状态显示而不是伪造请求参数。
   - Anthropic `reasoning_effort` 映射到 Anthropic thinking / budget 相关参数；具体预算映射必须集中在 provider 适配层，并通过测试覆盖。
   - Anthropic `service_label` 若无对应 provider 参数，必须显示为未应用状态；不得因此影响 reasoning effort 的应用。
10. 原生 Web 搜索默认开启：
   - OpenAI 分支通过 hosted `web_search` 注入。
   - Anthropic 分支通过 `additional_params.tools` 注入 `web_search_20250305` server tool。
11. 增加 provider/model 级 web search capability 判定：`native_web_search` 为 true 且 `web_search_supported = true` 时注入 hosted/server web search tool；`native_web_search` 为 true 且 `web_search_supported = false` 时不得注入工具，并在 capability 中暴露不支持原因。
12. provider 不支持或 hosted tool 报错时，映射为现有 Agent error event，不允许直接 panic、吞掉错误或改写成能力未知状态。Web 工作台仍必须可启动，用户可切换到禁用 web search 或支持 web search 的模型。
13. 保留现有 env fallback 的最小兼容，但用户可见文档以 `BUDN_AGENT_CONFIG=agents.toml` 为准。

### 验收标准

- OpenAI 与 Anthropic provider 都能构造 Rig Agent builder。
- OpenAI 与 Anthropic provider 都能执行模型列表发现，并与手动配置模型合并。
- 模型发现失败不会移除手动配置模型，也不会阻止 Web 工作台启动；错误通过 provider capability 暴露。
- 手动同 id 模型 override 只覆盖显式字段；手动额外模型在发现列表之外仍可选择。
- Anthropic `anthropic_version` 未配置时使用默认值；配置为空字符串时报配置错误。
- active model 的 `reasoning_effort` 和 `service_label` 能进入解析后的 run config，并按 provider 能力应用或暴露未应用状态。
- Anthropic active model 的 `reasoning_effort` 能应用到 Anthropic thinking / budget 参数；测试必须覆盖至少一个非默认 effort。
- native web search 默认开启，并可被 provider/model 覆盖为关闭。
- `native_web_search` 与 `web_search_supported` 是两个独立语义：前者表达配置意图，后者表达实际布尔能力。
- 开启 native web search 时，OpenAI request 附带 hosted `web_search`；Anthropic request 附带 Anthropic server tool JSON。
- 关闭 native web search 时，不向 provider request 注入 hosted/server web search tool。
- provider/model 标记不支持 web search 时，默认开启不会导致该模型默认 Agent run 失败；capability 与 Web UI 必须显示降级状态。
- provider/model 实际调用 web search 失败时必须暴露 Agent error，不允许用 capability 状态变更掩盖失败。
- hosted/server web search 工具不可用时，只影响本次 Agent run 的错误事件，不影响 Web 工作台、workspace 浏览、预览或后续切换模型。
- Agent tool registry、path policy 与 CadQuery staging 测试保持通过。
- `cargo test -p app-server-core rig_agent_config` 通过。
- `cargo test -p app-server-core agent_model_discovery` 通过或对应新测试名通过。
- `cargo test -p app-server-core rig_agent_additional_params` 通过或对应新测试名通过。
- `cargo test -p app-server-core agent` 通过。

## Phase 3 — Protocol 与 Studio common capability 扩展

### 输入

- `crates/app-server-protocol/src/protocol.rs`
- `crates/app-server-protocol/tests/borsh_payload_roundtrip_tests.rs`
- `crates/app-server-protocol/tests/wire_payload_contract_tests.rs`
- `crates/app-server-protocol-wasm/*`
- `crates/studio-common/src/managed_client/*`
- `crates/studio-common/tests/managed_client_tests.rs`
- `crates/studio-web-wasm/src/wasm_bridge/*`
- `crates/studio-web-wasm/tests/wasm_bridge_smoke.rs`
- `packages/app-server-protocol/src/index.ts`

### 前序目标保护

- 保护 Phase 1/2 的配置读取与 provider 分发。
- 保护 Web 只能通过 app server protocol 获得模型列表和切换模型。
- 保护现有 `agent_provider` capability 的语义，避免旧 Web 客户端完全失去配置状态提示。

### 操作步骤

1. 在 protocol 中增加 provider/model registry 的 DTO 和读取接口，表达：
   - provider id、kind、label。
   - model id、label。
   - model 来源：`discovered`、`manual` 或 `discovered_with_override`。
   - active provider/model。
   - reasoning effort 当前值、可选值和是否已应用到当前 provider request。
   - service label 当前值、可选值和是否已应用到当前 provider request。
   - provider 模型发现是否开启、发现状态、发现错误摘要。
   - native web search 是否默认开启、当前模型是否开启。
   - 当前 provider/model 是否支持 provider-native web search，以及不支持原因。
   - search sources 是否支持。
2. 增加 provider/model 列表读取 command 或 snapshot 字段，保证 Web 可以通过 wire protocol 获取完整 provider/model registry。
3. 增加模型切换 command，payload 使用 provider id 与 model id。
4. 增加模型参数切换 command，payload 使用 provider id、model id、可选 `reasoning_effort` 和可选 `service_label`；该 command 只更新运行时状态，不写入 `agents.toml`。
5. 扩展 `agent.invoke` / 发消息 request，必须携带本次使用的 provider id、model id、reasoning effort 和 service label；后端必须以请求携带参数作为本次 run 的模型参数快照。
6. host handshake snapshot 和 managed client snapshot 暴露 provider/model registry。
7. 切换模型或模型参数成功后通过 snapshot 或 response 反映新的 active model 与 active model params。
8. 保留现有 `llm_configured` 语义：至少一个 active provider/model 可用且对应 API key env 存在时为 true。
9. 保留现有 snapshot 字段兼容性：旧 `agent_provider` capability 不得无故消失，新增 registry 字段必须通过默认值或版本升级保护现有 wire contract。
10. 更新 Borsh / serde roundtrip、wire payload contract、studio-common inbound 和 wasm bridge 测试。
11. 重新生成 TypeScript protocol package。

### 验收标准

- protocol roundtrip 测试覆盖 provider/model registry、active model 和切换 command。
- protocol roundtrip 测试覆盖 provider/model 列表读取接口。
- protocol roundtrip 测试覆盖模型参数切换 command，包括只改 `reasoning_effort`、只改 `service_label` 和两者同时修改。
- protocol roundtrip 测试覆盖 `agent.invoke` / 发消息 request 携带 provider id、model id、reasoning effort 和 service label。
- protocol roundtrip 测试覆盖 discovered/manual/override 模型来源、reasoning effort、service label 和 model discovery 状态。
- wire payload contract 测试覆盖新增字段或明确 protocol version 升级，现有 snapshot 解码不被破坏。
- `studio-common` snapshot 能保存并更新 active model。
- Web WASM bridge 能把 provider/model registry 暴露给 TypeScript。
- `bun run protocol:build` 通过。
- `bun run protocol:check-generated` 通过。
- `cargo test -p app-server-protocol` 通过。
- `cargo test -p app-server-protocol-wasm` 通过。
- `cargo test -p studio-common` 通过。
- `cargo test -p studio-web-wasm` 通过。

## Phase 4 — Web 模型选择 UI 与状态管理

### 输入

- `packages/studio-web/src/state/protocol-store.ts`
- `packages/studio-web/src/workbench/chat-zone.tsx`
- `packages/studio-web/src/workbench/chat-actions.ts`
- `packages/studio-web/src/workbench/chat-runtime.tsx`
- `packages/studio-web/src/workbench/model-settings.tsx`
- `packages/studio-web/tests/unit/protocol-store.test.ts`
- `packages/studio-web/tests/unit/chat-zone.test.tsx`
- `packages/studio-web/tests/unit/chat-runtime.test.ts`
- `packages/studio-web/src/styles/workbench.css`
- `packages/studio-web/src/styles/workbench-zones.css`

### 前序目标保护

- 保护 Phase 3 的 protocol-only 模型切换边界。
- 保护 Web 不读取 `.env`、`agents.toml` 或任何 API key。
- 保护 Chat 现有 session 选择、stop、新建会话、Plan run 和 context pill 行为。

### 操作步骤

1. 在 Zustand protocol store 中接入 provider/model registry、model discovery 状态、active model、reasoning effort 和 service label。
2. 在 Chat header 增加模型选择控件；显示 label，value 使用稳定的 provider/model id，并区分发现模型、手动模型和 override 模型。
3. 切换模型时调用 app server protocol command；请求期间禁用选择控件，避免并发切换造成 UI 状态错乱。
4. 增加模型参数控件，支持对当前模型选择可用的 reasoning effort 和 service label；控件只通过 protocol command 更新运行时状态，不直接写 `agents.toml`。
5. UI 显示 native web search 状态；开启时继续展示 `web search` 状态。
6. UI 显示 provider model discovery 状态；发现失败时显示错误摘要，但手动模型仍可选择。
7. 当没有可用 provider/model、缺少 API key 或 active model 不支持 web search 时，沿用现有 LLM setup guide / capability 状态区域，但文案改为 `agents.toml` / `BUDN_AGENT_CONFIG`，并给出禁用 web search 或切换模型的可执行提示。
8. 发送 Chat 消息时，Web 必须把当前 provider id、model id、reasoning effort 和 service label 放入 `agent.invoke` / 发消息 request；不得只依赖后端全局 active state。
9. Web 测试覆盖：
   - 渲染多个 provider 的多个模型。
   - 同时渲染发现模型、手动模型和 override 模型。
   - 切换模型发出正确 command。
   - 切换 reasoning effort 和 service label 发出正确 command，并更新 UI 状态。
   - 发送消息 request 携带当前 provider/model/参数。
   - 缺少 provider 时展示配置提示。
   - provider 模型发现失败时展示错误摘要，但手动模型仍可选择。
   - native web search 状态随 active model 更新。
   - active model 不支持 web search 时展示降级状态且不禁用整个工作台。
10. 控件样式遵循当前工作台紧凑 UI，不新增大面积 landing 或营销式区域。

### 验收标准

- Web Chat header 可显示并切换模型。
- Web 可显示模型来源、model discovery 状态、reasoning effort 和 service label。
- Web UI 必须真实提供 provider/model 列表、模型选择、reasoning effort 和 service label 控件。
- 切换模型只通过 app server protocol command 完成。
- 修改 reasoning effort 和 service label 只通过 app server protocol command 完成，不直接写 `agents.toml`。
- 发送消息 API 必须携带当前 provider/model/参数。
- Web 端无 provider API key、`.env` 或 `agents.toml` 读取逻辑。
- active model 不支持 web search 或 provider 缺少 API key 时，workspace tree、files panel、preview 和非 Agent UI 仍可用。
- `bun run --cwd packages/studio-web typecheck` 通过。
- `bun run --cwd packages/studio-web test:unit` 通过。

## Phase 5 — Host 切换命令持久状态与 `bun run web` 验证

### 输入

- `crates/app-server-host/src/dispatcher.rs`
- `crates/app-server-host/tests/*`
- `scripts/run_studio_web_dev.ts`
- `scripts/run_websocket_host.ts`
- `.env`
- `agents.toml`
- `tests/run_websocket_host.test.ts`
- `tests/studio-web-smoke-workspace/*`

### 前序目标保护

- 保护 Phase 1/2/3/4 的配置、provider 分发、protocol 和 Web UI。
- 保护 `bun run web` 继续启动 WASM build、websocket host 和 Vite。
- 保护本机配置文件不进入仓库。

### 操作步骤

1. host 处理模型切换 command，更新当前运行时 active provider/model。
2. host 处理模型参数切换 command，更新当前运行时 active model 的 reasoning effort 和 service label override。
3. 明确切换模型和模型参数的作用范围：第一版为当前 host 进程内生效，不写回 `agents.toml`，避免 app server 修改本机私有密钥配置文件。
4. 新 Agent run 使用 `agent.invoke` / 发消息 request 携带的 provider/model/参数；若 request 未携带参数，仅允许测试 fallback 使用当前 active model 与运行时参数。
5. handshake 和切换 response 都能暴露最新 active model、reasoning effort、service label 和 discovery 状态。
6. `bun run web` 启动时验证：
   - `.env` 能被 Bun 读取。
   - websocket host 能读取 `BUDN_AGENT_CONFIG`。
   - 缺少 API key env 时给出清晰配置提示，不影响非 Agent 工作台启动。
7. 增加或更新 smoke 测试，覆盖新配置 env 指针下的 host 启动。
8. 当前本机执行一次 `bun run web` 验证，启动成功后停止进程。

### 验收标准

- 模型切换 command 在 host 中生效。
- 模型参数切换 command 在 host 中生效。
- 新 Agent run 使用 request 携带的 active model、reasoning effort 和 service label。
- 已运行中的 Agent run 不被模型切换命令中断，Chat history 和 run event 仍归属原 run。
- 切换命令不写入 `agents.toml`。
- 缺少 provider API key 时 `llm_configured` 为 false 或返回清晰配置错误，Web 工作台仍能启动。
- `cargo test -p app-server-host agent` 通过。
- `bun run web -- --workspace /tmp/budn-agent-provider-web --web-port 5197 --ws-url ws://127.0.0.1:38433` 能完成启动；验证后停止进程。

## Phase 6 — 文档、已知问题与最终验证

### 输入

- `README.md`
- `docs/getting-started.md`
- `docs/cadquery-mvp/python-runner.md`
- `docs/cadquery-mvp/decisions.md`
- `docs/known_issues.md`
- `agents.example.toml`
- `.gitignore`
- 本计划所有 Phase 的 diff 与测试结果

### 前序目标保护

- 保护前五个 Phase 已完成的配置格式、provider 分发、protocol、Web UI 和 `bun run web` 目标。
- 保护不提交本机密钥和私有配置的边界。
- 保护 native web search 只通过 provider hosted/server tool 接入。

### 操作步骤

1. 更新 README 与 getting started，说明：
   - 复制 `agents.example.toml` 为本机 `agents.toml`。
   - `.env` 设置 `BUDN_AGENT_CONFIG=agents.toml`。
   - provider API key 放入环境变量，不写入 `agents.toml`。
   - `discover_models` 默认开启，发现模型和手动模型配置会同时合并；手动同 id 只覆盖显式字段。
   - `reasoning_effort` 和 `service_label` 可在配置文件中设定，也可在 Web 中作为运行时参数切换。
2. 更新 CadQuery Agent 文档，说明 OpenAI Responses 与 Anthropic Messages provider 的当前支持范围。
3. 更新 `docs/known_issues.md`：
   - 保留 Rig 未暴露 OpenAI web search structured sources 的限制。
   - 若 Anthropic citations 仍未映射到 protocol sources，新增或更新对应记录。
4. 全仓库搜索旧配置关键词，确认没有当前文档继续指导使用旧配置：
   - `BUDN_LLM_CONFIG`
   - `llm.toml`
   - `BUDN_LLM_BASE_URL`
   - `Chat Completions`
   - `OpenAI-compatible`
5. 全仓库搜索密钥形态，确认没有将真实 key 写入仓库新增文件。
6. 全仓库搜索新增 `python` / `python3` 调用，确认除既有 CadQuery runner 例外边界外没有扩散新的 Python 工具链调用。
7. 运行最终验证命令。
8. 启动 Plan 级独立 review；review subagent 必须接收完整 plan、所有 Phase 结果记录、最终 diff、验证命令与输出摘要。若发现阻塞项，修复后重新验证并再次 review。
9. 确认 `plan-00-result.md` 已实时记录每个 Phase 的执行结果、验证证据、阶段提交和遗留问题。

### 验收标准

- 文档只推荐 `BUDN_AGENT_CONFIG=agents.toml` 和 `agents.example.toml` 复制流程。
- 文档说明 provider 模型发现默认开启、发现结果与手动配置同时生效、同 id 手动模型是字段级 override。
- 文档说明 reasoning effort 与 service label 的配置文件字段和 Web 运行时切换语义。
- `agents.toml`、`.env`、`llm.toml` 不出现在 Git diff 中。
- 新增文档和示例不包含真实 API key。
- `docs/known_issues.md` 准确记录 provider source/citation 降级限制。
- `cargo test -p app-server-core` 通过。
- `cargo test -p app-server-host` 通过。
- `cargo test -p app-server-protocol` 通过。
- `cargo test -p app-server-protocol-wasm` 通过。
- `cargo test -p studio-common` 通过。
- `cargo test -p studio-web-wasm` 通过。
- `cargo check --workspace` 通过，确认当前 workspace 中所有 Rust crate 仍可构建。
- `cargo test --workspace` 通过，确认当前 workspace 中所有 Rust crate 的既有测试仍通过。
- `bun run protocol:build` 通过。
- `bun run protocol:check-generated` 通过。
- `bun run --cwd packages/studio-web typecheck` 通过。
- `bun run --cwd packages/studio-web test:unit` 通过。
- `bun run web:smoke` 通过。
- `bun run web:smoke:browser` 通过，覆盖 Playwright 生产工作台回归。
- 针对 Web Chat 与 CadQuery 交互必须运行以下聚焦用例；若文件名在执行前已变更，只能用覆盖同一最低路径的用例替代，并必须在结果文档中列出替代文件和覆盖证据。最低路径包括：Agent Chat 发送/取消/配置提示或模型状态、CadQuery viewer canvas 交互和 selection / Ref layer、配置面板读写与 app server protocol 往返。
  - `packages/studio-web/tests/playwright/agent-chat-interaction.spec.ts`
  - `packages/studio-web/tests/playwright/canvas-interaction.spec.ts`
  - `packages/studio-web/tests/playwright/config-settings.spec.ts`
- `git diff --check` 通过。

## 执行前待确定项检查

本计划不包含开放待定项、未选择方案或缺失验收标准。执行时不得新增用户选择题；如遇 provider SDK 行为与文档不一致，以当前源码和官方文档为准收敛，并在 `docs/known_issues.md` 记录无法在本轮直接解决但影响后续判断的问题。
