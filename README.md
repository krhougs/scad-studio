# budn' (`budn`)

`budn'` 是一个 Web CAD 工作台；代码与配置标识符中统一使用 `budn`。当前生产 GUI 端是 `packages/studio-web`，通过 WebSocket 连接 app server，并共享 `app-server-protocol` 与 `studio-common` client 状态机。

## 坐标系约定

前端预览中用户看见和交互的空间，以及后端输出的所有 mesh 数据（包括 STL、3MF 和 protocol mesh payload），统一使用同一套项目坐标系：

- 右手系，满足 `+X × +Y = +Z`。
- `+X`：向右。
- `+Y`：向后，即板面内第二方向。
- `+Z`：向上，即层叠方向。
- `Top plane`：`XY`。
- `Front plane`：`XZ`。
- `Right plane`：`YZ`。

OpenSCAD 已经符合这套坐标系，不需要为了 Web 预览额外改写其输出轴向。前端相机、gizmo、网格、底板和坐标轴必须适配这套坐标系；未来其它 CAD 后端如果使用不同轴约定，才需要在对应 adapter / loader 边界转换到这套项目坐标系。

## 仓库结构

```
scad-studio/
├── crates/                        # Rust workspace
│   ├── app-server-protocol/       # 协议类型与线格式（ClientEnvelope/ServerEnvelope）
│   ├── app-server-core/           # 文件系统 I/O、OpenSCAD 调用、watch 聚合
│   ├── app-server-host/           # 可执行入口（websocket-host）
│   ├── app-server-transport/      # transport trait + WebSocket 客户端实现
│   ├── studio-common/             # 共享 client 状态机（ManagedClient）
│   ├── studio-web-wasm/           # wasm-bindgen 桥接（client / mesh / renderer）
│   └── scad-scene/                # mesh / STL / 3MF 纯数据能力
├── packages/                      # pnpm workspace（实际由 bun 驱动）
│   ├── studio-web-wasm/           # wasm 产物 npm 包（只 re-export generated/）
│   └── studio-web/                # React PWA：Vite 6 + React 18 + Zustand
├── scripts/                       # 所有 .ts 脚本由 bun 执行
├── tests/                         # 跨 crate 的 smoke 入口与 fixture workspace
├── docs/                          # 架构与设计文档
└── prompt-archives/               # 已归档的计划存档
```

## 快速开始

### 一次性准备

```bash
# Rust
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.117 --locked
cargo install wasm-pack                               # 仅跑 S1b smoke 需要

# JS
bun install
bun run --cwd packages/studio-web exec playwright install chromium   # 仅跑浏览器 smoke 需要
```

### 开发（热重载）

```bash
bun run web
```

启动两件事：
- `websocket-host` 进程，默认监听 `127.0.0.1:38421`
- Vite dev server，默认监听 `0.0.0.0:5173`

本机打开 `http://127.0.0.1:5173` 即可看到 `budn'` 五区工作台（Topbar / Rail / Chat / Canvas / Inspector）。同一局域网的设备可打开开发机 IP，例如 `http://192.168.1.20:5173`；前端默认通过同源路径 `/app-server/ws` 代理到 app server WebSocket，因此外部设备不需要额外添加 `?ws=`。

环境变量（全部可选）：

| 变量 | 默认 | 作用 |
|------|------|------|
| `SCAD_STUDIO_WS_URL` | `ws://127.0.0.1:38421` | websocket-host 绑定地址（完整 URL，端口从中解析） |
| `STUDIO_WEB_WORKSPACE` | `workspace/budn-web/` | host 的工作目录根；首次启动会自动创建 |
| `STUDIO_WEB_PORT` | `5173` | Vite dev 端口 |

显式覆盖 WebSocket 时仍可使用 `?ws=ws://host:port`，也可设置 `SCAD_STUDIO_WS_URL`。这两种方式会让前端直接连接指定地址；默认路径继续使用 `/app-server/ws` 代理，避免外部设备把 `127.0.0.1` 解析成设备自身。

单独启动：

```bash
bun run web:host   # 只启 websocket-host
bun run web:dev    # 只启 Vite dev（前端）
```

### 生产构建

```bash
bun run web:build
```

产物在 `packages/studio-web/dist/`：带 hash 的 wasm + Workbox Service Worker + `index.html`。`bun run --cwd packages/studio-web preview` 起静态服务器预览；Service Worker 仅在生产模式启用。

### 测试与 smoke

```bash
# 全量 web smoke（S1a rust unit → S1b wasm_bindgen → S1c wasm package diff
# → S2 browser → S3 watch → S4 pwa build）
bun run web:smoke

# 单条 case
bun run web:smoke -- --case browser_smoke
bun run web:smoke -- --case browser_watch_smoke
bun run web:smoke -- --case wasm_package_smoke
bun run web:smoke -- --case markdown_view        # Phase 6 扩展
bun run web:smoke -- --case image_view           # Phase 6 扩展
bun run web:smoke -- --case scad_preview         # Phase 6 扩展
bun run web:smoke -- --case canvas_interaction   # Phase 7 扩展
bun run web:smoke -- --case parameters_presets   # Phase 7 扩展
bun run web:smoke -- --case export_slicer        # Phase 7 扩展
bun run web:smoke -- --case config_settings      # Phase 7 扩展
bun run web:smoke -- --case scad_autorerender    # Phase 7 扩展

# Rust / TS 其它验证
cargo test --workspace --tests
bun run --cwd packages/studio-web typecheck
bun run --cwd packages/studio-web test:unit
bun run check:wasm-bindgen                       # 校验 Cargo.toml 与 CLI 版本对齐
```

## 进一步阅读

- `docs/getting-started.md`：完整安装流程、环境变量、故障排查
- `docs/architecture.md`：crate / package 能力边界与交互图
- `docs/design-system/studio-datasheet-workbench.md`：Buddin datasheet 设计规范
- `docs/web-platform-limits.md`：Web 平台约束
- `docs/known_issues.md`：已确认但当前 phase 不处理的协议 / 能力缺口
- `docs/feature-roadmap.md`：整体功能路线图
- `AGENTS.md`：项目协作规范（工具链、plan mode、架构长期约束）
- `prompt-archives/`：历次计划存档（不可变历史记录）

## 工具链选择

- Rust 构建：`cargo`
- JS 运行与测试：`bun`（唯一入口；`pnpm-workspace.yaml` 只作为 workspace 元数据，本项目不调用 `pnpm`）
- lockfile 策略：只提交 `Cargo.lock` 与 `bun.lock`；`pnpm-lock.yaml` 进 `.gitignore`
- Playwright 浏览器：chromium（`packages/studio-web/playwright.config.ts`）
- 浏览器端 wasm：`wasm-bindgen` 0.2.117 + Vite `vite-plugin-wasm` + `vite-plugin-top-level-await`

Python 禁止引入；所有脚本以 `bun` 运行 TypeScript。
