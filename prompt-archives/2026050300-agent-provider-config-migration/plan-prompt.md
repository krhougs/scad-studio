# Agent provider 配置迁移 Prompt 存档

## 原始需求

- 用户要求以实际可用为目标，把根目录旧 `llm.toml` 迁移至当前 Agent provider 配置。
- 用户进一步明确：
  - API key 可以直接写入本机开发环境配置。
  - `llm.toml` 中注释掉的 provider 之前是因为不支持多 provider；现在应按正确多 provider 写法直接迁移。
  - 不应该出现 `model registry unavailable` 这类软失败；如果没有配置任何 provider，程序不应该启动，终端日志应该输出醒目的错误。
  - 需要把“及时暴露错误、不要用 fallback 掩盖设计缺陷、复杂状态与行为管理必须有足够日志且控制 level”的通用规则整理进 `AGENTS.md`。

## 当前上下文

- 当前产品配置入口是本机私有 `agents.toml`，由 `BUDN_AGENT_CONFIG=agents.toml` 指向。
- `agents.toml` 与 `llm.toml` 均在 `.gitignore` 中，不提交真实开发环境配置。
- 旧 `llm.toml` 当前 provider：
  - GLHF：`base_url = "https://l.glhf.do"`，`model = "gemini-3.1-pro-preview"`。
  - token-plan-sgp：注释状态，`base_url = "https://token-plan-sgp.xiaomimimo.com/v1"`，`model = "mimo-v2.5-pro"`。
  - deepseek：注释状态，`base_url = "https://api.deepseek.com"`，`model = "deepseek-v4-pro"`。
- 代码已有三类 provider type：`openai_responses`、`openai_completions`、`anthropic`。
- OpenAI family `base_url` 规则：无尾斜杠补 `/v1`，尾部 `/` 保留路径，尾部 `#` 去掉 `#` 后原样使用。

## 强制约束

- 本机开发环境配置可以直接写 `api_key`。
- `agents.example.toml` 不得写真实 key。
- `llm.toml` 不再作为运行入口。
- provider 配置缺失或无可用 provider 必须在启动或握手阶段暴露为明确错误，并输出错误日志；禁止前端把这种问题展示成普通可用性状态。
- `AGENTS.md` 必须补充通用错误暴露与日志规则。

## 追加需求

- 用户询问当前 web search 是否正确调用、调用是否会在前端展示。
- 用户要求上网搜索 Rig 当前动态 tool 调用实现方式。
- 用户指出还需要显式要求 Agent 主动查看当前环境可用 tools，避免只回答 `Native web search` 或依赖宿主可能存在的隐式能力。
