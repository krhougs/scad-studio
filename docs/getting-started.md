# Getting started

本文档面向首次拉起仓库的工程师，覆盖安装、常用开发 / 构建 / 测试入口、环境变量、故障排查。

## 1. 前置依赖

### 1.1 Rust 工具链

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.117 --locked
```

`wasm-bindgen-cli` 版本必须与 `crates/studio-web-wasm/Cargo.toml` 里的 `wasm-bindgen` crate 版本严格一致；否则生成的 js wrapper 会与运行期 wasm 不兼容。执行 `bun run check:wasm-bindgen` 可随时校验两侧版本是否对齐。

### 1.2 JS 工具链

```bash
bun install
```

`bun` 会读取根 `package.json` 的 `workspaces` 字段与 `pnpm-workspace.yaml`，识别 `packages/studio-web` 与 `packages/studio-web-wasm` 两个成员，并把 `workspace:*` 内部引用解析到本地目录。`pnpm-workspace.yaml` 只做元数据声明，本项目不调用 `pnpm install`。

### 1.3 Playwright 浏览器

```bash
bun run --cwd packages/studio-web exec playwright install chromium
```

运行统一 smoke 的 S1b / S2 / S3 以及 Phase 6-7 扩展用例时需要。首次下载约 200 MB；容器环境请一并执行 `bun run --cwd packages/studio-web exec playwright install-deps chromium` 安装系统依赖。

## 2. WASM 包关系与构建流水线

项目有两个 WASM crate，理解它们的关系对避免构建不同步至关重要。

### 2.1 两个 WASM 包

| | `app-server-protocol-wasm` | `studio-web-wasm` |
|---|---|---|
| **角色** | 协议层的轻量 WASM 绑定 | web 应用的实际运行时 WASM |
| **暴露内容** | `protocol_encode_*` / `protocol_decode_*` 纯函数 | `client_create` / `client_receive_inbound` / `client_tick` 等完整客户端状态机 |
| **内嵌的 Rust 依赖** | `app-server-protocol` | `app-server-protocol` + `studio-common`（ManagedClient 握手/重连/watch 状态机）+ `scad-scene`（mesh 解码） |
| **wasm-bindgen target** | `--target web` | `--target bundler` |
| **输出位置** | `packages/app-server-protocol/generated/` | `packages/studio-web-wasm/generated/` |
| **使用场景** | smoke test、调试工具 | **web 应用运行时**（`@budn/studio-web` 导入） |
| **构建命令** | `bun run protocol:build` | `cargo build -p studio-web-wasm --target wasm32-unknown-unknown --release` + `wasm-bindgen ...` |

两者都在编译期嵌入 `app-server-protocol` 的 Borsh 序列化代码。**协议类型变更后，两个 WASM 包和 server 二进制三者必须同步重建**，否则 Borsh 结构体布局不匹配会导致运行时反序列化失败。

### 2.2 构建依赖图

```
crates/app-server-protocol    （协议类型 + Borsh 编解码）
  ├─► crates/app-server-protocol-wasm  → packages/app-server-protocol/generated/*.wasm
  ├─► crates/studio-common             （ManagedClient 状态机）
  │     └─► crates/studio-web-wasm     → packages/studio-web-wasm/generated/*.wasm
  └─► crates/app-server-host           （websocket-host 二进制）
```

三条路径的产物必须来自同一份 protocol 源码，任何一条产物过时都会导致 wire frame 解码失败。

### 2.3 生产构建

```bash
bun run web:build
```

`scripts/build_studio_web.ts` 按顺序执行：

1. `cargo build -p studio-web-wasm --target wasm32-unknown-unknown --release`
2. `wasm-bindgen ... --target bundler --out-dir packages/studio-web-wasm/generated/`
3. `bun run build`（Vite 生产构建，输出到 `packages/studio-web/dist/`）

生产构建**不含** `app-server-protocol-wasm` 和 `websocket-host`。protocol WASM 仅用于测试；server 二进制由部署环境独立编译。

单独重建某个 WASM 包：

```bash
# protocol WASM（测试/调试用）
bun run protocol:build

# studio-web WASM（web 运行时）
cargo build -p studio-web-wasm --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/studio_web_wasm.wasm \
  --target bundler \
  --out-dir packages/studio-web-wasm/generated \
  --out-name studio_web_wasm
```

## 3. 日常开发

### 3.1 拉起 web 工作台（默认）

```bash
bun run web
```

背后做三件事（`scripts/run_studio_web_dev.ts`）：

1. **WASM 初始构建 + 监听**：并行构建 `app-server-protocol-wasm` 和 `studio-web-wasm` 两个 WASM 包，然后监听 `crates/{app-server-protocol,app-server-protocol-wasm,studio-web-wasm,studio-common,scad-scene}/src/` 下的 `.rs` 文件变更，变更时自动重建两个 WASM。
2. **websocket-host**：`cargo run -p app-server-host --bin websocket-host`（cargo 会自动增量编译，包括 protocol 变更）。
3. **Vite dev server**：`bun x vite --port <STUDIO_WEB_PORT> --host 0.0.0.0`，带 WebSocket 代理。

本机浏览器访问 `http://127.0.0.1:5173`。Vite 默认监听 `0.0.0.0`，同一局域网的设备可访问开发机 IP，例如 `http://192.168.1.20:5173`。

默认情况下，前端 WebSocket 连接走同源代理路径 `/app-server/ws`，Vite 会把该路径转发到本机 `websocket-host`。这可以避免外部设备访问 dev server 时把 `127.0.0.1` 解析成设备自身。

### 3.2 单独启动

| 脚本 | 作用 |
|------|------|
| `bun run web:host` | 仅启 websocket-host |
| `bun run web:dev` | 仅启 Vite dev（不含 host） |
| `bun run web:build` | 生产构建：cargo 编 wasm → wasm-bindgen → vite build |
| `bun run --cwd packages/studio-web preview` | 生产构建后本地静态预览（Service Worker 在此模式下启用） |
| `cargo run -p studio-app` | 启动桌面端 |
| `cargo run -p app-server-host --bin websocket-host -- --workspace <path> --bind 127.0.0.1:38421` | 手动启 websocket-host（不经 `scripts/`） |

### 3.3 环境变量

脚本读取的环境变量：

| 变量 | 消费者 | 默认值 | 说明 |
|------|--------|--------|------|
| `SCAD_STUDIO_WS_URL` | `run_websocket_host.ts` / `run_studio_web_dev.ts` / Vite | `ws://127.0.0.1:38421` | 完整 WebSocket URL；host 从中解析 host/port。显式设置后，前端直接连接此地址 |
| `STUDIO_WEB_WORKSPACE` | `run_websocket_host.ts` | `workspace/studio-web/` | websocket-host 的根工作目录；首次启动会自动 `mkdir -p` |
| `STUDIO_WEB_PORT` | `run_studio_web_dev.ts` | `5173` | Vite dev server 端口 |
| `VITE_WS_URL` | Vite | 空 | 前端直接连接的 WebSocket URL；优先级低于 URL 参数 `?ws=...`，高于同源代理 fallback |
| `VITE_WS_PROXY_TARGET` | Vite dev server | `ws://127.0.0.1:38421` | `/app-server/ws` 的代理目标，通常由 `bun run web` 自动注入 |

手动试验不同工作目录：

```bash
STUDIO_WEB_WORKSPACE=/abs/path/to/my-workspace \
SCAD_STUDIO_WS_URL=ws://127.0.0.1:38888 \
bun run web
```

常见连接检查：

- 本机访问失败：确认 `websocket-host` 日志中监听地址与 `SCAD_STUDIO_WS_URL` 一致。
- 局域网设备访问页面成功但一直 connecting：优先保持默认代理路径 `/app-server/ws`；如果使用了 `?ws=` 或 `SCAD_STUDIO_WS_URL`，需要确保该地址对外部设备可达。
- 端口被占用：修改 `STUDIO_WEB_PORT` 或 `SCAD_STUDIO_WS_URL` 中的端口。

## 4. 测试矩阵

### 4.1 Rust 测试

```bash
cargo test --workspace --tests
```

包含（非详尽）：

- `app-server-protocol` envelope / protocol 序列化
- `app-server-transport` 往返 + WebSocket 编码解码
- `studio-common::managed_client`（15+ 测试）—— 握手 / 取消 / 超时 / watch 节流 / reconnect
- `studio-web-wasm` 纯函数 `mesh_decode`

### 4.2 Playwright wasm bridge smoke（S1b）

```bash
bun run web:smoke -- --case wasm_bridge_smoke
```

该用例启动 `websocket-host` + Vite，通过真实浏览器页面捕获 browser wasm bridge 发出的 WebSocket binary frame，并用 `@budn/app-server-protocol` 解码，覆盖 handshake 与 `workspace.current` 的 Borsh frame 边界。旧的 `wasm-pack test --headless --chrome crates/studio-web-wasm --test wasm_bridge_smoke` 可作为手动补充，但不属于默认 `web:smoke` 链路。

### 4.3 统一 smoke dispatcher

```bash
bun run web:smoke                                       # 全量 S1a→S4
bun run web:smoke -- --case <name>                      # 单条
```

可用的 `<name>`：

| case | 对应 phase | 入口 |
|------|-----------|------|
| `wasm_bridge_smoke` | Phase 5 S1b | `packages/studio-web/tests/playwright/wasm-bridge-smoke.spec.ts` |
| `wasm_package_smoke` | Phase 5 S1c | `scripts/smoke/wasm_package_smoke.ts`（generated/ diff） |
| `browser_smoke` | Phase 3 S2 | `packages/studio-web/tests/playwright/browser-smoke.spec.ts` |
| `browser_watch_smoke` | Phase 5 S3 | `packages/studio-web/tests/playwright/browser-watch-smoke.spec.ts` |
| `markdown_view` / `image_view` / `scad_viewer` | Phase 6 | browser-smoke.spec.ts 内的标签子集 |
| `canvas_interaction` | Phase 7 | `tests/playwright/canvas-interaction.spec.ts` |
| `parameters_presets` | Phase 7 | `tests/playwright/parameters-presets.spec.ts` |
| `export_slicer` | Phase 7 | `tests/playwright/export-slicer.spec.ts` |
| `config_settings` | Phase 7 | `tests/playwright/config-settings.spec.ts`（用 tmp HOME 隔离写入） |
| `scad_autorerender` | Phase 7 | browser-watch-smoke.spec.ts 内的 `@scad-autorerender` 标签 |

### 4.4 TypeScript 快速检查

```bash
bun run --cwd packages/studio-web typecheck      # tsc --noEmit
bun run --cwd packages/studio-web test:unit      # vitest（Markdown 安全 / preset-io / camera-controls 等纯函数）
bun run check:wasm-bindgen                       # 校验 CLI 与 Cargo.toml 版本一致
```

## 5. 最小冒烟链

第一次拉代码后，把这套跑通就可以放心动代码：

```bash
cargo check --workspace && \
bun run check:wasm-bindgen && \
bun run web:build && \
bun run web:smoke
```

大约 6–10 分钟（大头是 wasm 构建 + Playwright cold start）。之后增量跑视改动面挑单条 smoke。

## 6. 故障排查

### 6.1 `bun install` 失败 / 解析不到 `@budn/studio-web-wasm`

- 确认在仓库根目录执行，不是在 `packages/studio-web`。
- `.gitignore` 会忽略 `pnpm-lock.yaml` 与 `node_modules/`，仓库提交的是 `bun.lock`。
- 如果本地存在 `pnpm-lock.yaml`，删除后重跑 `bun install`。

### 6.2 `web:smoke` S1b 报 Playwright 浏览器启动失败

- 确认已执行 `bun run --cwd packages/studio-web exec playwright install chromium`。
- 容器环境同时执行 `bun run --cwd packages/studio-web exec playwright install-deps chromium`。
- 若仍失败，先单独运行 `bun run web:smoke -- --case wasm_bridge_smoke`，再查看 Playwright trace。

### 6.3 `web:smoke` S1c 报 generated/ drift

- 含义：当前工具链重新生成的 wasm-bindgen 产物与 `packages/studio-web-wasm/generated/` 里提交的版本不一致。
- 确认 `bun run check:wasm-bindgen` 通过（版本对齐）。
- 版本对齐但仍 drift，说明 crate 里新增 / 移除了 `#[wasm_bindgen]` 项（或改变了 `src/lib.rs` 的模块顺序），需要把新 generated 提交入库。
- 仅执行一次重新生成：
  ```bash
  cargo build -p studio-web-wasm --target wasm32-unknown-unknown --release
  wasm-bindgen target/wasm32-unknown-unknown/release/studio_web_wasm.wasm \
    --target bundler \
    --out-dir packages/studio-web-wasm/generated \
    --out-name studio_web_wasm
  ```

### 6.4 运行时 `InvalidWireFrame` / Borsh 反序列化失败

典型报错：`inbound binary frame: InvalidWireFrame: 解码 wire frame 失败: invalid utf-8 sequence of N bytes from index M`

**原因**：`studio-web-wasm` WASM 二进制与 `websocket-host` 的 Borsh 结构体布局不一致。最常见场景是修改了 `crates/app-server-protocol/src/protocol.rs` 中的协议类型（新增/删除/重排字段），但只重建了 server 二进制而没有重建 WASM。`cargo run` 会自动增量编译 server，但 WASM 需要手动触发。

**解决**：

```bash
# 重建 studio-web-wasm
cargo build -p studio-web-wasm --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/studio_web_wasm.wasm \
  --target bundler \
  --out-dir packages/studio-web-wasm/generated \
  --out-name studio_web_wasm

# 重建 protocol-wasm（如果 smoke test 也报错）
bun run protocol:build
```

然后刷新浏览器。如果使用 `bun run web` 启动（而非手动单独启动），dev 脚本会在启动时自动构建两个 WASM 包并监听源码变更。

**Agent 注意**：修改 `app-server-protocol` 中任何 `BorshSerialize` / `BorshDeserialize` 类型后，必须确保两个 WASM 包都已重建。直接用 `bun run web` 启动 dev 环境即可自动处理；若手动管理进程，需要分别重建。

### 6.5 `bun run web` 打开页面是空白

- 浏览器 devtools 看 Network：`studio_web_wasm_bg.wasm` 是否 200。
- 看 Console：`TS2307` / 模块找不到多半意味着 `generated/` 里 wasm-bindgen 产物缺失 —— `bun run web:build` 跑一次会生成。
- 连接状态：Topbar 右上应看到 `online`；若长期 `connecting`，检查 `websocket-host` 日志和 `/app-server/ws` 代理请求；若 `error`，检查 `SCAD_STUDIO_WS_URL` 与 `host` 实际监听地址是否一致。

### 6.6 Playwright smoke `port not ready`

- 大概率是 `cargo run -p app-server-host` 冷启动太慢。把 `target/` 编一次（`cargo build -p app-server-host --bin websocket-host`）再跑 smoke。
- 端口被占用：各 spec 使用不同端口（`_smoke-harness.ts` 里配置），清理残留进程 `lsof -i:<port>` + `kill`。

### 6.7 `@config-settings` 写到用户真实配置

- 正常不会：spec 已用 tmp `HOME` 隔离。如果发现自己的 `~/Library/Application Support/scad-studio/` 或 `~/.config/scad-studio/` 被写入，说明 `hostEnv` 没生效或 spec 没走 harness —— 回看 `packages/studio-web/tests/playwright/config-settings.spec.ts` 是否完整拷贝了 `TMP_HOME` + `CARGO_HOME` 配置。

## 7. 下一步

- `docs/architecture.md`：了解 crate / package 能力边界再动手。
- `docs/design-system/studio-datasheet-workbench.md`：改 UI 前读。
- `docs/known_issues.md` + `docs/web-platform-limits.md`：知道哪些差异是有意为之。
- `AGENTS.md`：提 PR / 改协议 / 新建 plan 前必读。
