# Studio web 架构重构与功能对齐计划

## Context

当前仓库已经完成统一 app-server 架构，但 web 端停留在 MVP：`crates/studio-web` 同时承担了 wasm 产物、浏览器 transport、协议客户端、DOM 壳、样式和页面逻辑。这个形态与新的目标冲突：

- 新目标要求根目录同时成为 Cargo workspace 和 pnpm workspace（pnpm 仅承担 workspace 描述与依赖解析；脚本驱动仍以 bun 为主，详见总体目标 1 与风险表）。
- web 端必须拆成 Rust 能力层、npm wasm 包、React PWA 三层。
- transport 必须留在 TypeScript React PWA。
- 设计系统和应用布局需要改用 Buddin 的工作台规则，而不是继续在 Rust 里拼 HTML/CSS。

Oracle 复核后的结论是：**这个拆分方向是对的，但要避免把 `studio-common` 里的共享 client 状态机错误迁回 web 专属 crate。** 因此，本计划的核心原则是：

1. `studio-common` 继续持有跨端 `AppServerClient<T>` 和 `AppServerTransportPort` 语义。
2. `crates/studio-web-wasm` 负责把共享 client 能力、mesh 解码和 renderer 控制封装成 wasm API，对上提供给 TS 使用。
3. `packages/studio-web` 才是浏览器 transport、路由、Zustand UI 状态、设计系统和页面布局的真正归属。

## 当前基线

- 根 `Cargo.toml` 当前仅为 Cargo workspace，仍包含 `crates/studio-web`。
- 根 `package.json` 当前只有 `web` / `web:smoke` 两个脚本，仍由 bun 驱动。
- `scripts/run_studio_web.ts` 当前直接执行 `cargo build -p studio-web`、`wasm-bindgen`、静态服务和 `websocket-host`。
- `tests/studio_web_smoke.sh` 当前直接在 `crates/studio-web` 内执行 `wasm-pack test`。
- `crates/studio-web/src/app.rs` 当前直接拼接 HTML 和样式，现有 UI 结构与 Buddin 设计系统完全不一致。

## 差距分析

### 当前桌面端已具备但 web 端仍未具备的主要能力

| # | 功能域 | 具体能力 | 现状 |
|---|--------|----------|------|
| 1 | 文档标签系统 | 多标签打开/关闭/切换 | 当前 wasm 壳无正式标签系统 |
| 2 | 文件内容查看 | Markdown / 图片 / `.scad` 源码显示 | 当前只支持目录树 + 文件列表 + mesh 预览 |
| 3 | 3D 交互 | 轨道旋转、缩放、平移、相机预设 | 当前仅静态 canvas 预览 |
| 4 | 参数编辑 | 参数覆写、恢复默认值 | 当前无 |
| 5 | 预设管理 | 保存 / 加载 / 删除参数预设 | 当前无 |
| 6 | 导出与切片器信息 | ExportRun / SlicerList 接入 | 当前无 |
| 7 | 配置与设置 | ConfigLoad / ConfigSave / OpenSCAD 路径设置 | 当前无 |
| 8 | 正式工作台布局 | Topbar / Rail / Chat / Canvas / Inspector | 当前 Rust 壳是卡片式 MVP 布局 |

### 协议命令使用差距

当前 web 端只用了 `WorkspaceCurrent`、`WorkspaceList`、`PreviewRequest` 和 watch 订阅。以下命令服务端已实现，但 web 端完全未接入：

- `ConfigLoad`
- `ConfigSave`
- `FileRead`
- `FileWriteText`
- `SlicerList`
- `ExportRun`

这说明当前瓶颈主要在前端壳层，不在 app-server。

## 总体目标

1. 根目录新增 pnpm workspace 用于描述 JS 包成员关系；JS 脚本入口默认仍由 bun 驱动（详见 Phase 0 工具链策略）。
2. 现有 `crates/studio-web` 重构为 `crates/studio-web-wasm`。
3. 新增 `packages/studio-web-wasm`，作为内部 npm 包，分发 crate 构建出的 wasm/js wrapper。
4. 新增 `packages/studio-web`，作为纯 TypeScript React PWA，采用 Vite、React Router、Zustand、PWA 标准组织方式。
5. 设计系统整理为项目内 markdown 文档与 CSS token，应用布局按 Buddin 五区工作台构建（不新增任何 `.claude/skills/` 或 `agents/` 路径）。
6. 在新壳层上继续完成 web 端的功能与界面对齐。

## 非目标

- 不回退现有 `app-server-*` / `studio-common` / `studio-app` 已经完成的边界。
- 不在 wasm 内保留 WebSocket transport。
- 不把 React 的路由、布局和 CSS 逻辑放回 Rust。
- 不让 `packages/studio-web-wasm` 成为手写业务代码的主目录，它只是产物包装和分发入口。
- 不在 web 端尝试本地启动切片器进程；切片器部分只做信息展示与服务端能力接入。
- 不在 React 侧重新实现 `AppServerClient<T>` / `WorkspaceSession` / `PreviewState` / `DirectoryWatchLifecycle` / request id registry。

---

## Phase 通用规范（每个 Phase 必须执行）

每个 Phase 执行结束后，下列动作均为强制项，缺一不可：

1. **结果存档**：在 `prompt-archives/2026042300-studio-web-feature-parity/plan-00-result.md` 对应 Phase 行写入：完成情况、变更文件范围、验证结果、前序目标回归结果、遗留问题。
2. **已知问题同步**：若发现本轮无法解决但会影响后续 Phase 判断的问题，按 `AGENTS.md` 要求写入 `docs/known_issues.md`，包含发现时间、来源、原因、影响范围、可能解法、当前处理方式。
3. **独立 subagent review**：调用独立 subagent 进行 review，输入必须包含：当前 Phase 的目标与验收标准、完整 `plan-00.md`、本次变更的 diff 或文件清单。Review 中所有 P1 / block 问题修复完毕后才能进入下一 Phase。
4. **回归验证**：重新运行本 Phase 列出的所有验收命令；任意一项失败均视为本 Phase 未完成。

---

## Phase 0：架构契约与命名矩阵

### 目标

在动手改任何文件之前，先把后续 Phase 共享的所有名字、产物、桥接 API、工具链选择固定下来，避免 rename 与桥接设计在执行过程中分裂。

### 前序目标保护

- 不修改任何 Rust 或 JS 源文件。
- 不改动 Cargo workspace member 列表。
- 仅产出文档与契约定义。

### 输入

- 现有 `Cargo.toml`
- 现有 `crates/studio-web/Cargo.toml`
- 现有 `crates/studio-common/src/app_server_client.rs`
- `AGENTS.md` 工具链与边界约束

### 操作步骤

1. **命名矩阵**（写入 `prompt-archives/2026042300-studio-web-feature-parity/plan-00-naming.md`）：

   | 维度 | 取值 |
   |------|------|
   | Cargo package name | `studio-web-wasm` |
   | Rust lib crate name | `studio_web_wasm` |
   | crate-type | `["cdylib", "rlib"]` |
   | wasm-bindgen target | `bundler` |
   | wasm-bindgen out-dir | `packages/studio-web-wasm/generated/` |
   | wasm js wrapper 文件名 | `studio_web_wasm.js` |
   | wasm 二进制文件名 | `studio_web_wasm_bg.wasm` |
   | npm package name | `@scad-studio/studio-web-wasm` |
   | React PWA npm package name | `@scad-studio/studio-web` |
   | Vite import path | `@scad-studio/studio-web-wasm` |
   | `websocket-host` 启动归属 | `scripts/run_studio_web.ts`（dev / smoke 共用，禁止 Vite 隐式启动） |
   | `SCAD_STUDIO_WS_URL` 默认值 | `ws://127.0.0.1:38421` |

2. **wasm 桥接 API 契约**（写入 `prompt-archives/2026042300-studio-web-feature-parity/plan-00-bridge.md`）：

   - 必须暴露并仅暴露以下函数（命名以 `wasm_bindgen` 角度描述）：

     发起命令：
     - `client_dispatch_workspace_current(handle) -> RequestId`
     - `client_dispatch_workspace_list(handle) -> RequestId`
     - `client_dispatch_preview_request(handle, params: PreviewParams) -> RequestId`
     - `client_dispatch_file_read(handle, params: FileReadParams) -> RequestId`
     - `client_dispatch_file_write_text(handle, params: FileWriteParams) -> RequestId`
     - `client_dispatch_config_load(handle) -> RequestId`
     - `client_dispatch_config_save(handle, params: ConfigSaveParams) -> RequestId`
     - `client_dispatch_slicer_list(handle) -> RequestId`
     - `client_dispatch_export_run(handle, params: ExportParams) -> RequestId`
     - `client_subscribe_directory_watch(handle, params: WatchParams) -> RequestId`
     - 设计约束：每个命令对应一个 wasm export，禁止 React 自行序列化 envelope；命令的入参 / 出参类型由 wasm 端集中定义并通过 `wasm_bindgen` 导出。

     transport 接缝：
     - `client_create() -> ClientHandle`
     - `client_next_outbound(handle) -> Option<EnvelopeBytes>`
     - `client_receive_inbound(handle, bytes: EnvelopeBytes) -> Result<(), ClientError>`
     - `client_mark_transport_closed(handle, reason: TransportCloseReason)`
     - `client_cancel(handle, request_id: RequestId)`

     状态读取：
     - `client_drain_events(handle) -> Vec<ClientEvent>`
     - `client_snapshot(handle) -> ClientSnapshot`

     `ClientSnapshot` 至少包含：workspace tree、当前目录文件列表、预览任务状态、当前激活预览目标、预览错误、watch 生命周期摘要、最近一次错误。React 只引用 snapshot 或派生显示数据，不在 React 侧累积业务状态。

     渲染：
     - `mesh_decode(bytes: Bytes) -> MeshHandle`
     - `renderer_create(canvas_id: &str) -> RendererHandle`
     - `renderer_resize(handle, width: u32, height: u32, device_pixel_ratio: f32)`
     - `renderer_render(handle, mesh: MeshHandle, camera: CameraState)`
     - `renderer_destroy(handle)`

   - 错误模型：
     - `ClientError` 枚举：`DecodeError` / `UnknownRequest` / `TransportClosed` / `Cancelled` / `ProtocolError { code, message }`。
     - 命令派发函数同步返回 `RequestId`，所有失败结果通过 `client_drain_events` 中的 `RequestFailed { request_id, error }` 事件返回，不在 wasm 端抛 panic。

   - 超时策略：
     - 超时由 `studio-common` 内部实现（基于 inbound 序列号 + 计时事件输入）。
     - JS 端通过 `client_tick(handle, now_ms)` 定期推进 wasm 时间；wasm 在 `client_drain_events` 中产出 `RequestTimedOut`。
     - JS 端不维护 per-request 超时表。

   - watch 节流策略：
     - 仅在 `studio-common` / wasm 端做节流；TS 不重复节流。
     - 节流参数由 watch 订阅命令携带；默认值写入 `plan-00-bridge.md`。

   - reconnect 策略：
     - 已发送但未响应的请求：reconnect 后由 wasm 自动通过 `client_next_outbound` 重发，TS 不感知。
     - 未发送的请求（仍在 outbound 队列）：reconnect 后正常排队发送。
     - watch 订阅：reconnect 后由 wasm 自动重订阅；事件中产出 `WatchResubscribed { request_id }` 供 React 决定是否提示用户。

   - trait 适配审查：
     - 把 `studio-common::AppServerTransportPort` 的实际方法签名摘录到 `plan-00-bridge.md`。
     - 逐方法说明 wasm 适配器如何实现：
       - 同步本地队列方法直接实现。
       - 若 trait 定义了异步收发或返回 `impl Future`，先在 `studio-common` 增加 headless pump 方法（例如 `Client::pump(&mut self, now: Instant)`），由 wasm 在每次 inbound / tick 后调用；不在 wasm 中伪造 async transport。
   - 硬约束：
     - wasm 内部不持有任何 JS Promise，不通过 `wasm_bindgen_futures` 等待 JS 异步结果。
     - JS 端负责 WebSocket 生命周期、envelope 传递、watch push 投递。
     - wasm 端只维护 protocol client 状态机、mesh decode、renderer。
     - cancel / reconnect / watch push 全部通过上述固定 API 表达，禁止新增隐式状态出口。

3. **工具链策略**（写入 `prompt-archives/2026042300-studio-web-feature-parity/plan-00-toolchain.md`）：

   默认决策（无需在 Phase 0 等待用户确认）：

   - `pnpm-workspace.yaml` 仅作为 workspace 描述文件提交；本计划执行期间不调用 `pnpm install`、`pnpm run`、`pnpm exec`。
   - JS 安装、运行、构建、测试入口统一使用 `bun`：`bun install`、`bun run web`、`bun run web:build`、`bun run web:smoke`。
   - 仅提交两份 lockfile：`Cargo.lock` 与 `bun.lockb`；`pnpm-lock.yaml` 不提交，`.gitignore` 显式排除。
   - 禁止新增 `python` / `python3` 调用。
   - 若未来需要切换到 pnpm 主入口，另开计划并先修订 `AGENTS.md` 工具链约束。

4. **状态归属表**（写入 `prompt-archives/2026042300-studio-web-feature-parity/plan-00-ownership.md`）：

   | 状态种类 | 归属 crate / package |
   |----------|-----------------------|
   | `WorkspaceSession` / 当前 workspace / 当前目录文件列表 | `studio-common` |
   | 预览任务状态 / 当前激活预览目标 / 预览错误 | `studio-common` |
   | `DirectoryWatchLifecycle` | `studio-common` |
   | `AppServerClient<T>` / request id registry | `studio-common` |
   | mesh decode 结果（中间表示） | `studio-web-wasm` |
   | renderer handle / camera state | `studio-web-wasm` |
   | canvas DOM ref / route / 面板开关 / modal 状态 / 输入草稿 | `packages/studio-web` |

5. **smoke 矩阵**（写入 `prompt-archives/2026042300-studio-web-feature-parity/plan-00-smoke.md`），按 Phase 5 验收使用：

   | 编号 | 名称 | 入口命令 | 覆盖范围 | 退出码语义 |
   |------|------|----------|----------|-----------|
   | S1a | rust_unit_smoke | `cargo test -p studio-web-wasm` | wasm crate 在 host 下的单元测试（client 状态机、mesh decode 纯逻辑） | 非 0 视为失败 |
   | S1b | wasm_bindgen_smoke | `wasm-pack test --headless --chrome crates/studio-web-wasm` | wasm 在浏览器环境下的 wasm_bindgen exports / bridge 行为测试（watch push / request response / cancel / reconnect） | 非 0 视为失败 |
   | S1c | wasm_package_smoke | `bun run web:smoke -- --case wasm_package_smoke` | 验证 `@scad-studio/studio-web-wasm` 可被 `packages/studio-web` import；并验证 `packages/studio-web-wasm/generated/` 与重新生成的产物一致 | 非 0 视为失败 |
   | S2 | browser_smoke | `bun run web:smoke -- --case browser_smoke` | 启动 `websocket-host` + Vite，验证 `WorkspaceCurrent` / `WorkspaceList` / `PreviewRequest` | 非 0 视为失败 |
   | S3 | browser_watch_smoke | `bun run web:smoke -- --case browser_watch_smoke` | 验证 watch 推送进入 wasm client event 与 React 渲染 | 非 0 视为失败 |
   | S4 | pwa_build_smoke | `bun run web:build` | 构建 React PWA 并验证 wasm 资源被 Vite 正常引用 | 非 0 视为失败 |

   通用启动约束：

   - S2 / S3 启停 `websocket-host` 的端口由 `SCAD_STUDIO_WS_URL` 控制，所有 smoke 复用 `scripts/run_studio_web.ts` 内同一启动器。
   - S2 / S3 在 Playwright context 启动前必须清空 Service Worker 注册和 Cache Storage，并在测试开始时断言 `navigator.serviceWorker.getRegistrations()` 为空。
   - S1c 中 generated 一致性校验流程：
     1. 复制 `packages/studio-web-wasm/generated/` 到临时目录。
     2. 重新执行 wasm-bindgen 命令生成产物。
     3. 与临时目录逐文件比较，不一致直接失败。

6. **Buddin 设计参考可获取性兜底**（写入 `plan-00-toolchain.md` 末尾）：

   - Phase 4 输入文件位于 `/Users/krhougs/LocalCodes/buddin/...`。
   - 在 Phase 4 开始前，先检查这些路径是否可读：
     - 若不可读，暂停 Phase 4，请求用户提供材料，不进入实现。
     - 若可读，把所引用内容的摘要与 license / 来源信息写入 `docs/design-system/source-notes.md`。
   - Phase 4 之后，所有引用必须指向仓库内文档，不再依赖外部绝对路径。

### 验收标准

- `plan-00-naming.md`、`plan-00-bridge.md`、`plan-00-toolchain.md`、`plan-00-ownership.md`、`plan-00-smoke.md` 五份契约文件已存在。
- 命名矩阵中所有名称在后续 Phase 步骤中可被原文复用。
- `plan-00-naming.md` 必须包含完整 wasm 产物生成命令，至少：
  ```text
  cargo build -p studio-web-wasm --target wasm32-unknown-unknown --release
  wasm-bindgen target/wasm32-unknown-unknown/release/studio_web_wasm.wasm \
    --target bundler \
    --out-dir packages/studio-web-wasm/generated \
    --out-name studio_web_wasm
  ```
- 桥接 API 契约包含命令派发函数、`client_snapshot`、错误模型、超时策略、watch 节流、reconnect 策略、trait 适配审查，且不包含 wasm 内部等待 JS 异步的语义。
- 工具链策略采用默认决策（仅 bun + Cargo.lock + bun.lockb），无任何待用户确认的阻塞项。

---

## Phase 1：工作区与包边界重构

### 目标

按 Phase 0 命名矩阵建立 Cargo + pnpm 双工作区骨架，并把现有 `crates/studio-web` 改造成后续可持续演进的三层结构骨架。

### 前序目标保护

- 不破坏当前 Cargo workspace 的可编译状态。
- 不破坏现有 `websocket-host`、browser smoke、wasm build 入口，哪怕它们在本 Phase 结束时仍通过兼容脚本运行。
- 不改动 app-server 协议和 desktop 端代码。
- 不删除任何旧 DOM 壳代码（仅做位置移动，参见步骤 6）。

### 输入

- Phase 0 产出的五份契约文件。
- 根 `Cargo.toml`
- 根 `package.json`
- `scripts/run_studio_web.ts`
- `tests/studio_web_smoke.sh`
- `crates/studio-web/Cargo.toml`

### 操作步骤

1. 在根目录新增 `pnpm-workspace.yaml`，明确 `packages/*` 为 JS workspace 成员；根 `package.json` 增加 `workspaces` 描述字段并保留 bun 脚本入口（按 Phase 0 工具链策略，不调用 `pnpm install`）。`.gitignore` 显式排除 `pnpm-lock.yaml`。
2. 把 `crates/studio-web` 重命名为 `crates/studio-web-wasm`，按 Phase 0 命名矩阵同步更新：
   - Cargo workspace member 列表
   - Cargo package name 与 lib crate name
   - `crate-type`
   - `wasm-bindgen` 输出路径
   - `scripts/run_studio_web.ts` 中所有 `studio-web` 字符串引用
   - `tests/studio_web_smoke.sh` 中所有引用
   - `websocket-host` 命令参数中对 wasm 产物路径的引用
3. 新建 `packages/studio-web-wasm/`，目录与文件骨架按以下硬约束：
   - 仅允许：`package.json`、`README.md`、`generated/`、`src/index.ts`（仅 re-export `generated`）。
   - 禁止：任何 protocol 状态、WebSocket 代码、React 代码、renderer 业务封装。
   - `generated/` 默认提交（避免 CI 必须先跑 wasm-pack）；提交规则与版本配套写入 README。
   - generated 一致性由 S1c 校验，不依赖人工对账。
4. 新建 `packages/studio-web/`，目录骨架仅允许：
   - `package.json`、`tsconfig.json`、`vite.config.ts`、`index.html`
   - `src/`、`public/`、`tests/`
   - 暂不引入任何业务代码。
5. 固定 `wasm-bindgen` 与 `wasm-bindgen-cli` 版本：
   - `crates/studio-web-wasm/Cargo.toml` 中 `wasm-bindgen` crate 版本与 npm `wasm-bindgen-cli` 版本必须一致。
   - 在 `package.json` 的 `devDependencies` 中固定 `wasm-bindgen-cli` 来源（npm 包或脚本下载，二选一并写入 README）。
6. 旧 DOM 壳的过渡处理：
   - 在 `crates/studio-web-wasm/src/` 下新增 `legacy_dom_shell.rs`，把当前 `src/app.rs` 的 HTML / CSS / DOM 拼接逻辑整体移入，不修改逻辑。
   - `legacy_dom_shell` 与对应的旧 transport 引用全部置于 Cargo `feature = "legacy-shell"` 之下，默认 feature 不启用；`Cargo.toml` 中 `app-server-transport` 改为 `optional = true` 并仅在 `legacy-shell` feature 下启用。
   - 顶层 `lib.rs` 中所有 `legacy_dom_shell` 引用必须包裹 `#[cfg(feature = "legacy-shell")]`；新 wasm public API 不允许直接或间接引用 `legacy_dom_shell` 模块。
   - 验收：
     - `cargo check -p studio-web-wasm`（默认 feature）通过且不引入 `app-server-transport`。
     - `cargo check -p studio-web-wasm --features legacy-shell` 通过。
     - `rg "legacy_dom_shell" crates/studio-web-wasm/src` 命中行均位于 `#[cfg(feature = "legacy-shell")]` 块内。

### 验收标准

- 根目录同时存在 Cargo workspace 与 pnpm workspace 描述。
- `crates/studio-web-wasm` 在 Cargo workspace 中可见，`cargo check --workspace` 通过。
- `packages/studio-web-wasm` 与 `packages/studio-web` 被 `pnpm-workspace.yaml` 识别（仅作为元数据，不执行 pnpm 命令）。
- `bun run web` 与 `bun run web:smoke`（兼容路径，启用 `legacy-shell` feature）仍可运行旧能力。
- `legacy_dom_shell` 仅在 `legacy-shell` feature 下编译，未污染默认 feature 与新 wasm public API。
- `pnpm-lock.yaml` 已加入 `.gitignore`。

---

## Phase 2：抽出 headless wasm 能力层

### 目标

把当前 wasm 中与浏览器 transport、DOM 壳和样式耦合的部分剥离掉，按 Phase 0 桥接 API 契约实现 wasm 侧 headless 能力。

### 前序目标保护

- 不改变 `studio-common` 现有 `AppServerTransportPort` / `AppServerClient<T>` 的归属。
- 不破坏现有 `scad-scene` 的 wasm 渲染入口。
- 不让 transport 语义在 JS 和 wasm 两侧各自维护一份。
- Phase 1 的命名矩阵、`legacy_dom_shell` 边界继续生效。

### 输入

- Phase 0 桥接 API 契约（`plan-00-bridge.md`）。
- `crates/studio-web-wasm/src/lib.rs`
- `crates/studio-web-wasm/src/app.rs`（旧逻辑现位于 `legacy_dom_shell.rs`）
- `crates/studio-web-wasm/src/transport_port.rs`
- `crates/studio-web-wasm/src/preview_canvas.rs`
- `crates/studio-common/src/app_server_client.rs`

### 操作步骤

1. 默认 feature 下从 `crates/studio-web-wasm` 中删除对 `app-server-transport` 的引用：
   - 移除 `src/transport_port.rs` 及其它 WebSocket 包装类型在默认 feature 下的可见性。
   - `legacy_dom_shell` 内对 transport 的依赖一并放在 `#[cfg(feature = "legacy-shell")]` 下。
   - 在 `studio-common` 内（如果 trait 适配审查发现现有 `AppServerTransportPort` 无法无阻塞实现）增加 headless pump，例如 `Client::pump(&mut self, now: Instant)`，并把适配方式写回 `plan-00-bridge.md`。
2. 实现 Phase 0 契约中列出的命令派发与 transport 接缝 API：
   - 命令派发函数（`client_dispatch_*`）直接调用 `studio-common::AppServerClient<T>` 的对应方法，把请求写入 wasm-local outbound 队列。
   - `client_next_outbound` / `client_receive_inbound` / `client_mark_transport_closed` / `client_cancel` 全部基于 `studio-common` 已有 cancel / lifecycle API，不在 wasm 内重复实现。
   - `client_tick` 在 inbound / outbound / 周期性调用时推进 `studio-common` 内部超时与节流计算。
3. 实现 `client_drain_events` 与 `client_snapshot`：
   - `client_drain_events` 返回自上次调用后产生的事件序列，包含 `RequestSucceeded` / `RequestFailed` / `RequestTimedOut` / `WatchEvent` / `WatchResubscribed`。
   - `client_snapshot` 返回当前完整业务视图（按 `plan-00-bridge.md` 列出的字段），React 不在自己侧累积业务状态。
4. 实现 `mesh_*` 与 `renderer_*` API：
   - `MeshData` 解码通过 `mesh_decode`。
   - `Renderer::new_for_canvas(...)` 由 `renderer_create` 包装；resize / render / destroy 必须幂等，可被 React StrictMode 双 mount 安全调用。
   - renderer 不持有 protocol 状态，不调用任何 client_* API。
5. 把 `FakeChatState` 从 wasm 中移除（React 侧 Phase 3 处理）。
6. wasm crate 对外只暴露 Phase 0 契约 API；其它符号一律 `pub(crate)`。
7. 旧壳处理：
   - `legacy_dom_shell.rs` 仅在 `legacy-shell` feature 下保留并可编译运行；默认 feature 下不可达。
   - compat smoke 在 Phase 2 起停止使用旧壳，改为基于新 client API 的最小测试入口（具体测试命令属于 S1b）。
   - 验收：`rg "legacy_dom_shell" crates/studio-web-wasm/src` 命中行均位于 `#[cfg(feature = "legacy-shell")]` 块内；`cargo check -p studio-web-wasm`（默认 feature）不引入 `app-server-transport`。

### 验收标准

- 默认 feature 下 `crates/studio-web-wasm` 不再依赖 `app-server-transport`。
- wasm crate 不再持有任何 WebSocket 连接对象。
- wasm crate 暴露的 public API 与 Phase 0 契约文件一一对应。
- wasm crate 仍可完成：接收协议消息、推进共享 client 状态、解码 mesh、控制 canvas renderer。
- S1a (rust_unit_smoke) 通过：覆盖 client 状态机、mesh decode、错误模型等纯逻辑。
- S1b (wasm_bindgen_smoke) 通过；至少包含：
  - watch push → `client_drain_events` 出现 `WatchEvent`
  - request → response → `client_drain_events` 出现 `RequestSucceeded`
  - cancel 请求后再次接收响应不进入 `RequestSucceeded`，应进入 `RequestFailed { error: Cancelled }`
  - transport closed → reconnect 后未响应请求自动重发（`client_next_outbound` 再次输出该 envelope）
  - watch 订阅在 reconnect 后自动重订阅并产出 `WatchResubscribed`
  - request 超时通过 `client_tick` 推进，`client_drain_events` 出现 `RequestTimedOut`

---

## Phase 3：建立 React PWA 壳与 transport 层

### 目标

在 `packages/studio-web` 中建立纯 TypeScript React PWA，承担浏览器 transport、应用内路由、Zustand UI 状态和与 wasm 的桥接。

### 前序目标保护

- wasm 层不回退成 DOM 壳。
- `packages/studio-web-wasm` 继续保持内部产物包角色，不承接业务 UI。
- 不在 React 壳中复制 `WorkspaceSession` / `PreviewState` / `DirectoryWatchLifecycle` / `AppServerClient` / request id registry。
- canvas 生命周期遵循 Phase 0 契约。

### 输入

- Phase 0 命名矩阵、桥接契约、状态归属表。
- `packages/studio-web/package.json`
- `packages/studio-web-wasm/package.json`
- `crates/studio-web-wasm` 暴露的 wasm API
- `studio-common::AppServerTransportPort` 语义

### 操作步骤

1. 用 Vite 建立 React PWA 壳，启用 TypeScript、React Router、PWA 插件、严格 tsconfig。
   - 开发模式禁用 Service Worker（vite-plugin-pwa `devOptions.enabled = false`）。
   - 生产构建启用 hashed wasm 资源；构建产物中 wasm 文件名包含 hash。
2. 在 TypeScript 侧实现浏览器 transport：
   - WebSocket 生命周期（connect / close / reconnect / backoff）
   - envelope 序列化与投递（调用 wasm `client_next_outbound` / `client_receive_inbound`）
   - 错误回调与 `client_mark_transport_closed` 的接线
   - 所有 transport 状态仅作为 wasm 输入，不在 TS 侧维护并行业务状态
3. 实现一层最薄的 JS / wasm 适配层：
   - 在每次 wasm 状态可能变化后调用 `client_drain_events`（用于命令完成回调与一次性事件）和 `client_snapshot`（用于 React 渲染所需的业务视图）。
   - 业务命令统一调用 `client_dispatch_*` 函数，禁止 React 自行序列化 envelope。
   - 适配层只允许使用 Phase 0 契约 API。
   - 适配层周期性调用 `client_tick` 推进 wasm 内部超时与节流；调用频率至少 30 Hz 或基于 `requestAnimationFrame`。
4. Zustand 边界硬约束：
   - 仅保存 UI 壳状态（route、面板开关、选中标签、输入草稿、modal 开关）。
   - 不允许出现 `WorkspaceSession` / `PreviewState` / `DirectoryWatchLifecycle` / `AppServerClient` / request id registry 类型，也不允许在 TS 端定义同名状态机。
   - React 业务渲染数据来源唯一：`client_snapshot` 的只读引用或派生显示数据。
   - 加入 grep 验收：`rg "WorkspaceSession|PreviewState|DirectoryWatchLifecycle|AppServerClient|requestId" packages/studio-web/src` 必须人工确认没有协议状态机实现。
5. 建立 CanvasRendererController：
   - React 仅维护一个 canvas DOM ref。
   - controller 在 effect 中调用 `renderer_create`；卸载时调用 `renderer_destroy`；StrictMode 重复 mount 必须幂等。
   - 路由切换离开工作台时，必须销毁 renderer。
   - resize、device pixel ratio 变化通过 `renderer_resize` 接入。
   - 在 `tests/` 内增加最小组件级测试：StrictMode 双 mount 后 canvas 仍可渲染，resize 后无错误日志。
6. 建立应用内路由，但本 Phase 只挂载最少页面：
   - `/` 工作台
   - `/settings` 或等价设置页
   - 其它路径保持占位
7. 首个 React 页面只证明最小端到端流程：canvas 挂载、transport 接通、`WorkspaceCurrent` + `WorkspaceList` + `PreviewRequest` 成功。

### 验收标准

- `packages/studio-web` 是纯 TypeScript React PWA，不含 Rust 业务代码。
- transport 在 TS 侧工作，wasm 不再持有 WebSocket。
- React 页面能通过 wasm API 驱动 workspace 列表与预览。
- Zustand 不存在协议业务状态（grep 验收通过）。
- CanvasRendererController 通过 StrictMode 双 mount + resize 测试。
- dev 构建时无 Service Worker 注册；生产构建产物 wasm 带 hash。
- S2 (browser_smoke) 在新壳上通过。

---

## Phase 4：设计系统文档与五区工作台壳层

### 目标

把 Buddin 设计系统整理为项目内 markdown 文档与 CSS token，并按其五区工作台结构在 React PWA 里建立新的应用壳层。

### 前序目标保护

- 设计系统约束不进入 wasm。
- CSS token、布局和图标体系只在 React PWA 侧实现。
- 不新增任何 `.claude/skills/` 或 `agents/` 路径。
- GUI 跨端共享语义（颜色语义、字体 fallback、toolbar/statusbar 命名约定）必须评估是否进入 `scad-ui`，不允许默认全部塞进 web。

### 输入

- `/Users/krhougs/LocalCodes/buddin/README.md`
- `/Users/krhougs/LocalCodes/buddin/SKILL.md`
- `/Users/krhougs/LocalCodes/buddin/ui_kits/app/README.md`
- `/Users/krhougs/LocalCodes/buddin/ui_kits/app/index.html`
- `/Users/krhougs/LocalCodes/buddin/ui_kits/app/colors_and_type.css`

### 操作步骤

1. 在项目内新增设计系统文档（路径硬约束）：
   - `docs/design-system/studio-datasheet-workbench.md`：基于 Buddin README 与 SKILL 改写，保留以下硬约束：
     - dark-only
     - Geist + Geist Mono
     - sentence case
     - 零圆角
     - 1px hairline
     - 无 emoji、无渐变 CTA、无玻璃化滥用
   - 不创建 `.claude/skills/...` 或 `.agents/skills/...` 路径下的任何文件。
2. 在 `packages/studio-web/src/styles/` 下新增设计 token：
   - `tokens.css`：颜色、字体、间距、边框、阴影、动效变量。
   - `workbench.css`：五区布局基础样式。
3. 评估跨端共享语义：
   - 颜色语义、字体 fallback、toolbar / statusbar 命名等若可被 desktop（egui）复用，对应常量进入 `scad-ui`。
   - 若决定不进入 `scad-ui`，必须在 `docs/design-system/studio-datasheet-workbench.md` 中说明理由（例如 React/CSS 与 egui 不共享渲染管线）。
4. 在 React PWA 中实现五区工作台布局：
   - Topbar
   - Rail
   - Chat
   - Canvas
   - Inspector
5. 把 Phase 3 里临时挂载的目录树、文件列表、预览、假聊天迁移进新的五区壳层，禁止继续沿用旧 Rust 卡片式布局结构。
6. 明确 `studio-app` 与 `studio-web` 的关系：功能对齐继续追求，但页面组织形式以 Buddin 五区工作台为准，不再要求逐像素仿照 desktop 端旧布局。

### 验收标准

- 项目内存在 `docs/design-system/studio-datasheet-workbench.md` 与 `packages/studio-web/src/styles/tokens.css` / `workbench.css`。
- `rg "\.claude/skills|\.agents/skills" prompt-archives/2026042300-studio-web-feature-parity packages docs` 没有命中（除本计划禁止条款本身的引用外）。
- React PWA 壳层符合 Buddin 五区工作台结构。
- 旧的 Rust 卡片式 HTML 壳不再是前端主界面来源。
- 跨端共享评估有书面结论。

---

## Phase 5：迁移现有 MVP 功能到新壳

### 目标

先把当前已经完成的 web MVP 能力完整迁入新的 React + wasm 架构，确保不出现重构完成但功能回退。

### 前序目标保护

- 当前已完成的 workspace 树、文件列表、mesh 预览、watch 刷新、browser smoke 必须都保住。
- Phase 0–4 的命名矩阵、桥接 API、状态归属表、设计系统约束继续生效。

### 输入

- 现有 browser smoke
- 现有 `PreviewState` / `DirectoryWatchLifecycle`（位于 `studio-common`）
- 现有 `PreviewCanvasState`
- Phase 0 smoke 矩阵

### 操作步骤

1. 在新 React 壳里接回：
   - workspace tree
   - 当前目录文件列表
   - `.stl` / `.3mf` 预览
   - 假聊天占位
2. 把 `browser_smoke`、`browser_watch_smoke` 改造为新架构下的测试入口，并新增 `rust_unit_smoke` / `wasm_bindgen_smoke` / `wasm_package_smoke` / `pwa_build_smoke`，全部接入 Phase 0 smoke 矩阵：
   - S1a `cargo test -p studio-web-wasm`
   - S1b `wasm-pack test --headless --chrome crates/studio-web-wasm`
   - S1c `bun run web:smoke -- --case wasm_package_smoke`
   - S2 `bun run web:smoke -- --case browser_smoke`
   - S3 `bun run web:smoke -- --case browser_watch_smoke`
   - S4 `bun run web:build`
3. 更新构建脚本与 smoke 脚本，按以下顺序：
   - 先构建 Rust crate（`cargo build -p studio-web-wasm --target wasm32-unknown-unknown --release`）
   - 再使用 Phase 0 命名矩阵中的 wasm-bindgen 命令生成 wrapper 到 `packages/studio-web-wasm/generated/`
   - 再启动 `packages/studio-web`
   - `websocket-host` 由 `scripts/run_studio_web.ts` 启动，端口取自 `SCAD_STUDIO_WS_URL`
   - S2 / S3 启动 Playwright context 前清空 Service Worker 注册和 Cache Storage
4. 让新壳在功能上达到旧 MVP 的等价状态后，确认默认 feature 下不再编译 `legacy_dom_shell`：
   - 默认 `bun run web` / `bun run web:smoke` 不启用 `legacy-shell` feature。
   - `legacy_dom_shell.rs` 与对应 `legacy-shell` feature 在 Phase 8 删除。

### 验收标准

- S1a / S1b / S1c / S2 / S3 / S4 六个 smoke 全部通过。
- 旧 MVP 功能在新架构下等价可用（每条能力对应一个 smoke 或组件测试，禁止仅靠主观判断）。
- 默认 feature 下 `cargo check -p studio-web-wasm` 不引入 `app-server-transport`，且不编译 `legacy_dom_shell`。

---

## Phase 6：文档与内容查看能力补齐

### 目标

在新的 React PWA 壳中补齐当前缺失的文档型能力：标签系统、Markdown 查看、图片查看、`.scad` 源码与预览联动。

### 前序目标保护

- Phase 0–5 的新边界不被回退。
- 不能为了快而把新页面结构重新塞回 wasm。

### 操作步骤与验收（按能力分项，每项独立验收）

1. **文档标签系统**：
   - 操作：实现多标签打开、关闭、切换、聚焦；保存当前打开标签到 Zustand UI 状态。
   - 验收：组件级测试覆盖打开 3 个文件、关闭中间标签、刷新后状态保留策略明确（默认会话内保留，刷新清空且写入 README）。
2. **Markdown 查看**：
   - 操作：通过 `client_*` API 触发 `FileRead`；将返回内容渲染为 markdown，至少支持标题、列表、代码块、链接、行内代码。
   - 验收：S2 扩展用例：`browser_smoke -- --case markdown_view` 通过。
3. **图片查看**：
   - 操作：通过 `FileRead` 读取 png / jpg；提供缩放与平移；超大图加 sanity check。
   - 验收：S2 扩展用例：`browser_smoke -- --case image_view` 通过。
4. **`.scad` 源码 + 预览双视图**：
   - 操作：源码视图 + 预览视图共享同一标签；`PreviewRequest` 错误显示在源码视图。
   - 验收：S2 扩展用例：`browser_smoke -- --case scad_split_view` 通过。
5. **文件点击行为扩展**：
   - 操作：`.stl` / `.3mf` 与上述新类型共享统一打开入口。

### 验收标准

- Markdown、图片、`.scad` 文件在 React 壳中都能作为正式文档标签打开。
- `FileRead` 在 web 端正式进入主流程。
- 上述每条能力均有对应 smoke / 组件测试且通过。

---

## Phase 7：3D 交互、参数、预设、导出与设置

### 目标

在新的 React 壳中继续完成当前缺失的交互与工作流能力，达到对 desktop 的主要功能对齐。

### 前序目标保护

- Phase 0–6 的架构边界与 MVP 能力不回退。

### 操作步骤与验收（按子能力拆，每子能力独立验收）

1. **3D 交互**：
   - 操作：轨道旋转、缩放、平移、相机预设、工具栏、状态栏。
   - 验收：S2 扩展用例 `browser_smoke -- --case canvas_interaction`：完成预设切换 + 旋转输入 + resize 后 canvas 正常。
2. **参数编辑与预设管理**：
   - 操作：参数覆写、恢复默认值、预设保存 / 加载 / 删除。
   - 验收：S2 扩展用例 `browser_smoke -- --case parameters_presets`：覆盖 3 条预设的增删读。
3. **导出与切片器信息**：
   - 操作：接入 `ExportRun`、`SlicerList`。
   - 验收：S2 扩展用例 `browser_smoke -- --case export_slicer`。
4. **配置与设置**：
   - 操作：接入 `ConfigLoad`、`ConfigSave`、OpenSCAD 路径设置。
   - 验收：S2 扩展用例 `browser_smoke -- --case config_settings`：写入 → 重新加载验证。
5. **日志面板与 `.scad` 自动重渲染**：
   - 操作：watch 推送驱动重渲染；显示最近 N 条日志。
   - 验收：S3 扩展用例 `browser_watch_smoke -- --case scad_autorerender`。
6. **平台限制项明示**：
   - 在 `docs/design-system/studio-datasheet-workbench.md` 或单独的 `docs/web-platform-limits.md` 中列明：浏览器无法直接启动本地 OpenSCAD、文件路径可见性差异、Service Worker 缓存对 wasm 更新的影响，以及对应处理方式。

### 验收标准

- 所有上述子能力对应的 smoke 用例通过。
- `ConfigLoad`、`ConfigSave`、`FileWriteText`、`SlicerList`、`ExportRun` 全部进入 web 主流程。
- 平台限制项已书面明示，禁止用“平台限制”作为掩盖未完成能力的说辞。

---

## Phase 8：清理旧 web 壳与统一脚本

### 目标

在新架构稳定后，删除旧的 Rust DOM 壳和过渡脚本，统一仓库内的 web 开发与测试入口。

### 前序目标保护

- 不删除仍被 smoke 或生产入口使用的文件。
- 删除前先确保新入口已经完全接管。

### 操作步骤

1. 删除 `crates/studio-web-wasm/src/legacy_dom_shell.rs` 及其相关 compat feature 开关、`FakeChatState` 等遗留类型。
2. 检查脚本：根 `package.json`、前端脚本、测试脚本统一改成 bun 主入口（按 Phase 0 工具链策略）；只在用户确认必要的位置使用 pnpm。
3. 更新 README / 计划存档中的 web 启动口径。
4. 补齐最终 smoke 与构建命令矩阵：
   - `cargo check --workspace`
   - `cargo test --workspace`
   - `bun install`
   - `bun run web` / `bun run web:build` / `bun run web:smoke`
   - S1a / S1b / S1c / S2 / S3 / S4 全通过
5. 验证清理彻底：
   - `rg "legacy_dom_shell|FakeChatState|inner_html" crates/studio-web-wasm packages/studio-web` 无业务命中。
   - `rg "app-server-transport" crates/studio-web-wasm` 无命中。
   - `rg "feature *= *\"legacy-shell\"" crates/studio-web-wasm` 无命中。

### 验收标准

- 仓库内不存在仍在使用的 Rust DOM 壳。
- JS 依赖管理、前端构建、web smoke 的主线入口统一清晰。
- 所有清理性 grep 验收通过。

---

## 主要风险

1. **共享 client 状态机归属出错**
   - 风险：把 `studio-common` 里的共享 client 逻辑搬成 web 专属实现。
   - 处理：坚持 `studio-common` 持有 `AppServerClient<T>` 语义，wasm 只做桥接与导出；Phase 3 grep 验收检查。

2. **JS / wasm 接缝过厚**
   - 风险：reconnect、watch、cancel、错误恢复在 TS 和 wasm 两边各维护一份。
   - 处理：Phase 0 桥接 API 契约把接缝压到 envelope / callback 级别，wasm 不复制状态机。

3. **React 与 renderer 生命周期冲突**
   - 风险：React 重渲染或 StrictMode 双 mount 把 canvas 重建，导致 wasm renderer 状态丢失。
   - 处理：Phase 3 实现 CanvasRendererController + 幂等 renderer_create / destroy + 测试覆盖。

4. **pnpm 与 bun 双主线混乱**
   - 风险：安装与运行命令入口分裂，CI 难以维护。
   - 处理：Phase 0 工具链策略默认 bun-only，`pnpm-workspace.yaml` 仅作元数据；本计划不调用任何 pnpm 命令。

5. **lockfile 漂移**
   - 风险：`Cargo.lock`、`bun.lockb`、`pnpm-lock.yaml` 共存导致依赖在本地与 CI 不一致。
   - 处理：仅提交 `Cargo.lock` 与 `bun.lockb`；`pnpm-lock.yaml` 进入 `.gitignore`；CI 增加 lockfile drift 校验。

6. **`wasm-bindgen` 版本漂移**
   - 风险：`wasm-bindgen` crate 版本与 CLI 版本不一致，构建通过但运行时异常。
   - 处理：Phase 1 固定两侧版本；CI 增加版本一致性校验。

7. **PWA Service Worker 缓存旧 wasm**
   - 风险：dev 模式下 Service Worker 缓存旧 wasm，修复后浏览器仍加载旧二进制，smoke 不稳定。
   - 处理：Phase 3 dev 模式禁用 Service Worker；生产构建使用 hashed wasm；smoke 启动前清空 Cache Storage。

8. **`.claude/skills/` 与 `agents/` 路径误用**
   - 风险：误把设计系统整理进 AI skill 路径，违反本轮约束。
   - 处理：Phase 4 路径硬约束；grep 验收。

## 结论

这次重构不是给当前 `crates/studio-web` 换一层样式，而是把 web 端从“Rust 直接拼页面”迁到“Rust 提供能力、React 负责产品壳”。先用 Phase 0 把契约固定下来，再改边界，再迁 MVP，再继续追功能与界面对齐，才是风险最低的执行顺序。
