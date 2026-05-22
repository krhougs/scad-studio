# Agent provider 配置迁移计划

## 背景

旧 `llm.toml` 是本地开发时期的单 provider 配置。当前 budn' 已有 `agents.toml` 多 provider 配置入口，但现有本机 `agents.toml` 中的旧迁移块使用了无效的 `legacy_base_url`，且 provider 块仍处于注释状态，不满足“实际可用”的目标。

## Phase 1 — 配置解析与本机配置迁移

### 输入

- `llm.toml`
- `agents.toml`
- `crates/app-server-core/src/llm/config.rs`
- `crates/app-server-core/tests/llm_tests.rs`

### 前序目标保护

- 保护当前三类 provider type 语义。
- 保护 `base_url` 解析规则。
- 保护仓库示例配置不包含真实 API key。

### 操作步骤

1. 让 `agents.toml` provider 支持本机私有的直接 `api_key` 字段，同时继续支持 `api_key_env`。
2. 用测试覆盖 active provider 直接 `api_key`、`base_url` 原样规则和 `openai_completions` 兼容 provider。
3. 将旧 `llm.toml` 中当前 provider 与原注释 provider 迁移为本机 `agents.toml` 的可用多 provider 配置。
4. 删除旧 `llm.toml`，避免后续误用旧入口。

### 验收标准

- `agents.toml` 能被 TOML 解析，active provider 为 GLHF，三个 provider 都有模型与 key。
- `cargo test -p app-server-core --test llm_tests` 通过。
- `agents.example.toml` 不包含真实 API key。

## Phase 2 — Provider 缺失硬失败与日志

### 输入

- `crates/app-server-host/src/bin/websocket-host.rs`
- `crates/app-server-host/src/dispatcher.rs`
- `crates/app-server-host/tests/shared_dispatcher_roundtrip_tests.rs`
- `crates/app-server-host/tests/websocket_smoke_roundtrip.rs`
- `packages/studio-web/src/workbench/chat-zone.tsx`

### 前序目标保护

- 保护 WebSocket handshake 协议版本协商错误仍优先返回协议版本错误。
- 保护正常配置下 handshake、WebSocket smoke 和 Agent model registry 能正常工作。
- 保护前端只展示真实运行状态，不用软失败掩盖启动期配置错误。

### 操作步骤

1. websocket host 启动前强制加载 Agent provider registry；缺失或损坏时拒绝启动并输出醒目错误。
2. handshake 能力生成路径不得吞掉 provider registry 加载错误；错误必须写入日志并返回 protocol error。
3. 移除前端 `model registry unavailable` 这类软失败文案，保留短暂加载态和 active model 缺失态。
4. 补充缺失 provider config 的 handshake 测试。

### 验收标准

- 未配置 provider 时，host 启动或 handshake 返回明确错误，并有 `error` 日志。
- `rg "model registry unavailable"` 无结果。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests` 通过。
- `cargo test -p app-server-host --test websocket_smoke_roundtrip` 通过。

## Phase 3 — 规则归档与文档同步

### 输入

- `AGENTS.md`
- `README.md`
- `docs/getting-started.md`

### 前序目标保护

- 保护项目沟通语言和文档风格。
- 保护真实 API key 只进入被忽略的本机配置。

### 操作步骤

1. 在 `AGENTS.md` 增加通用错误暴露与日志规则。
2. 更新 README / getting started 中关于 `agents.toml` 支持直接 `api_key` 与 `api_key_env` 的说明。
3. 记录执行结果和验证结果。

### 验收标准

- `AGENTS.md` 明确要求及时暴露错误和设计缺陷、分析边界情况、复杂状态与行为管理保留足够日志，并按 level 控制日志。
- 文档不再说明根目录 `llm.toml` 是当前可用配置入口。
- `git diff --check` 通过。

## 执行前检查

本计划没有 `TBD`、待用户选择的方案或缺失验收标准。
