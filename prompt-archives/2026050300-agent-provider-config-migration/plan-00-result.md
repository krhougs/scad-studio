# Agent provider 配置迁移执行结果

## 当前状态

- Phase 1 已完成：配置解析支持直接 `api_key`，本机 `agents.toml` 已按多 provider 写法迁移，旧 `llm.toml` 已删除。
- Phase 2 已完成：provider registry handshake 软失败路径已改为错误返回，websocket host 启动前 provider 校验已加入，前端软失败文案已移除。
- Phase 3 已完成：`AGENTS.md` 错误暴露与日志规则已补充，README / getting started 已同步说明。

## 验证记录

- `agents.toml` TOML 解析通过，确认 active provider / active model / provider 列表可读取，所有 provider 均包含直接 `api_key`。
- `env -u BUDN_AGENT_CONFIG -u BUDN_AGENT_OPENAI_API_KEY -u OPENAI_API_KEY CADQUERY_RUNNER_PYTHON=/opt/homebrew/bin/python3.11 cargo run -p app-server-host --bin websocket-host -- --workspace workspace/budn-web --bind 127.0.0.1:0` 按预期退出，终端输出醒目的 `AGENT PROVIDER CONFIG ERROR`。
- `cargo test -p app-server-core --test llm_tests -- --test-threads=1` 通过。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests -- --test-threads=1` 通过。
- `cargo test -p app-server-host --test websocket_smoke_roundtrip -- --test-threads=1` 通过。
- `bun run --cwd packages/studio-web test:unit -- chat-zone.test.tsx` 通过；仍存在既有 React `act(...)` warning。
- `rg -n "model registry unavailable" packages/studio-web/src packages/studio-web/tests crates README.md docs AGENTS.md` 无结果。
- `rustfmt --edition 2024 --check ...` 通过。
- `git diff --check` 通过。

## 遗留说明

- `agents.toml` 含本机开发密钥，仍为 ignored 本地配置，不进入提交。
- `llm.toml` 为旧本地配置，已从工作区删除；该文件同样为 ignored 文件。

## 追加修正

- Agent model discovery 失败现在会在 `app-server-core` 所在进程输出 `error` 日志，包含 provider、provider kind、base URL 和底层错误信息；前端只显示友好摘要，不再把 provider 原始响应、解析细节或配置路径塞进业务状态区域。
- OpenAI-compatible `/models` 解析已改为本项目宽松解析，只要求 `data[].id`，兼容 deepseek 这类不返回 `created` 字段的模型列表。
- `bun run web` 启动 Vite 时已禁用清屏和颜色控制，并由启动器过滤 Vite 输出中的控制字符，避免清掉 websocket-host 和 wasm-watch 的终端历史。
- `AGENTS.md` 已补充用户友好错误展示、非致命错误隐藏内部细节、错误发生处终端日志优先的通用规则。
- Agent system prompt 已补充动态工具能力规则；每轮 runtime context 现在会明确列出 provider-native capabilities 与当前 host 注册的 app tools，要求 Agent 回答工具能力问题前查看当前 turn 的工具列表和 schema，避免把 hosted native web search 误说成显式 function tool。
- 产品 Agent prompt 已补充 web search 决策规则：用户明确要求上网搜索但当前 turn 没有搜索能力时，Agent 必须停止并如实反馈；遇到可由外部事实改善决策的模糊需求时，优先使用 web search 支撑判断，纯用户意图或本地 workspace 状态不清时仍应询问或读取本地上下文。
