# 执行结果存档：`2026042300-phase6-watch-refresh`

## 任务摘要

- 目标：补齐 Phase 6 最小 watch 订阅/刷新闭环，让 `studio-web` 真正消费 watch 推送并刷新当前目录列表。
- 范围：`studio-common` 共享 helper、`studio-web` 接线、浏览器 smoke。

| Phase | 状态 | 摘要 | 遗留问题 |
|-------|------|------|----------|
| 1 | 已完成 | 已确认旧 Phase 6 约束、`studio-web` 现状、桌面参考实现与 smoke 入口 | 无 |
| 2 | 已完成 | 已按 TDD 补齐共享 helper 单测与浏览器 watch smoke，并先验证失败 | 初版 shell 背景写文件触发过早，已改为测试内第二连接写文件，避免时序假失败 |
| 3 | 已完成 | 已实现 `studio-common` 共享 watch 生命周期 helper、`studio-web` 订阅/退订/刷新接线，并完成验证 | 本机缺少 `rust-analyzer`，LSP 诊断无法执行；已用编译与测试验证替代 |

## Phase 1 记录

- archived Phase 6 明确要求：watch 事件后的刷新行为收敛到 `studio-common`，网页与桌面共享处理语义。
- `crates/studio-web/src/app.rs` 现状：只处理 `WorkspaceCurrent` / `WorkspaceList` / `PreviewReady`，所有 push 事件都只是 `status = format!("push event received: ...")`。
- `crates/studio-app/src/protocol_client.rs` 现有桌面参考语义：`subscribe_path()` 拿到 `subscription_id`，`Drop` 时退订，`dispatch_watch_changed()` 按 `subscription_id` 把变更分发给订阅者。
- 浏览器 smoke 现有入口：`tests/studio_web_smoke.sh` 启动 `websocket-host`，然后在 `crates/studio-web` 下执行 `wasm-pack test --headless --chrome --features browser-smoke`。
- 本轮收敛的机械化证明路径：在 repo-local fixture workspace 根目录中临时创建一个文件，等待浏览器文件列表因为 watch 驱动的 `workspace.list` 重拉而显示该文件；最终实现改为由浏览器测试内的第二条 WebSocket 连接通过协议写文件，shell 只负责前后清理。

## Phase 2 记录

- 新增 `crates/studio-common/tests/watch_lifecycle_tests.rs`，先定义共享 helper 的三个行为：
  - 初次进入 root 时发起 `watch.subscribe`；
  - 切换目录时先退订旧订阅，再订阅新目标；
  - `WatchChanged` 仅在匹配当前活动订阅时触发目录刷新。
- 新增 `crates/studio-web/tests/browser_watch_smoke.rs`，先定义浏览器 watch smoke：启动 app，确认初始列表已加载，再制造一次 repo-local 文件变更并等待列表出现新文件。
- Red 阶段证据：
  - `cargo test -p studio-common --test watch_lifecycle_tests` 初次失败，原因是 `DirectoryWatchLifecycle` / `WatchLifecycleRequest` 尚不存在。
  - `tests/studio_web_smoke.sh` 初次新增 watch smoke 失败，原因是浏览器列表中看不到 `watch-smoke-generated.txt`。
- 调试记录：最初使用 shell 后台延迟写文件，发现时序容易早于浏览器测试启动，导致出现“文件预先存在”的假失败；因此把 smoke 触发方式收敛为测试内第二连接通过协议写文件。

## Phase 3 记录

### 变更摘要

- `crates/studio-common/src/watch_lifecycle.rs`
  - 新增 `DirectoryWatchLifecycle` 与 `WatchLifecycleRequest`。
  - helper 只依赖 `app-server-protocol` 类型，内部维护 `desired_directory`、`active_subscription` 和单个 `pending_request`。
  - 提供 `enter_directory()`、`record_sent_request()`、`handle_watch_subscribed()`、`handle_watch_unsubscribed()`、`refresh_directory_for()`，把订阅生命周期与命中刷新判断收敛在共享层。
- `crates/studio-common/src/lib.rs`
  - 导出 `DirectoryWatchLifecycle` 与 `WatchLifecycleRequest` 给端壳层消费。
- `crates/studio-web/src/app.rs`
  - `StudioWebApp` 新增共享 `watch` 状态。
  - `WorkspaceList` 成功后驱动共享 helper 对当前目录/root 建立目标订阅。
  - 复用现有 `AppServerClient::subscribe()` / `unsubscribe()` 发送协议请求，并把 request id 回填给 helper。
  - `WatchSubscribed` / `WatchUnsubscribed` response 用于推进共享生命周期。
  - `ServerPushEvent::WatchChanged` 命中当前活动订阅时，调用 `list_directory(app, current_dir)` 重新拉取当前目录列表；`WatchError` 仅更新状态文本，不扩展范围。
- `crates/studio-web/tests/browser_watch_smoke.rs`
  - smoke 通过第二条 `WebSocketClientTransport` 连接完成 handshake -> `workspace.current` -> `file.write_text`，把 `watch-smoke-generated.txt` 写入 root，并等待 app 列表出现新文件。
- `tests/studio_web_smoke.sh`
  - 负责在 smoke 前后清理 `watch-smoke-generated.txt`。
  - 分别执行 `browser_smoke` 与 `browser_watch_smoke` 两个 wasm 测试目标。

### 验证结果

- `cargo fmt --all`：通过。
- `cargo check -p studio-common`：通过。
- `cargo check -p studio-web --target wasm32-unknown-unknown --tests --features browser-smoke`：通过。
- `cargo test -p studio-common --test watch_lifecycle_tests`：通过（3 个测试）。
- `bash -n tests/studio_web_smoke.sh`：通过。
- `bash tests/studio_web_smoke.sh`：通过。
  - `browser_smoke.rs` 3 个现有 smoke 全部通过。
  - `browser_watch_smoke.rs` 1 个新增 watch smoke 通过，证明文件写入后浏览器列表可见新文件。
- 独立 review：已使用 oracle subagent 做目标/约束复核，结论 PASS，无阻塞项。

### 环境说明

- 试图对变更文件执行 `lsp_diagnostics` 时，环境返回 `Unknown binary 'rust-analyzer' in official toolchain 'stable-aarch64-apple-darwin'`，因此本轮无法给出 LSP 级别诊断结果。
- 为避免改动本机工具链配置，本轮未安装或修改 `rust-analyzer`；改用 `cargo check`、单测和完整 smoke 作为替代证据。
