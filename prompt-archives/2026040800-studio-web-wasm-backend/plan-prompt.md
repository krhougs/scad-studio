# Plan prompt 存档

本目录对应任务：**Studio 浏览器端（WASM + egui/wgpu）与 Web 后端协同**。

## 用户原始请求（按时间顺序）

1. **Studio 如何在浏览器中运行**  
   上下文：仓库内 `scad-studio` 当前为 winit + egui-winit 桌面应用，无现成 wasm 目标。

2. **约束与简化**  
   希望在浏览器中直接跑现有 egui/wgpu 技术栈；不需要多窗口；打开工作区改为用户填写目录位置；系统菜单可以不实现。

3. **架构说明**  
   `notify` 与目录访问不放在浏览器沙箱内完成，而是**单独运行 Web 后端**，由后端访问其部署环境中的文件系统并承担监控职责；前端通过协议与后端协作。

4. **按项目要求编写 plan**  
   在 `prompt-archives` 下按 `YYYYMMDDNN-description` 建立存档，并撰写可分 Phase 执行的计划文档（见 `plan-00.md`）。

5. **OpenSCAD 与 3MF（补充拍板）**  
   Web 模式与桌面 **同一策略**：在 **后端运行环境** 中查找 OpenSCAD 可执行文件并调用 CLI，将 **生成的 3MF** 通过 API **传给前端**；前端用现有 3MF 解析与 Viewer 更新路径加载网格。

## 执行时注意

- 以仓库当前代码为准；计划与实现不一致时以代码与既定行为为准。
- 每个 Phase 完成后按 `AGENTS.md` 更新 `plan-00-result.md`，Review 使用独立 subagent，不在主对话中代替 review 写入结论到 plan 正文。
