# Studio UI 全面改造计划

## Context

SCAD Studio 在 2026040600 完成了基础多标签工作区框架，但存在多个架构和功能性问题：

1. **3D 渲染全窗口覆盖**：Renderer 将 3D 场景作为整个窗口的背景渲染（`CLEAR_COLOR` 覆盖全 surface），非 ViewerTab（Welcome、Markdown）也能看到深蓝色背景
2. **浮动面板定位错误**：参数面板、相机面板、日志面板使用 `ctx.content_rect()` 全局坐标而非标签页区域坐标，导致位置偏移
3. **3D 视口无鼠标/键盘响应**：camera interaction 传入的是全窗口尺寸而非视口区域，且未限制点击区域
4. **macOS 菜单结构不符合系统规范**：缺少独立的 App menu（muda 的第一个 Submenu 被 macOS 识别为 App 菜单，当前的 "File" 被吞为 App menu）
5. **欢迎页行为异常**：打开 workspace 后仍能看到欢迎标签页
6. **Markdown 渲染能力有限**：手写 pulldown-cmark 渲染，无表格/图片/语法高亮支持

本轮优先修复功能性 bug，UI 样式优化另起任务。

---

## Phase 1：Renderer 支持视口区域渲染 + 背景色修复

### 目标
- Renderer 支持在指定矩形区域内渲染 3D 场景，区域外显示纯黑背景
- 提供 egui-only 渲染模式（无 3D 场景时使用）

### 前序保护
无前序 Phase。

### 操作步骤

1. **修改 `crates/scad-scene/src/renderer.rs`**：
   - 定义新的 `APP_BG_COLOR`（纯黑 `{r: 0.0, g: 0.0, b: 0.0, a: 1.0}`），用于窗口背景
   - 修改 `render()` 签名，新增 `viewport: Option<[f32; 4]>` 参数（`[x, y, width, height]` 物理像素）
   - 场景渲染流程变更：
     a. `draw_scene_pass` 始终先用 `APP_BG_COLOR` 清屏整个 surface
     b. 当 `viewport` 为 `Some` 时，设置 `render_pass.set_viewport()` 和 `render_pass.set_scissor_rect()` 限制 3D 绘制区域
     c. 在 viewport 区域内再用 `CLEAR_COLOR`（3D 场景天空色）清一个全屏 quad，或者直接分为两个 pass：先全屏黑色 clear pass → 再 viewport 区域 3D scene pass
   - 新增 `render_egui_only(&mut self, egui_paint: EguiPaintData)` 方法：仅用 `APP_BG_COLOR` 清屏 + 绘制 egui pass，不执行 shadow/scene pass
   - 实际方案选型：**采用两段式 clear**——scene pass 使用 `LoadOp::Clear(APP_BG_COLOR)` 清整个 surface，然后在 viewport 内通过 `set_viewport` + `set_scissor_rect` 限定网格/阴影绘制范围。viewport 区域的天空色通过 3D 场景的 grid/fog 自然呈现（已有 fog_enabled 控制）

2. **更新 `EguiPaintData` 或新增 `RenderFrame` 参数结构**（如果签名过于复杂则封装）

### 涉及文件
- `crates/scad-scene/src/renderer.rs`

### 验收标准
- `render()` 接受 viewport 参数后编译通过
- `render_egui_only()` 新方法编译通过
- 现有 scad-viewer 单独运行不受影响（传入 `viewport: None` 时行为与之前一致）

---

## Phase 2：Studio 主循环集成视口渲染

### 目标
- ViewerTab 活跃时：3D 只渲染在标签页内容区域
- 非 ViewerTab 活跃时：使用 egui-only 模式渲染黑色背景
- 每个标签页的视口区域独立（切换标签时更新 viewport rect）

### 前序保护
- Phase 1 的 Renderer API 不能被改回

### 操作步骤

1. **修改 `src/viewer_tab.rs`**：
   - `run_model_tab_frame()` 返回 `viewport_rect` 作为 `ViewerUiOutcome` 的新字段
   - 将 `viewport_rect` 从 egui 逻辑坐标转换为物理像素坐标（乘以 `pixels_per_point`）

2. **修改 `src/work_area.rs`**：
   - 移除 viewer_active 时的 `Frame::NONE` 透明帧逻辑——所有标签页统一使用默认帧（背景由 Renderer 负责）
   - 非 ViewerTab 使用 `egui::Frame::default()` 即可（主题 panel_fill 已是深色）

3. **修改 `src/main.rs` 的 `render_ui()`**：
   - 从 `ViewerUiOutcome` 取出 viewport_rect，传给 `renderer.render(viewport: Some(rect))`
   - 无活跃 ViewerTab 时调用 `renderer.render_egui_only(paint_data)`
   - 将 `redraw_window()` 中的 `ViewerSceneSnapshot` 增加 `viewport_rect` 字段

4. **修改 `src/layout.rs`**：
   - `show_studio_chrome` 判断条件调整：无论当前是否 ViewerTab，status bar 和 log panel 都正常显示（不再因为 viewer 透明帧而隐藏）

### 涉及文件
- `src/main.rs`、`src/viewer_tab.rs`、`src/work_area.rs`、`src/layout.rs`

### 验收标准
- 打开 ViewerTab：3D 模型仅在标签页内容区域渲染，外部区域（左侧面板、Tab 栏、底部状态栏）为深色背景
- 切换到 MarkdownTab 或 WelcomeTab：背景为纯黑/深色，无蓝色 3D 天空
- 切换回 ViewerTab：3D 正常显示在标签内容区域内

---

## Phase 3：浮动面板和覆盖层定位修复

### 目标
- 所有 Viewer 浮动面板（参数面板、相机面板、日志面板）约束在标签页视口区域内
- Gizmo（XYZ 轴图示）正确定位在视口左下角
- 浮动面板不可拖出标签页区域

### 前序保护
- Phase 1 的 Renderer 视口支持不能破坏
- Phase 2 的标签页内 3D 渲染不能退化为全窗口

### 操作步骤

1. **修改 `crates/scad-viewer/src/ui/side_panel.rs`**：
   - `show()` 新增 `viewport_rect: egui::Rect` 参数
   - 默认位置改为相对 `viewport_rect` 计算（右上角偏移）
   - 使用 `egui::Window::constrain_to(viewport_rect)` 限制拖动范围
   - 保存/恢复位置时使用相对于 viewport_rect 的偏移量，而非绝对坐标

2. **修改 `crates/scad-viewer/src/ui/camera_overlay.rs`**：
   - 同上，`show()` 新增 `viewport_rect` 参数
   - 默认位置和约束范围改为 viewport 相对

3. **修改 `crates/scad-viewer/src/ui/log_panel.rs`**：
   - 同上处理

4. **修改 `crates/scad-scene/src/gizmo.rs`**：
   - `overlay_center()` 已经正确使用 viewport_rect——确认 viewport_rect 传入值正确即可
   - 验证 gizmo 在视口左下角正确显示

5. **修改 `crates/scad-viewer/src/ui/mod.rs` 的 `show_viewer_overlays()`**：
   - 将 viewport_rect 传递给所有子面板

6. **修改 `src/viewer_tab.rs`**：
   - 确保传给 `show_viewer_overlays()` 的 viewport_rect 是正确的标签内容区域（不包含工具栏和状态栏）

### 涉及文件
- `crates/scad-viewer/src/ui/side_panel.rs`
- `crates/scad-viewer/src/ui/camera_overlay.rs`
- `crates/scad-viewer/src/ui/log_panel.rs`
- `crates/scad-viewer/src/ui/mod.rs`
- `crates/scad-scene/src/gizmo.rs`
- `src/viewer_tab.rs`

### 验收标准
- 参数面板、相机面板、日志面板默认出现在视口区域内
- 拖动面板无法超出视口区域边界
- XYZ 轴 gizmo 出现在视口区域左下角
- 窗口缩放后面板位置仍在视口区域内

---

## Phase 4：3D 视口鼠标/键盘交互修复

### 目标
- 鼠标操作（旋转、缩放、平移相机）正确限制在 3D 视口区域内
- 点击视口外区域（左面板、Tab 栏）不触发相机操作
- 键盘快捷键（W/E 切面编辑模式）不与 egui 输入冲突

### 前序保护
- Phase 2 的视口 rect 计算结果不能被改动
- Phase 3 的面板约束逻辑不能被破坏

### 操作步骤

1. **在 `StudioRuntime` 或 `StudioApp` 中记录当前活跃视口区域** `last_viewport_rect: Option<egui::Rect>`
   - 在 `redraw_window()` 中从 ViewerUiOutcome 提取并保存

2. **修改 `src/main.rs` 的 window_event 处理**：
   - 鼠标事件（MouseInput、CursorMoved、MouseWheel）传给 `handle_window_event()` 前，检查鼠标位置是否在 viewport_rect 内
   - 在 viewport_rect 外的鼠标事件不传递给 ViewerTab
   - `viewport_size` 参数改为传入实际 viewport rect 的尺寸，而非整个窗口尺寸

3. **修改 `src/viewer_tab.rs` 的 `handle_window_event()`**：
   - 接收 `viewport_rect: egui::Rect` 参数（替代 `viewport_size: Vec2`）
   - 将鼠标坐标转换为相对于 viewport_rect 的本地坐标
   - `CameraInteraction` 使用 viewport 尺寸而非窗口尺寸

4. **修改 `crates/scad-scene/src/camera.rs`（CameraInteraction）**：
   - 确认 `handle_event()` 使用的坐标系与视口一致
   - 如果 CameraInteraction 当前使用全窗口坐标，需要增加 viewport offset 参数

### 涉及文件
- `src/main.rs`
- `src/viewer_tab.rs`
- `crates/scad-scene/src/camera.rs`（如需修改）

### 验收标准
- 在 3D 视口内拖动鼠标旋转相机正常
- 在左面板/Tab 栏/状态栏区域操作不影响 3D 相机
- 滚轮缩放仅在视口内生效
- 截面编辑（W/E + 拖动）在视口内正常工作

---

## Phase 5：macOS App Menu 修复

### 目标
- macOS 菜单栏显示标准结构：App Name menu → File → View → Help
- App Name menu 包含 About、Separator、Quit
- File menu 恢复独立显示

### 前序保护
- Phase 1-4 的所有功能不受影响

### 操作步骤

1. **修改 `src/platform_menu.rs` 的 `build_menu()`**：
   - 在 macOS 上，先创建 App menu（Submenu 名为 `APP_NAME`），包含：
     - `About {APP_NAME}`（MenuItem）
     - Separator
     - `Quit {APP_NAME}`（MenuItem，Cmd+Q）
   - 从 File menu 中移除 About 和 Quit（这两项已在 App menu 中）
   - 菜单挂载顺序：App menu → File → View → Help
   - 非 macOS 平台保持原有结构（About 在 Help、Quit 在 File）

2. **使用条件编译 `#[cfg(target_os = "macos")]`** 分别处理菜单结构

3. **更新 `PlatformMenu` 结构体**：
   - `about_id` 和 `quit_id` 已是 `Option<String>`，兼容跨平台差异

### 涉及文件
- `src/platform_menu.rs`

### 验收标准
- macOS 上菜单栏显示：`SCAD Studio | File | View | Help`
- App menu（SCAD Studio）包含 About 和 Quit
- File menu 包含 New Window、Open Folder、Recent Workspaces、Close Window
- Windows 上菜单结构不变

---

## Phase 6：欢迎页行为修复

### 目标
- 打开 workspace 后，欢迎页自动关闭且不再允许打开
- 未打开 workspace 时，欢迎页作为唯一入口

### 前序保护
- Phase 1-5 的功能不受影响

### 操作步骤

1. **修改 `src/app.rs` 的 `set_workspace_path()`**：
   - 打开 workspace 后，如果存在 WelcomeTab 则关闭

2. **修改 `src/app.rs` 的 `ensure_welcome_tab()`**：
   - 增加条件：仅在 `workspace_path.is_none()` 且无标签页时创建欢迎标签页
   - 已有 workspace 时不再创建欢迎页

3. **修改 `src/welcome.rs` 的 `WelcomeTab`**：
   - 当 workspace 已打开时，`is_closable()` 返回 true（允许关闭）
   - 或者直接不再创建

### 涉及文件
- `src/app.rs`
- `src/welcome.rs`（可能）

### 验收标准
- 启动应用无 workspace：显示欢迎页
- 通过欢迎页或菜单打开 workspace：欢迎页自动关闭
- 关闭所有标签页后（有 workspace）：不再弹出欢迎页，显示空白工作区
- 关闭所有标签页后（无 workspace）：显示欢迎页

---

## Phase 7：Markdown 渲染方案升级

### 目标
- 使用 `egui_commonmark` 替换手写 Markdown 渲染
- 支持表格、图片、代码语法高亮等标准 CommonMark 功能

### 前序保护
- Phase 1-6 的功能不受影响

### 操作步骤

1. **在 `Cargo.toml` 中添加依赖**：
   - `egui_commonmark = { version = "0.19", features = ["better_syntax_highlighting"] }`（版本号需查 Context7 确认与 egui 0.33 兼容的版本）

2. **修改 `crates/scad-ui/src/markdown.rs`**：
   - 用 `egui_commonmark::CommonMarkViewer` 替换手写 `MarkdownDocument`
   - `MarkdownDocument` 保留接口（`parse()`、`show()`），内部实现改用 egui_commonmark
   - 或直接简化为包装 `CommonMarkViewer`

3. **修改 `src/markdown_tab.rs`**：
   - 适配新的 MarkdownDocument 接口
   - `CommonMarkCache` 需要持久化在 MarkdownTab 中（egui_commonmark 的缓存机制）

4. **清理 `crates/scad-ui/Cargo.toml`**：
   - 如果 egui_commonmark 替代了 pulldown-cmark 的直接使用，移除 pulldown-cmark 依赖

### 涉及文件
- `Cargo.toml`（workspace）
- `crates/scad-ui/Cargo.toml`
- `crates/scad-ui/src/markdown.rs`
- `src/markdown_tab.rs`

### 验收标准
- 打开 .md 文件显示正确渲染的 Markdown（标题、段落、列表、代码块）
- 表格正确渲染
- 代码块有语法高亮
- 文件热重载仍正常工作

---

## Phase 8：UI 样式优化（后续任务，本轮不执行）

标记为后续独立任务，本轮不执行。包括：
- 参考 VS Code/Zed 重新设计面板布局结构
- Tab 栏样式优化
- 左面板（Chat/Files）交互优化
- 全局间距、字体层级、交互反馈优化

---

## 验证方案

每个 Phase 完成后的验证流程：

```
1. cargo build --release 2>&1 | head -50   # 编译通过
2. cargo test                                # 测试通过
3. cargo run                                 # 启动应用，手动验证：
   - 欢迎页 → 打开 workspace → 欢迎页消失
   - 打开 .scad 文件 → 3D 渲染仅在标签内容区
   - 切换到 Markdown 标签 → 背景黑色，无蓝色
   - 浮动面板在视口内，不可拖出
   - 鼠标在视口内旋转相机正常
   - macOS 菜单栏显示 App Name | File | View | Help
```

## 执行顺序与依赖

```
Phase 1 (Renderer)
    ↓
Phase 2 (Studio 集成)
    ↓
Phase 3 (面板定位) ←─── Phase 4 (输入修复) [可并行]
    ↓
Phase 5 (macOS 菜单) ← 独立
Phase 6 (欢迎页) ← 独立
Phase 7 (Markdown) ← 独立
```

Phase 5/6/7 互相独立，可在 Phase 4 完成后并行推进。
