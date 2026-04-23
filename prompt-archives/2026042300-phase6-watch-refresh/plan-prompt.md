# Prompt Archive

## User Request

1. TASK: Implement the smallest shared watch subscription/refresh loop needed to satisfy archived Phase 6, and wire `studio-web` to use it.
2. EXPECTED OUTCOME: A shared, pure-Rust watch-lifecycle helper in `studio-common` owns the current watched directory and subscription lifecycle; `studio-web` uses it to subscribe when entering a directory/root, unsubscribe when switching targets, and refresh the current directory listing when a matching `WatchChanged` push arrives. Success criteria: browser-side watch flow is real, not just transport capability; a mechanical smoke proves a filesystem change becomes visible in the browser listing.
3. REQUIRED TOOLS: Read, edit/apply patch, bash for targeted verification only.
4. MUST DO: Follow TDD. Start with failing tests: add unit tests for the shared watch-lifecycle helper and a failing browser smoke path for watch-driven refresh. Keep scope tight: only current-directory subscription lifecycle and reload behavior; no broad state-machine redesign, no debounce framework, no desktop refactor. Reuse existing protocol types (`WatchSubscribeRequest`, `WatchSubscriptionAck`, `WatchChangedEvent`, `WatchUnsubscribeRequest`) and existing `AppServerClient` subscribe/unsubscribe methods. It is acceptable to extend `tests/studio_web_smoke.sh` with a tiny background file mutation in the existing fixture workspace to prove watch refresh, as long as cleanup is safe and repo-local.
5. MUST NOT DO: Do not change protocol types, do not alter directory-tree visuals beyond what is required for refresh, do not change `scad-scene`, do not update docs, do not commit, do not move browser APIs into `studio-common`, do not redesign desktop watch handling.
6. CONTEXT: Oracle judged that lack of actual watch consumption is a blocker for calling Phase 6 complete. Current facts: `studio-web` transport supports subscribe/unsubscribe via `WebSocketAppServerTransportPort`; `studio-common::AppServerClient` exposes subscribe/unsubscribe; `studio-web/src/app.rs` currently handles `ServerPushEvent::WatchChanged` only as a generic status string and never subscribes. Desktop has existing watch logic in `crates/studio-app/src/protocol_client.rs` (`subscribe_path`, `WatchSubscriptionHandle`, `dispatch_watch_changed`) that you may use as a semantic reference, but do not refactor desktop now. The existing smoke fixture already has root files and nested `examples/notes.txt`; you may add a temporary runtime-created file during the smoke script to prove refresh.

## Local Context Collected Before Changes

- Archived Phase 6 lives in `prompt-archives/2026042200-studio-app-server-unification/plan-00.md` and explicitly requires watch 事件后的客户端共享处理逻辑位于 `studio-common`。
- `crates/studio-web/src/app.rs` 目前在 `AppServerClientEvent::Push` 分支中只把 push 事件写成状态字符串，没有订阅、退订，也没有 watch 驱动的 `workspace.list` 重拉。
- `crates/studio-app/src/protocol_client.rs` 已有桌面端的订阅/退订与 `WatchChanged` 分发语义，可作为最小共享 helper 的语义参考，但本次不改桌面端。
- 现有浏览器 smoke 位于 `crates/studio-web/tests/browser_smoke.rs`，`tests/studio_web_smoke.sh` 会起 `websocket-host` 后执行 `wasm-pack test --headless --chrome --features browser-smoke`。
