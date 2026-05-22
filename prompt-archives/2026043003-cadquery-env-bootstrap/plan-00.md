# CadQuery Environment Bootstrap Plan

## 背景

`bun run web` 启动 Web dev server 与 websocket host 后，CadQuery tool call 在运行 Python runner 时失败，错误为 `ModuleNotFoundError: No module named 'cadquery'`。根因初步确认是 Host 默认使用 `python3`，而当前 `.env` 没有设置 `CADQUERY_RUNNER_PYTHON`；本机默认 `/usr/bin/python3` 不能导入 CadQuery，本机 `/opt/homebrew/bin/python3.11` 可以导入 CadQuery 2.7.0。

## Phase 1 — 根因固定与启动前验证

输入：

- 当前 `scripts/run_studio_web_dev.ts`、`scripts/run_websocket_host.ts`。
- 当前 `app-server-host` CadQuery runner 入口。
- 当前 `.env`。

前序目标保护：

- 保留既有 `CADQUERY_RUNNER_PYTHON` 覆盖能力。
- 保留测试使用 fake runner 脚本的能力。
- 不新增项目内 Python 辅助脚本。

操作步骤：

1. 增加最小失败测试，覆盖 websocket host 启动前会验证 CadQuery Python 环境。
2. 验证失败测试确实失败，失败原因应指向缺少验证行为。
3. 在启动链路中加入 CadQuery runner 环境验证：验证所选 Python 可启动 `budn_cad_runner` 并能导入 CadQuery 依赖。
4. 验证失败时让 `bun run web` 在 host 启动阶段失败，并给出包含 Python 路径、`CADQUERY_RUNNER_PYTHON` 和修复建议的错误。

验收标准：

- 相关 Bun 测试通过。
- 现有 fake runner 相关测试不受影响。
- `CADQUERY_RUNNER_PYTHON` 缺失或指向错误 Python 时，启动阶段能提前失败，而不是等到 tool call。

## Phase 2 — 当前本机 `.env` 设置

输入：

- 当前 `.env`。
- 本机可用 Python：`/opt/homebrew/bin/python3.11`。

前序目标保护：

- 保留 Phase 1 的启动前验证。
- 不覆盖用户已有 LLM 配置。

操作步骤：

1. 将 `CADQUERY_RUNNER_PYTHON=/opt/homebrew/bin/python3.11` 写入当前 `.env`。
2. 使用 `bun` 环境验证 `.env` 能被读取。
3. 使用 runner 现有测试或最小 runner 命令验证该 Python 能运行 `budn_cad_runner`。

验收标准：

- `bun -e` 可读取 `CADQUERY_RUNNER_PYTHON`。
- CadQuery runner 相关测试或最小 runner 命令通过。

## Phase 3 — 后续部署方案记录

输入：

- 当前启动脚本与 CadQuery runner 边界。
- 本轮根因与修复结果。

前序目标保护：

- 不把部署方案写成测试专用逻辑。
- 不引入 Python 辅助脚本到项目工具链。

操作步骤：

1. 在结果文档中记录推荐部署方案：显式提供 Python 环境、启动前 verify、容器镜像预装 CadQuery、环境变量注入和失败提示。
2. 若已有开发文档适合补充，按最小范围补充 `CADQUERY_RUNNER_PYTHON` 配置说明。

验收标准：

- 结果文档说明当前修复和长期部署建议。
- 若修改开发文档，内容不与产品命名和工具链约束冲突。

## Phase 4 — 最终验证与归档

输入：

- 本轮所有代码、配置和文档改动。

前序目标保护：

- 保留前面 Phase 已完成的启动前验证、当前 `.env` 设置和部署建议。

操作步骤：

1. 运行相关 Bun 测试。
2. 运行相关 Rust 测试或 smoke 验证。
3. 更新 `plan-00-result.md`，记录根因、变更、验证证据、当前 `.env` 设置和后续部署建议。

验收标准：

- 验证命令有明确结果。
- `plan-00-result.md` 能让后续会话无上下文判断当前状态。
