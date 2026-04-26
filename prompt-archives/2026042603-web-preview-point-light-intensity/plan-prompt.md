# Prompt

用户要求在 web 模型预览中新增 per model 的点光源强度设置，并持久化。

## 上下文

- 当前任务发生在 `scad-studio` 仓库，产品可见名称为 `budn'`。
- 预览外观配置已经通过 `PreviewAppearance` 按 `.scad` 文件写入对应 `.scad.json`。
- 现有点光源设置包括 `pointLightMode` 与 `pointLightPosition`，已经支持 off / auto / manual、shadow 开启时运行时强制 auto、手动位置 reset。
- 当前工作区已有未提交改动：点光源改为无距离衰减、增强默认点光源强度、shadow 开启时临时显示 build plate 作为阴影接收面，并让 mesh 不接收自阴影。

## 本次需求

- 新增点光源强度设置。
- 配置按模型持久化。结合现有实现，本轮按 `.scad` 文件的 `previewAppearance` 持久化处理。
- 不破坏既有 appearance 配置不触发 OpenSCAD 重新渲染的边界。
- 不破坏 off / auto / manual 状态流、shadow 强制开启但不修改配置的行为。
