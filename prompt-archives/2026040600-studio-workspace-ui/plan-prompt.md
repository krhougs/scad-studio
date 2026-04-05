# SCAD Studio Workspace UI 构建

## 背景

当前项目是一个基于 egui + wgpu 的 OpenSCAD 文件查看器（"SCAD Viewer"），所有代码集中在单个 crate 的 `src/` 目录下，约 14000 行 Rust 代码。用户希望将其演化为完整的 "SCAD Studio" IDE 应用。

## 目标

创建一个 GUI 程序 "SCAD Studio"，包含：
1. Agent Chat 面板（本轮仅做 UI 和排版，后端集成后续讨论）
2. Workspace 目录树状结构
3. 预览区域（多标签页）

## 具体需求

1. **二进制分离**：当前二进制实际为 "SCAD Viewer"，需要将入口迁移到独立的二进制中，Studio 作为新的主二进制
2. **代码结构重构**：模型查看器、UI 框架/Utility、不同的二进制分别存在于各自的 crate 中
3. **Agent Chat**：暂时只做好 UI 和排版，具体实现后续再讨论
4. **Workspace 机制**：Studio 要求用户打开一个目录作为 Workspace，Agent 会在其中创建、修改 3D 模型并实时变更
5. **UI 布局**：
   - 左边：Side Panel，有多个 tab，目前只有 Agent Chat 和树状文件选择器
   - 右边：工作区域，类似 Chrome 多标签页机制
     - 在文件选择器中选择 3D 模型 → 切换到对应文件的 Viewer Tab
     - 选择 .md 文件 → 在标签页中显示渲染好的 markdown
     - 后续会支持多种 tab 类型，需要做好接口设计

## 当前代码结构概要

- 单 crate 项目，`Cargo.toml` workspace members 为空
- 入口 `src/main.rs`：winit 事件循环 + `DesktopApp` 实现 `ApplicationHandler`
- 应用状态 `src/app.rs`：`StudioApp`、`ViewerState`、`UiActions` 等
- 渲染管线 `src/renderer.rs`、`src/pipeline.rs`：wgpu 多管线渲染
- UI 层 `src/ui/`：egui 实现的 toolbar、side_panel、log_panel、camera_overlay 等
- 数据层：document、params、presets、config、export 等
- 3D 场景：camera、mesh、lighting、shadow、grid、gizmo、cross_section 等
- OpenSCAD 集成：openscad runner、three_mf parser、file watcher

## 注意事项

- 重构过程中必须保证现有 Viewer 功能完整可用
- egui 版本 0.33，wgpu 版本 27
- 项目使用 Rust edition 2024
- 代码规模约束：文件不超过 500 行，函数不超过 50 行
