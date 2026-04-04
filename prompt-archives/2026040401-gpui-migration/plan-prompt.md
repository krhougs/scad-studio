# gpui 迁移计划 Prompt

## 背景

scad-studio 当前使用 wgpu + egui + winit 架构。查看器渲染即将定型，后续大量功能（UI 面板、交互逻辑）需要扩展。egui 即时模式不适合复杂 UI 长期维护，gpui 的 retained mode + Entity 状态管理 + Tailwind 风格 API 更适合后续开发。

## 目标

将 UI 层从 egui 迁移到 gpui，将 3D 渲染封装为独立的 gpui ViewComponent，实现：

1. 3D 渲染层解耦为独立模块（不依赖 egui 或 gpui）
2. 渲染结果通过 offscreen texture 嵌入 gpui 视口
3. 所有 UI 面板用 gpui 重写
4. 鼠标事件从 gpui 透传到 3D 视口控制相机

## 注意事项

- macOS 平台 gpui 使用 Metal，需要桥接 Metal 和 wgpu（通过 IOSurface/MTLTexture 共享）
- gpui 不暴露 wgpu Device/Queue，需要从 gpui_wgpu crate 内部获取或使用 gpui 的 canvas() API
- 渐进式迁移：先验证 3D 纹理嵌入可行性，再迁移 UI 面板
