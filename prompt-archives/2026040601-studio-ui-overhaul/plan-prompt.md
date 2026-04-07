# Studio UI 全面改造

## 用户原始需求

1. 全局按照重新设计现代最佳实践重新设计UI布局（包括但不限于文件列表、Tab、Chat）
2. 欢迎页面在打开workspace之后不应该允许再在当前窗口打开新的页面
3. macOS菜单应该有独立的File menu
4. 修复模型预览区域各个组件位置不正常的问题
   4.1 模型预览区域应该独立于app全局，不应该为app的背景
   4.1.1 每个标签页的模型预览区域应该独立
   4.2 工具栏、状态栏、xyz轴图示位置不正确，需要修复
   4.2.1 floating panel不能移动出标签页区域
   4.3 模型预览区域没有相应鼠标键盘操作，需要修复
5. 全局背景颜色应该为默认背景黑色，而不是3d预览界面的蓝色
6. markdown预览区域应该使用外部成熟的渲染方案，实在不行塞个webview

## 背景

基于 2026040600-studio-workspace-ui 完成后的代码状态。当前 Studio 已具备基本的多标签页工作区框架（文件树、Chat 面板、ViewerTab、MarkdownTab、欢迎页），但存在以下核心问题：

### 架构问题
- 3D 渲染器是全局单例，全窗口作为背景渲染，而非 per-tab 独立
- 浮动面板（参数面板、相机面板、日志面板）使用 `ctx.content_rect()` 全局坐标定位，与标签页内容区域无关
- egui 的 CentralPanel 设置为透明帧以让背景 3D 穿透，导致全局背景色是 3D 场景的天空色（蓝色）
- 非 ViewerTab 标签页（Markdown、Welcome）也看到 3D 蓝色背景

### UI 布局问题
- 工具栏、状态栏位于 ViewerTab 内部的 Ui layout 中，但浮动面板使用全局 egui::Context 定位
- Gizmo 使用 viewport_rect 定位但实际坐标参考了 CentralPanel 偏移
- 浮动面板无边界约束，可以拖出标签页区域
- 面板保存的位置无校验，窗口大小改变后可能超出屏幕

### 功能问题
- 欢迎页在打开 workspace 后仍可重新打开
- Markdown 渲染使用手写的 pulldown-cmark → egui 渲染，功能有限（无表格、无图片、无语法高亮）
- macOS 菜单已有 File menu（New Window、Open Folder、Recent、Close、Quit），但需确认是否需要补充
- 3D 视口的鼠标/键盘交互可能被 egui 层拦截

## 注意事项

- `scad-ui` 是两个 GUI 应用（scad-viewer、scad-studio）的共享层，新增 GUI 基础组件应优先放入 scad-ui
- 当前渲染管线：winit → egui::Context.run() → layout::show() → ViewerTab → Renderer::render(camera, settings, egui_paint)
- Renderer 在 scad-scene crate 中，拥有 egui_wgpu::Renderer 用于合成
- 每个窗口一个 StudioRuntime，包含 egui_context + Renderer
