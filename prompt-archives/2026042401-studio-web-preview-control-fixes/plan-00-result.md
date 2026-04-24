# Plan-00 Result：Studio Web 预览控制与坐标修正

## 返工状态总览

- 状态：未完成，2026-04-24 坐标目标返工后重新整理。
- 最新目标：
  - OpenSCAD 已经符合项目坐标系，本轮不通过修改后端 STL / 3MF / protocol mesh payload 解决视图问题。
  - Web 前端按本计划规定的摄像机方向展示 mesh、网格、底板、轴线和相机。
  - 摄像机 preset 方向为：Top `+Z`、Bottom `-Z`、Front `-Y`、Back `+Y`、Right `+X`、Left `-X`。
  - Top 视图屏幕上方对应 `+Y`，Bottom 视图屏幕上方对应 `-Y`，Front / Back / Left / Right 视图屏幕上方对应 `+Z`。
- 失效内容：
  - 旧 Phase 1 将问题判断为“预览区域三轴辅助线需要展示 OpenSCAD 语义轴”，与最新要求冲突。
  - 旧坐标轴测试、旧失败验证和旧 Phase 1 review 结论已失效，不再作为完成依据。
  - `scad-scene::mesh::openscad_to_viewer` 只作为历史背景读取；本轮不允许修改 STL / 3MF / protocol mesh payload 或后端 OpenSCAD 输出链路。

## Phase 0：失败测试与依据核对

- 状态：失败测试与依据核对已完成，等待最终 review 通过后提交。
- 本轮依据核对：
  - `README.md` 与 `docs/architecture.md` 已固定项目坐标系：右手系，`+X` 向右，`+Y` 向后，`+Z` 向上；前端展示和交互必须遵守同一坐标系。
  - Three.js 官方文档 / Context7 已确认：`camera.up` 决定屏幕上方，`lookAt` 用相机位置和 target 定向；修改 fov、aspect、near、far 或 orthographic frustum 后必须调用 `updateProjectionMatrix()`；官方 3MF 示例使用 Z-up。
  - Base UI 官方文档 / Context7 已确认：`NumberField.Root` 支持 controlled `value`、`onValueChange`、`onValueCommitted` 以及 Input / Increment / Decrement 组合。
  - `crates/scad-scene/src/camera.rs` 已确认 desktop `OrbitalCamera::orbit` 使用 `wrap_angle`，允许越过顶部 / 底部；`PAN_SPEED = 0.002`，`ZOOM_SPEED = 0.12`，滚轮缩放 factor 为 `(1 - delta * 0.12).clamp(0.2, 5.0)`。
- 测试变更摘要：
  - 删除旧 `packages/studio-web/tests/unit/openscad-axis.test.ts`，旧测试以额外轴映射补偿为目标，与当前计划冲突。
  - `packages/studio-web/tests/unit/camera-controls.test.ts` 新增六向 camera preset、非零 center bounds 下的 `fitCameraToBounds` 六向方向、orbit 越过顶部 / 底部测试。
  - `packages/studio-web/tests/unit/viewport-gizmo-model.test.ts` 新增三轴投影、相机变化后投影变化、六向 preset 下 horizontal / vertical 轴屏幕方向测试。
  - `packages/studio-web/tests/unit/mesh-render-metrics.test.ts` 新增真实 mesh info、真实 viewport、device pixel ratio、projection mode、helper 尺寸、gizmo 尺寸、fog、远距离 clipping、renderer adapter 保持 project-coordinate mesh payload 的测试。
  - `packages/studio-web/tests/playwright/canvas-interaction.spec.ts` 新增 ViewportGizmo 三轴可见、六向点击和初次加载 overlay 的浏览器失败测试。
  - 新增 `packages/studio-web/src/workbench/viewport-gizmo-model.ts` 与 `packages/studio-web/src/viewers/mesh-render-metrics.ts` 的极薄可导入 API，避免 suite 只因 import 缺失失败；当前 API 只返回占位结果，后续 Phase 负责实现。
- 失败验证：
  - `bun x vitest run tests/unit/camera-controls.test.ts tests/unit/viewport-gizmo-model.test.ts tests/unit/mesh-render-metrics.test.ts`
    - 结果：3 个测试文件均可导入并执行；21 个测试中 11 个失败。
    - 失败原因：bottom preset 的 up 仍为 `+Y`；`orbitBy` 仍夹紧顶部 / 底部；`fitCameraToBounds` 六向方向仍不符合项目坐标系；ViewportGizmo 轴仍为零长度且不随相机变化；render metrics 仍忽略 viewport / dpr / projection；visible plane 恒为 `xz`；远距离 far clipping 不足。
  - `bun x playwright test tests/playwright/canvas-interaction.spec.ts --grep "ViewportGizmo click switches view|initial mesh preview exposes prominent loading"`
    - 结果：2 个浏览器用例均失败。
    - 失败原因：`viewport-gizmo-axis-x` 不存在；初次预览期间 `mesh-loading-overlay` 不存在。
  - `cargo test -p scad-scene load_stl_from_reader_maps_openscad_xy_plane_to_viewer_ground_plane -- --exact`
    - 结果：通过，用于确认本轮未把修复路径转移到后端 STL 解析。
  - `cargo test -p app-server-core collect_process_logs_ignores_blank_lines_and_tags_stdout_as_info -- --exact`
    - 结果：通过，用于确认本轮未引入 app-server-core 坐标 payload 改动。
- Review 处理：
  - 第一轮 review 指出 Phase 0 不能用缺失模块 import 失败作为合格红灯；已新增极薄 API，使失败落在行为断言上。
  - 第二轮 review 指出 ViewportGizmo 三轴、真实 viewport / DPR / projection、renderer mesh payload 语义覆盖不足；已补齐对应测试。
  - 第三轮 review 指出结果记录缺失、Playwright 红灯未验证、六向和 gizmo 断言不够严格；已补充结果记录、运行浏览器红灯验证，并强化六向 azimuth / elevation 与 gizmo 方向断言。
- 当前处理方式：
  - Phase 0 只提交测试、极薄可导入 API、旧测试删除和本结果记录。
  - 生产实现改动留到后续 Phase；不得把当前失败测试弱化为通过。

## Phase 1：参数与相机数值控件回归确认

- 状态：已完成。
- 已保留实现摘要：
  - 引入 `react-knob-headless` 与 `@base-ui/react`，参数和相机数值项统一使用共享 `NumericControl`。
  - `NumericControl` 同时渲染 knob、Base UI NumberField 输入框与增减按钮；输入框和 knob 保留稳定尺寸约束。
  - knob 写值显式按 step 归一化。
  - 参数 restore 按钮改为固定占位的 disabled 状态。
  - 参数行固定布局限制在 `.parameter-row`。
  - 参数 `sliderBounds` 的无显式范围推导改为只基于默认值，不再随 current value 扩大。
  - 相机 target、distance、azimuth、elevation 改用同一数值编辑模式。
- 已有历史验证：
  - `bun x vitest run tests/unit/parameter-model.test.ts tests/unit/numeric-control.test.ts` 曾通过。
  - `bun x playwright test tests/playwright/parameters-presets.spec.ts --grep "typed controls drive current defines|knob number field updates preview|save, load, delete round-trip"` 曾通过。
  - `bun x playwright test tests/playwright/canvas-interaction.spec.ts --grep "preview info and camera controls"` 曾通过。
- 本轮验证：
  - `bun x vitest run tests/unit/parameter-model.test.ts tests/unit/numeric-control.test.ts`
    - 结果：2 个测试文件通过，9 个测试通过。
  - `bun x playwright test tests/playwright/parameters-presets.spec.ts --grep "typed controls drive current defines|knob number field updates preview|save, load, delete round-trip"`
    - 结果：3 个浏览器用例通过。
  - `bun x playwright test tests/playwright/canvas-interaction.spec.ts --grep "preview info and camera controls"`
    - 结果：1 个浏览器用例通过。
- 独立 review：
  - Phase 1 review 无 blocker / important。
  - review 确认 `NumericControl` 仍使用 `react-knob-headless` 与 `@base-ui/react/number-field`，参数面板和相机面板都复用该控件。
  - review 确认参数范围推导不随 current value 扩大，参数与相机控件的浏览器测试覆盖 knob、number field、stepper、即时 preview 和布局稳定性。
- 变更摘要：
  - 本 Phase 没有代码变更，只做 Phase 0 返工后的回归确认。
- 遗留风险：
  - 布局稳定性测试主要比较 row / field 的宽高，未额外比较相邻 label / restore button 的 `x/y` 位置；当前不阻塞 Phase 1，后续如继续强化输入过程稳定性可补充位置断言。

## Phase 2：ViewportGizmo 当前相机指示

- 状态：未完成，2026-04-24 返工后调整为 Phase 2。
- 前序目标保护：
  - 不改变 Phase 1 的数值控件结构、测试标识和稳定布局。
  - 不通过改写预览区域三轴映射来掩盖前端场景/相机问题。
  - 不修改后端 STL / 3MF / protocol mesh payload。
- 待执行：
  - 让 ViewportGizmo 展示 X/Y/Z 三条轴线，并随当前相机状态变化。
  - 让 ViewportGizmo 点击覆盖 Top / Bottom / Front / Back / Right / Left 六个正交方向；iso 可以保留。
  - 运行 ViewportGizmo 三条轴线投影、实时变化和六向点击测试。
  - 独立 subagent review Phase 2 diff 或涉及文件清单。

## Phase 3：项目坐标系前端适配与相机交互

- 状态：未完成，2026-04-24 返工退回。
- 返工原因：
  - 独立 review 指出 Web orbit 仍不能像 desktop 一样越过模型顶部/底部。
  - 独立 review 指出 ViewportGizmo 与 camera preset 仍沿用旧坐标理解，front/back/left/top 与项目坐标系不一致。
  - 用户补充现象：front 显示底部，back 显示顶部，left 显示逆时针旋转 90 度的右视图，top 显示正视图。
- 新目标：
  - OpenSCAD 已经符合项目坐标系，本轮不通过修改后端 STL / 3MF / protocol mesh payload 解决视图问题。
  - Web 前端按本计划规定的摄像机方向展示 mesh、网格、底板、轴线和相机。
  - front / back / left / right / top / bottom preset 按本计划规定的摄像机方向切换，且不出现滚转 90 度的错误朝向。
- 已确认保留：
  - Phase 1 数值控件变更保持有效，但需重新回归。
  - Phase 2 中 ViewportGizmo 点击入口可以保留，但必须补充三轴实时指示并修正为六向点击。
- 待执行：
  - 核对现有 mesh payload 形态，只作为 renderer 输入，不改后端输出。
  - 让前端 renderer 在现有 mesh payload 输入下呈现项目坐标系的用户可见空间。
  - 让六个 camera preset 的观察方向和屏幕上方方向符合本计划。
  - 对齐 Web 相机拖拽、平移、缩放与 desktop OrbitControls 的方向和速度关系。
  - 独立 subagent review Phase 3 diff 或涉及文件清单。

## Phase 4：预览可读性、加载状态与真实尺寸时机

- 状态：未完成。
- 待执行：
  - 调整预览呈现，使背景稍灰且模型主体、辅助线和控件层级更清楚。
  - 将视图、投影、plate、网格、gizmo、fog 和裁切相关计算与真实数据、真实 viewport 的可用状态绑定。
  - 让加载状态覆盖初次渲染前、参数重新渲染、同一文件刷新和远端异步等待场景。
  - 运行加载状态、真实尺寸时机和辅助元素尺寸相关测试。
  - 独立 subagent review Phase 4 diff 或涉及文件清单。

## Phase 5：远距离相机与完整回归

- 状态：未完成。
- 待执行：
  - 让相机投影范围跟随当前视图和模型 bounds。
  - 使用针对性测试验证远距离场景。
  - 运行 `studio-web` typecheck、unit、e2e 与 build。
  - 调用独立 subagent 做只读完整 review；修复 review 发现的问题后重新回归。
  - 完成后记录最终验证结果。
