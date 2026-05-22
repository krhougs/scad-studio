# CadQuery Environment Bootstrap Prompt

## 原始用户输入

用户使用 `bun run web` 启动整套 Web + app server 后，在 CadQuery tool call 阶段遇到：

```text
ModuleNotFoundError: No module named 'cadquery'
```

用户诉求：

1. 后端调用 CadQuery 的服务在启动时先 verify 当前环境能把这一坨 Python 正确跑起来。
2. 设置当前 `.env`，让 `bun run web` 能正确调用。
3. 思考如何优雅地把环境自动设置好，用于后期对外部署。

## 已确认上下文

- `bun run web` 执行 `bun scripts/run_studio_web_dev.ts`。
- `run_studio_web_dev.ts` 启动 `launchWebsocketHost`，后者通过 `cargo run -p app-server-host --bin websocket-host` 启动 app server host。
- Bun 会自动读取仓库根目录 `.env`；当前 `.env` 只有 `BUDN_LLM_CONFIG=llm.toml`。
- `app-server-host` 当前通过 `CADQUERY_RUNNER_PYTHON` 选择 Python，未设置时回退到 `python3`。
- 本机 `/usr/bin/python3` 不能导入 `cadquery`，报 `ModuleNotFoundError`。
- 本机 `/opt/homebrew/bin/python3.11` 可以导入 `cadquery`，版本为 `2.7.0`。

## 强制约束

- 不新增项目内任意 Python 辅助脚本；CadQuery Python 子进程只能作为 `budn_cad_runner` 外部 CAD 工具边界存在。
- 启动前验证应由 app server host 或其 Rust/Bun 启动链路完成，不应把验收对象或测试专用语义写入产品代码。
- 当前 `.env` 是用户本机配置，可以按本机可用 Python 设置。
- 实现必须避免破坏既有 `CADQUERY_RUNNER_PYTHON` 覆盖能力和测试 fake runner 能力。
