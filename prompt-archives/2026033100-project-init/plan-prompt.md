# scad-studio 项目初始化

## 原始需求

1. 创建一个 Rust 项目，使用 GUI 框架创建桌面应用
2. GUI 包含两个功能：
   - 选择文件打开 OpenSCAD 文件，并提供实时预览
   - 打开的文件在外部被修改时重新读取文件并渲染
3. 参考 https://github.com/thijsdaniels/vscode-openscad-preview 并实现该项目预览窗口的所有功能
4. 使用 Rust 原生的方式渲染 3D 图形，不使用 webview

## 用户决策

- **UI 框架**: 改用 winit + wgpu + egui（gpui 不支持 3D 渲染）
- **功能范围**: 最小可用版本——文件打开 + 3D 预览（旋转/缩放/平移）+ 文件变更监听自动刷新

## 背景信息

### 参考项目功能清单（vscode-openscad-preview）

预览窗口完整功能：
- 相机控制：透视/正交切换、轨道旋转、平移、缩放
- 渲染模式：实体/线框/X 光（半透明）
- 环境：空白/网格/打印平台
- 光照：环境光 + 方向光 + 聚光灯 + 点光源，可切换阴影
- 截面：交互式切割平面，支持平移/旋转，Ctrl 吸附网格
- 坐标轴：角落坐标轴指示器（XYZ）
- 参数编辑：从 .scad 解析变量，生成滑块/开关/下拉框，实时更新
- 预设系统：保存/加载/删除参数预设（JSON 文件）
- 导出：一键导出 STL/3MF，一键发送到切片软件
- 文件监控：监听 .scad 和配套 .json 文件变更，自动重新渲染

### 渲染管线

```
.scad 文件 → OpenSCAD CLI (headless) → STL → 解析为顶点/面数据 → wgpu 渲染
```

OpenSCAD CLI 调用方式：
```bash
openscad --export-format binstl -o output.stl input.scad
```

### 技术栈

- **窗口管理**: winit
- **3D 渲染**: wgpu（原生 WebGPU API）
- **UI 覆盖**: egui（通过 egui-wgpu 和 egui-winit 集成）
- **STL 解析**: nom_stl 或 stl_io
- **文件监控**: notify crate
- **文件对话框**: rfd (Rusty File Dialogs)
