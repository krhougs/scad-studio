# Agent Provider / Model 配置与 Web 切换执行结果

## 当前状态

- Phase 1 已完成并通过独立 review。
- Phase 2 已完成并通过独立 review，准备进入 Phase 3。
- 执行前已检查：当前计划无 `TBD`、`TODO`、待确认项、未选择方案或缺失验收标准。
- 约束来源已核对：原始用户需求、后续补充需求、根 `AGENTS.md` 的 Plan Mode / 工具链 / app server / protocol / Web 边界要求。

## 独立 Review 结论

- 已启动独立只读 reviewer 审查 `plan-prompt.md`、`plan-00.md`、`plan-00-result.md` 和根 `AGENTS.md`。
- 最终结论：未发现阻塞项、高风险或需要修改计划的普通问题。
- reviewer 确认计划覆盖以下强制要求：
  - OpenAI Responses API 与 Anthropic API。
  - 多 provider / 多模型 `agents.toml`，且 `agents.toml` 被 ignore。
  - provider 模型发现默认开启，发现结果与手动配置同时生效，同 id 手动配置是字段级 override。
  - 配置文件与 Web UI 支持 `reasoning_effort` 和 `service_label`，Anthropic 也支持 reasoning effort。
  - Web UI 真实支持 provider/model 列表、模型切换、reasoning effort 和 service label 控件。
  - wire protocol、WASM bridge、TypeScript package 和前端接口支持读取 provider/model 列表。
  - `agent.invoke` / 发消息 API 携带 provider、model、reasoning effort 和 service label。
  - native web search 默认开启；`web_search_supported` 是布尔值；provider 实际调用失败按 Agent error 暴露。
  - `bun run web` 按新 env / config 格式验证。
  - 现有 Web、CadQuery、protocol、workspace tree、preview、Agent run 等功能受保护。

## 执行阶段需重点验证

- `web_search_supported` 默认按 provider kind 推导为 `true` 时，新发现模型如果实际不支持 web search，错误必须按 Agent error 暴露，且不得影响 Web 工作台启动和模型切换。
- Anthropic `reasoning_effort` 到 thinking / budget 的映射必须在实现阶段核对当前 Rig 与官方 API 行为。
- 生产 Web 发消息路径必须始终携带 provider/model/reasoning/service 参数；缺参 fallback 只能用于测试。

## Phase 记录

阶段完成时统一记录：完成情况、变更摘要、验证证据、独立 review 结果、阶段提交 SHA、遗留问题；Phase 6 额外记录 Plan 级 review 结果。

| Phase | 名称 | 状态 |
| --- | --- | --- |
| 1 | 配置格式与 ignore 基线 | 已完成 |
| 2 | Provider registry 与 Agent 执行分发 | 已完成 |
| 3 | Protocol 与 Studio common capability 扩展 | 未执行 |
| 4 | Web 模型选择 UI 与状态管理 | 未执行 |
| 5 | Host 切换命令持久状态与 `bun run web` 验证 | 未执行 |
| 6 | 文档、已知问题与最终验证 | 未执行 |

## Phase 1 — 配置格式与 ignore 基线

### 完成情况

- `.gitignore` 已新增 `agents.toml`，`.env`、`.env.local`、`llm.toml` 继续忽略。
- 新增 `agents.example.toml`，只包含 env 变量名、provider/model 示例和安全默认值，不包含真实 API key。
- `crates/app-server-core/src/llm/config.rs` 已支持 `agents.toml` 结构：
  - 顶层 `active_provider`、`active_model`。
  - `[defaults]` 中 `timeout_secs`、`max_tokens`、`temperature`、`native_web_search`、`discover_models`。
  - 多 `[[providers]]` 与 `[[providers.models]]`。
  - OpenAI Responses 与 Anthropic Messages provider kind。
  - `anthropic_version` 可选，Anthropic provider 默认 `2023-06-01`。
  - `native_web_search` 与 `discover_models` 默认开启。
  - `reasoning_effort`、`service_label`、`web_search_supported` 和不支持原因。
- 保留现有 env fallback，便于 CI 与最小本地调试。
- 本机 `.env` 已改为 `BUDN_AGENT_CONFIG=agents.toml`，并保留 `CADQUERY_RUNNER_PYTHON=/opt/homebrew/bin/python3.11`。
- 本机 `agents.toml` 已创建且被 `.gitignore` 忽略；旧 `llm.toml` 继续 ignored，未进入 Git diff。
- 更新 `docs/getting-started.md`、`docs/cadquery-mvp/python-runner.md`、`docs/cadquery-mvp/decisions.md` 和 `docs/known_issues.md` 中与 Phase 1 直接冲突的配置说明。

### 验证证据

- `cargo test -p app-server-core rig_agent_config` 通过；`llm_tests` 中 13 个 `rig_agent_config*` 用例全部通过。
- `cargo check -p app-server-core` 通过。
- `bun -e 'console.log(process.env.BUDN_AGENT_CONFIG)'` 输出 `agents.toml`。
- `git status --short --ignored=matching .env agents.toml llm.toml agents.example.toml .gitignore` 显示 `.gitignore` 已修改、`agents.example.toml` 未跟踪，`.env`、`agents.toml`、`llm.toml` 都是 ignored。
- `git diff --check` 通过。
- 搜索 `默认关闭|BUDN_AGENT_NATIVE_WEB_SEARCH=true|llm.toml|BUDN_LLM_CONFIG|BUDN_LLM_BASE_URL|OpenAI-compatible|Chat Completions` 后，剩余命中为历史 known issue 和后续 Web UI 配置提示；Phase 1 文档不再把 `llm.toml` 作为当前推荐格式。

### 独立 Review 结果

- 第一轮 Phase 1 review 发现两个阻塞项：
  - 手动同 id override 曾会覆盖未显式配置字段。
  - 发现模型曾未继承 `[defaults]`。
- 已修复并补充测试：
  - `rig_agent_config_manual_override_preserves_unspecified_discovered_fields`
  - `rig_agent_config_discovered_models_inherit_provider_defaults`
- 复审结论：未发现阻塞项；普通问题为 `docs/cadquery-mvp/decisions.md` 仍描述 native web search 默认关闭，已修正。

### 遗留问题

- Phase 1 未实现真实 provider 模型发现请求与 Agent 分发；这些属于 Phase 2 范围。
- Web UI 中仍有旧配置提示文案，按计划在 Phase 4 更新。

## Phase 2 — Provider registry 与 Agent 执行分发

### 完成情况

- `RigAgentConfig` 已扩展为携带 active provider/model 的运行配置：
  - `provider_id`
  - `provider_kind`
  - `service_label`
  - `anthropic_version`
- `AgentProviderKind` 已支持：
  - `openai_responses`
  - `anthropic_messages`
- 增加 provider/model registry 的 discovery 状态模型：
  - `AgentModelSource`
  - `ModelDiscoveryStatus`
- 增加 provider 模型发现入口：
  - OpenAI 通过 Rig `ModelListingClient` 调用 models list。
  - Anthropic 通过 Rig `ModelListingClient` 调用 models list。
  - discovery 单 provider 超时为 10 秒。
  - discovery 失败时保留手动模型，并记录 `ModelDiscoveryStatus::Failed`。
- Agent 执行已按 provider 分发：
  - OpenAI 分支继续走 Rig OpenAI Responses provider。
  - Anthropic 分支走 Rig Anthropic Messages provider。
  - 两个分支复用同一套 tool registry、stream drain、timeout、cancel、tool observer 和 Chat history 写入路径。
- provider 请求参数已按 Phase 2 目标收敛：
  - OpenAI `reasoning_effort` 映射到 `reasoning.effort`。
  - OpenAI `service_label` 只在值为 `auto`、`default`、`flex` 时映射到 `service_tier`。
  - Anthropic `reasoning_effort` 映射到 `thinking.budget_tokens`。
  - Anthropic thinking budget 保持 `>= 1024` 且 `< max_tokens`；当 `max_tokens <= 1024` 或 unknown effort 时不注入 thinking。
  - Anthropic 启用 thinking 时不设置 temperature，避免生成 provider 不接受的请求组合。
  - Anthropic `service_label` 不注入 provider 请求参数。
- native web search 注入已按 provider 分发：
  - OpenAI 注入 hosted `web_search`。
  - Anthropic 注入 `web_search_20250305` server tool。
  - `native_web_search = true` 且 `web_search_supported = false` 时不注入 provider web search tool。
- 生产路径已接入 discovery：
  - core 默认 `run_rig_agent_turn` 使用 `load_rig_agent_config_with_discovery`。
  - host Agent run 使用 `load_rig_agent_config_with_discovery`。
- host legacy capability 与 run meta 已按 active provider 输出 provider kind，不再硬编码为 OpenAI。
- timeout 错误文案已改为 provider 中性。

### 验证证据

- 新增函数行数核算通过：
  - `run_openai_rig_agent_turn_with_config`：47 行。
  - `run_anthropic_rig_agent_turn_with_config`：44 行。
  - `run_rig_stream_future`：45 行。
  - `anthropic_client`：12 行。
- `cargo test -p app-server-core rig_agent_additional_params` 通过；7 个相关用例通过。
- `cargo test -p app-server-core rig_agent_temperature_param` 通过；3 个相关用例通过。
- `cargo test -p app-server-core rig_agent_config` 通过；16 个相关用例通过。
- `cargo test -p app-server-core agent_model_discovery` 通过；1 个相关用例通过。
- `cargo check -p app-server-core` 通过。
- `cargo test -p app-server-core agent` 通过；其中 `llm_tests` 相关过滤运行 29 个用例通过。
- `cargo check -p app-server-host` 通过。
- `cargo test -p app-server-host agent_capability_meta_records_native_web_search_state` 通过。
- `cargo test -p app-server-host rig_agent_errors_map_timeout_separately_from_provider_errors` 通过。
- `git diff --check` 通过。

### 独立 Review 结果

- 第一轮 Phase 2 review 发现 OpenAI `service_label` 被无条件写入 `service_tier`，已修复为只允许 `auto`、`default`、`flex`。
- 第二轮 Phase 2 review 发现 Anthropic thinking budget 可能不小于 `max_tokens`，已修复为 clamp 到 `max_tokens - 1`，并在预算不足时不注入 thinking。
- 第三轮 Phase 2 review 发现 Anthropic thinking 会与 temperature 同时发送，已修复为启用 thinking 时不设置 temperature。
- 第四轮 Phase 2 review 发现 provider 分支函数超过 50 行硬约束，已抽出共享 stream helper 和 Anthropic client helper。
- 第五轮 Phase 2 review 发现 discovery 未接入生产 run 路径、host capability 仍硬编码 OpenAI，已修复。
- 最终 Phase 2 review 结论：未发现阻塞项，Phase 2 可以提交。

### 阶段提交

- 阶段提交随本结果段一并提交；提交哈希以提交后的 `git log -1 --oneline` 为准。

### 遗留问题

- legacy handshake capability 当前仍只暴露简化的 active provider/model 信息，完整 provider/model registry、discovery status、reasoning/service 应用状态按计划在 Phase 3 扩展 protocol。
- discovery 当前在加载 discovery registry 时逐个 provider 执行，单 provider 超时 10 秒；后续 Phase 3/5 接入 Web registry 与启动链路时需要评估是否增加缓存或后台刷新。
- Web UI 中旧配置提示和模型切换控件仍未更新，属于 Phase 4 范围。
