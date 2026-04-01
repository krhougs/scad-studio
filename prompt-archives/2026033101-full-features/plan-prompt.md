# Prompt 存档：全功能实现

## 用户原始请求

> feature-roadmap.md 根据项目规范创建plan，完成所有未完成项目。着重注意设计查看器的gui（包括但不限于各种按钮和panel的排布）

## 背景

scad-studio 的 MVP（plan-00 的 Phase 1-5）已全部完成，涵盖：

- 核心管线：OpenSCAD CLI 调用、STL 解析、进程管理、错误展示
- 文件管理：文件对话框、文件变更监控、去抖动、监控切换
- 3D 渲染：wgpu 管线、Blinn-Phong 光照、实体渲染模式
- 相机：轨道相机（旋转/缩放/平移）、自动 fit 包围盒、透视投影
- UI：平台菜单栏（macOS/Windows 原生）、状态栏、窗口 resize

`docs/feature-roadmap.md` 中仍有 50+ 个未完成功能项（标记为 `[ ]`），涉及渲染模式、相机增强、环境场景、光照系统、交互式截面、参数编辑、预设系统、导出和 GUI 完善。

## 注意事项

- 本次计划覆盖 feature-roadmap.md 中所有 `[ ]` 标记的功能项
- 重点关注查看器 GUI 的整体布局与交互设计（工具栏按钮分组、侧边面板排布、日志面板、坐标轴指示器等）
- 技术栈：Rust + winit + wgpu + egui（已在 MVP 中建立）
- 参考前序计划：`prompt-archives/2026033100-project-init/plan-00.md`
- 代码规模约束：文件不超过 500 行，函数不超过 50 行
- 当前 `renderer.rs` 已有 484 行，新增多管线后需要拆分

## 后续对话记录

- `2026-04-01`：用户追加指令“@prompt-archives/2026033101-full-features/plan-00.md 干活”
