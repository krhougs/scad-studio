# Studio UI 全面改造执行结果

## 执行时间

- 2026-04-06

## Phase 1：Renderer 支持视口区域渲染 + 背景色修复

- 完成情况：已完成。
- 变更摘要：
  - `crates/scad-scene/src/renderer.rs` 新增应用背景清屏色，并让 `render()` 接受可选视口参数。
  - 3D scene pass 改为全窗口黑色清屏后，仅在视口矩形内设置 viewport/scissor 绘制网格、阴影和模型。
  - 新增 `render_egui_only()`，在没有活跃 ViewerTab 时只清黑背景并绘制 egui。
- 前序保护：
  - 保持了 `scad-viewer` 独立运行场景，独立 viewer 继续使用 `viewport: None` 的全窗口渲染路径。

## Phase 2：Studio 主循环集成视口渲染

- 完成情况：已完成。
- 变更摘要：
  - `src/viewer_tab.rs` 将 Viewer 标签页拆为工具栏、视口、状态栏三段布局，并返回视口物理像素矩形。
  - `src/main.rs` 在 StudioRuntime 中记录最近一次视口矩形；有活跃 ViewerTab 时把该矩形传给 renderer，无活跃 ViewerTab 时改走 `render_egui_only()`。
  - `src/work_area.rs` 移除了 ViewerTab 的透明中央面板特判，所有标签页统一使用默认 frame。
  - `src/layout.rs` 无论当前是否 ViewerTab，Studio 自己的日志面板和底部状态栏都继续显示。
- 前序保护：
  - 没有回退 Phase 1 中的 renderer 新接口和局部视口渲染约束。

## Phase 3：浮动面板和覆盖层定位修复

- 完成情况：已完成。
- 变更摘要：
  - `crates/scad-viewer/src/ui/side_panel.rs`
  - `crates/scad-viewer/src/ui/camera_overlay.rs`
  - `crates/scad-viewer/src/ui/log_panel.rs`
  - 上述三个浮动面板现在都接收标签页视口矩形，默认位置按视口相对偏移计算，拖动范围通过 `constrain_to(viewport_rect)` 限制在视口内。
  - 面板位置持久化改为保存相对视口左上角的偏移量，窗口缩放和布局变化后仍能落在视口区域内。
  - `crates/scad-viewer/src/ui/mod.rs` 把视口矩形传递给所有覆盖层；gizmo 继续使用视口左下角定位。
- 前序保护：
  - 没有破坏 Phase 2 的标签页内局部渲染。

## Phase 4：3D 视口鼠标/键盘交互修复

- 完成情况：已完成。
- 变更摘要：
  - `src/main.rs` 新增最近光标位置和最近视口矩形记录，只在视口内转发鼠标按下、滚轮事件；拖拽开始后允许移动与释放事件继续回流给 ViewerTab。
  - `src/viewer_tab.rs` 的事件处理改为接收视口矩形，并把光标坐标转换为视口局部坐标后再交给截面编辑和相机交互逻辑。
  - `crates/scad-scene/src/camera.rs` 暴露了更细粒度的鼠标按下、光标移动、滚轮处理接口，便于 Studio 在视口边界上做精确分发。
- 前序保护：
  - 保持了 Phase 2 中的视口矩形来源与 Phase 3 中的浮层约束方式。

## Phase 5：macOS App Menu 修复

- 完成情况：已完成。
- 变更摘要：
  - `src/platform_menu.rs` 通过条件编译区分 macOS 与其他平台。
  - macOS 下菜单顺序调整为 `SCAD Studio | File | View | Help`，About/Quit 被移到 App menu；非 macOS 平台仍保持原有结构。
- 前序保护：
  - 菜单事件 ID 与命令映射没有改坏，现有最近工作区和窗口命令仍保持兼容。

## Phase 6：欢迎页行为修复

- 完成情况：已完成。
- 变更摘要：
  - `src/app.rs` 在设置 workspace 后主动关闭欢迎页。
  - `ensure_welcome_tab()` 改为仅在“没有 workspace 且没有任何标签页”时创建欢迎页。
- 前序保护：
  - 没有改动已有标签页管理器的关闭策略，只收紧欢迎页的创建条件。

## Phase 7：Markdown 渲染方案升级

- 完成情况：已完成。
- 变更摘要：
  - 依赖切换到 `egui_commonmark = 0.22.0`，并开启 `better_syntax_highlighting`。
  - `crates/scad-ui/src/markdown.rs` 改为包装 `CommonMarkViewer`，保留 `MarkdownDocument::parse()` 接口。
  - `src/markdown_tab.rs` 新增持久化 `CommonMarkCache`，文件热重载时重置缓存。
  - `crates/scad-ui/tests/markdown_tests.rs` 改为验证 Markdown 源文本保留与 CommonMark 渲染调用可正常执行。
- 前序保护：
  - 选择 `0.22.0` 是因为它依赖 `egui 0.33`；`0.23.0` 已提升到 `egui 0.34`，不适合当前工作区。

## 验证

- 已执行：`cargo build`
- 已执行：`cargo build --release`
- 已执行：`cargo test`
- 结果：
  - 编译通过。
  - Release 编译通过。
  - 测试通过。
- 未执行：
  - `cargo run` 的人工 GUI 验证未在当前终端会话中执行，因此视口外观、面板拖拽边界和 macOS 原生菜单栏观感仍需要带图形界面的人工确认。

## 最终 review 修复

- 已调用独立 subagent 做最终代码审查，并按其结论完成以下修复：
  - `crates/scad-ui/src/markdown.rs` 恢复垂直滚动容器，避免长 Markdown 文档无法完整浏览。
  - `src/main.rs` 调整活跃 Viewer 快照逻辑，避免“当前是 ViewerTab 但暂时没有 mesh”时错误降级为 `render_egui_only()`，从而保留视口渲染路径和无 mesh 场景的 grid / preview 能力。
  - `crates/scad-viewer/src/ui/log_panel.rs` 与 `crates/scad-viewer/src/ui/mod.rs` 补上日志面板位置与尺寸变化后的 `SaveSettings` 信号，确保相对视口位置可持久化。

## 遗留事项

- 本轮仅处理功能性边界与渲染行为，没有进入 plan 中明确标记为后续任务的 UI 样式优化。
