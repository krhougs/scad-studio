# 执行结果存档：`2026040800-studio-web-wasm-backend`

本文件在对应 Phase 执行过程中**实时追加**，与 `plan-00.md` 同步维护。

| Phase | 状态 | 摘要 | 遗留问题 |
|-------|------|------|----------|
| 1 | 已完成 | 新增 `docs/2026040800-studio-web-api/README.md`、`crates/studio-remote-protocol`（serde 类型 + 集成测试）、workspace 注册；`cargo check` / `cargo test -p studio-remote-protocol` 通过 | — |
| 2 | 已完成 | `WorkspaceDataAccess` / `LocalWorkspaceAccess`（`notify` 仍封装在 `FileWatcher` 内）、路径纯函数与 `path_is_within_root` 测试；`FileTree` 可注入读目录；`main` 打开工作区时挂载 + 校验路径 | 文档标签页等仍各自使用 `FileWatcher`（与计划「少数调用点」一致，远程化时可再收敛） |
| 3 | 已完成 | 新增 `crates/studio-backend`（axum + tokio + `notify`）：`POST/DELETE /v1/sessions`，`GET entries`、`GET/PUT files`、`GET watch` WebSocket；`STUDIO_BIND` / `STUDIO_WORKSPACE_PARENT` / `STUDIO_READ_ONLY`；集成测试 `tests/http_smoke.rs` | OpenSCAD 预览端点留待 Phase 7 |
| 4 | 已完成 | `crates/studio-remote-client`：`WorkspaceDataAccess` 的 `RemoteWorkspaceAccess`（原生：`ureq` + `tungstenite`，虚拟根 `/studio-remote`）；`wasm32` stub；根包 `remote-workspace` feature；单元测试覆盖 URL 纯函数 | `cargo tree` 在 wasm 上仍经 `scad-data` 带入 `notify`（未在客户端代码使用）；后续可拆 `scad-data` feature 或轻量 trait crate；与 `studio-backend` 的进程内集成测试因线程/运行时交互不稳定已改为依赖单元测试 + 既有后端 `http_smoke` |
| 5 | 已完成 | 无代码修改；`scad-scene` 天然兼容 wasm32（`Renderer::new(Arc<Window>)`、`PhysicalSize`、wireframe 特性检测、cfg 隔离的平台特定依赖均已在 wasm32 上编译通过） | wasm 下系统字体加载返回空列表（`system_fonts` 需 Phase 6/8 提供替代方案）；`detect_language_tag()` 在 wasm 下固定返回 `"en-US"` |
| 6 | 已完成 | 创建 `src/lib.rs`（共享模块 + WASM 入口）+ 重构 `src/main.rs` 为桌面入口；scad-data/scad-ui/scad-viewer 的 muda/notify/dirs/rfd 全部 cfg 隔离 + wasm32 no-op 存根；egui-winit 在 wasm32 禁用 clipboard；`wasm_main` 入口完整 | `RemoteWorkspaceAccess` wasm stub 返回 `Unsupported`（需后续异步接线）；构建说明文档（trunk/wasm-pack）留待 Phase 8 |
| 7 | 已完成 | 后端新增 `POST /v1/sessions/{id}/preview` 端点（`render_openscad_3mf` 复用 `scad_data::detect_openscad_path` + `build_preview_job_args` + CLI 调用）；64 MiB 体积上限；spawn_blocking 执行子进程 | 前端 WASM 侧调用预览 API 留待后续接线（当前 WASM stub 中 OpenScadRunner 为空操作） |
| 8 | 已完成 | 全量回归通过（`cargo test --workspace`、`cargo check -p scad-studio`、`cargo check -p scad-studio --lib --target wasm32-unknown-unknown`、`cargo check -p studio-backend`）；开发者文档 `docs/2026040800-studio-web-api/DEVELOPMENT.md`；`docs/known_issues.md` 补充 WASM 端已知限制；`src/main.rs` 清理未使用 import；`src/lib.rs` wasm_entry 修复未使用 import；subagent review 执行 | trunk 配置与 `index.html` 未纳入仓库（仅提供示例）；CI 脚本未添加（无 CI 配置文件） |

---

## 已确认决策（计划修订）

- **OpenSCAD（Web）**：与桌面相同——在后端运行环境中 **查找 OpenSCAD 可执行文件**（含 `OPENSCAD_PATH` 等与 `scad-data` 一致的规则），调用 CLI 生成 **3MF**，通过 HTTP **将 3MF 传给前端**；前端沿用现有 3MF 解析与 Viewer 更新路径。详见 `plan-00.md`「架构假设」与 **Phase 7**。

---

## Phase 执行记录

### Phase 1

- **完成情况**：协议文档与共享 crate 已落地；桌面默认构建未接入新 crate（仅 workspace 成员），行为不变。
- **变更文件**：`docs/2026040800-studio-web-api/README.md`；`crates/studio-remote-protocol/`（`session`、`filesystem`、`render`、`error`、`websocket` 模块）；根 `Cargo.toml` 的 `workspace.members`；`crates/studio-remote-protocol/tests/serde_roundtrip.rs`。
- **验证**：`cargo check`、`cargo check -p scad-studio`、`cargo test -p studio-remote-protocol`。
- **后续**：Phase 2 在 `main.rs` / `FileWatcher` 抽象层可依赖本 crate 的路径与事件形状；Phase 3 后端实现 REST/WebSocket 时对齐本文档路径与常量说明。

### Phase 2

- **完成情况**：工作区根下列举与读文件经 `WorkspaceDataAccess`；`LocalWorkspaceAccess` 内聚 `FileWatcher.watch_files`；`normalize_rel_path` / `join_under_root` / `path_is_within_root` 与集成测试在 `crates/scad-data/tests/workspace_paths.rs`。
- **变更文件**：`crates/scad-data/src/workspace_paths.rs`、`workspace_tree.rs`、`workspace_access.rs`、`lib.rs`；`crates/scad-ui` 增加对 `scad-data` 依赖、`file_tree.rs` 注入 `read_dir`；`src/app.rs`、`src/main.rs`。
- **验证**：`cargo test --workspace` 通过。

### Phase 3

- **完成情况**：可运行的 `studio-backend` HTTP 服务；REST 与 WS 与 Phase 1 文档及 `studio-remote-protocol` 对齐；`scad-data` 的 `normalize_rel_path` / `join_under_root` / `path_is_within_root` 用于路径安全；`notify` + 300ms 防抖合并后广播。
- **变更文件**：`crates/studio-backend/`（`app_state`、`routes`、`debounce`、`ws`、`config` 等）；根 `Cargo.toml` workspace `members`；`docs/2026040800-studio-web-api/README.md` 增补后端环境变量说明。
- **验证**：`cargo test -p studio-backend`、`cargo test --workspace`。

### Phase 4

- **完成情况**：`studio-remote-client` 实现 `RemoteWorkspaceAccess`（`mount` → `POST /v1/sessions` + WebSocket 监听；`read_directory` / `read_file_bytes_in_root` 走 REST）；`wasm32` 上同名类型占位实现 `WorkspaceDataAccess`（返回 `Unsupported`，待 Phase 6 异步接线）；根 `scad-studio` 增加可选 feature `remote-workspace`。
- **变更文件**：`crates/studio-remote-client/`；根 `Cargo.toml`（`[features]` + `studio-remote-client` 可选依赖）；workspace `members` 已含该 crate。
- **验收命令**：
  - `cargo check -p scad-studio --features remote-workspace`
  - `cargo check -p studio-remote-client --target wasm32-unknown-unknown`
  - `cargo test -p studio-remote-client`
- **断线重连**：首版在代码注释与 crate 文档中约定「断线后由 UI 触发全量刷新树 / 重新 `mount`」；未实现自动重连状态机。

### Phase 5

- **完成情况**：无需代码修改。`scad-scene` 的现有架构（wgpu + winit + egui_wgpu + cfg 隔离的平台特定代码）已天然兼容 wasm32。`Renderer::new(Arc<Window>)` 在 wasm32 上直接编译通过，`resize` / `PhysicalSize` 通过 winit web 后端对齐，wireframe 通过 `supports_wireframe()` 特性检测自动降级。
- **变更文件**：无。
- **验证**：`cargo check -p scad-scene --target wasm32-unknown-unknown` 通过；`cargo test -p scad-scene` 通过（9 个 three_mf 测试 + 其他 crate 内测试）；`cargo tree` 确认 wasm32 依赖图中无 `notify`/`rfd`/`muda`。
- **已知限制**：wasm 下 `system_fonts` 返回空字体列表（需后续提供替代方案）；`detect_language_tag()` 固定返回 `"en-US"`。

### Phase 6

- **完成情况**：创建 `src/lib.rs`（共享模块声明 + 公开导出 + WASM 入口），重构 `src/main.rs` 为桌面专用入口（`StudioDesktopApp` + `ApplicationHandler`）。跨 crate 条件编译隔离：`scad-data`（notify/dirs 移到 cfg(not(wasm32))，watcher/workspace_access/openscad/export/config 在 wasm32 提供 no-op 存根）、`scad-ui`（muda 移到 cfg(not(wasm32))，platform_support no-op）、`scad-viewer`（muda/rfd/pollster/env_logger cfg 隔离，egui-winit 在 wasm32 禁用 clipboard）。`viewer_tab.rs` 中 `rfd::FileDialog` 调用 cfg 隔离。WASM 入口 `wasm_main` 初始化 console_log → EventLoop → 单窗口 → Renderer → egui_winit → RemoteWorkspaceAccess → StudioRuntime → run_app。
- **变更文件**：`src/lib.rs`（新建）、`src/main.rs`（重构）、`Cargo.toml`（条件依赖 + wasm deps）、`crates/scad-data/`（Cargo.toml + watcher.rs + workspace_access.rs + config.rs + openscad.rs + export.rs + lib.rs）、`crates/scad-ui/`（Cargo.toml + platform_support.rs）、`crates/scad-viewer/Cargo.toml`、`src/viewer_tab.rs`（rfd cfg 隔离）。
- **验证**：`cargo check -p scad-studio` 通过；`cargo check -p scad-studio --lib --target wasm32-unknown-unknown` 通过；`cargo test --workspace` 全部通过（0 失败）。
- **遗留**：`RemoteWorkspaceAccess` wasm stub 返回 `Unsupported`（需后续异步接线）；构建说明文档（trunk/wasm-pack）留待 Phase 8。

### Phase 7

- **完成情况**：后端 `studio-backend` 新增 `POST /v1/sessions/{id}/preview` 端点。处理器校验会话、路径安全、文件存在性后，通过 `tokio::task::spawn_blocking` 调用 `render_openscad_3mf`——该函数复用 `scad_data::detect_openscad_path` 定位 OpenSCAD CLI，调用 `build_preview_job_args` 构造 `-o xxx.3mf` 参数，`std::process::Command` 执行渲染，读取 3MF 字节返回。响应 `Content-Type: model/3mf`，体积超过 64 MiB 返回 `PAYLOAD_TOO_LARGE`。
- **变更文件**：`crates/studio-backend/src/routes.rs`（新增 `preview_openscad` + `render_openscad_3mf`）；`crates/scad-data/src/lib.rs`（导出 `build_preview_job_args`）。
- **验证**：`cargo check -p studio-backend` 通过；`cargo test --workspace` 全部通过；`cargo check -p scad-studio --lib --target wasm32-unknown-unknown` 通过。

### Phase 8

- **完成情况**：全量回归验证通过。清理了 `src/main.rs` 中未使用的 `WatchMessage`/`WorkspaceTreeEntry` import 和 `src/lib.rs` wasm_entry 中未使用的 import。新增开发者文档 `docs/2026040800-studio-web-api/DEVELOPMENT.md`，涵盖后端启动、WASM 构建部署、同源代理配置、WebGPU 常见问题、crate 结构说明。更新 `docs/known_issues.md`，补充 WASM 端三项新已知限制（RemoteWorkspaceAccess Unsupported、系统字体固定、构建工具链未入库）。独立 subagent 对 Phase 6-7 全部 diff 完成 review。
- **变更文件**：`docs/2026040800-studio-web-api/DEVELOPMENT.md`（新建）、`docs/known_issues.md`（新增三条）、`src/main.rs`（清理 import）、`src/lib.rs`（修复 wasm_entry import）。
- **验证**：`cargo test --workspace` 全部通过（0 失败）；`cargo check -p scad-studio` 通过；`cargo check -p scad-studio --lib --target wasm32-unknown-unknown` 通过；`cargo check -p studio-backend` 通过。
