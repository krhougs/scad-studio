# Phase 0 契约 · Smoke 矩阵

本文件固定所有 Phase 的 smoke 与构建命令、覆盖范围、退出码语义、环境隔离要求。

## 1. Smoke 总览

| 编号 | 名称 | 入口命令 | 覆盖范围 | 退出码 |
|------|------|----------|----------|--------|
| S1a | `rust_unit_smoke` | `cargo test -p studio-web-wasm` | wasm crate 在 host 下的单元测试：`ManagedClient` 状态机、mesh decode 纯逻辑、错误模型序列化 | 非 0 = 失败 |
| S1b | `wasm_bindgen_smoke` | `wasm-pack test --headless --chrome crates/studio-web-wasm` | wasm 在浏览器环境下的 `#[wasm_bindgen]` exports / bridge 行为：watch push / request 响应 / cancel / reconnect / 超时 / renderer 幂等 | 非 0 = 失败 |
| S1c | `wasm_package_smoke` | `bun run web:smoke -- --case wasm_package_smoke` | 验证 `@scad-studio/studio-web-wasm` 可从 `packages/studio-web` import；验证 `packages/studio-web-wasm/generated/` 与重新生成的产物一致（diff） | 非 0 = 失败 |
| S2 | `browser_smoke` | `bun run web:smoke -- --case browser_smoke` | 启动 `websocket-host` + Vite，Playwright 驱动：`WorkspaceCurrent` / `WorkspaceList` / `PreviewRequest` 端到端 | 非 0 = 失败 |
| S3 | `browser_watch_smoke` | `bun run web:smoke -- --case browser_watch_smoke` | watch 推送进入 wasm `ClientEvent::WatchEvent` → React 重新渲染目录列表 | 非 0 = 失败 |
| S4 | `pwa_build_smoke` | `bun run web:build` | Vite 构建 React PWA，wasm 资源被正常引用；生产构建 wasm 文件名带 hash | 非 0 = 失败 |

## 2. Phase → Smoke 映射

| Phase | 必须通过的 smoke |
|-------|------------------|
| Phase 0 | 无（只写文档） |
| Phase 1 | 旧 `web` / `web:smoke`（启用 `legacy-shell` feature）兼容路径仍可跑 |
| Phase 2 | S1a、S1b |
| Phase 3 | S2 |
| Phase 4 | S2（视觉影响不引入回退） |
| Phase 5 | S1a、S1b、S1c、S2、S3、S4 全通过 |
| Phase 6 | S1a、S1b、S1c、S2、S3、S4，加 S2 扩展用例（markdown / image / scad_split_view） |
| Phase 7 | S1a、S1b、S1c、S2、S3、S4，加 S2 / S3 扩展用例（canvas_interaction / parameters_presets / export_slicer / config_settings / scad_autorerender） |
| Phase 8 | 上述所有 + `rg` 清理性验收命令 |

## 3. 环境隔离要求

### 3.1 通用

- `websocket-host` 启停地址由环境变量 `SCAD_STUDIO_WS_URL` 控制（完整 WebSocket URL），默认 `ws://127.0.0.1:38421`；启动器从该 URL 解析 host/port，不读取独立的端口变量。
- 所有 smoke 必须复用 `scripts/run_studio_web.ts` 内同一启动器；禁止 smoke 脚本内手写 `cargo run -p websocket-host ...`。
- 每个 smoke 启动前必须 ensure 端口空闲（检测占用则终止旧进程或换端口）；结束时必须清理启动的进程树。

### 3.2 S2 / S3（浏览器 smoke）

Playwright context 启动前：

1. 清空 Service Worker 注册：
   ```js
   for (const reg of await navigator.serviceWorker.getRegistrations()) {
     await reg.unregister();
   }
   ```
2. 清空 Cache Storage：
   ```js
   for (const key of await caches.keys()) {
     await caches.delete(key);
   }
   ```
3. 断言：`(await navigator.serviceWorker.getRegistrations()).length === 0`。
4. Vite dev server 必须禁用 Service Worker（`vite-plugin-pwa` `devOptions.enabled = false`）。

以上断言命中失败 = S2 / S3 失败（视为测试环境未清理）。

### 3.3 S1c（wasm 包 generated 一致性校验）

执行顺序：

1. 把 `packages/studio-web-wasm/generated/` 复制到临时目录 `__snapshot/`。
2. 执行命名矩阵定义的 wasm-bindgen 命令，重新生成产物到 `packages/studio-web-wasm/generated/`。
3. 逐文件 diff `__snapshot/` 与 `packages/studio-web-wasm/generated/`，任何差异视为失败。diff 清单必须覆盖 wasm-bindgen 在 `--target bundler` 下产出的全部文件：
   - `studio_web_wasm.js`
   - `studio_web_wasm_bg.wasm`
   - `studio_web_wasm_bg.js`
   - `studio_web_wasm.d.ts`
   - `studio_web_wasm_bg.wasm.d.ts`
   - `package.json`（wasm-bindgen 自动生成的；区别于 `packages/studio-web-wasm/package.json`）
   - 目录内任何其它文件（用文件系统遍历兜底，不写死白名单）
4. 把 `packages/studio-web-wasm/generated/` 还原为 `__snapshot/`（避免测试污染工作树）。

## 4. S2 / S3 扩展用例命名规范

| 命名 | 所属 Phase | 含义 |
|------|-----------|------|
| `browser_smoke -- --case markdown_view` | 6 | Markdown 查看 |
| `browser_smoke -- --case image_view` | 6 | 图片查看 |
| `browser_smoke -- --case scad_split_view` | 6 | `.scad` 源码 + 预览双视图 |
| `browser_smoke -- --case canvas_interaction` | 7 | 3D 交互（旋转 / 缩放 / 平移 / 预设） |
| `browser_smoke -- --case parameters_presets` | 7 | 参数与预设 |
| `browser_smoke -- --case export_slicer` | 7 | 导出与切片器信息 |
| `browser_smoke -- --case config_settings` | 7 | 配置 / 设置 |
| `browser_watch_smoke -- --case scad_autorerender` | 7 | `.scad` 自动重渲染 |

扩展用例必须与 `--case browser_smoke` / `--case browser_watch_smoke` 共享启动器；禁止引入独立 websocket-host 进程。

## 5. Rust 单元测试硬约束（S1a）

- `cargo test -p studio-web-wasm` 运行**非 wasm target**下的单元测试。
- 覆盖内容必须包括：
  - `ManagedClient::dispatch_*` 命令写入 outbound 队列的序列化正确性；
  - `ManagedClient::poll` 消费 `AppServerTransportEvent` 后产生的 `ClientEvent`；
  - 超时触发路径（`tick(now_ms)` 推进至超时阈值后，`drain_events` 出现 `RequestTimedOut`）；
  - cancel 路径（`cancel` 后再收到 server 响应，进入 `UnknownRequest`，不回传 `RequestSucceeded`）；
  - watch 节流路径（窗口内多次事件合并为 1 次 `WatchEvent`）；
  - reconnect 路径（`mark_transport_closed` → `next_outbound` 再次出队原 envelope + watch 重订阅帧）；
  - `ClientError` / `ClientEvent` 的 serde 序列化稳定性（对外格式冻结）。

## 6. 浏览器 bindgen 测试硬约束（S1b）

S1b 至少覆盖（按 Phase 2 验收拆分）：

- 握手成功 → `HandshakeAccepted`；
- request → response → `RequestSucceeded`；
- cancel 请求后再次接收响应不进入 `RequestSucceeded`，应进入 `RequestFailed { error: Cancelled }`；
- transport closed → reconnect 后未响应请求自动重发（`client_next_outbound` 再次输出该 envelope）；
- watch 订阅在 reconnect 后自动重订阅并产出 `WatchResubscribed`；
- request 超时通过 `client_tick` 推进，`client_drain_events` 出现 `RequestTimedOut`；
- renderer_create / destroy 幂等（同一 handle destroy 两次不 panic，resize 早于 render 调用无错误）。

## 7. 失败处理

- 任一 smoke 未通过：Phase 不得标记完成；必须修复后重新跑所有对应 Phase 的 smoke。
- 偶发失败：不能靠“重跑一次”规避；必须定位根因并写入 `docs/known_issues.md`。
- 因外部工具缺失（`wasm-pack` 未安装、浏览器未配置）导致的失败：修复开发环境后再跑；禁止把 smoke 降级为 `skipped`。
