# 执行结果存档：`2026042200-studio-app-server-unification`

本文件在对应 Phase 执行过程中**实时追加**，与 `plan-00.md` 同步维护。

## 锁定基线

- 锁定提交：`7b232bdbdb751da84adbe6ec7d4fa28175b8cf97`（短哈希 `7b232bd`）
- 核心要求：本轮所有执行结果都必须保护该提交中**所有已完成功能**，不得出现构建、测试或用户可见行为回退。

| Phase | 状态 | 摘要 | 遗留问题 |
|-------|------|------|----------|
| 1 | 已完成 | 已完成基线清单、事实核查、等价覆盖矩阵、`scad-viewer` 瘦身与 Phase 1 验证 | 当前会话仍缺桌面 GUI 逐点击自动化，仅有启动 smoke；该限制已登记到 `docs/known_issues.md` |
| 2 | 已完成 | 已建立 `app-server-protocol` / `app-server-transport`，完成测试、workspace 回归与 wasm-clean 验证，并补写 `scad-ui` / `scad-scene` wasm 化方案文档 | 真正的 session runtime、mpsc / WebSocket adapter 与 host 生命周期实现留到 Phase 3 之后 |
| 3 | 已完成 | 已完成 `app-server-core` / `app-server-host` 首版提取、`scad-data` 删除、session 生命周期测试与 `websocket_smoke_roundtrip` | `app-server-host` 仍是最小 host 实现；更完整的桌面接线与 browser 入口在 Phase 5 / Phase 6 继续扩展 |
| 4 | 已完成（physical crate split） | 已完成 root 业务代码迁移、`studio-app` 桌面入口收敛、根 `Cargo.toml` virtual workspace 化，以及 workspace/wasm 回归 | CLI 会话仍缺桌面 GUI 逐点击自动化；`scad-scene` 的端无关渲染入口抽象不在本次 physical split 收尾范围 |
| 5 | 已完成 | 桌面端已切到同进程 `app-server-host` + `tokio::mpsc` transport，协议旁路守门与 `studio-app --smoke-exit` 验证均已通过 | 当前环境仍缺桌面 GUI 逐点击自动化，仅能持续依赖 smoke 与人工回归 |
| 6 | 已完成 | 已完成 `studio-web` 的共享预览状态、本地 mesh 渲染、目录树、watch 刷新闭环与增强 browser smoke | 当前环境缺少 `rust-analyzer`，本 Phase 只能用 cargo/build/smoke 代替 LSP 诊断 |
| 7 | 已完成 | 已完成重复职责扫描、workspace member diff 与 `scad-viewer` 去留评估；当前结论为保留纯共享 lib | 当前环境仍缺桌面 GUI 逐点击自动化，本 Phase 对 Phase 1 等价覆盖矩阵的回归以既有 smoke 与机械化扫描为主 |
| 8 | 已完成 | 已完成终态回归、shell 导出修复、守门脚本复跑与扩展点落档 | 当前环境仍缺桌面 GUI 逐点击自动化，因此桌面端交互回归仍只能以 smoke + 既有已知问题记录配合人工补验 |

---

## 记录要求

- 每个 Phase 完成后，必须补充：完成情况、变更文件范围、验证结果、遗留问题。
- 若某 Phase 发现锁定基线能力被破坏，必须在该 Phase 内回滚或修复，并如实记录。

---

## Phase 1 执行中记录

### 2026-04-22 当前进度

- 已完整审阅 `plan-00.md`，确认当前任务是从 Phase 1 起顺序执行整份归档计划，而不是只做单点研究。
- 由于主工作树原本停留在 `main` 且已有本地未提交改动，本轮未在 `main` 上直接实施；已切到分支 `ulw/20260422-studio-app-server-unification` 继续执行。
- 已启动并行事实核查，覆盖：
  - Phase 1 需要审计的根桌面运行时、`scad-viewer`、`scad-data`、相关文档与调用点；
  - 当前仓库实际的构建 / 测试 / 桌面启动命令；
  - 独立 `scad-viewer` 的用户可见能力与 Studio 现有等价覆盖关系；
  - `crates/scad-data/tests/*` 的测试清单与模块映射。

### 当前已确认的前置结论

- 用户明确拒绝 worktree，因此本轮隔离手段收敛为“同一工作树内切专用功能分支”，不再额外创建 worktree。
- 当前不能把 Phase 1 视为纯文档阶段：如果后续等价覆盖核实发现 Studio 对独立 `scad-viewer` 存在能力缺口，按计划必须先在 Phase 1 内补齐，再允许瘦身 `scad-viewer`。

### 当前未决事项

- 待并行核查结果返回后，补写：
  - Phase 1 不可回退清单；
  - `scad-viewer` 瘦身名单；
  - `scad-data` 模块 / 调用点 / tests 清单；
  - 等价覆盖矩阵；
  - 基线自动化命令与人工回归路径。

### 已完成的事实核查（持续补充）

#### 1. 根桌面运行时实际文件清单

- 根桌面入口与事件循环：`src/main.rs`
- 根桌面壳层 / 应用状态：`src/app.rs`
- 工作区与最近工作区：`src/workspace.rs`
- 平台菜单：`src/platform_menu.rs`
- 文档工作区 / 文档会话 / 文档模型：`src/document_workspace.rs`、`src/document_session.rs`、`src/studio_document.rs`
- Viewer / Markdown / Image 打开与刷新路径：`src/viewer_tab.rs`、`src/markdown_tab.rs`、`src/image_tab.rs`
- Viewer 事件 / 相机 / 视口：`src/viewer_event_routing.rs`、`src/viewer_camera.rs`、`src/viewer_viewport.rs`
- 左栏 / 布局 / 工作区主区域 / 工作区框架 / 日志面板 / 欢迎态：`src/left_panel.rs`、`src/layout.rs`、`src/work_area.rs`、`src/work_area_frame.rs`、`src/log_panel.rs`、`src/welcome.rs`
- 图片解码 / 缩放：`src/image_decode.rs`、`src/image_zoom_math.rs`
- macOS 标题栏接线：`src/macos_fused_titlebar.rs`

#### 2. `scad-viewer` 当前二进制 / lib 形态与瘦身目标输入

- crate 入口：`crates/scad-viewer/Cargo.toml`
  - 当前仍含 `[lib]`（`src/lib.rs`）与 `[[bin]]`（`src/main.rs`）。
  - 当前依赖中包含计划点名的桌面应用专属依赖：`egui-winit`、`env_logger`、`muda`、`pollster`、`rfd`、`winit`。
  - `cargo metadata --format-version 1 --no-deps` 实测显示 `scad-viewer` package 当前 targets 为：`scad_viewer`（lib）、`font_probe`（bin）、`scad-viewer`（bin），说明 Phase 1 的 bin 瘦身对象不止一个。
- 二进制主入口：`crates/scad-viewer/src/main.rs`
  - 当前直接持有 `winit` 事件循环、`egui_winit::State`、`PlatformMenu`、`FileWatcher`、`OpenScadRunner`、`Renderer`，属于 Phase 1 需要瘦身移除的独立 Viewer 应用壳层。
- 额外 bin 目标：`crates/scad-viewer/src/bin/font_probe.rs`
  - 该目录存在，符合 Phase 1 需要一并移除 `src/bin/` 的条件。
- lib 入口：`crates/scad-viewer/src/lib.rs`
  - 当前仅导出 `app`、`ui`、`wrap_line_pack` 三个模块，说明 Phase 1 可先删除独立应用壳层，保留 lib 暴露面。
- lib 共享状态 / 命令面：`crates/scad-viewer/src/app.rs`
  - 当前导出 `StudioApp`、`UiActions`、`UiCommand`、`CameraAction`、`ViewerState`，属于根 crate `src/viewer_tab.rs` 仍在消费的共享 lib 内容。
- 根 crate 当前对 `scad_viewer::` 的直接消费（grep 实测）：
  - `src/viewer_tab.rs`：消费 `scad_viewer::app::{CameraAction, StudioApp, UiActions, UiCommand, UiFrame}` 与 `scad_viewer::ui::{show_viewer_overlays, status_bar, toolbar}`。
  - `src/main.rs`：消费 `scad_viewer::app::UiCommand::*`，用于 Viewer 标签页后的保存 preset / 删除 preset / 导出 / 发送到 slicer / 保存设置分发。
- 独立 Viewer 桌面菜单：`crates/scad-viewer/src/platform_menu.rs`
- 当前 `ui/` 目录：`crates/scad-viewer/src/ui/mod.rs`、`toolbar.rs`、`status_bar.rs`、`side_panel.rs`、`log_panel.rs`、`camera_overlay.rs`、`settings_dialog.rs`、`param_editor.rs`
  - 这些文件需要进一步区分“只服务独立 Viewer 应用”与“根 crate 仍复用的共享 UI”。
- 基于当前代码形态，Phase 1 可直接列入“优先瘦身候选”的项：
  - `crates/scad-viewer/src/main.rs`
  - `crates/scad-viewer/src/bin/font_probe.rs`
  - `crates/scad-viewer/src/platform_menu.rs`
  - `crates/scad-viewer/Cargo.toml` 中与独立桌面应用壳层绑定的 `[[bin]]`、`egui-winit`、`env_logger`、`muda`、`pollster`、`rfd`、`winit`
  - 但 `src/app.rs` 与 `src/ui/*` 不能直接整体删除，因为根 crate 仍在复用其中的 app/ui 暴露面。

#### 3. `scad-data` 当前 public module 与调用点

- public module 定义点：`crates/scad-data/src/lib.rs`
  - 当前模块实际为：`config.rs`、`document.rs`、`export.rs`、`openscad.rs`、`params.rs`、`presets.rs`、`watcher.rs`。
  - 当前 re-export 的核心类型 / 函数包括：`AppConfig`、`DocumentState`、`ExportFormat`、`SlicerInstall`、`OpenScadMessage`、`OpenScadRunner`、`RenderedArtifact`、`Parameter*`、`PresetFile`、`FileWatcher`、`load_config`、`save_config`、`export_model`、`load_presets`、`save_preset` 等。
- 根 crate 直接消费 `scad_data::` 的文件（grep 实测）：
  - `src/main.rs`：`AppConfig`、`FileWatcher`、`OpenScadMessage`、`WatchMessage`、`load_config`、`save_config`
  - `src/app.rs`：`LogEntry`、`LogLevel`
  - `src/viewer_tab.rs`：`AppConfig`、`DocumentState`、`FileWatcher`、`OpenScadRunner`、`RenderedArtifact`、`SlicerConfig`、`build_export_filename`、`delete_preset`、`detect_slicer_paths`、`export_model`、`load_presets`、`save_preset`、`send_to_slicer`
  - `src/markdown_tab.rs`、`src/image_tab.rs`：`FileWatcher`、`WatchMessage`
  - `src/layout.rs`、`src/work_area.rs`：`AppConfig`
  - `src/log_panel.rs`：`LogLevel`
- `scad-viewer` 当前直接消费 `scad_data::` 的文件（grep 实测）：
  - `crates/scad-viewer/src/main.rs`
  - `crates/scad-viewer/src/app.rs`
  - `crates/scad-viewer/src/ui/side_panel.rs`
  - `crates/scad-viewer/src/ui/log_panel.rs`
  - `crates/scad-viewer/src/ui/camera_overlay.rs`
  - `crates/scad-viewer/src/ui/settings_dialog.rs`
  - `crates/scad-viewer/src/ui/param_editor.rs`

#### 4. `crates/scad-data/tests/*` 当前测试清单（已实读）

- `crates/scad-data/tests/config_tests.rs`
  - `config_json_round_trip_preserves_paths`：覆盖 `config.rs` 中 `AppConfig` JSON 往返与路径字段保真。
  - `config_file_path_uses_platform_config_directory`：覆盖 `config_file_path()` 的平台配置目录落点。
- `crates/scad-data/tests/document_tests.rs`
  - `loading_source_builds_parameter_state_and_watch_list`：覆盖 `document.rs` 加载源码后的参数状态与 watch 路径。
  - `reparsing_source_preserves_existing_parameter_override`：覆盖重解析时参数覆写保留。
  - `applying_preset_updates_parameter_values`：覆盖 preset 应用到 `DocumentState`。
- `crates/scad-data/tests/export_tests.rs`
  - `export_filename_uses_selected_format_extension`：覆盖 `export.rs` 的导出扩展名生成。
  - `manual_slicer_paths_are_returned_before_auto_detected_paths`：覆盖手工 slicer 配置优先于自动探测。
- `crates/scad-data/tests/openscad_tests.rs`
  - `collect_process_logs_ignores_blank_lines_and_tags_stdout_as_info`
  - `collect_process_logs_tags_stderr_as_error_when_process_fails`
  - 均覆盖 `openscad.rs` 日志采集与 stdout/stderr 等级映射。
- `crates/scad-data/tests/openscad_command_tests.rs`
  - `build_cli_args_includes_defines_before_source_path`
  - `preview_job_args_force_3mf_output`
  - `preview_job_uses_3mf_temp_filename`
  - `resolve_openscad_path_prefers_configured_path`
  - `resolve_openscad_path_keeps_generic_missing_cli_message`
  - `finalize_job_cleans_preview_file_when_output_collection_fails`
  - 以上均覆盖 `openscad.rs` 的命令行参数、预览作业输出、路径解析与失败清理。
- `crates/scad-data/tests/params_tests.rs`
  - `parses_grouped_visible_and_hidden_parameters`
  - `parameter_store_preserves_overrides_on_reparse`
  - `parameter_store_builds_cli_defines_and_restore_default`
  - 覆盖 `params.rs` 的解析、状态合并、CLI define 生成与恢复默认值。
- `crates/scad-data/tests/presets_tests.rs`
  - `preset_path_uses_matching_scad_json_name`
  - `save_load_and_delete_presets_round_trip`
  - 覆盖 `presets.rs` 的路径命名与保存 / 加载 / 删除往返。
- `crates/scad-data/tests/public_api_tests.rs`
  - `config_defaults_expose_empty_slicer_list`
  - `document_starts_without_source`
  - 覆盖 crate 顶层 public API 的默认值契约。
- `crates/scad-data/tests/watcher_tests.rs`
  - `matches_path_accepts_canonicalized_equivalent_paths`
  - `matches_path_rejects_unrelated_paths`
  - `matches_any_watched_path_when_preset_file_changes`
  - `matches_any_path_accepts_changes_inside_watched_directory`
  - 覆盖 `watcher.rs` 的路径匹配与目录内变更判定。
- 当前统计：9 个测试文件，25 个具名测试；后续 Phase 3 迁移时要求用例数差为 0。

#### 5. 当前已确认的相关文档

- `docs/known_issues.md`
  - 已记录与后续 Phase 3 直接相关的问题：`DocumentWorkspace` 仍保留 `DocumentKey` / `TabId` 双身份体系、真实运行时分支缺少自动化测试，以及本地 OpenSCAD CLI 缺失等。
  - 本轮若新增阻塞项，需要继续按此文件格式追加。

#### 6. 当前 workspace 现实结构（来自根 `Cargo.toml`）

- 根 crate 仍是业务 package：`[package] name = "scad-studio"`，且 `[[bin]]` 仍指向 `src/main.rs`。
- 当前 workspace members 只有：`crates/scad-data`、`crates/scad-scene`、`crates/scad-ui`、`crates/scad-viewer`。
- 与目标长期结构相比，当前仍缺：`crates/app-server-protocol`、`crates/app-server-transport`、`crates/app-server-core`、`crates/app-server-host`、`crates/studio-common`、`crates/studio-app`、`crates/studio-web`。
- 根 crate 当前直接依赖：`scad-data`、`scad-scene`、`scad-ui`、`scad-viewer`，且同时持有桌面壳层依赖 `egui-winit`、`env_logger`、`muda`、`pollster`、`rfd`、`winit`；这与计划中的“根 crate 只保留 workspace 根”终态仍有明显差距。

#### 7. 当前可直接执行的基线验证命令（已实跑）

- `cargo check --workspace`
  - 2026-04-22 实跑结果：通过；输出包含 `Checking scad-data`、`scad-ui`、`scad-viewer`、`scad-studio`，最终 `Finished 'dev' profile`。
- `cargo test --workspace`
  - 2026-04-22 实跑结果：通过。
  - 关键可复用观察：
    - `scad-data` 当前覆盖 9 个 integration test 文件；
    - 根 crate `scad-studio` 当前已有 `document_workspace_tests`、`studio_app_tests`、`platform_menu_tests`、`viewer_event_routing_tests`、`viewer_camera_tests`、`viewer_viewport_tests`、`work_area_frame_tests` 等回归；
    - **瘦身前** `scad-viewer` 当前有 `platform_menu_tests`、`public_api_tests`、`toolbar_block_wrap_tests`、`ui_state_tests`、`wrap_line_pack_tests`，且还会跑 `src/bin/font_probe.rs` 与 `src/main.rs` 的 unit target（均为 0 tests）。
- 当前桌面启动入口（来自根 `Cargo.toml`）：
  - 根 package：`scad-studio`
  - `[[bin]]`：`name = "scad-studio"`，`path = "src/main.rs"`
  - 当前人工启动口径可收敛为：`cargo run --bin scad-studio`
  - 2026-04-22 使用 15 秒 timeout 做启动 smoke：进程在超时前保持运行，stderr 仅显示 `Finished 'dev' profile` 与 `Running 'target/debug/scad-studio'`，说明当前桌面应用至少可以进入运行态而非立即崩溃。
- `cargo build -p scad-viewer --bin scad-viewer`
  - 2026-04-22 实跑结果：通过；证明 Phase 1 瘦身前独立 `scad-viewer` 二进制当前可被正常构建。
  - 2026-04-22 使用 15 秒 timeout 做启动 smoke：进程在超时前保持运行，stderr 仅显示 `Finished 'dev' profile` 与 `Running 'target/debug/scad-viewer'`，说明独立 Viewer 二进制当前至少可以进入运行态而非立即崩溃。
- 当前仓库未提供的辅助命令入口（glob 实测）：
  - 不存在 `README.md`
  - 不存在 `Justfile`
  - 不存在 `Makefile`
  - 不存在 `package.json`
  - 结论：现阶段基线验证口径需要直接以 Cargo 命令和代码内测试目标为准，不能依赖仓库级脚本封装。

#### 8. `scad-viewer` Phase 1 机械化断言的执行前基线（瘦身前记录）

- 文件存在性：
  - `crates/scad-viewer/src/main.rs` 当前存在。
  - `crates/scad-viewer/src/bin/font_probe.rs` 当前存在，因此 `src/bin/` 目录当前非空。
- `crates/scad-viewer/Cargo.toml`
  - 当前 `[[bin]]` 段计数：1。
- `cargo metadata --format-version 1 --no-deps` 中 `scad-viewer` 当前 targets：
  - `scad_viewer: ['lib']`
  - `font_probe: ['bin']`
  - `scad-viewer: ['bin']`
  - `platform_menu_tests: ['test']`
  - `public_api_tests: ['test']`
  - `toolbar_block_wrap_tests: ['test']`
  - `ui_state_tests: ['test']`
  - `wrap_line_pack_tests: ['test']`
- 同一份 metadata 中 `scad-viewer` 当前 dependencies：
  - `egui`
  - `egui-winit`
  - `env_logger`
  - `glam`
  - `log`
  - `muda`
  - `pollster`
  - `rfd`
  - `scad-data`
  - `scad-scene`
  - `scad-ui`
  - `winit`
- `crates/scad-viewer/src/bin/font_probe.rs`
  - 当前是一个字体 fallback 探测工具，直接依赖 `egui` 与 `scad_scene::system_fonts`，仅输出终端信息，不属于 Studio 用户可见正式能力；可直接纳入 Phase 1 删除名单。

#### 9. `scad-viewer` 当前测试目标与 Phase 1 处理提示

- `crates/scad-viewer/tests/platform_menu_tests.rs`
  - 执行前通过 `#[path = "../src/platform_menu.rs"]` 直接测试独立 Viewer 的 `platform_menu.rs`；Phase 1 已删除该文件与该测试，执行后不再保留。
- `crates/scad-viewer/tests/public_api_tests.rs`
  - 只验证 `scad_viewer::app::StudioApp` 默认日志面板关闭；属于 lib 暴露面测试，应在 Phase 1 瘦身后继续保留。
- `crates/scad-viewer/tests/toolbar_block_wrap_tests.rs`
  - 覆盖 `scad_viewer::ui::toolbar` 的嵌入式高度计算；根 crate `viewer_tab.rs` 当前仍复用该 UI，因此该测试应继续保留。
- `crates/scad-viewer/tests/ui_state_tests.rs`
  - 通过 `#[path = "../src/app.rs"]` / `ui/mod.rs` / `wrap_line_pack.rs` 直接测试共享 lib 内的 UI 状态；只要这些模块继续保留在 lib 中，该测试应继续保留。
- `crates/scad-viewer/tests/wrap_line_pack_tests.rs`
  - 覆盖 `wrap_line_pack` 纯算法；属于共享 lib 测试，应继续保留。

#### 10. `scad-viewer` 瘦身后的机械化断言与 QA 结果

- 文件断言：
  - `crates/scad-viewer/src/main.rs` 已删除。
  - `crates/scad-viewer/src/platform_menu.rs` 已删除。
  - `crates/scad-viewer/src/bin/font_probe.rs` 已删除；`crates/scad-viewer/src/bin/` 当前为空目录。
  - `crates/scad-viewer/tests/platform_menu_tests.rs` 已删除。
- manifest 断言：
  - `crates/scad-viewer/Cargo.toml` 当前 `[[bin]]` 段计数为 0。
  - `scad-viewer` 依赖已收敛为：`egui`、`scad-data`、`scad-scene`、`scad-ui`。
  - 已删除的独立桌面应用专属依赖：`egui-winit`、`env_logger`、`glam`、`log`、`muda`、`pollster`、`rfd`、`winit`。
- `cargo metadata --format-version 1 --no-deps`（执行后）
  - `scad-viewer` 当前 targets：
    - `scad_viewer: ['lib']`
    - `public_api_tests: ['test']`
    - `toolbar_block_wrap_tests: ['test']`
    - `ui_state_tests: ['test']`
    - `wrap_line_pack_tests: ['test']`
  - 不再存在任何 `kind` 含 `bin` 的 target。
- `cargo build -p scad-viewer --bin scad-viewer`
  - 执行后结果：失败，错误为 `no bin target named 'scad-viewer' in 'scad-viewer' package`；符合 Phase 1 预期。
- `cargo check --workspace`
  - 执行后结果：通过。
- `cargo test --workspace`
  - 执行后结果：通过。
  - 执行后 `scad-viewer` 仅保留 `public_api_tests`、`toolbar_block_wrap_tests`、`ui_state_tests`、`wrap_line_pack_tests` 四个测试 target；共享 lib 暴露面未断裂。

#### 11. Studio ↔ 独立 `scad-viewer` 等价覆盖矩阵（Phase 1 结论）

| 能力点 | 独立 `scad-viewer` 证据 | Studio 现有对等实现 | 结论 |
|---|---|---|---|
| 打开 `.scad` 文件并进入预览流程 | `crates/scad-viewer/src/ui/toolbar.rs` 的“打开”按钮会设置 `actions.open_file`；`crates/scad-viewer/src/main.rs` 通过 `rfd::FileDialog` 选文件并 `open_source_file()` | Studio 以 workspace + 文档标签路径打开文件：`src/main.rs` 分发文档事件，`src/viewer_tab.rs::ViewerTab::open()` 直接接收路径并建立 watcher / OpenSCAD runner | **已覆盖**。Studio 不需要保留独立 Viewer 自带的文件对话框壳层 |
| OpenSCAD 生成 3MF 预览并把结果挂到视口 | `crates/scad-viewer/src/main.rs` 维护 `OpenScadRunner`、接收 `OpenScadMessage::Started/Finished` 并在完成后更新 mesh / camera | `src/viewer_tab.rs::handle_openscad_message()` 与 `handle_render_result()` 承接同一类 `OpenScadMessage`，完成后更新 viewer 状态、mesh 与相机 | **已覆盖** |
| 直接打开 `.stl` / `.3mf` 网格文件 | `crates/scad-viewer/src/main.rs` 通过 `detect_viewer_kind` / `load_direct_mesh` 打开非 `.scad` 网格文件 | `src/viewer_tab.rs::ViewerTab::open()` 同样按扩展名选择 `ViewerSourceKind::{Stl,ThreeMf}`，并在 `load_initial_state()` / `load_direct_mesh()` 路径下加载 | **已覆盖** |
| 渲染模式、颜色模式、投影模式、网格/底板/坐标轴/阴影/雾效/剖切开关 | `crates/scad-viewer/src/ui/toolbar.rs` 的 `render_mode_group`、`color_mode_group`、`projection_group`、`toggle_group` | Studio `src/viewer_tab.rs` 复用 `scad_viewer::ui::{toolbar,status_bar,show_viewer_overlays}`，同一组控件直接画在标签页内；状态存于同一个 `scad_viewer::app::ViewerState` | **已覆盖，且实现完全共用** |
| 相机面板中的数值输入（目标 XYZ、距离、方位角、仰角）与预设视角按钮 | `crates/scad-viewer/src/ui/camera_overlay.rs` 提供 `DragValue` 输入与 `ViewFront/ViewBack/ViewLeft/ViewRight/ViewTop/ViewBottom` 按钮 | `src/viewer_tab.rs:254-270` 直接调用 `show_viewer_overlays()`，其内部就会画出同一个 `camera_overlay::show()`；后续 `apply_camera_action()` 把数值动作应用到 Studio 的 `OrbitalCamera` | **已覆盖**。此前“Studio 缺数值相机面板”的怀疑已被直接代码阅读排除 |
| 状态栏显示当前文件、渲染状态、投影模式 | `crates/scad-viewer/src/ui/status_bar.rs` | Studio `src/viewer_tab.rs` 同样复用 `status_bar::paint_status_row()` | **已覆盖，且实现完全共用** |
| 侧边栏中的参数、预设、导出、发送到 slicer | `crates/scad-viewer/src/ui/side_panel.rs` 使用 `DocumentState`、`detect_slicer_paths()`、`UiCommand::{SavePreset,DeletePreset,ExportModel,SendToSlicer}` | Studio `src/viewer_tab.rs` 复用同一 `side_panel`，并在 `src/main.rs` 中分发相同 `UiCommand::*` 到 `save_preset`、`delete_preset`、`export_current_model` 等逻辑 | **已覆盖，且实现完全共用** |
| 日志面板与错误自动展开 | `crates/scad-viewer/src/ui/log_panel.rs` + `crates/scad-viewer/src/app.rs::push_log()` | Studio 的 viewer 标签页使用同一 `log_panel` / `StudioApp::push_log()` 逻辑 | **已覆盖，且实现完全共用** |
| 独立窗口、多窗口菜单、独立 Viewer 自己的 Open/Settings/Quit 菜单 | `crates/scad-viewer/src/main.rs` + `crates/scad-viewer/src/platform_menu.rs` | Studio 在根壳层已有自己的 `src/main.rs` + `src/platform_menu.rs` 处理多窗口、Open Folder、Recent Workspaces、Toggle Panels 等，但不再提供“独立 Viewer 产品边界” | **不作为缺口**。这是本轮明确要删除的独立产品 / 二进制壳层，而不是需要保留的共享预览能力 |
| `font_probe` 字体探测工具 | `crates/scad-viewer/src/bin/font_probe.rs` | Studio 无对等入口 | **不作为缺口**。这是独立 Viewer 调试工具，不属于用户可见正式能力 |

- 结论：按 Phase 1 要求核对后，Studio 已覆盖独立 `scad-viewer` 的用户可见预览能力；本轮删除的是独立产品壳层（二进制入口、平台菜单、文件对话框与调试工具），**没有发现必须先补齐才能继续瘦身的能力缺口**。

#### 12. `scad-data` 依赖清单与逐模块目标边界（Phase 1 固化版本）

- 当前 `crates/scad-data/Cargo.toml` 依赖：
  - `dirs`：当前由 `config.rs` 的 `config_file_path()` 使用。
  - `log`：当前由 `openscad.rs` 在预览文件清理失败时输出 warning 使用。
  - `notify`：当前由 `watcher.rs` 使用。
  - `regex`：当前由 `params.rs` 使用。
  - `rfd`：当前在 `crates/scad-data/src/` 中**无实际调用点**；后续按计划不进入 `app-server-core`，桌面文件对话框归 `studio-app`。
  - `scad-scene`：当前由 `openscad.rs` 使用 `MeshData` / `three_mf`。
  - `serde` / `serde_json`：当前由 `config.rs`、`params.rs`、`presets.rs` 使用。
  - `stl_io`：当前未在 `src/` 中 grep 到直接调用点，但按计划仍属于 `export.rs` 未来应归入 `app-server-core` 的导出职责。
- 逐模块目标边界（按 Phase 1 计划固化）：
  - `openscad.rs` → `app-server-core` 的 OpenSCAD / preview service。
  - `watcher.rs` → `app-server-core` 的 watch service。
  - `export.rs` → `app-server-core` 的 file/export service。
  - `config.rs` → **三分**：
    - server 配置（如 `openscad_path`、slicers、`recent_workspaces`）→ `app-server-core`
    - 桌面壳层配置 → `studio-app`
    - 共享 UI 状态（面板位置 / 透明度等）→ `studio-common` 或各端壳层（Phase 4 最终落位）
  - `document.rs` → **二分**：
    - 纯可序列化数据 → `app-server-protocol`
    - 含 `PathBuf` / `Instant` / UI 输入态的 `DocumentState` 等 stateful 部分 → Phase 3 暂存根 crate，Phase 4 迁入 `studio-common`
  - `params.rs` → **二分**：
    - 纯可序列化参数定义 / 解析结果 → `app-server-protocol`
    - stateful store / 覆写状态 → Phase 3 暂存根 crate，Phase 4 迁入 `studio-common`
  - `presets.rs` → **二分**：
    - 纯预设文件结构 → `app-server-protocol`
    - 文件 I/O 与 stateful 读写流程 → `app-server-core`

#### 13. Phase 1 完成情况（当前结论）

- 代码改动：
  - 删除 `crates/scad-viewer/src/main.rs`
  - 删除 `crates/scad-viewer/src/platform_menu.rs`
  - 删除 `crates/scad-viewer/src/bin/font_probe.rs`
  - 删除 `crates/scad-viewer/tests/platform_menu_tests.rs`
  - 更新 `crates/scad-viewer/Cargo.toml`，移除 `[[bin]]` 与独立桌面应用专属依赖
  - 持续更新 `prompt-archives/2026042200-studio-app-server-unification/plan-00-result.md`
- 验证结果：
  - `cargo check --workspace`：通过
  - `cargo test --workspace`：通过
  - `cargo build -p scad-viewer --bin scad-viewer`：按预期失败（无 bin target）
- 独立 review 结论：在补齐等价覆盖矩阵、执行后断言结果和 `scad-data` 模块映射后，Phase 1 要求已满足。
- 额外记录：由于当前会话无法完成真实桌面窗口的逐点击交互回归，已将该限制记录到 `docs/known_issues.md`，供 Phase 5 / Phase 8 持续处理。

---

## Phase 2 执行中记录

### 2026-04-22 当前进度

- 已新建并注册 workspace members：`crates/app-server-protocol`、`crates/app-server-transport`。
- 已在 `app-server-protocol` 落地首版协议 DTO：
  - 结构化 `PathHandle { workspace_id, path_segments }`
  - `ClientCommand` / `ClientRequestEnvelope`
  - `ServerResponseEnvelope` / `ServerPushEnvelope`
  - `CapabilityHandshakeRequest/Response`
  - `SessionToken` / `RequestId` / `SubscriptionId`
  - `PreviewRequestKind` 与自定义 `PreviewMeshPayload` / `PreviewArtifact3mf` / `PreviewRenderedImagePayload`
  - `ProtocolError` / `ProtocolErrorCode`
  - `PathHandle` 当前字段已私有化，且反序列化会复用同一套 segment 校验逻辑。
- 已在 `app-server-transport` 落地首版 transport trait 与内存内参考实现：
  - `ClientTransport`
  - `InMemoryTransport::pair()`
  - `ClientEnvelope` / `ServerEnvelope`
  - `TransportErrorFrame` / `TransportError`

### Protocol / Transport 设计落点

#### 1. `app-server-protocol` 当前文件清单

- `crates/app-server-protocol/Cargo.toml`
  - 仅依赖 `serde`、`serde_json` 与 `unicode-normalization`；无 transport / 平台依赖。
- `crates/app-server-protocol/src/path.rs`
  - `WorkspaceId` / `PathHandle` 与 `PathHandleValidationError`；对 segment 执行 NFC 规范化，拒绝空串、`.`、`..`、`/`、`\\`。
- `crates/app-server-protocol/src/protocol.rs`
  - 协议命令、响应、事件、错误、能力协商、预览 DTO、会话 token、取消 / 订阅语义。
- `crates/app-server-protocol/tests/path_handle_tests.rs`
  - `path_handle_rejects_dot_dot_segment`
  - `path_handle_rejects_single_dot_segment`
  - `path_handle_rejects_empty_segment`
  - `path_handle_rejects_native_separator`
  - `path_handle_nfc_canonical_equivalent`
- `crates/app-server-protocol/tests/serde_roundtrip_tests.rs`
  - command / response / event / error / capability / preview payload 的 serde round-trip
  - 版本区间协商
  - 小 / 大 preview payload round-trip

#### 2. `app-server-transport` 当前文件清单

- `crates/app-server-transport/Cargo.toml`
  - 仅依赖 `app-server-protocol`；无 `tokio`、WebSocket、HTTP、`mpsc` 或平台依赖。
- `crates/app-server-transport/src/lib.rs`
  - 定义 `ClientTransport` trait：显式覆盖 handshake / reconnect / request / subscribe / unsubscribe / cancel / close / receive。
  - 提供内存内参考实现 `InMemoryTransport` 与测试用 `InMemoryTransportHarness`。
- `crates/app-server-transport/tests/in_memory_transport_tests.rs`
  - request/response roundtrip
  - handshake / reconnect roundtrip
  - cancel request 发送
  - push 订阅 / 退订与退订后丢弃旧 subscription push
  - close 之后拒绝新请求
  - transport error frame 传播

### Phase 2 验证结果（已实跑）

- `cargo test -p app-server-protocol`
  - 结果：通过（11 个测试通过）。
- `cargo test -p app-server-transport`
  - 结果：通过（6 个测试通过）。
- `cargo check -p app-server-protocol --target wasm32-unknown-unknown`
  - 结果：通过。
- `cargo check -p app-server-transport --target wasm32-unknown-unknown`
  - 结果：通过。
- `cargo check --workspace`
  - 结果：通过；新 crate 与现有 desktop/workspace 一起编译通过。
- `cargo test --workspace`
  - 结果：通过；新 crate 加入后，整个 workspace 测试仍为绿色。
- `rg "PathBuf|std::path::Path[^B]" crates/app-server-protocol/src`
  - 结果：零匹配；协议公开模型当前未泄漏 `PathBuf` / `Path`。
- 独立 review 结论：Phase 2 的代码与验证已满足计划要求；此前唯一 blocker 是结果文档状态未更新，现已修正。

### 当前协议语义与计划对齐情况

- 已落地的核心点：
  - 结构化路径句柄，禁止裸传 OS 原生路径。
  - `request_id`、`cancel(request_id)`、`session.reclaim`、`watch.subscribe`、`watch.unsubscribe`。
  - `preview.request` 明确区分 `geometry_artifact` 与 `rendered_image`。
  - Web client 默认 `file.read` 拒绝扩展名：`.scad`、`.stl`、`.3mf`（通过 `web_file_read_capability()` 暴露）。
  - 预览几何 DTO 使用协议自定义 payload，不复用 `scad_scene::MeshData`。
- 当前仍留给 Phase 3 的部分：
  - `path_handle_symlink_resolved_to_canonical`
  - `path_handle_stale_after_session_close`
  - 实际 session lifecycle / reclaim runtime
  - 真正的 mpsc / WebSocket adapter

### 协议先例与 wasm-clean 现状判断

- 仓库内未发现现成的 command/event protocol crate，也未发现已有的 in-memory transport 实现；本轮 Phase 2 为首个正式协议 / transport 骨架。
- 历史先例主要来自归档计划：
  - `prompt-archives/2026042200-studio-app-server-unification/plan-00.md`：给出当前协议命名、session reclaim、cancel、watch 等设计要求。
  - `prompt-archives/2026040800-studio-web-wasm-backend/plan-00.md`：提供旧 Web/wasm 方向的约束背景。
- 当前仓库代码里**没有**现成的 `cfg(target_arch = "wasm32")` / `cfg(not(target_arch = "wasm32"))` 模式；现有 gating 主要是 `target_os = "macos" | "linux" | "windows"`。因此本轮新建的 protocol / transport crate 直接通过“完全不引入平台 API”实现 wasm-clean，而不是复用既有 wasm gating 模式。

### `scad-ui` / `scad-scene` wasm 化方案文档（Phase 2 只落档，不实施）

#### `scad-ui` 当前阻塞点与 Phase 4 目标

- 证据：
  - `crates/scad-ui/Cargo.toml` 当前直接依赖 `muda`、`winit`。
  - `crates/scad-ui/src/lib.rs` 当前导出 `font_setup`、`platform_support`。
  - `crates/scad-ui/src/platform_support.rs` 直接消费 `muda::{Menu, MenuEvent}` 与 `winit::{EventLoopBuilder, Window}`，并含 macOS / Windows 平台分支。
  - `crates/scad-ui/src/font_setup.rs` 直接调用 `scad_scene::system_fonts::configure_egui_fonts()`。
- Phase 4 目标：
  - 把 `platform_support.rs` 整体迁入 `studio-app`。
  - 把 `font_setup.rs` 中依赖本地系统字体的部分迁出 `scad-ui`，避免 `scad-ui` 继续绑定平台字体发现路径。
  - Phase 4 完成后，`scad-ui` 应只保留 `egui` / `egui_commonmark` + 共享纯 UI 组件；不再直接依赖 `muda` / `winit`。

#### `scad-scene` 当前阻塞点与 Phase 4 目标

- 证据：
  - `crates/scad-scene/Cargo.toml` 当前直接依赖 `egui-wgpu`、`fontdb`、`wgpu`、`winit`，并带 macOS / Windows target-specific deps。
  - `crates/scad-scene/src/camera.rs` 当前直接消费 `winit::event::{WindowEvent, MouseScrollDelta, MouseButton, ElementState}`。
  - `crates/scad-scene/src/renderer.rs` 当前 `Renderer::new(window: Arc<Window>)` 直接绑定 `winit::window::Window` 与 surface 创建。
  - `crates/scad-scene/src/system_fonts.rs` 直接使用 `std::fs`、`env` 与按 macOS / Linux / Windows 分叉的本地字体发现逻辑。
- Phase 4 目标：
  - 把 `camera.rs` 的输入事件从 `winit::event::*` 抽象为端无关事件枚举。
  - 把 `renderer.rs` 从 `Renderer::new(Arc<Window>)` 拆成桌面 / web 可选入口，去掉对 `winit::Window` 的强绑定。
  - 为 `system_fonts.rs` 增加 wasm 下的替代实现或禁用入口，避免 `std::fs` / 本地字体扫描进入 wasm 目标。
  - 浏览器端锁定 WebGPU，不引入 WebGL2 fallback。

### Phase 1 不可回退清单（首版）

- **桌面 GUI 启动与多窗口事件循环**
  - 证据文件：`src/main.rs`
  - 当前行为：根二进制通过 `winit` `ApplicationHandler` 启动桌面 GUI，维护 `StudioDesktopApp` 与多窗口 `HashMap<WindowId, StudioRuntime>`。
- **工作区打开、最近工作区与窗口标题更新**
  - 证据文件：`src/app.rs`、`src/workspace.rs`、`src/platform_menu.rs`
  - 当前行为：`StudioApp::set_workspace_path()` 会更新 `recent_workspaces` 与 `file_tree`；`window_title()` 基于当前 workspace 名生成标题；平台菜单包含 `Open Folder` 与 `Recent Workspaces`。
- **文档标签与会话分发**
  - 证据文件：`src/document_workspace.rs`、`src/document_session.rs`、`src/studio_document.rs`、`src/app.rs`
  - 当前行为：`DocumentWorkspace` 负责 open / activate / close / active tab 切换，`StudioApp` 通过 `active_viewer_mut()`、`active_markdown_mut()`、`active_image_mut()` 分发到具体会话。
- **Viewer / Markdown / Image 文件打开路径**
  - 证据文件：`src/viewer_tab.rs`、`src/markdown_tab.rs`、`src/image_tab.rs`
  - 当前行为：
    - `ViewerTab::open()` 根据扩展名打开 `.scad` / `.stl` / `.3mf` 并建立 watcher / OpenSCAD runner；
    - `MarkdownTab::open()` 与 `reload()` 直接读取 Markdown 文件；
    - `ImageTab::open()` + `try_load_texture()` 读取图片并生成纹理。
- **文件变更后的 watcher 驱动刷新路径**
  - 证据文件：`src/main.rs`、`src/viewer_tab.rs`、`src/markdown_tab.rs`、`src/image_tab.rs`
  - 当前行为：各 tab 通过 `FileWatcher` 发送 `UserEvent::SourceChanged/WatchError`；根事件循环在 `src/main.rs` 统一接收后触发 Viewer / Markdown / Image 刷新与错误日志。
- **Studio 内预览能力（本轮保护的是能力，不是独立 `scad-viewer` 二进制壳）**
  - 证据文件：`src/viewer_tab.rs`、`src/viewer_camera.rs`、`src/viewer_viewport.rs`、`crates/scad-viewer/src/app.rs`、`crates/scad-viewer/src/ui/*`
  - 当前行为：Studio 的 Viewer 标签页复用 `scad_viewer` 的 app/ui 能力，支持相机动作、工具栏、状态栏、侧栏、日志面板、相机 overlay 与网格 / 轴 gizmo / clip plane 等预览交互。

---

## Phase 3 执行中记录

### 2026-04-22 当前进度

- 已完成 Phase 3 的第一刀：建立 `crates/app-server-core`，先承接 preview/watch 运行时能力。
- 已完成 Phase 3 的第二刀：建立 `crates/app-server-host` 骨架，并落地 `tokio::sync::mpsc` transport adapter 的 repo-local 实现与测试。
- 已完成 Phase 3 的第三刀：把 `ExportFormat`、`PresetFile`、`Parameter*` 纯数据类型抽到 `app-server-protocol`，同时保持 `scad-data` 继续以 re-export 方式向旧调用方兼容。
- 已完成 Phase 3 的第四刀：在 `app-server-host` 中补上 `HostSession` 纯状态模型，先覆盖 session reclaim 窗口、断开时清空 in-flight / subscriptions、保留 workspace/path handles 等规则的第一批测试。
- 已完成 Phase 3 的第五刀：把 `build_export_filename` 与 preset 文件 I/O（`preset_path_for_source` / `load_presets` / `save_preset` / `delete_preset`）迁入 `app-server-core`，并让根 crate 的 viewer 逻辑改为优先消费 `app-server-core` 版本。
- 已完成 Phase 3 的第六刀：把根 crate 的直接文件读取 / canonicalize 调用改成 `app-server-core` 的 `read_text_file` / `read_binary_file` / `canonicalize_or_original`，当前 `src/` 下已无 `std::fs::read` / `read_to_string` / `File::open` 直接命中。
- 已完成 Phase 3 的第七刀：建立 `crates/studio-common`，承接 `AppConfig` / `SlicerConfig`、`DocumentState`、`ParameterStore` 与参数解析逻辑；根 crate 与 `scad-viewer` 已切到 `studio-common` / `app-server-core`，`crates/scad-data` 已从 workspace 与依赖图中移除。
- 已完成 Phase 3 的第八刀：在 `app-server-core` 中补齐 `workspace service` 与 `file.read` 协议辅助，新增 `current_workspace`、`list_workspace_entries`、`resolve_workspace_path`、`read_file_response`，并补了对应测试。
- 已完成 Phase 3 的第九刀：在 `app-server-host` 中补上 `websocket.rs`，形成单 workspace 单进程的 WebSocket host 暴露面，并落地 `websocket_smoke_roundtrip`。
- 已完成 Phase 3 的第十刀：补齐 session lifecycle / cancel / shutdown 相关具名测试与 GUI 关停 subprocess smoke example。
- 已把以下类型 / 函数迁入 `app-server-core`：
  - `OpenScadRunner`
  - `OpenScadMessage`
  - `RenderedArtifact`
  - `OpenScadError`
  - `CliOutputFormat`
  - `build_cli_args`
  - `build_preview_job_args`
  - `collect_process_logs`
  - `detect_openscad_path`
  - `resolve_openscad_path`
  - `FileWatcher`
  - `WatchMessage`
  - `WatchError`
  - `matches_path`
  - `matches_any_path`
  - `LogEntry` / `LogLevel`
- 已新增 `child_terminator` 基础抽象，当前由 `terminate_child()` 包装默认 `Child::kill()`，为后续 session cancel / child termination 留出收敛点。

### Phase 3 第一刀涉及文件

- 新增：
  - `crates/app-server-core/Cargo.toml`
  - `crates/app-server-core/src/lib.rs`
  - `crates/app-server-core/src/child_terminator.rs`
  - `crates/app-server-core/src/preview.rs`
  - `crates/app-server-core/src/watch.rs`
  - `crates/app-server-core/src/export.rs`
  - `crates/app-server-core/src/file.rs`
  - `crates/app-server-core/src/presets.rs`
  - `crates/app-server-core/tests/openscad_tests.rs`
  - `crates/app-server-core/tests/openscad_command_tests.rs`
  - `crates/app-server-core/tests/watcher_tests.rs`
  - `crates/app-server-core/tests/export_tests.rs`
  - `crates/app-server-core/tests/file_tests.rs`
  - `crates/app-server-core/tests/presets_tests.rs`
  - `crates/app-server-host/Cargo.toml`
  - `crates/app-server-host/src/lib.rs`
  - `crates/app-server-host/src/in_process.rs`
  - `crates/app-server-host/src/mpsc_transport.rs`
  - `crates/app-server-host/src/session.rs`
  - `crates/app-server-host/src/websocket.rs`
  - `crates/app-server-host/tests/mpsc_transport_tests.rs`
  - `crates/app-server-host/tests/session_tests.rs`
  - `crates/app-server-host/tests/session_lifecycle_tests.rs`
  - `crates/app-server-host/tests/websocket_smoke_roundtrip.rs`
  - `crates/app-server-host/examples/gui_shutdown_abort_smoke.rs`
  - `crates/app-server-protocol/src/export.rs`
  - `crates/app-server-protocol/src/params.rs`
  - `crates/app-server-protocol/src/presets.rs`
  - `crates/studio-common/Cargo.toml`
  - `crates/studio-common/src/lib.rs`
  - `crates/studio-common/src/config.rs`
  - `crates/studio-common/src/document.rs`
  - `crates/studio-common/src/params.rs`
  - `crates/studio-common/src/presets.rs`
  - `crates/studio-common/tests/config_tests.rs`
  - `crates/studio-common/tests/document_tests.rs`
  - `crates/studio-common/tests/params_tests.rs`
  - `crates/studio-common/tests/public_api_tests.rs`
  - `crates/app-server-core/src/config.rs`
  - `crates/app-server-core/src/workspace.rs`
  - `crates/app-server-core/tests/config_tests.rs`
  - `crates/app-server-core/tests/workspace_tests.rs`
- 依赖 / 调用点更新：
  - 根 `Cargo.toml`：新增 `app-server-core` dependency 与 workspace member
  - 根 `Cargo.toml`：新增 `app-server-host` workspace member
  - `crates/scad-viewer/Cargo.toml`：新增 `app-server-core` dependency
  - `crates/scad-data/Cargo.toml`：在删除前用于承接 `app-server-protocol` / `app-server-core` / `studio-common` 过渡依赖，随后整个 crate 已物理删除
  - `crates/scad-viewer/src/app.rs`：`LogEntry` / `LogLevel` 改由 `app-server-core` 提供
  - `src/app.rs`、`src/log_panel.rs`：日志类型改由 `app-server-core` 提供
  - `src/main.rs`：`FileWatcher`、`OpenScadMessage`、`WatchMessage`、`LogLevel` 改由 `app-server-core` 提供
  - `src/markdown_tab.rs`、`src/image_tab.rs`：`FileWatcher` / `WatchMessage` 改由 `app-server-core` 提供
  - `src/viewer_tab.rs`：preview/watch/log 运行时类型改由 `app-server-core` 提供；`AppConfig` / `DocumentState` 已切到 `studio-common`
  - `src/viewer_tab.rs`：`build_export_filename`、`load_presets`、`save_preset`、`delete_preset` 已改为消费 `app-server-core`
  - `src/document_session.rs`、`src/markdown_tab.rs`、`src/image_tab.rs`、`src/viewer_tab.rs`：文件读取与 canonicalize 已改为消费 `app-server-core` 的 file service 辅助函数
  - `src/main.rs`、`src/layout.rs`、`src/work_area.rs`：`AppConfig` 已切到 `studio-common`
  - `crates/scad-viewer/src/app.rs` 与 `src/ui/*`：`AppConfig` / `DocumentState` / `Parameter*` / `ExportFormat` 已切到 `studio-common`
  - 根 `Cargo.toml` 与 `crates/scad-viewer/Cargo.toml`：已移除 `scad-data` dependency
  - 根 `Cargo.toml` workspace members：已移除 `crates/scad-data`，新增 `crates/studio-common`

### Phase 3 第一刀验证结果（已实跑）

- `cargo test -p app-server-core`
  - 结果：通过（`openscad_tests` 2 个、`openscad_command_tests` 6 个、`watcher_tests` 4 个、`export_tests` 2 个、`presets_tests` 2 个、`file_tests` 3 个，共 19 个测试通过）。
- `cargo test -p app-server-host`
  - 结果：通过（`mpsc_transport_tests` 4 个、`session_tests` 3 个、`session_lifecycle_tests` 5 个、`websocket_smoke_roundtrip` 1 个，共 13 个测试通过）。
- `cargo test -p studio-common`
  - 结果：通过（`config_tests` 1 个、`document_tests` 3 个、`params_tests` 3 个、`public_api_tests` 2 个，共 9 个测试通过）。
- `cargo test -p app-server-host websocket_smoke_roundtrip -- --nocapture`
  - 结果：通过；已按 `workspace.current -> workspace.list -> file.read -> preview.request` 顺序完成一次真实请求往返。
- `cargo run -p app-server-host --example gui_shutdown_abort_smoke`
  - 结果：进程非 0 退出（本地实测 `exit_code: -6`），stderr 含 `warning: GUI shutdown exceeded 5s timeout, aborting process`。
- `cargo check --workspace`
  - 结果：通过。
- `cargo test --workspace`
  - 结果：通过。
- `rg "std::fs::|std::process::Command|File::open|read_to_string|notify::|stl_io::" src/ crates/scad-viewer/src/`
  - 结果：零匹配；当前根 crate 与共享 viewer lib 已不再直接触碰本地 I/O / 外部调用。
- `cargo metadata --format-version 1 --no-deps`
  - workspace packages 当前为：`app-server-core`、`app-server-protocol`、`app-server-transport`、`app-server-host`、`scad-scene`、`studio-common`、`scad-ui`、`scad-viewer`、`scad-studio`；已无 `scad-data`。

### 当前边界状态

- 已完成的迁移：
  - root crate 与共享 viewer lib 对 preview/watch/log 运行时的直接消费，已经不再依赖 `scad-data`。
  - `ExportFormat`、`PresetFile`、`ParameterDefinition`、`ParameterEntry`、`ParameterKind`、`ParameterValue`、`ParsedParameters` 已有 protocol 唯一定义点。
  - `build_export_filename` 与 preset 文件 I/O 已在 `app-server-core` 有唯一定义点；root viewer 逻辑已切到 core 版本。
  - 根 `src/` 目录下直接文件 I/O 已降为零命中；当前文件读取与路径规范化统一经过 `app-server-core` 辅助层。
  - `AppConfig` / `SlicerConfig`、`DocumentState`、`ParameterStore` 与参数解析逻辑已迁入 `studio-common`。
  - `load_config` / `save_config` / `config_file_path` 已迁入 `app-server-core`。
  - `workspace.current` / `workspace.list` / `file.read` / `preview.request` 已有 repo-local WebSocket smoke 覆盖。
  - 协议层 `workspace.open` 禁止约束已由 `workspace_open_variant_does_not_exist` 与 `workspace_open_serde_unknown_variant` 覆盖。
- 已完成的额外迁移：
  - `crates/scad-data` 已物理删除，workspace packages 当前为：`app-server-core`、`app-server-protocol`、`app-server-transport`、`app-server-host`、`scad-scene`、`studio-common`、`scad-ui`、`scad-viewer`、`scad-studio`。
- 测试迁移对应关系（原 `crates/scad-data/tests/*` → 新位置）：
  - `config_tests.rs` → `crates/app-server-core/tests/config_tests.rs` + `crates/studio-common/tests/config_tests.rs`
  - `document_tests.rs` → `crates/studio-common/tests/document_tests.rs`
  - `export_tests.rs` → `crates/app-server-core/tests/export_tests.rs`
  - `openscad_command_tests.rs` → `crates/app-server-core/tests/openscad_command_tests.rs`
  - `openscad_tests.rs` → `crates/app-server-core/tests/openscad_tests.rs`
  - `params_tests.rs` → `crates/studio-common/tests/params_tests.rs`
  - `presets_tests.rs` → `crates/app-server-core/tests/presets_tests.rs`
  - `public_api_tests.rs` → `crates/studio-common/tests/public_api_tests.rs`
  - `watcher_tests.rs` → `crates/app-server-core/tests/watcher_tests.rs`
- 结论：`app-server-core` 与 `app-server-host` 的核心纯逻辑、mpsc adapter、session lifecycle 测试、WebSocket smoke 与 GUI 关停 subprocess smoke 已具备；Phase 3 目标已满足，可以进入 Phase 4。

---

## Phase 4 执行中记录

### 2026-04-22 physical crate split 收尾

#### 完成情况

- `crates/studio-app` 已不再是占位壳：
  - `src/main.rs` 现承接原根 `src/main.rs` 的桌面多窗口运行时、菜单接线、watcher 分发、viewer 刷新链路与配置保存逻辑。
  - 新增并迁入 `src/app.rs`、`src/layout.rs`、`src/left_panel.rs`、`src/log_panel.rs`、`src/markdown_tab.rs`、`src/image_tab.rs`、`src/studio_document.rs`、`src/work_area.rs`。
  - `src/viewer_tab/` 采用 `mod.rs` / `io.rs` / `input.rs` 三段拆分，承接原根 `src/viewer_tab.rs` 的 Viewer 标签页状态、OpenSCAD 预览流、导出 / 预设逻辑与视口交互逻辑。
  - `Cargo.toml` 已接住原根包的桌面依赖与 macOS bundle metadata；`[[bin]]` 明确为 `studio-app`，并设置 `test = false`，保持与原根二进制相同的测试策略。
- 按当前仓库状态复核后确认：根 `Cargo.toml` 仍为纯 virtual workspace，根 `src/` 仍为空目录；用户列出的根业务文件在本轮开始前已经不存在，说明 Phase 4 剩余问题主要是 crate 归属漂移，而不是再次执行根目录物理搬迁。
- `crates/studio-app/src/lib.rs` 现正式导出 `app`、`layout`、`left_panel`、`log_panel`、`markdown_tab`、`image_tab`、`studio_document`、`viewer_tab`、`work_area`，并集中定义桌面运行时复用的 `UserEvent`；`src/main.rs` 改为直接消费 `studio-app` lib，而不再在二进制内重复声明私有 `mod`。
- `crates/studio-app/tests/app_state_tests.rs` 现通过 `studio_app::app::StudioApp` 走 crate API 验证状态，不再使用 `#[path = "../src/app.rs"]` 直拉源码文件；`smoke_tests.rs` 继续校验 `platform_menu::APP_NAME`。
- `crates/studio-app/src/app.rs` 已移除仅服务测试的 `#[cfg(test)]` 文档会话分支，测试改为依赖正式公开行为 `has_open_documents()`，避免测试路径与生产路径分叉。
- 根 `Cargo.toml` 已转为纯 virtual workspace：仅保留 `[workspace]`、`[workspace.package]` 与 `[workspace.dependencies]`。
- 根 `src/` 下原有业务文件与 thin wrapper 已全部删除；当前目录仍存在为空目录，但不再包含任何 Rust 源文件或业务代码。

#### 本次确认的 crate 边界

- `studio-app`：桌面入口、多窗口生命周期、平台菜单、目录/文档打开链路、viewer/markdown/image tab 的桌面运行时。
- `studio-common`：共享配置、文档/参数状态、文档工作区与最近工作区逻辑。
- `scad-ui`：共享 UI helper，包括 `image_decode`、`image_zoom_math`、`viewer_camera`、`viewer_event_routing`、`viewer_viewport`、`welcome`、`work_area_frame` 等。
- 根 workspace：不再是 Rust package，`cargo metadata --format-version 1 --no-deps` 输出的 workspace members 中已不存在 `scad-studio` package。

#### 验证结果

- `lsp_diagnostics`：已尝试对 `crates/studio-app/src` 与 `crates/studio-app/tests` 运行；当前环境缺少 `rust-analyzer` 二进制，初始化超时，无法提供语言服务诊断。
- `cargo test -p studio-app --test app_state_tests`：通过，证明 `studio-app` 状态测试已从源码路径切换到正式 crate API 且保持绿色。
- `cargo check --workspace`：通过。
- `cargo test --workspace`：通过。
- `cargo check -p studio-app --bin studio-app`：通过，证明桌面二进制入口可编译。
- `cargo check -p scad-ui --target wasm32-unknown-unknown`：通过。
- `cargo check -p scad-scene --target wasm32-unknown-unknown`：通过。
- `cargo check -p studio-web --target wasm32-unknown-unknown`：通过。
- `cargo metadata --format-version 1 --no-deps`：通过；workspace members 为 `app-server-*`、`scad-scene`、`scad-ui`、`scad-viewer`、`studio-app`、`studio-common`、`studio-web`，不含 `scad-studio` package。

#### 仍保留的后续事项

- `scad-scene::Renderer` 与 `CameraInteraction` 的端无关抽象仍未完成；当前 wasm `cargo check` 仍保持绿色，但更完整的 renderer / 输入事件平台解耦不在本次 physical crate split 收尾范围内。

---

## Phase 5 执行中记录

### 2026-04-22 transport / client foundation

#### 完成情况

- `crates/studio-common/src/app_server_client.rs`：新增面向 protocol 的共享客户端 facade，包含：
  - `AppServerTransportPort` 抽象 transport 端口；
  - `AppServerTransportEvent` / `AppServerTransportError` transport 级事件与错误；
  - `AppServerClient` 与 `AppServerClientEvent`，负责 request id 分配、handshake 状态缓存、`workspace.current` / `session.reclaim` 响应后的共享状态更新，以及 polling 式消费 server 事件。
- `crates/app-server-host/src/dispatcher.rs`：新增 `HostRequestDispatcher`，把 host 请求处理从 `runtime.rs` / `websocket.rs` 中抽成共享路径，并统一承接：
  - handshake response 生成；
  - `workspace.current`、`workspace.list`、`file.read`、`file.write_text`、`preview.request`；
  - `watch.subscribe` / `watch.unsubscribe` push 分发；
  - `cancel` / `session.reclaim` 基础处理；
  - 为兼容当前绿色基线，继续承接 `config.load` / `config.save` / `slicer.list` / `export.run`。
- `crates/app-server-host/src/runtime.rs`：in-process mpsc host 已改为复用 `HostRequestDispatcher`，workspace 重绑继续只走 `InProcessHost::rebind_workspace(PathBuf)`。
- `crates/app-server-host/src/websocket.rs`：WebSocket host 已改为复用同一份 `HostRequestDispatcher`，watch push 通过内部 channel 转成 websocket server message 发送。
- `crates/studio-app/src/transport_port.rs`：新增 `MpscAppServerTransportPort` wrapper，把现有 `MpscTransportAdapter` 包成 `studio-common::AppServerTransportPort` 可消费的适配器；此步骤只建立接线基础，**尚未**把 `DesktopProtocolClient` 全量切换到新 facade。

#### 本次新增 / 修改文件

- 新增：
  - `crates/studio-common/src/app_server_client.rs`
  - `crates/studio-common/tests/app_server_client_tests.rs`
  - `crates/app-server-host/src/dispatcher.rs`
  - `crates/app-server-host/tests/shared_dispatcher_roundtrip_tests.rs`
  - `crates/app-server-host/tests/in_process_roundtrip_tests.rs`
  - `crates/studio-app/src/transport_port.rs`
- 修改：
  - `crates/studio-common/src/lib.rs`
  - `crates/app-server-host/src/lib.rs`
  - `crates/app-server-host/src/runtime.rs`
  - `crates/app-server-host/src/session.rs`
  - `crates/app-server-host/src/websocket.rs`
  - `crates/studio-app/src/lib.rs`

#### 验证结果（已实跑）

- `lsp_diagnostics`：已尝试对 `crates/app-server-host`、`crates/studio-common`、`crates/studio-app` 运行；当前环境缺少 `rust-analyzer` 二进制，初始化超时，无法提供语言服务诊断。
- `cargo test -p studio-common --test app_server_client_tests`：通过（2 个测试），覆盖 facade 的 handshake/request/poll 基础行为。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests`：通过（1 个测试），覆盖 shared dispatcher 的 handshake → workspace.current → workspace.list → file.read → preview.request 往返。
- `cargo test -p app-server-host`：通过，覆盖 mpsc adapter、session lifecycle、websocket smoke、新 in-process roundtrip 与 shared dispatcher roundtrip。
- `cargo test -p studio-common`：通过。
- `cargo check --workspace`：通过。
- `cargo test --workspace`：通过。
- `cargo build --workspace`：通过。

#### 当前剩余的 Phase 5 工作（本次**未做**）

- `crates/studio-app/src/protocol_client.rs` 仍保留旧的桌面私有客户端实现，尚未切到 `studio-common::AppServerClient` + `MpscAppServerTransportPort`。
- `studio-app/src/` 内仍存在用户在本轮任务描述中点名的 `app_server_core` 直接消费点；本次 foundation 只建立“可切换的新 transport/client 层”，**没有**关闭所有协议旁路。
- Phase 5 计划中的 Cargo 依赖守门、源码级 `rg` 守门、完整桌面 GUI 行为回归、以及“除 `rebind_workspace(PathBuf)` 外关闭所有 host-local Rust API 旁路”的终态校验，仍待后续继续执行。
- 当前合法保留的 host-local 边界仍只有 `InProcessHost::rebind_workspace(PathBuf)`；本次没有扩展该边界的职责。

### 2026-04-22 desktop mpsc transport 接线完成

#### 本次完成情况

- `crates/studio-app/src/protocol_client.rs` 已成为桌面端统一的协议客户端：
  - 通过 `spawn_in_process_mpsc_host()` 启动同进程 host；
  - 通过 `MpscTransportAdapter` 完成 handshake / request / watch subscribe / preview / config / export 流程；
  - 不再直接 import `app_server_core`。
- `crates/studio-app/src/main.rs` 已切到 protocol client：
  - 桌面启动时先 `DesktopProtocolClient::connect()` 再加载配置；
  - `--smoke-exit` 路径已接上 `DesktopProtocolClient::run_smoke_check()`；
  - workspace 绑定通过 `DesktopProtocolClient::rebind_workspace()` 只把路径转交给 in-process host，再走 `workspace.current` / `workspace.list` / `watch.subscribe`。
- `crates/studio-app/src/markdown_tab.rs`、`image_tab.rs`、`viewer_tab/*` 的文件读取、预览、watch、预设、导出、切片器枚举都已走 protocol client，不再直接依赖 `app_server_core`。
- `crates/studio-app/Cargo.toml` 当前已不再依赖 `app-server-core`，而是依赖 `app-server-host`、`app-server-protocol`、`app-server-transport`、`studio-common` 等 Phase 5 允许的 crate。

#### 依赖守门结果

- `cargo metadata --format-version 1 --no-deps` 依赖快照：
  - `studio-app`：依赖 `app-server-host`、`app-server-protocol`、`app-server-transport`、`scad-scene`、`scad-ui`、`scad-viewer`、`studio-common`，以及桌面壳层允许项 `rfd`、`muda`、`winit`、`egui-winit`。
  - `studio-common`：仅依赖 `app-server-protocol`、`regex`、`serde`、`serde_json`，无 transport / 平台 crate。
  - `scad-ui`：依赖 `egui`、`egui_commonmark`、`image`、`log`、`scad-scene`，无 `muda` / `winit`。
  - `app-server-protocol`：仅依赖 `serde`、`serde_json`、`unicode-normalization`。
  - `app-server-transport`：仅依赖 `app-server-protocol`。
  - `app-server-core`：依赖 `app-server-protocol`、`dirs`、`notify`、`scad-scene`、`serde_json`、`studio-common`。
  - `app-server-host`：依赖 `app-server-core`、`app-server-protocol`、`app-server-transport`、`tokio`、`tokio-tungstenite` 等；`stl_io` 只在 host 测试的 dev-dependencies 中用于生成 smoke fixture。
- 结论：Phase 5 要求的 Cargo 依赖守门已满足；`studio-app` 不再直接依赖 `app-server-core`，其余禁止侧也未引入 `notify` / `stl_io` / `rfd` / `dirs` 等不该出现的依赖。

#### 源码守门结果

- `rg 'app_server_core::|std::fs::|std::process::Command|File::open|read_to_string|write!.*to_file|notify::|stl_io::|tokio::fs::' crates/studio-app/src`：零匹配。
- `rg 'std::fs::|std::process::Command|File::open|read_to_string|write!.*to_file|notify::|stl_io::|tokio::fs::|app_server_core::' crates/studio-common/src`：零匹配。
- `rg 'std::fs::|std::process::Command|File::open|read_to_string|write!.*to_file|notify::|stl_io::|tokio::fs::|app_server_core::' crates/scad-ui/src`：零匹配。
- `rg 'rebind_workspace\(' crates/studio-app/src` 当前命中：
  - `crates/studio-app/src/main.rs`：通过 `DesktopProtocolClient::rebind_workspace()` 调用
  - `crates/studio-app/src/protocol_client.rs`：内部唯一一次真实调用 `host.rebind_workspace(path.clone())`
- 结论：桌面端协议旁路已收敛到单一 host-local rebind 语义；当前保留的唯一非协议路径就是 `InProcessHost::rebind_workspace(PathBuf)`，`DesktopProtocolClient::rebind_workspace()` 只是这条路径的桌面壳层包装。

#### Smoke 与验证结果

- `cargo run -p studio-app --bin studio-app -- --smoke-exit`：通过，进程 0 退出，说明 in-process host 启动 + transport 接线 + 一次 handshake / workspace roundtrip 已完成。
- `cargo check --workspace`：通过。
- `cargo test --workspace`：通过。
- 现有桌面 smoke 测试：
  - `crates/studio-app/tests/smoke_tests.rs` 中 `desktop_smoke_roundtrip_succeeds`：通过。

#### Phase 5 结论

- 结论：桌面端已切到同进程 host + mpsc transport，`studio-app` 不再依赖 `app-server-core` 的 runtime I/O / 外部调用直连，依赖守门与源码守门均通过；Phase 5 要求已满足，可以进入 Phase 6。

---

## Phase 6 执行中记录

### 2026-04-23 browser client hardening 与 watch 闭环收尾

#### 完成情况

- 已修正 `crates/studio-web/tests/browser_smoke.rs` 的 wasm browser harness：测试不再覆盖整个 `document.body().innerHTML`，而是仅在缺少 `#studio-web-root` 时追加 root 节点，避免破坏 `wasm-bindgen-test` 自带的日志 DOM。
- 已把浏览器侧预览状态从 `studio-web` 本地原始字符串收回 `studio-common`：
  - 新增 `crates/studio-common/src/preview_state.rs`
  - 更新 `crates/studio-common/src/lib.rs`
  - 新增 `crates/studio-common/tests/preview_state_tests.rs`
  - `crates/studio-web/src/app.rs` 改为持有 `PreviewState`，不再直接持有 `preview_status` / `preview_summary` 两个原始字符串字段。
- 已接通浏览器本地 mesh 渲染链路：
  - `crates/scad-scene/src/mesh.rs` 新增 `MeshData::from_indexed_buffers(...)`
  - `crates/scad-scene/src/renderer.rs` 新增 wasm/canvas 入口 `Renderer::new_for_canvas(HtmlCanvasElement)` 与 `EguiPaintData::empty()`
  - `crates/scad-scene/tests/mesh_tests.rs` 新增 `from_indexed_buffers_builds_renderable_mesh_from_raw_buffers`
  - 新增 `crates/studio-web/src/preview_canvas.rs`
  - 更新 `crates/studio-web/src/lib.rs`、`crates/studio-web/Cargo.toml`、`crates/studio-web/src/app.rs`
  - 更新 `crates/studio-app/src/protocol_client.rs`，桌面端 preview payload 转换改为复用 `MeshData::from_indexed_buffers(...)`
  - `studio-web` 收到 `PreviewArtifact::Mesh` 后会挂载真实 `<canvas>` 并通过 `scad-scene` 渲染一帧，而不是只显示文本摘要。
- 已把 `studio-web` 的 workspace 面板从扁平目录按钮提升为真实目录树：
  - `crates/studio-web/src/app.rs` 现持有目录树缓存、展开集合与当前目录文件列表联动状态
  - 浏览器端已支持展开/折叠目录、点击目录切换当前目录、当前目录文件列表随之刷新、空目录显示空状态。
- 已增强 browser smoke 的验证强度：
  - `crates/studio-web/tests/browser_smoke.rs` 现在除了原有 workspace / preview / tree 导航外，还会验证 `navigator.gpu.requestAdapter()` 可用、`#preview-summary` 中 mesh 统计非空、`#preview-mesh-canvas` 的输出不是统一底色空白画布。
  - `crates/studio-web/src/app.rs` 新增 `id="preview-summary"`，供 smoke 稳定读取 `PreviewState` 生成的摘要文案。
  - `crates/studio-web/webdriver.json` 现使用 `--enable-unsafe-webgpu`，不再传 `--disable-gpu`。
  - `crates/studio-web/Cargo.toml` 为 browser smoke 补充了 `js-sys`、`CanvasRenderingContext2d` 与 `ImageData` 能力。
- 已补齐 archived Phase 6 缺失的 watch 行为闭环：
  - 新增 `crates/studio-common/src/watch_lifecycle.rs`
  - 更新 `crates/studio-common/src/lib.rs`
  - 新增 `crates/studio-common/tests/watch_lifecycle_tests.rs`
  - `crates/studio-web/src/app.rs` 现在会在 `WorkspaceList` 成功后建立或切换 watch，`WatchSubscribed` / `WatchUnsubscribed` 会推进共享生命周期，匹配 `WatchChanged` push 时会重拉当前目录的 `workspace.list`。
  - 新增 `crates/studio-web/tests/browser_watch_smoke.rs`
  - 更新 `tests/studio_web_smoke.sh`，脚本会清理 `tests/studio-web-smoke-workspace/watch-smoke-generated.txt`，并分别运行 `browser_smoke` 与 `browser_watch_smoke`。
  - watch smoke 会通过第二条 WebSocket 协议连接执行一次 `file.write_text`，机械化证明“文件变化 → watch push → 页面列表刷新”这条链路真实成立。

#### 本次新增 / 修改文件

- 新增：
  - `crates/studio-common/src/preview_state.rs`
  - `crates/studio-common/src/watch_lifecycle.rs`
  - `crates/studio-common/tests/preview_state_tests.rs`
  - `crates/studio-common/tests/watch_lifecycle_tests.rs`
  - `crates/studio-web/src/preview_canvas.rs`
  - `crates/studio-web/tests/browser_watch_smoke.rs`
- 修改：
  - `crates/studio-common/src/lib.rs`
  - `crates/scad-scene/src/mesh.rs`
  - `crates/scad-scene/src/renderer.rs`
  - `crates/scad-scene/tests/mesh_tests.rs`
  - `crates/studio-web/Cargo.toml`
  - `crates/studio-web/src/app.rs`
  - `crates/studio-web/src/lib.rs`
  - `crates/studio-web/tests/browser_smoke.rs`
  - `crates/studio-web/webdriver.json`
  - `crates/studio-app/src/protocol_client.rs`
  - `tests/studio_web_smoke.sh`

#### 验证结果（已实跑）

- `cargo test -p studio-common --test preview_state_tests`
  - 结果：通过（6 个测试全部通过）。
- `cargo test -p scad-scene from_indexed_buffers_builds_renderable_mesh_from_raw_buffers`
  - 结果：通过。
- `cargo test -p studio-web`
  - 结果：通过（原生 `chat_state_tests` 2 个、`public_api_tests` 1 个通过）。
- `cargo check -p studio-common`
  - 结果：通过。
- `cargo check -p scad-scene --target wasm32-unknown-unknown`
  - 结果：通过。
- `cargo check -p studio-web --target wasm32-unknown-unknown`
  - 结果：通过。
- `cargo check -p studio-web --target wasm32-unknown-unknown --tests --features browser-smoke`
  - 结果：通过。
- `cargo check -p app-server-transport`
  - 结果：通过。
- `cargo check -p app-server-transport --target wasm32-unknown-unknown`
  - 结果：通过。
- `cargo test -p studio-common --test watch_lifecycle_tests`
  - 结果：通过（3 个测试全部通过）。
- `cargo build -p studio-web --target wasm32-unknown-unknown`
  - 结果：通过。
- `bash tests/build_studio_web_shell.sh`
  - 结果：通过，构建产物输出到 `target/studio-web-shell`。
- `bash -n tests/studio_web_smoke.sh`
  - 结果：通过。
- `bash tests/studio_web_smoke.sh`
  - 结果：通过。
  - `browser_smoke` 当前 3 个测试全部通过：
    - `browser_smoke_loads_workspace_listing_and_preview`
    - `browser_smoke_mounts_mesh_preview_canvas_when_mesh_arrives`
    - `browser_smoke_directory_tree_expand_and_navigate`
  - `browser_watch_smoke` 当前 1 个测试通过：
    - `browser_smoke_refreshes_listing_after_watch_change`

#### 环境限制说明

- 本 Phase 多次尝试使用 `lsp_diagnostics` 检查 Rust 变更文件，但当前环境缺少 `rust-analyzer` 可执行文件，无法拿到可用的语言服务诊断结果。
- 因此本 Phase 的完成性判断完全基于 cargo 编译、target-specific check、shell 构建与 browser smoke 的真实输出，而不是基于 LSP 清洁断言。

#### Phase 6 结论

- 结论：`studio-web` 现已通过与桌面端相同的 protocol / transport 路径接入 `app-server-host`，并补齐了共享预览状态、本地 mesh 渲染、真实目录树、watch 订阅刷新闭环与增强 browser smoke；`app-server-transport` 在 native / wasm 双 target 下均保持通过，浏览器 shell 构建与 browser smoke 也已稳定通过。按 archived `plan-00.md` 的 Phase 6 约束，当前实现已满足 Phase 6 的硬验收，可以进入 Phase 7。

---

## Phase 7 执行中记录

### 2026-04-23 重复职责扫描与 `scad-viewer` 去留核查

#### 完成情况

- 已按当前代码事实完成 Phase 7 的重复职责扫描。由于 Phase 4 结果文档当时没有显式落下“共享状态机类型名清单”，本 Phase 未假装沿用一份并不存在的清单，而是直接用当前共享层已落地的类型名完成扫描，并把这一点作为扫描前提写入结果。
- 当前用于重复状态机扫描的共享类型名为：`PreviewState`、`DocumentState`、`DocumentWorkspace`、`DocumentTab`、`DirectoryWatchLifecycle`。
- `studio-app/src/` 与 `studio-web/src/` 中都没有重新定义以上状态机类型；定义点全部位于 `studio-common/src/`：
  - `crates/studio-common/src/preview_state.rs`：`PreviewState`
  - `crates/studio-common/src/document.rs`：`DocumentState`
  - `crates/studio-common/src/document_workspace.rs`：`DocumentTab`、`DocumentWorkspace<T>`
  - `crates/studio-common/src/watch_lifecycle.rs`：`DirectoryWatchLifecycle`
- 协议接线扫描结果如下：
  - `studio-app/src/transport_port.rs` 与 `studio-web/src/transport_port.rs` 都存在 `app_server_protocol` / `app_server_transport` 直接引用，这是当前端壳层 transport adapter 的预期接入点。
  - `studio-app/src/protocol_client.rs` 仍保留桌面专属 wrapper，用于 in-process host、`rebind_workspace(PathBuf)`、桌面 watch handler 与若干桌面运行时编排；这不是新的重复定义点，而是仍待未来进一步收敛的桌面壳层包装。
  - `studio-web/src/app.rs` 中的协议引用主要是浏览器端当前目录、preview 与 watch glue；实际 request/poll/subscribe/unsubscribe API 仍经 `studio-common::AppServerClient` 统一消费。
- UI / 责任重叠扫描结果如下：
  - `scad-ui/src/chat_panel.rs` 提供桌面端 `ChatPanel`；`studio-web/src/chat.rs` 提供网页端 `FakeChatState`。两者虽同属“聊天界面”领域，但当前并非同一个产品边界：archived Phase 6 已明确 fake chatbox 只保留在 `studio-web`，因此这里记录为**意图明确的端壳层差异**，不是当前必须收敛的重复实现。
  - `scad-ui/src/file_tree.rs` 仍是桌面端共享 `FileTree` 组件，使用 `PathBuf` / `FileTreeEntry`；`studio-web/src/app.rs` 现在有自己的 `PathHandle` 树缓存与 DOM 渲染逻辑。这里存在“文件树职责横跨两个 crate”的现象，但由于两端当前数据形态与 UI 技术栈不同（egui + `PathBuf` vs browser DOM + `PathHandle`），本 Phase 记录为**边界差异而非直接重复定义**，不在当前 Phase 内强行收敛。
  - `PreviewState` 与 `DirectoryWatchLifecycle` 均只在 `studio-common` 有定义点，`studio-web` 只消费，不存在重复状态机定义。
- 已完成 `scad-viewer` 现状核查：
  - `crates/scad-viewer/Cargo.toml` 当前只有 `[lib]`，无 `[[bin]]`。
  - `cargo metadata --format-version 1 --no-deps` 显示 `scad-viewer` 当前 targets 仅有 `scad_viewer`（lib）与 5 个测试 target，不存在任何 bin target。
  - 同一份 metadata 显示 `scad-viewer` 当前 dependencies 仅为：`app-server-core`、`egui`、`scad-scene`、`scad-ui`、`studio-common`；没有重新引入桌面应用专属依赖。
  - `rg 'scad_viewer::' crates/` 仍命中 `studio-app` 的多个运行时文件，至少包括：
    - `crates/studio-app/src/app.rs`
    - `crates/studio-app/src/log_panel.rs`
    - `crates/studio-app/src/main.rs`
    - `crates/studio-app/src/protocol_client.rs`
    - `crates/studio-app/src/viewer_tab/mod.rs`
    - `crates/studio-app/src/viewer_tab/io.rs`
  - 结论：`scad-viewer` 已经保持为纯共享 lib，没有独立应用职责回流；但它仍被 `studio-app` 运行时代码直接消费，当前**不能物理删除**，Phase 7 的正确决策是保留该 crate 为纯共享 lib。
- 已完成 workspace 终态成员 diff：`cargo metadata --format-version 1 --no-deps` 当前 packages 为 `app-server-core`、`app-server-host`、`app-server-protocol`、`app-server-transport`、`scad-scene`、`scad-ui`、`scad-viewer`、`studio-app`、`studio-common`、`studio-web`，与 archived Phase 7 预期完全一致，无缺项、无额外项。

#### 验证结果（已实跑）

- `rg 'struct\s+(PreviewState|DocumentState|DocumentWorkspace|DocumentTab|DirectoryWatchLifecycle)|enum\s+(PreviewState|DocumentState|DocumentWorkspace|DocumentTab|DirectoryWatchLifecycle)' crates/studio-app/src`
  - 结果：零匹配。
- `rg 'struct\s+(PreviewState|DocumentState|DocumentWorkspace|DocumentTab|DirectoryWatchLifecycle)|enum\s+(PreviewState|DocumentState|DocumentWorkspace|DocumentTab|DirectoryWatchLifecycle)' crates/studio-web/src`
  - 结果：零匹配。
- `rg 'struct\s+(PreviewState|DocumentState|DocumentWorkspace|DocumentTab|DirectoryWatchLifecycle)|enum\s+(PreviewState|DocumentState|DocumentWorkspace|DocumentTab|DirectoryWatchLifecycle)' crates/studio-common/src`
  - 结果：仅命中共享层唯一定义点：`PreviewState`、`DocumentState`、`DocumentTab`、`DocumentWorkspace<T>`、`DirectoryWatchLifecycle`。
- `rg 'app_server_protocol|app_server_transport' crates/studio-app/src`
  - 结果：命中 `crates/studio-app/src/transport_port.rs` 与 `crates/studio-app/src/protocol_client.rs`，符合当前桌面 transport adapter + desktop protocol wrapper 预期。
- `rg 'app_server_protocol|app_server_transport' crates/studio-web/src`
  - 结果：命中 `crates/studio-web/src/transport_port.rs` 与 `crates/studio-web/src/app.rs`，符合当前 browser transport adapter + browser glue 预期。
- `rg 'ChatPanel|FakeChatState|chat_panel|file_tree|PreviewState|DirectoryWatchLifecycle' crates/`
  - 结果：
    - `ChatPanel` 仅定义于 `crates/scad-ui/src/chat_panel.rs`，由 `studio-app` 消费。
    - `FakeChatState` 仅定义于 `crates/studio-web/src/chat.rs`。
    - `FileTree` 仅定义于 `crates/scad-ui/src/file_tree.rs`，由 `studio-app` 消费；`studio-web` 仅持有自己的 `PathHandle` 树缓存和 DOM 渲染逻辑。
    - `PreviewState` 与 `DirectoryWatchLifecycle` 仅定义于 `studio-common`，由 `studio-web` 消费。
- `cargo metadata --format-version 1 --no-deps`
  - 结果：workspace packages 与 Phase 7 预期清单完全一致；无缺项、无额外项。
- `python3` 解析 `cargo metadata --format-version 1 --no-deps`
  - 结果：`scad-viewer` 当前 targets 为 `scad_viewer`（lib）和 5 个 tests，dependencies 为 `app-server-core`、`egui`、`scad-scene`、`scad-ui`、`studio-common`。
- `cargo check --workspace`
  - 结果：通过。当前仅有 `app-server-core/src/watch.rs` 的既有 dead_code warning，未引入新的 Phase 7 回归。
- `cargo test --workspace`
  - 结果：通过。

#### Phase 7 结论

- 结论：当前代码中不存在 `PreviewState` / `DocumentState` / `DocumentWorkspace` / `DocumentTab` / `DirectoryWatchLifecycle` 的重复定义点，重复状态机问题已被收敛在 `studio-common`。`studio-app` 与 `studio-web` 仍各自保留端壳层需要的 protocol glue，这里记录为当前边界差异，而不是新的重复状态机定义。`scad-viewer` 已保持为纯共享 lib 且没有独立应用职责回流，但由于 `studio-app` 仍直接依赖其运行时类型与 viewer UI API，本 Phase 的正确处理方式是**保留 `scad-viewer` crate**，不做物理删除。按 archived Phase 7 的扫描、workspace member diff 与 `scad-viewer` 去留要求，当前 Phase 7 已完成，可以进入 Phase 8。

---

## Phase 8 执行中记录

### 2026-04-23 终态回归、shell 导出修复与文档收束

#### 完成情况

- 在 Phase 8 手工网页回归中发现 `target/studio-web-shell/index.html` 的真实加载路径存在导出缺口：浏览器控制台报错 `./studio_web.js does not provide an export named 'boot_studio_web_from_window'`。根因是 `crates/studio-web/src/lib.rs` 仅 re-export 了 Rust 函数，但没有通过 `#[wasm_bindgen]` 把页面入口真正导出给 `wasm-bindgen` 生成的 JS 模块。
- 已在 `crates/studio-web/src/lib.rs` 中补齐 wasm 导出包装：
  - `boot_studio_web(url: &str)`
  - `boot_studio_web_from_window()`
  这两个入口现已通过 `#[wasm_bindgen]` 暴露给 shell 页面脚本消费。
- 已重新构建 `studio-web` shell，并在真实浏览器中验证 `http://127.0.0.1:8010/index.html?ws=ws://127.0.0.1:39180` 能正常进入页面，不再出现导出缺失错误。
- 已完成一轮真实网页端手工回归（通过本地静态服务 + 本地 `websocket-host` + 浏览器自动操作）：
  - 页面可见 `studio-web wasm shell`
  - root workspace 加载成功，根目录文件列表显示 `README.md` 与 `model.stl`
  - 展开 `examples` 目录后，当前目录文件列表正确切到 `notes.txt`
  - preview 区显示 `preview ready`、mesh 统计摘要与 `mesh render ready`
  - fake chat 输入框可发送消息，消息列表能出现用户消息与占位回复
  - fake chat 可清空，清空后回到 `No fake chat messages yet.` 空状态
  - 当前页面会话内新增 console errors / warnings 为 0（历史会话中旧的 shell 导出错误不再复现）
- `docs/known_issues.md` 未新增条目：本轮没有发现新的“当前无法直接解决但会影响后续判断”的仓库级问题；已存在的桌面 GUI 交互自动化缺口仍作为唯一需要延续的已知问题保留。

#### 验证结果（已实跑）

- `cargo check --workspace`
  - 结果：通过。仅保留 `crates/app-server-core/src/watch.rs` 的既有 dead_code warning。
- `cargo test --workspace`
  - 结果：通过。
- `cargo check -p app-server-protocol --target wasm32-unknown-unknown`
  - 结果：通过。
- `cargo check -p app-server-transport --target wasm32-unknown-unknown`
  - 结果：通过。
- `cargo check -p scad-ui --target wasm32-unknown-unknown`
  - 结果：通过。
- `cargo check -p scad-scene --target wasm32-unknown-unknown`
  - 结果：通过。
- `cargo check -p studio-web --target wasm32-unknown-unknown`
  - 结果：通过。
- `cargo build -p studio-web --target wasm32-unknown-unknown`
  - 结果：通过。
- `cargo test -p app-server-host websocket_smoke_roundtrip -- --nocapture`
  - 结果：通过。
- `cargo test -p app-server-host`
  - 结果：通过。
- `cargo test -p app-server-core`
  - 结果：通过。
- `cargo run -p studio-app --bin studio-app -- --smoke-exit`
  - 结果：通过，桌面 smoke 可 0 退出。
- `rg 'std::fs::|read_dir|std::process::Command|File::open|read_to_string|notify::|stl_io::' crates/scad-ui/src`
  - 结果：零匹配。
- `rg 'app_server_core::|std::fs::|std::process::Command|File::open|read_to_string|write!.*to_file|notify::|stl_io::|tokio::fs::' crates/studio-app/src`
  - 结果：零匹配。
- `rg 'std::fs::|std::process::Command|File::open|read_to_string|write!.*to_file|notify::|stl_io::|tokio::fs::|app_server_core::' crates/studio-common/src`
  - 结果：零匹配。
- `cargo metadata --format-version 1 --no-deps`
  - 结果：workspace packages 与 Phase 7 预期清单一致，无缺项、无额外项。
- `bash tests/build_studio_web_shell.sh`
  - 结果：通过，shell 产物输出到 `target/studio-web-shell`。
- `bash tests/studio_web_smoke.sh`
  - 结果：通过，`browser_smoke` 3 个测试与 `browser_watch_smoke` 1 个测试全部通过。
- 本地网页端手工回归（静态服务 + `websocket-host` + 浏览器自动操作）
  - 结果：通过，目录树、当前目录文件列表、preview、fake chat 发送/清空均已验证。

#### 环境限制说明

- 桌面 GUI 逐点击交互回归能力仍然缺失，这一点继续沿用 `docs/known_issues.md` 中的既有记录，不在本轮新增重复条目。
- 本 Phase 同样无法从 `lsp_diagnostics` 获得 Rust 语言服务证据，因为当前环境仍缺少 `rust-analyzer`；最终完成性判断继续基于 cargo/build/smoke 与真实浏览器回归输出。

#### 后续扩展点（供后续会话直接承接）

- **云 Agent / 沙盒接入点**：继续以 `app-server-protocol` 作为唯一命令 / 事件面，新增远端能力时优先扩展 `ClientCapabilities` / `ServerCapabilities`，不要在端壳层私造协议。
- **新 transport 扩展点**：保持 `app-server-transport::ClientTransport` / `app-server-host` 的 host bridge 抽象，未来若引入远端 transport，应复用现有 envelope / handshake / subscribe / cancel 语义，而不是复制 `studio-web` 或 `studio-app` 的壳层逻辑。
- **`scad-viewer` 去留的后续条件**：若未来把 `studio-app` 仍直接消费的 `LogEntry` / `LogLevel` / `SlicerInstall` / viewer UI surface 继续迁出，再重新评估是否物理删除该 crate；在那之前维持纯共享 lib 形态。
- **网页端进一步强化方向**：当前已有 browser smoke 覆盖 WebGPU adapter、非空 mesh、非空白 canvas、目录树导航与 watch 刷新；后续若要增强回归，可在不改协议的前提下继续补更多页面级断言，但不应回退到端壳层私有能力路径。

#### Phase 8 结论

- 结论：当前工作树已完成 archived `plan-00.md` 的 Phase 1 至 Phase 8 要求。锁定基线相关能力没有出现新的机械化回归；桌面与网页统一协议架构已建立；`plan-00-result.md` 与 `docs/known_issues.md` 已能支持后续多会话继续推进；除已登记的桌面 GUI 逐点击自动化缺口外，本轮未发现新的未收敛阻塞项。
