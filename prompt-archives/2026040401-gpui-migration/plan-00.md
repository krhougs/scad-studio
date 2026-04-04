# gpui 迁移实施计划（Workspace 版）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- []`) syntax for tracking.

**Goal:** 将 scad-studio 从单 crate 拆分为 Cargo workspace，UI 层从 egui 迁移到 gpui，3D 渲染封装为独立 gpui ViewComponent。

**Architecture:** 渐进式迁移——先拆 workspace + 解耦渲染层，再验证 gpui 纹理嵌入，最后迁移 UI。渲染输出到 offscreen texture，通过 gpui canvas 嵌入。macOS 用 Metal direct，其他平台走 wgpu。

**Tech Stack:** gpui (crates.io 0.2.2), wgpu 27, Metal (macOS), glam, bytemuck

---

## 最终 Workspace 结构

```
scad-studio/
├── Cargo.toml                     -- workspace root
├── crates/
│   ├── scene/                     -- 3D 渲染核心（零 UI 依赖）
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs           -- RenderMode, ColorMode, ProjectionMode, RenderSettings
│   │       ├── renderer.rs        -- SceneBackend trait + WgpuRenderer 实现
│   │       ├── pipeline.rs        -- 渲染管线创建
│   │       ├── scene_bindings.rs  -- Uniform buffer / bind group
│   │       ├── mesh.rs            -- Vertex, MeshData, Bounds
│   │       ├── camera.rs          -- OrbitalCamera, CameraInteraction
│   │       ├── grid.rs            -- 地面网格
│   │       ├── lighting.rs        -- 光源管理
│   │       ├── shadow.rs          -- Shadow map
│   │       ├── section.rs         -- 截面渲染
│   │       ├── cross_section.rs   -- 切割平面
│   │       └── shaders/
│   │           ├── shader.wgsl
│   │           ├── shader_xray.wgsl
│   │           ├── shader_grid.wgsl
│   │           ├── shader_section.wgsl
│   │           ├── shader_shadow.wgsl
│   │           └── shader_blur.wgsl
│   │
│   ├── scad-data/                 -- 数据处理（零 UI / 渲染依赖）
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── openscad.rs        -- OpenSCAD CLI 调用
│   │       ├── three_mf.rs        -- 3MF 解析
│   │       ├── document.rs        -- 文档状态、参数、预设
│   │       ├── params.rs          -- 参数解析
│   │       ├── presets.rs         -- 预设管理
│   │       ├── export.rs          -- 导出功能
│   │       ├── config.rs          -- 配置持久化
│   │       ├── watcher.rs         -- 文件监控
│   │       └── mesh.rs            -- MeshData 的 IO（STL 加载等）
│   │
│   └── scad-ui/                   -- gpui UI 层 + 3D 视口嵌入
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── app.rs             -- App 状态、ViewerState、UiActions
│           ├── viewport.rs        -- ViewportComponent（嵌入 scene 渲染结果）
│           ├── toolbar.rs
│           ├── side_panel.rs
│           ├── status_bar.rs
│           ├── log_panel.rs
│           ├── camera_overlay.rs
│           ├── param_editor.rs
│           ├── settings_dialog.rs
│           ├── gizmo.rs
│           └── theme.rs
│
├── src/
│   └── main.rs                    -- 最终二进制入口（可能只做 thin wrapper）
│
├── tests/                         -- 集成测试（保留，更新路径）
└── prompt-archives/
```

---

## Phase 0: PoC 验证

**目标：** 验证 gpui 窗口 + wgpu 渲染纹理嵌入可行性。

**保护目标：** 现有代码不动，PoC 在独立分支进行。

### Task 0.1: 创建 PoC 分支和项目

- [ ] **Step 1: 创建分支**

```bash
git checkout -b poc/gpui-texture-embedding
```

- [ ] **Step 2: 创建 PoC 目录和 Cargo.toml**

```bash
mkdir -p poc-gpui/src
```

```toml
# poc-gpui/Cargo.toml
[package]
name = "poc-gpui"
version = "0.1.0"
edition = "2024"

[dependencies]
gpui = { git = "https://github.com/zed-industries/zed.git", package = "gpui" }
wgpu = "27"
bytemuck = { version = "1.24", features = ["derive"] }
glam = "0.30"
pollster = "0.4"
```

- [ ] **Step 3: 最小 gpui 窗口**

```rust
// poc-gpui/src/main.rs
use gpui::*;

struct PocApp;

impl gpui::App for PocApp {
    fn init(app: &mut App) {
        app.open_window::<Self>(WindowOptions::default()).unwrap();
    }
}

impl gpui::Render for PocApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .size_full()
            .bg(rgb(0x1a1a2e))
            .child("gpui PoC")
    }
}

fn main() {
    gpui::application(|_| PocApp).run();
}
```

- [ ] **Step 4: 编译验证**

```bash
cd poc-gpui && cargo check 2>&1
```

### Task 0.2: 验证 canvas API 和纹理嵌入

- [ ] **Step 1: 实现 wgpu offscreen 渲染到 texture，通过 gpui paint_image 嵌入**
- [ ] **Step 2: 验证鼠标事件透传**
- [ ] **Step 3: 编写验证结果文档**

### Task 0.3: 验证 macOS Metal 桥接

- [ ] **Step 1: 调研 gpui_macos Metal 渲染路径**
- [ ] **Step 2: 验证 Metal texture 共享方案**

---

## Phase 1: Workspace 拆分

**目标：** 将单 crate 拆为 workspace，按职责分出 scene / scad-data / scad-ui 三个 crate。

**保护目标：** 所有现有功能不变，编译通过，测试通过。

### Task 1.1: 创建 workspace 骨架

**Files:**
- Create: `Cargo.toml` (workspace root, 重写)
- Create: `crates/scene/Cargo.toml`
- Create: `crates/scene/src/lib.rs`
- Create: `crates/scad-data/Cargo.toml`
- Create: `crates/scad-data/src/lib.rs`
- Create: `crates/scad-ui/Cargo.toml`
- Create: `crates/scad-ui/src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: 创建 workspace root Cargo.toml**

```toml
[workspace]
members = [
    "crates/scene",
    "crates/scad-data",
    "crates/scad-ui",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"

[workspace.dependencies]
wgpu = "27"
glam = "0.30"
bytemuck = { version = "1.24", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
log = "0.4"
```

- [ ] **Step 2: 创建 scene crate Cargo.toml**

```toml
[package]
name = "scene"
version.workspace = true
edition.workspace = true

[dependencies]
wgpu.workspace = true
glam.workspace = true
bytemuck.workspace = true
log.workspace = true
```

- [ ] **Step 3: 创建 scad-data crate Cargo.toml**

```toml
[package]
name = "scad-data"
version.workspace = true
edition.workspace = true

[dependencies]
serde.workspace = true
serde_json.workspace = true
log.workspace = true
regex = "1.12"
roxmltree = "0.21.1"
zip = { version = "8.5.0", default-features = false, features = ["deflate"] }
stl_io = "0.7"
notify = "8.2"
dirs = "6"
glam.workspace = true
bytemuck.workspace = true
```

- [ ] **Step 4: 创建 scad-ui crate Cargo.toml**

```toml
[package]
name = "scad-ui"
version.workspace = true
edition.workspace = true

[dependencies]
scene = { path = "../scene" }
scad-data = { path = "../scad-data" }
gpui = { git = "https://github.com/zed-industries/zed.git", package = "gpui" }
glam.workspace = true
serde.workspace = true
serde_json.workspace = true
log.workspace = true
rfd = "0.15"
```

- [ ] **Step 5: 更新 src/main.rs**

```rust
// src/main.rs
fn main() {
    scad_ui::run();
}
```

- [ ] **Step 6: 创建空的 lib.rs 和编译验证**

```bash
cargo check 2>&1
```

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/ src/main.rs
git commit -m "chore: scaffold workspace with scene, scad-data, scad-ui crates"
```

### Task 1.2: 迁移渲染代码到 scene crate

**Files:**
- Move: `src/renderer.rs` → `crates/scene/src/renderer.rs`
- Move: `src/pipeline.rs` → `crates/scene/src/pipeline.rs`
- Move: `src/scene_bindings.rs` → `crates/scene/src/scene_bindings.rs`
- Move: `src/mesh.rs` → `crates/scene/src/mesh.rs`
- Move: `src/camera.rs` → `crates/scene/src/camera.rs`
- Move: `src/grid.rs` → `crates/scene/src/grid.rs`
- Move: `src/lighting.rs` → `crates/scene/src/lighting.rs`
- Move: `src/shadow.rs` → `crates/scene/src/shadow.rs`
- `src/section.rs` → `crates/scene/src/section.rs`
- Move: `src/cross_section.rs` → `crates/scene/src/cross_section.rs`
- Move: `src/shader*.wgsl` → `crates/scene/src/shaders/`
- Create: `crates/scene/src/types.rs`

- [ ] **Step 1: 创建 types.rs（渲染枚举 + RenderSettings）**

```rust
// crates/scene/src/types.rs
use glam::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Solid,
    Wireframe,
    XRay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Color,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionMode {
    Perspective,
    Orthographic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderSettings {
    pub render_mode: RenderMode,
    pub color_mode: ColorMode,
    pub projection_mode: ProjectionMode,
    pub wireframe_supported: bool,
    pub show_grid: bool,
    pub show_build_plate: bool,
    pub show_axis_gizmo: bool,
    pub shadows_enabled: bool,
    pub fog_enabled: bool,
    pub clip_plane_enabled: bool,
}
```

- [ ] **Step 2: 移动渲染文件，更新 crate 内路径**

将所有 `use crate::app::{...}` 替换为 `use crate::types::{...}`。将 `use egui_wgpu::wgpu` 替换为 `use wgpu`。移除所有 egui 相关 import。

- [ ] **Step 3: 编写 lib.rs 导出**

```rust
// crates/scene/src/lib.rs
pub mod types;
pub mod renderer;
pub mod pipeline;
pub mod scene_bindings;
pub mod mesh;
pub mod camera;
pub mod grid;
pub mod lighting;
pub mod shadow;
pub mod section;
pub mod cross_section;

pub use renderer::Renderer;
pub use camera::{OrbitalCamera, CameraMatrices, CameraInteraction};
pub use mesh::{MeshData, Vertex, Bounds};
pub use cross_section::ClipPlane;
pub use types::*;
```

- [ ] **Step 4: 移除 scene crate 中的 egui 依赖**

从 renderer.rs 中删除 `egui_renderer` 字段、`EguiPaintData`、`draw_egui_pass`、`upload_egui_resources`、`release_egui_textures`、`screen_descriptor` 等方法。

render 方法签名改为：

```rust
pub fn render(
    &mut self,
    camera: &OrbitalCamera,
    settings: &RenderSettings,
    clip_plane: Option<&ClipPlane>,
) -> Result<(), RendererError>
```

- [ ] **Step 5: 编译 scene crate**

```bash
cargo check -p scene 2>&1
```

- [ ] **Step 6: Commit**

```bash
git add crates/scene/
git commit -m "refactor: move render code to scene crate, remove egui dependency"
```

### Task 1.3: 迁移数据处理到 scad-data crate

**Files:**
- Move: `src/openscad.rs` → `crates/scad-data/src/openscad.rs`
- Move: `src/three_mf.rs` → `crates/scad-data/src/three_mf.rs`
- Move: `src/document.rs` → `crates/scad-data/src/document.rs`
- Move: `src/params.rs` → `crates/scad-data/src/params.rs`
- Move: `src/presets.rs` → `crates/scad-data/src/presets.rs`
- Move: `src/export.rs` → `crates/scad-data/src/export.rs`
- Move: `src/config.rs` → `crates/scad-data/src/config.rs`
- Move: `src/watcher.rs` → `crates/scad-data/src/watcher.rs`

- [ ] **Step 1: 移动文件，处理依赖关系**

openscad.rs / export.rs / document.rs 可能引用 mesh.rs —— 将 MeshData 的 IO 部分（STL 加载）也移到 scad-data，或在 scad-data 中定义基础 MeshData，scene crate 依赖 scad-data 获取。

**决策**: MeshData 定义放在 scene crate（因为渲染需要），scad-data 依赖 scene crate 获取 MeshData 类型。three_mf.rs 和 stl_io 的加载结果产出 scene::MeshData。

```toml
# crates/scad-data/Cargo.toml
[dependencies]
scene = { path = "../scene" }
# ...
```

- [ ] **Step 2: 更新 crate 内路径引用**

将 `use crate::mesh::` 改为 `use scene::mesh::` 或 `use scene::MeshData`。

- [ ] **Step 3: 编写 lib.rs 导出**

```rust
// crates/scad-data/src/lib.rs
pub mod openscad;
pub mod three_mf;
pub mod document;
pub mod params;
pub mod presets;
pub mod export;
pub mod config;
pub mod watcher;
```

- [ ] **Step 4: 编译 scad-data crate**

```bash
cargo check -p scad-data 2>&1
```

- [ ] **Step 5: Commit**

```bash
git add crates/scad-data/
git commit -m "refactor: move data processing to scad-data crate"
```

### Task 1.4: 暂时的 scad-ui 入口（桥接旧代码）

**Files:**
- Modify: `crates/scad-ui/src/lib.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: scad-ui 依赖 scene + scad-data，暴露最小入口**

```rust
// crates/scad-ui/src/lib.rs
pub use scene;
pub use scad_data;

pub fn run() {
    // TODO: gpui Application 入口
    println!("scad-ui placeholder");
}
```

- [ ] **Step 2: 更新 src/main.rs**

```rust
fn main() {
    scad_ui::run();
}
```

- [ ] **Step 3: 编译验证整个 workspace**

```bash
cargo check 2>&1
```

- [ ] **Step 4: 运行测试**

```bash
cargo test --workspace 2>&1
```

- [ ] **Step 5: Commit**

```bash
git add crates/scad-ui/ src/main.rs
git commit -m "chore: wire up scad-ui as thin entry point"
```

### Task 1.5: 迁移测试到对应 crate

- [ ] **Step 1: 将 tests/ 目录中的测试按职责移动**

- 渲染相关测试 → `crates/scene/tests/`
- 数据处理相关测试 → `crates/scad-data/tests/`
- UI 相关测试 → `crates/scad-ui/tests/`（或暂时删除 egui 相关测试）

- [ ] **Step 2: 更新测试中的 use 路径**

```rust
// 旧: use scad_studio::camera::OrbitalCamera;
// 新: use scene::camera::OrbitalCamera;
```

- [ ] **Step 3: 更新每个 crate 的 Cargo.toml 添加 dev-dependencies**

- [ ] **Step 4: 运行全部测试**

```bash
cargo test --workspace 2>&1
```

- [ ] **Step 5: Commit**

```bash
git add tests/ crates/*/tests/
git commit -m "chore: migrate tests to corresponding workspace crates"
```

### Task 1.6: 清理旧的 src/ 目录

- [ ] **Step 1: 删除已迁移的旧文件**

```bash
rm src/renderer.rs src/pipeline.rs src/scene_bindings.rs src/mesh.rs
rm src/camera.rs src/grid.rs src/lighting.rs src/shadow.rs
rm src/section.rs src/cross_section.rs src/shader*.wgsl
rm src/openscad.rs src/three_mf.rs src/document.rs src/params.rs
rm src/presets.rs src/export.rs src/config.rs src/watcher.rs
rm src/blur.rs
```

- [ ] **Step 2: 更新 main.rs 的 mod 声明**

删除已迁移模块的 mod 声明，仅保留 `mod platform_menu; mod system_fonts;` 和 `mod ui;`（临时）。

- [ ] **Step 3: 编译验证**

```bash
cargo check 2>&1
```

- [ ] **Step 4: Commit**

```bash
git add src/
git commit -m "chore: remove migrated source files from old src/"
```

---

## Phase 2: gpui 集成

**目标：** 实现 gpui 窗口管理和 3D 视口嵌入。

**保护目标：** Phase 1 的 workspace 结构不变，scene crate 接口不变。

### Task 2.1: 实现 gpui Application 入口

**Files:**
- Modify: `crates/scad-ui/src/lib.rs`
- Create: `crates/scad-ui/src/app.rs`

- [ ] **Step 1: 实现 gpui App + Render**

```rust
// crates/scad-ui/src/app.rs
use gpui::*;

pub struct ScadStudioApp {
    // 持有 ViewerState, DocumentState 等
}

impl gpui::App for ScadStudioApp {
    fn init(cx: &mut App) {
        cx.open_window::<Self>(WindowOptions {
            title: Some("scad-studio".into()),
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::new(100.0, 100.0),
                size: Size::new(1280.0, 800.0),
            })),
            ..Default::default()
        }).unwrap();
    }
}

impl gpui::Render for ScadStudioApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child("scad-studio loading...")
    }
}
```

- [ ] **Step 2: 更新 lib.rs 入口**

```rust
pub fn run() {
    gpui::application(|_| app::ScadStudioApp).run();
}
```

- [ ] **Step 3: 编译验证**

```bash
cargo check -p scad-ui 2>&1
```

- [ ] **Step 4: Commit**

### Task 2.2: 实现 ViewportComponent

**Files:**
- Create: `crates/scad-ui/src/viewport.rs`

- [ ] **Step 1: 实现 ViewportComponent**

```rust
use gpui::*;
use scene::{Renderer, OrbitalCamera, CameraInteraction, RenderSettings, ClipPlane, MeshData};

pub struct ViewportComponent {
    renderer: Renderer,
    camera: OrbitalCamera,
    camera_interaction: CameraInteraction,
    clip_plane: ClipPlane,
    render_settings: RenderSettings,
}
```

- [ ] **Step 2: 实现纹理嵌入**

根据 PoC 结果选择方案，将 renderer 输出嵌入 gpui canvas 区域。

- [ ] **Step 3: 实现鼠标事件透传**

```rust
impl ViewportComponent {
    fn handle_mouse_down(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) { ... }
    fn handle_mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) { ... }
    fn handle_scroll(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) { ... }
}
```

- [ ] **Step 4: 编译验证 + Commit**

### Task 2.3: 根布局组装

**Files:**
- Modify: `crates/scad-ui/src/app.rs`

- [ ] **Step 1: 组装根视图**

```rust
impl gpui::Render for ScadStudioApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(ToolbarView::new(...))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .child(ViewportComponent::new(...))
                    .child(SidePanelView::new(...))
            )
            .child(StatusBarView::new(...))
    }
}
```

- [ ] **Step 2: Commit**

---

## Phase 3: UI 面板迁移

**目标：** 所有 egui UI 面板用 gpui 重写。

**保护目标：** ViewportComponent 正常工作，渲染功能不变。

### Task 3.1: Toolbar

**Files:**
- Create: `crates/scad-ui/src/toolbar.rs`

- [ ] **Step 1: 实现 ToolbarView**
- [ ] **Step 2: Commit**

### Task 3.2: SidePanel + ParamEditor

**Files:**
- Create: `crates/scad-ui/src/side_panel.rs`
- Create: `crates/scad-ui/src/param_editor.rs`

- [ ] **Step 1: 实现 SidePanelView**
- [ ] **Step 2: 实现 ParamEditor 控件**
- [ ] **Step 3: Commit**

### Task 3.3: StatusBar

**Files:**
- Create: `crates/scad-ui/src/status_bar.rs`

- [ ] **Step 1: 实现 StatusBarView**
- [ ] **Step 2: Commit**

### Task 3.4: LogPanel

**Files:**
- Create: `crates/scad-ui/src/log_panel.rs`

- [ ] **Step 1: 实现 LogPanelView**
- [ ] **Step 2: Commit**

### Task 3.5: CameraOverlay

**Files:**
- Create: `crates/scad-ui/src/camera_overlay.rs`

- [ ] **Step 1: 实现 CameraOverlayView**
- [ ] **Step 2: Commit**

### Task 3.6: SettingsDialog

**Files:**
- Create: `crates/scad-ui/src/settings_dialog.rs`

- [ ] **Step 1: 实现 SettingsDialogView**
- [ ] **Step 2: Commit**

### Task 3.7: Gizmo

**Files:**
- Create: `crates/scad-ui/src/gizmo.rs`

- [ ] **Step 1: 用 gpui paint_path 绘制坐标轴**
- [ ] **Step 2: Commit**

---

## Phase 4: 清理收尾

### Task 4.1: 移除 egui 依赖

- [ ] **Step 1: 从旧 Cargo.toml 移除 egui/egui-wgpu/egui-winit/winit**
- [ ] **Step 2: 删除旧 src/ui/ 目录和 blur.rs**
- [ ] **Step 3: 删除旧 src/platform_menu.rs / system_fonts.rs（如不再需要）**
- [ ] **Step 4: 编译验证**

### Task 4.2: 更新测试

- [ ] **Step 1: 更新全部测试路径引用**
- [ ] **Step 2: cargo test --workspace**

### Task 4.3: 最终验证

- [ ] **Step 1: cargo check --workspace**
- [ ] **Step 2: cargo test --workspace**
- [ ] **Step 3: 手动运行应用验证**
- [ ] **Step 4: 更新 docs/known_issues.md**
- [ ] **Step 5: 最终 Commit**

---

## 风险矩阵

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| gpui 不暴露 wgpu Device/Queue | macOS 上无法共享 GPU 资源 | Phase 0 验证 Metal 桥接方案 |
| gpui API 不稳定 | 后续升级困难 | 锁定版本；viewport.rs 隔离 gpui 代码 |
| MeshData 跨 crate 依赖方向 | scad-data 依赖 scene 引入循环风险 | MeshData 基础类型可考虑提为独立 crate |
| 鼠标事件坐标转换 | 3D 视口交互异常 | Phase 0 严格验证 |
| 测试迁移遗漏 | 回归 | 逐 crate 验证 test 通过 |
