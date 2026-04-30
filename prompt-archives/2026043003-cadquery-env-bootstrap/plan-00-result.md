# CadQuery Environment Bootstrap Result

## 当前状态

- 已创建计划存档。
- Phase 1 已完成：新增 websocket host 启动前 CadQuery Python 环境验证，并通过 red/green 测试固定行为。
- Phase 2 已完成：当前 `.env` 已设置 `CADQUERY_RUNNER_PYTHON=/opt/homebrew/bin/python3.11`，并验证 Bun 能读取、runner 测试能通过。
- Phase 3 已完成：已在 CadQuery Python runner 文档中记录开发启动配置和后续部署建议。
- Phase 4 已完成：相关测试、格式检查、diff 检查和实际 websocket host 启动验证均已执行。

## 根因

`bun run web` 由 Bun 自动读取仓库根目录 `.env`。此前 `.env` 只包含 `BUDN_LLM_CONFIG=llm.toml`，没有设置 `CADQUERY_RUNNER_PYTHON`。`app-server-host` 因此回退到默认 `python3`，当前本机该命令解析到 `/usr/bin/python3`，不能导入 `cadquery`，所以 CadQuery tool call 阶段报 `ModuleNotFoundError`。

本机 `/opt/homebrew/bin/python3.11` 可以导入 CadQuery，版本为 `2.7.0`。

## 已完成变更

- 新增 `app_server_host::verify_cadquery_runner_environment`，通过所选 Python 执行 import probe，验证 `cadquery` 和 `budn_cad_runner` 可导入。
- `websocket-host` 在绑定端口前执行验证；失败时直接退出，并输出 Python 路径、`CADQUERY_RUNNER_PYTHON` 和修复建议。
- `run_websocket_host.ts` 增加 `waitForHostReady`，当 host 进程在端口就绪前退出时立即失败，避免 `bun run web` 继续等待端口超时。
- 当前 `.env` 增加 `CADQUERY_RUNNER_PYTHON=/opt/homebrew/bin/python3.11`。
- `docs/cadquery-mvp/python-runner.md` 增加开发启动配置和部署建议。

## 验证记录

- `python3 -c 'import sys; print(sys.executable); import cadquery; print(cadquery.__version__)'`：失败，`/usr/bin/python3` 报 `ModuleNotFoundError: No module named 'cadquery'`。
- `python3.11 -c 'import sys; print(sys.executable); import cadquery; print(cadquery.__version__)'`：通过，`/opt/homebrew/opt/python@3.11/bin/python3.11`，CadQuery `2.7.0`。
- `cargo test -p app-server-host --test cadquery_env_tests`：先因缺少 `verify_cadquery_runner_environment` 按预期失败；实现后 2 passed，0 failed。
- `bun test tests/run_websocket_host.test.ts`：先因缺少 `waitForHostReady` 按预期失败；实现后 3 passed，0 failed。
- `bun -e 'console.log(process.env.CADQUERY_RUNNER_PYTHON)'`：输出 `/opt/homebrew/bin/python3.11`。
- `bun test tests/cadquery_runner.test.ts --timeout 30000`：9 passed，0 failed。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests`：15 passed，0 failed。
- `bun scripts/run_websocket_host.ts --workspace /tmp/budn-cq-env-web --ws-url ws://127.0.0.1:38431`：成功启动并输出 `ws://127.0.0.1:38431`，说明 `.env` 中的 Python 配置已被 Bun 启动链路传递给 host，启动前验证通过；验证后已停止进程。
- `lsof -nP -iTCP:38431 -sTCP:LISTEN`：停止后无监听，exit 1。
- `bun run web -- --workspace /tmp/budn-cq-env-full-web --web-port 5196 --ws-url ws://127.0.0.1:38432`：成功完成 WASM build、websocket host 启动和 Vite 启动，输出 `Local: http://localhost:5196/`；验证后手动停止进程，因此 wrapper 最终 exit 143。
- `lsof -nP -iTCP:38432 -iTCP:5196 -sTCP:LISTEN`：停止后无监听，exit 1。
- `cargo fmt --check`：通过，exit 0。
- `git diff --check`：通过，exit 0。

## 当前本机配置

当前 `.env` 已包含：

```bash
BUDN_LLM_CONFIG=llm.toml
CADQUERY_RUNNER_PYTHON=/opt/homebrew/bin/python3.11
```

`.env` 属于本机配置文件，不作为仓库交付 diff 的一部分。

## 后续部署建议

- 对外部署时应提供预置 CadQuery 的 Python 3.11 环境，并显式注入 `CADQUERY_RUNNER_PYTHON`。
- 不要依赖服务器默认 `python3`；默认解释器在 macOS、Linux 发行版和容器基础镜像中差异很大。
- 将启动前验证保留为健康检查的一部分，验证失败直接退出，由 supervisor 或平台探针报告配置错误。
- 中长期应把 `budn_cad_runner` 打成标准 Python package，在镜像构建阶段安装；这样部署时不依赖仓库当前工作目录进入 Python import path。
