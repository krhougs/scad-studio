# Plan Prompt：Studio Web 预览控制与坐标修正

## 原始用户反馈

2026-04-24 用户在 Plan-02 执行后继续反馈：

1. `parameters` 和 `camera` 使用 `react-knob-headless` 绘制 knob，并使用 `@base-ui/react/number-field` 绘制输入框；需要符合当前设计系统。
2. 参数象限范围只应根据模型参数初始值变化，不应根据 current value 变化。
3. 渲染颜色依然看不清，handle 不工作。
4. 预览的 XYZ 三轴方向和 OpenSCAD 中不一样，需要查文档和代码。
5. 需要补加载状态展示。
6. 距离和宽度计算不能在初次渲染或初次 loading 完成前固定，必须在真实 mesh bounds 与 viewport 可用后计算。
7. 预览区域背景颜色改成稍微灰一点。
8. bug：当摄影机距离太远时模型展示不完全。

## 追加澄清

2026-04-24 用户补充：

1. Plan 中提到的 handle 指的是 `ViewportGizmo`，不是相机面板入口。
2. 相机实际拖拽交互行为需要参考 `studio-app` OrbitControls，并在 `packages/studio-web/src/canvas/camera-controls.ts` 中保持一致。
3. plan 的具体条目只描述背景、症状、目标、解决思路、验收方式；非必要不要写具体实现方式。该规则已先写入仓库根目录 `AGENTS.md`。
4. ViewportGizmo 只需要支持点击切换视角。
5. 写 plan 前需要判断用户输入是否包含强制约束；如果存在强制约束，必须写进 plan。`react-knob-headless` 与 `@base-ui/react/number-field` 在本场景属于必要路径。
6. 所有数值输入在输入过程中必须保持输入控件和相关 UI 排版稳定，不得出现宽高变化、位移或相邻元素跳动。
7. 初次渲染前、参数改变需要重新渲染时，以及任何等待远端异步加载的情况下，需要在 UI 的显眼处展示加载状态。
8. 渲染部分需要严格检查类似的异步加载导致提前执行的问题，包括摄像机距离判断、plate 和网格大小等依赖真实渲染数据的计算；找到后必须纳入本 plan 的修复范围。

2026-04-24 用户再次澄清坐标目标：

1. 项目坐标系固定为右手系：`+X` 向右，`+Y` 向后 / 板面内第二方向，`+Z` 向上 / 层叠方向；`Top plane = XY`，`Front plane = XZ`，`Right plane = YZ`。
2. OpenSCAD 已经符合这套坐标系，不需要在后端为了 Web 预览改写 OpenSCAD 输出轴向。
3. 本轮坐标修正重点是前端预览架构适配这套项目坐标系，包含相机 preset、相机交互、ViewportGizmo、网格、底板和坐标轴。
4. 摄像机 preset 必须按项目坐标系的平面定义解释：Top 从 `+Z` 侧看向原点，Front 从 `-Y` 侧看向原点，Right 从 `+X` 侧看向原点；对应反向视图 Bottom / Back / Left 分别从 `-Z` / `+Y` / `-X` 侧看向原点。Top 视图屏幕上方对应 `+Y`，Bottom 视图屏幕上方对应 `-Y`，Front / Back / Left / Right 视图屏幕上方对应 `+Z`。

## 上下文

- 本任务基于 `prompt-archives/2026042400-studio-web-parity-audit-plan/plan-02.md` 的未提交实现继续修改。
- 当前 `studio-web` 已有参数自动预览、右侧 Camera panel、Three.js mesh viewer、loading overlay 的初步实现，但仍有上述行为问题。
- 当前分支存在大量未提交变更，执行时不得回退前序实现。

## 已核对文档

- Base UI NumberField 文档：`NumberField.Root` 支持受控 `value` 与 `onValueChange`，并包含 `Group`、`Input`、`Increment`、`Decrement` 等 parts。
- `react-knob-headless` 文档：`KnobHeadless` 使用 `valueRaw`、`valueMin`、`valueMax`、`valueRawRoundFn`、`valueRawDisplayFn`、`onValueRawChange`；键盘控制通过 `useKnobKeyboardControls`。
- OpenSCAD 文档：旋转和坐标遵循右手规则；OpenSCAD 坐标已经符合项目约定。本轮不得把 `scad-scene::mesh::openscad_to_viewer` 这类 viewer 私有映射当作后端 mesh 输出契约，前端预览架构需要适配项目坐标系。
- 仍需核对 `studio-app` 的 OrbitControls 行为，并将网页相机拖拽、平移、缩放语义与其保持一致。

## 执行要求

- 按当前源码与官方文档实现，不凭记忆写 API。
- 先补失败测试，再改实现。
- 新增纯函数必须放入单元测试。
- 完成后运行针对性测试与完整回归，并更新 `plan-00-result.md`。
