# 执行结果存档：`2026042300-studio-web-feature-parity`

本文件在对应 Phase 执行过程中**实时追加**，与 `plan-00.md` 同步维护。

## 记录要求

- 每个 Phase 完成后，必须补充：完成情况、变更文件范围、验证结果、遗留问题。
- 若某 Phase 发现前序目标被破坏，必须在该 Phase 内修复并记录。

| Phase | 状态 | 摘要 | 遗留问题 |
|-------|------|------|----------|
| 0 | 已完成 | 产出 5 份契约：`plan-00-naming.md` / `plan-00-bridge.md` / `plan-00-toolchain.md` / `plan-00-ownership.md` / `plan-00-smoke.md`。覆盖：命名矩阵、wasm 桥接 API（含 `client_begin_handshake` / `client_destroy` / `ManagedClient` 方法清单）、params 类型与 `app-server-protocol` 映射、错误模型（含 `InvalidHandle` / `NotReady`）、超时 / watch 节流 / reconnect / trait 适配审查、bun-only 工具链、状态归属、S1a–S4 smoke 矩阵。Buddin 设计参考可获取性兜底写入 toolchain §5。 | 无 block；Phase 4 启动前需再次检查 `/Users/krhougs/LocalCodes/buddin/*` 可读性。 |
| 1 | 已完成 | 1) `pnpm-workspace.yaml` + 根 `package.json` `workspaces` 字段创建；`.gitignore` 追加 `pnpm-lock.yaml` / `node_modules/`。2) `crates/studio-web` 通过 `git mv` 重命名为 `crates/studio-web-wasm`；`src/app.rs` → `src/legacy_dom_shell.rs`。3) `Cargo.toml` 改 package name `studio-web-wasm`、lib name `studio_web_wasm`；`app-server-transport` 置为 `optional = true`；新增 features `legacy-shell = ["dep:app-server-transport"]`、`browser-smoke = ["legacy-shell"]`；`wasm-bindgen` 锁定 `=0.2.117`。4) 根 `Cargo.toml` workspace member 同步更新。5) `src/lib.rs` 将 `legacy_dom_shell` / `preview_canvas` / `transport_port` 模块与 boot 函数 全部置于 `#[cfg(all(target_arch = "wasm32", feature = "legacy-shell"))]` 之下；三个 wasm 侧文件的模块级 `#![cfg]` 同步更新。6) `scripts/run_studio_web.ts` / `tests/build_studio_web_shell.sh` / `tests/studio_web_smoke.sh` / `web/index.html` 更新到新 crate 名与新产物名；`browser_smoke.rs` / `browser_watch_smoke.rs` / `public_api_tests.rs` / `chat_state_tests.rs` 中 `studio_web::` 全部改为 `studio_web_wasm::`；标签字符串改为 `studio-web-wasm shell`。7) `packages/studio-web-wasm` 骨架（package.json / README.md / generated/.gitkeep / src/index.ts）与 `packages/studio-web` 骨架（package.json / tsconfig.json / vite.config.ts / index.html / src/main.ts / public/.gitkeep / tests/.gitkeep）创建。8) 新增 `scripts/check_wasm_bindgen_version.ts` 并接入 `bun run check:wasm-bindgen`，强制 Cargo.toml 与 CLI 版本一致；README 同步更新说明。9) `packages/studio-web-wasm/src/index.ts` 对 Phase 2 再生的空 generated 加 `@ts-expect-error` 保护。验证：`cargo check --workspace` 通过；默认 feature 下 `cargo tree -p studio-web-wasm` 不含 `app-server-transport`；`cargo check -p studio-web-wasm --features legacy-shell --target wasm32-unknown-unknown` 通过；`cargo test -p studio-web-wasm` host 测试全通过（public_api + chat_state）；产物路径 `target/wasm32-unknown-unknown/debug/studio_web_wasm.wasm` 生成正确；`rg "legacy_dom_shell" crates/studio-web-wasm/src` 全部命中均位于 `#[cfg(all(target_arch = "wasm32", feature = "legacy-shell"))]` 之下；`bun run check:wasm-bindgen` 通过。Review：P0 无；P1-1 已修（wasm-bindgen-cli 版本锁定入口落到 bun 脚本 + package.json script）；P1-2（文件级 `#![cfg]` 与 mod 级 cfg 冗余）列为 Phase 2 清理候选；P2-2 已修。 | 无 block；Phase 2 计划清理 `legacy_dom_shell` 并在 `studio-common` 新增 `ManagedClient`。 |
| 2 | 已完成 | Phase 2a (`studio-common`)：新增 `ManagedClient<T>` 监督层，覆盖 Phase 0 契约 §9 全部方法 —— 命令派发 / 事件 drain / snapshot / 超时 tick / 取消 / watch 节流与重订阅 / reconnect 重放。新类型（`ClientEvent` / `ClientError` / `ClientSnapshot` / `TransportStatus` / `TransportCloseReason` / `WatchParams` / `WatchEventPayload` / `ClientTimeouts`）的 serde 格式冻结有快照测试保护。Phase 2a 经 codex 两轮 review：第一轮抓到 [P1] wire envelope 用了自造 `{"frame"}` tag → 将 `ClientEnvelope` / `ServerEnvelope` / `TransportErrorFrame` 从 `app-server-transport` 迁入 `app-server-protocol` 并在 transport crate re-export，`studio-common` 使用协议 crate 的 envelope；第二轮抓到 3 条 [P2]（陈旧 preview 响应覆盖、重连期 cancel 乱序、watch subscribe 忽略 timeout）全部修掉。`cargo test -p studio-common --test managed_client_tests` 15 条通过（含 3 条新增边角测试）。Phase 2b (`studio-web-wasm`)：新增 `wasm_bridge/` 暴露完整 bridge API —— 15 个 `client_*` 导出（含 `client_create` 无参与 `client_create_with_timeouts` 两入口）、`mesh_decode` / `mesh_destroy`、以及 `renderer_*` API-complete stub（真实 wgpu 初始化因需 `wasm_bindgen_futures` 等待 JS Promise 违反契约 §10，推到 Phase 3 由 TS 侧或 `scad_scene` 同步构造函数处理）。`NullTransport` 满足 `AppServerTransportPort` 形参要求但不被 `ManagedClient` 实际调用，envelope 走字节队列路径。`FakeChatState` 迁入 `legacy-shell` feature。`ClientHandle` 用 `Option<ManagedClient>` 实现 destroy 幂等，post-destroy 所有方法返回 `ClientError::InvalidHandle`。Phase 2b 经 codex review：首轮 0 P1；抓到 [P2] `client_destroy` 按值消费 handle 与 [P3] `client_create(JsValue)` 违反契约 §2 的无参签名，均已修掉。`cargo test -p studio-common --tests` 全部绿色；`cargo check --workspace` / `cargo check -p studio-web-wasm --target wasm32-unknown-unknown`（default / legacy-shell 两套）均通过；`wasm-pack test --headless --chrome crates/studio-web-wasm --test wasm_bridge_smoke` 9/9 通过（S1b 全部必覆盖场景 + 1 条 destroy 幂等测试）。`bun run check:wasm-bindgen` 通过。 | Phase 3 前置依赖：renderer 真实 wgpu 接入（TS 侧 drive adapter/device 请求或 scad-scene 暴露同步 constructor）；`ClientTimeouts` serde 形态建议在 `plan-00-bridge.md` §5 补文档；reconnect 期间 watch subscribe deadline 不暂停（低概率误报 timeout）属已知小口风险。 |
| 3 | 未开始 |  |  |
| 4 | 未开始 |  |  |
| 5 | 未开始 |  |  |
| 6 | 未开始 |  |  |
| 7 | 未开始 |  |  |
| 8 | 未开始 |  |  |

## Phase 0 Review 归档

- 独立 subagent review 发现 4 条 P0 + 5 条 P1 + 3 条 P2。
- P0#1 `client_dispatch_*` 签名冲突：已改为 `Result<RequestId, ClientError>`（bridge §1 / §5）。
- P0#2 首次握手触发时机未定义：已新增 `client_begin_handshake(handle, params)`（bridge §2）。
- P0#3 params / payload 字段缺席：已新增 §1.1 params → `app-server-protocol` 映射表；`WatchEventPayload` schema 写入 §7。
- P0#4 ownership §2 禁止项误伤 TS resolver table：已在 ownership §3 显式允许单一文件 `packages/studio-web/src/wasm-bridge/request-resolvers.ts`；bridge §10 同步放行。
- P1#5 `client_destroy` 语义补齐、`InvalidHandle` 纳入错误枚举。
- P1#6 `WatchEventPayload` 字段固定。
- P1#7 watch registry 归属显式写明在 `ManagedClient`。
- P1#8 `SCAD_STUDIO_WS_URL` URL vs 端口措辞统一。
- P1#9 S1c diff 文件清单明示。
- P2#10 `ManagedClient` 定名，删除备选。
- P2#11 `RequestId` 明确为 `app_server_protocol::RequestId` newtype，跨 JS 用 `bigint`。
- P2#12 本 Phase 0 结果存档已写入本文件（本块即是）。
