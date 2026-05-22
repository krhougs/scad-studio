# Agent Provider / Model 配置与 Web 切换 Prompt 存档

## 用户输入

用户要求研究并规划以下方向：

1. 同时兼容 OpenAI Responses API 和 Anthropic API。
2. `agents.toml` 支持配置多个 Provider，Provider 内支持配置多个模型，前端支持切换模型。
3. `bun run web` 启动链路同步跟进，并正确配置各种 ignore。
4. 原生 Web 搜索工具默认开启。
5. 按新的 env 和配置文件格式配置当前本地环境。

后续用户进一步明确：

- `anthropic_version` 应该是可选值，不应作为 Anthropic provider 必填项。
- `agents.toml` 应该被 ignore，仓库只提交示例配置。
- 用户要求先输出 plan。
- 用户要求带上原始需求和 `AGENTS.md` 作为约束，启动独立 reviewer 进行审查。
- 用户要求必须保证现有功能完全被保护。
- 用户补充：除了手动设置的模型，还需要从 provider 模型列表读取模型；该行为需要开关，默认开启。
- 用户补充：模型发现和手动配置同时生效，不是降级关系；手动配置同 id 模型视为 override。
- 用户补充：配置文件和前端都需要支持对部分模型配置 reasoning effort 与 service label，例如 GPT 系列的 `xhigh`、`fast` 等。
- 用户纠正：Anthropic 也支持 reasoning effort，不得把 Anthropic 第一版写成不支持 reasoning effort。
- 用户补充：前端不仅仅做 command 切换，Web UI 必须实现；对应 wire protocol 和接口必须支持读取 provider/model 列表；发消息 API 也必须携带 provider / 模型参数。
- 用户修正：模型是否支持 web search 做成单纯布尔值即可；实际调用有问题就报错，不要用复杂状态掩盖问题。

## 计划审查输入

已启动独立只读 reviewer 审查本计划。reviewer 输入包含原始需求、用户补充、根 `AGENTS.md` 约束摘要，并要求读取根 `AGENTS.md` 原文核对。审查重点包括：

- 是否完整覆盖原始需求与用户补充。
- 是否充分保护 Rig-only Agent、OpenAI Responses 现有能力、native web search、Chat history、CadQuery staging、workspace tools / path policy、WebSocket host、studio-common managed client、studio-web-wasm bridge、Web Chat UI、`bun run web`、现有 ignore 和本机密钥安全。
- 是否存在会破坏现有功能的风险，例如切换模型状态破坏正在运行的 Agent run、protocol 变更破坏现有 snapshot、Anthropic 分支引入旧 provider 抽象、native web search 默认开启导致 provider 不支持时整个 Web 工作台不可用。
- 是否符合 `AGENTS.md` 的 Plan Mode 要求，包括 Phase 前序目标保护、执行前待确定项检查、review 循环和结果归档。
- 是否缺少必要验收命令或测试覆盖。

## 当前代码背景

- 当前分支：`plan/2026042902-agent-plan-workspace-flow`。
- 当前生产 Agent 已基于 Rig 0.35.0 和 OpenAI Responses API 单 provider 路径运行。
- 当前配置入口位于 `crates/app-server-core/src/llm/config.rs`，只表达单个 `RigAgentConfig`：
  - `BUDN_AGENT_CONFIG`
  - `BUDN_AGENT_OPENAI_API_KEY` / `OPENAI_API_KEY`
  - `BUDN_AGENT_MODEL`
  - `BUDN_AGENT_TIMEOUT_SECS`
  - `BUDN_AGENT_MAX_TOKENS`
  - `BUDN_AGENT_TEMPERATURE`
  - `BUDN_AGENT_REASONING_EFFORT`
  - `BUDN_AGENT_NATIVE_WEB_SEARCH`
- 当前 Agent 执行入口在 `crates/app-server-core/src/agent.rs`，使用 `rig::providers::openai::Client::new(&config.api_key)` 创建 OpenAI Responses provider。
- 当前 protocol 只暴露单个 `AgentProviderCapabilities`：
  - `provider`
  - `model`
  - `native_web_search_enabled`
  - `search_sources_supported`
- 当前 Web Chat header 只展示当前 provider capability，不支持 provider / model 列表和模型切换。
- 当前 `.env` 是本机文件，已被 `.gitignore` 忽略；内容仍包含旧 `BUDN_LLM_CONFIG=llm.toml`。
- 当前 `llm.toml` 是本机旧配置文件，已被 `.gitignore` 忽略，且含明文密钥；本计划不得提交其内容。
- 当前 `.gitignore` 已忽略 `.env`、`.env.local`、`llm.toml`，但尚未忽略 `agents.toml`。
- `bun run web` 通过 `scripts/run_studio_web_dev.ts` 启动 WASM build、websocket host 和 Vite；Bun 会自动读取仓库根目录 `.env`。

## 已核对资料

- OpenAI Responses API 支持 built-in / hosted tools，包括 `web_search`。Responses API 的 `tools` 可包含 `{ "type": "web_search" }`。
- OpenAI web search 文档显示：
  - Responses API 推荐使用 `web_search`，而不是旧 preview search model。
  - 来源列表可通过 `include = ["web_search_call.action.sources"]` 请求。
  - domain filtering、user location、`search_context_size` 等为 provider-native hosted tool 配置。
- Rig 0.35.0 本地源码显示：
  - OpenAI provider 使用 Responses API 作为当前 completion provider。
  - OpenAI Responses `additional_params.tools` 会被解析为 hosted tool 并追加到 function tools 后面。
  - Rig 0.35.0 的 OpenAI Responses `Include` enum 暂未包含 `web_search_call.action.sources`，因此结构化来源仍需要后续适配。
- Anthropic API 文档显示：
  - Messages API 请求必须带 `anthropic-version` header，当前通用版本为 `2023-06-01`。
  - web search 通过 server tool 配置，例如 `{ "type": "web_search_20250305", "name": "web_search", "max_uses": 5 }`。
  - 新的 `web_search_20260209` 需要 code execution 参与动态过滤，第一版不作为默认路径。
  - Web search response 包含 search result 与 citation 结构，但 Rig 当前 Anthropic provider 会把额外 tools 作为 JSON 透传到 Anthropic body，未直接抽象为 budn' protocol source 字段。
  - Anthropic reasoning / extended thinking 通过 thinking 配置表达，适合映射 budn' 的 `reasoning_effort`。
- Rig 0.35.0 本地源码显示：
  - Anthropic provider 支持 `Client::new`、`ClientBuilder::anthropic_version(...)` 和 `ANTHROPIC_API_KEY`。
  - Anthropic streaming 路径会从 `additional_params.tools` 中取出额外 tool JSON，并与 Rig function tools 合并。
- OpenAI Models API 支持列出模型。
- Anthropic Models API 支持列出模型。

## 用户强制约束识别

- 新配置文件名为 `agents.toml`。
- `agents.toml` 支持多个 provider，每个 provider 支持多个模型。
- 前端必须支持模型切换。
- `agents.toml` 必须被 `.gitignore` 忽略；仓库只提交 `agents.example.toml`。
- `anthropic_version` 是可选配置；未配置时使用后端默认值。
- 原生 Web 搜索默认开启。
- `bun run web` 必须能按新 env / config 格式启动。
- 当前本机环境需要迁移到新 `.env` 和 `agents.toml` 格式，但真实密钥不得进入仓库。

## 本计划目标

输出并执行一个分 Phase 计划，使 budn' 的 Agent 配置和 Web UI 满足：

- 后端通过统一配置读取 OpenAI Responses 和 Anthropic Messages provider。
- 配置文件支持 provider registry、model registry、active provider/model、默认参数和 provider/model 级覆盖。
- Provider 模型发现与手动配置同时参与模型 registry 合并；手动配置同 id 模型只覆盖对应字段。
- 模型 registry 支持 reasoning effort 与 service label，并在 Web 中展示/配置。
- OpenAI 与 Anthropic 都支持 reasoning effort；实现时按各自 provider API 语义映射，不得把 Anthropic 视为不支持 reasoning effort。
- Web UI 必须真实提供 provider/model 列表读取、模型选择、reasoning effort 和 service label 控件；不能只实现后端 command。
- `agent.invoke` / 发消息 API 必须携带本次使用的 provider / model / reasoning effort / service label 参数。
- `web_search_supported` 是单纯布尔值；如果配置或 provider 实际调用失败，按 Agent error 暴露，不用 unknown 状态吞掉问题。
- Web 通过 app server protocol 获取可用模型列表，并通过 protocol command 切换当前模型。
- 原生 Web 搜索默认开启，但只通过 provider hosted tool 接入，不新增本地互联网搜索工具。
- `bun run web` 使用新的 `.env` 和 `agents.toml` 配置链路。
- ignore 与示例配置能防止本机密钥和个人模型偏好进入仓库。
