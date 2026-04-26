# Prompt Archive：Web 预览点光源控制

## 2026-04-26 18:19:07 CST 用户需求

新增功能：

1. 额外点光源开关（关，自动，手动），per file 持久化（开启 shadow 时强制开启，但不修改配置）。
2. 手动点光源位置（X Y Z），per file 持久化，默认为自动位置。
3. 自动位置默认为摄像机 front 面右上角 45 度方向位置，distance 计算方式同摄像机位置计算方式。
4. 整理按钮状态切换的状态流。

## 2026-04-26 后续补充

- manual 位置设置需要增加 reset 按钮。
- reset 按钮把手动点光源位置重置为当前自动位置。

## 当前背景

- 上一轮已完成 Web 预览 appearance 配置：
  - `PreviewAppearance` 位于 `packages/studio-web/src/viewers/viewer-options.ts`。
  - `.scad.json` 读写位于 `packages/studio-web/src/workbench/preset-io.ts`。
  - 右侧栏 appearance 面板位于 `packages/studio-web/src/workbench/preview-appearance-panel.tsx`。
  - Three.js 渲染位于 `packages/studio-web/src/viewers/mesh-three.ts`。
- 当前 appearance 配置按 `.scad` 文件写入现有 `<stem>.scad.json`，并且修改 appearance 不触发 `.scad` preview request。

## 强制约束

- 继续使用现有 `<stem>.scad.json` 作为 per file 配置文件。
- shadow 开启时运行时必须强制启用额外点光源，但不能把该强制状态写回配置。
- 自动点光源位置必须沿用相机 framing 的 distance 计算方式。
- 需要整理按钮状态流，作为实现和验收依据。
- 手动位置 reset 必须写回当前自动位置，但不切换出 manual 模式。
