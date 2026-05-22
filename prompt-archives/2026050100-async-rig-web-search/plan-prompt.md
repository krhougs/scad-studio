# Async 后端 / Rig Agent / 模型原生联网搜索 Prompt 存档

## 用户输入

用户要求研究三个方向：

1. 后端完整 async 化。
2. 去掉自己实现的 Agent，直接使用 `rig.rs`。
3. Agent 支持模型自己的联网搜索。

研究结论形成后，用户补充强制约束：

- 当前产品未发布，不要保留技术债。
- 旧同步后端、旧自研 Agent、旧 LLM HTTP/SSE 路径不应作为合并后的生产路径保留。
- 不保留生产级双路径；测试 mock 可以保留，但必须明确限定在测试边界内。

用户随后要求生成 plan。

## 后续追加输入

- 用户补充：Rust 桌面 app 可以完全删除。
- 用户进一步要求：删除桌面 app 调整到最前面，避免误导。

## 当前代码背景

- 当前分支：`plan/2026042902-agent-plan-workspace-flow`。
- 当前 workspace 仍包含 Rust 桌面端相关 crate：`crates/studio-app`、`crates/scad-ui`、`crates/scad-viewer`。
- 当前 `app-server-host` 仍包含桌面端 in-process / mpsc 生产路径：
  - `crates/app-server-host/src/in_process.rs`
  - `crates/app-server-host/src/mpsc_transport.rs`
  - `crates/app-server-host/src/runtime.rs`
- 当前后端请求入口仍以同步 dispatcher 为主：
  - `crates/app-server-host/src/dispatcher.rs`
  - `crates/app-server-host/src/runtime.rs`
  - `crates/app-server-host/src/websocket.rs`
- 当前 app server core 中大量能力仍使用同步 I/O 或同步子进程边界：
  - `crates/app-server-core/src/workspace.rs`
  - `crates/app-server-core/src/file.rs`
  - `crates/app-server-core/src/chat.rs`
  - `crates/app-server-core/src/preview.rs`
  - `crates/app-server-core/src/config.rs`
  - `crates/app-server-core/src/presets.rs`
  - `crates/app-server-core/src/export.rs`
  - `crates/app-server-core/src/watch.rs`
  - `crates/app-server-core/src/agent/plan_package.rs`
  - `crates/app-server-core/src/agent/tools/*`
  - `crates/app-server-core/src/cadquery/*`
  - `crates/app-server-host/src/cadquery_env.rs`
- 当前 LLM / Agent 路径仍是项目自建抽象：
  - `crates/app-server-core/src/llm/mod.rs`
  - `crates/app-server-core/src/llm/openai_compat.rs`
  - `crates/app-server-core/src/agent.rs`
  - `crates/app-server-core/src/agent/tools.rs`
- 当前 `app-server-core` 使用 `ureq` 执行 OpenAI-compatible Chat Completions 风格的阻塞 HTTP/SSE 请求。
- 当前 `app-server-host` 的 Agent worker 使用 `std::thread::spawn` 与 `thread::sleep`。
- 当前 `crates/app-server-core/src/lib.rs` 对外导出旧 Agent / LLM 类型与旧 tool loop 函数，Rig 替换时必须同步移除生产公开 API。
- 当前工具注册、路径权限、CadQuery staging、Chat history 和 Agent protocol 事件已经表达了 budn' 的产品契约，不能因为替换 Agent 执行引擎而删除。

## 已核对资料

- `docs/cadquery-mvp/decisions.md` 记录过 Rig 候选方向，要求按 crates.io / docs.rs 当前版本评估 tool use、streaming 与自定义 Agent loop 能力。
- `docs/known_issues.md` 已记录“Agent 后端尚未接入真实 LLM provider 配置”，并提到后续基于 Rig provider 接入 tool call、streaming、cancel 和错误映射。
- Rig 文档显示其 Agent 支持 async multi-turn streaming、tool call / tool result 事件与 OpenAI Responses API provider。
- OpenAI 官方文档显示 Responses API 支持 provider-native hosted tools，其中包括 `web_search`。

## 本计划目标

输出一个可执行的分 Phase 实施计划，使合并后的生产路径满足：

- Phase 1 先删除 Rust 桌面 app 及桌面专属生产路径。
- Web 成为唯一生产 GUI 端。
- 后端请求调度与 Agent 执行路径完整 async 化。
- Rig 成为唯一生产 Agent 执行引擎。
- 模型原生联网搜索通过 provider-native hosted tool 接入。
- 旧同步 dispatcher、旧自研 LLM provider、旧 OpenAI-compatible SSE parser、旧自研多轮 tool loop 不作为生产路径保留。
- 保留并继续强化 budn' 的工具权限、路径策略、CadQuery staging、Chat 记录和 protocol 事件契约。
