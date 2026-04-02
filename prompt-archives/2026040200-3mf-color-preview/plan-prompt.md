# Prompt 存档：3MF 彩色预览

## 用户原始请求

> 模型里的颜色信息被丢失了，你应该输出3mf，然后按照3mf的颜色渲染

## 背景

当前 `scad-studio` 预览链路仍以 OpenSCAD CLI 输出的 STL 为主，`src/openscad.rs` 在渲染预览时固定使用 `CliOutputFormat::BinaryStl`。STL 不携带颜色信息，因此即使 `.scad` 中使用了 `color()`，进入查看器后也只能用本地着色器的单色/伪彩色逻辑渲染。

近期已在当前工作树中存在尚未提交的本地修复：

- `src/camera.rs` / `tests/camera_tests.rs`：相机允许完整轨道旋转
- `src/gizmo.rs` / `src/ui/mod.rs` / `tests/gizmo_tests.rs`：XYZ 轴指示器锚定到真实视口
- `src/mesh.rs` / `tests/mesh_tests.rs`：STL 导入时把 OpenSCAD 的 `Z-up` 转换为查看器内部 `Y-up`

后续实现 3MF 彩色预览时，必须保护以上目标，不允许为了接入新格式把这些已收敛行为重新破坏。

## 参考与调研结论

- 用户补充参考仓库：`https://github.com/thijsdaniels/vscode-openscad-preview`
- 该参考实现的 README 明确说明：
  - 预览格式支持 `3mf` / `stl`
  - `3mf` 用于保留模型颜色
  - 该能力依赖 OpenSCAD Nightly
- 本地调研确认该参考仓库在前端侧使用 `Three.js` 的 `ThreeMFLoader` 直接消费 3MF 颜色数据，而不是自己手写完整 3MF 渲染语义层。

## 已确认的需求边界

- 预览链路强制走 `3MF`，不再对预览悄悄回退 `STL`
- 如果当前 OpenSCAD 环境不支持可用于预览的 `3MF`，需要明确报错
- 颜色支持范围选择为：
  - 支持 3MF 中更细粒度的颜色信息，例如逐三角面或逐顶点颜色
  - 不在本轮实现贴图 / PBR / 更完整材质语义
- 当前工具栏的 `Color / Mono` 开关保留：
  - `Color`：按 3MF 颜色渲染
  - `Mono`：忽略 3MF 颜色，使用单色渲染
- 默认显示模式改为 `Color`

## 注意事项

- 本轮计划以当前代码为准，而不是以旧 plan 或 roadmap 的抽象描述为准
- 需要新增 3MF ZIP/XML 解析能力，并与现有 `wgpu` 渲染管线对接
- 代码规模约束仍有效：新文件不超过 500 行，新函数不超过 50 行
- 纯函数必须补单元测试，优先在 `tests/` 目录下新增解析与颜色映射回归
- 当前本机未检测到 `openscad` 可执行文件，且无法在本机确认 Nightly 是否可用；这一点会影响端到端验收

## 后续对话记录

- 用户补充参考仓库：`https://github.com/thijsdaniels/vscode-openscad-preview`
- 方案选择：预览强制走 `3MF`
- 颜色粒度选择：支持逐三角面 / 逐顶点颜色
- 默认值选择：工具栏默认显示 `Color`
