# Plan prompt 存档

本目录对应任务：**把当前 web 方案重构为 Cargo + pnpm 双工作区架构，并将现有 `studio-web` 拆分为 Rust wasm crate、npm wasm 包、React PWA 三层结构，同时继续完成 web 端功能与界面对齐。**

## 背景

- 旧计划 `prompt-archives/2026042200-studio-app-server-unification/` 已完成 app-server / protocol / transport / host / crate 拆分等基础设施，也交付了一个最小可运行的 `crates/studio-web`。
- 实际代码审阅显示，当前 `crates/studio-web` 仍是一个 Rust `cdylib` crate，直接依赖 `app-server-transport`，在 `src/app.rs` 里用字符串拼 HTML/CSS，并由 `web/index.html` 直接引导 wasm 启动。
- 根目录当前只有一个很薄的 `package.json`，脚本仍通过 `bun scripts/run_studio_web.ts` 和 `tests/studio_web_smoke.sh` 驱动，没有 `pnpm-workspace.yaml`、没有 React / Vite / Zustand / PWA 工程骨架。
- 用户最新要求：
  1. 根目录同时作为 **Cargo workspace** 和 **pnpm workspace**。
  2. 所有 Rust 包继续放在 `crates/`。
  3. 所有 JS 包放在 `packages/`。
  4. web 端拆成三层：
     - `crates/studio-web-wasm`：Rust crate，本体留在 Cargo workspace。
     - `packages/studio-web-wasm`：npm 包，只承接并分发上面 crate 产出的 wasm 和 js wrapper。
     - `packages/studio-web`：纯 TypeScript React PWA，使用 Vite 社区最佳实践、应用内路由、Zustand，依赖 `studio-web-wasm`。
  5. **transport 留在 TypeScript React PWA**，不放进 wasm；wasm 负责协议状态机暴露、共享 app-server client 对接、mesh 解码、renderer 控制。
  6. 设计系统要从 `/Users/krhougs/LocalCodes/buddin/README.md` 复制为项目内维护的 skill；app 布局参考 `/Users/krhougs/LocalCodes/buddin/ui_kits/app`。

## 关键澄清

- 用户在一次补充里写了“`studio-app-wasm` 是一个 rust crate”，结合前后文，这里按 **`studio-web-wasm`** 解释；若后续确认只是包名想保留 `studio-app-wasm`，影响的只是命名，不影响本计划中的边界和迁移顺序。
- 本次目标不再是“沿着 Rust DOM 壳继续堆功能”，而是先把 web 边界纠正，再在 React PWA 上完成功能补齐。
- Buddin 参考不是“灵感来源”，而是明确约束：暗色技术图纸风格、零圆角、1px hairline、Geist + Geist Mono、句式大小写、无 emoji、五区工作台（Topbar / Rail / Chat / Canvas / Inspector）。

## 当前代码事实（作为计划输入）

- `Cargo.toml` 当前成员里仍是 `crates/studio-web`。
- `crates/studio-web/Cargo.toml` 当前直接依赖 `app-server-transport`。
- `crates/studio-web/src/transport_port.rs` 当前在 wasm 内直接包了一层 `WebSocketClientTransport`。
- `crates/studio-web/src/app.rs` 当前在 Rust 侧持有 `AppServerClient<WebSocketAppServerTransportPort>`，并直接拼装 DOM 壳。
- `crates/studio-common/src/app_server_client.rs` 已经提供了跨端的 `AppServerTransportPort` trait 和 `AppServerClient<T>`，这说明共享 client 逻辑本来就有合适归属，不应该被重新塞回 web 专属 crate。

## 这份合并后计划的目标

在不破坏既有 app-server 架构的前提下，完成以下两件事：

1. **架构纠偏**：把当前 Rust DOM 壳重构成「Rust wasm 能力层 + npm wasm 包 + React PWA 壳」三层结构。
2. **功能补齐**：在新的 React PWA 壳上继续完成 web 端与 desktop 端的功能与界面对齐。
