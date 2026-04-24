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

- 状态：已完成。
- 前序目标保护：
  - 不改变 Phase 1 的数值控件结构、测试标识和稳定布局。
  - 不通过改写预览区域三轴映射来掩盖前端场景/相机问题。
  - 不修改后端 STL / 3MF / protocol mesh payload。
- 本轮变更摘要：
  - 实现 `projectViewportGizmoAxes(camera, size)`，基于当前 camera forward / up 投影项目坐标 X/Y/Z 三轴。
  - `CanvasZone` 增加 ViewportGizmo SVG 三轴显示，并扩展视角入口到 `iso/front/back/left/right/top/bottom`。
  - `WorkbenchLayout` 将当前 `cameraState` 传入 `CanvasZone`，让 gizmo 随相机状态更新；无相机状态时使用 active preset fallback。
  - 修正 bottom preset 的 screen up 为 `-Y`，并让 `fitCameraToBounds` 按六向 project-coordinate direction / up 定位。
  - 同步旧 view pill top 浏览器断言为 `90.000`，与本计划 Top 从 `+Z` 侧看向原点一致。
- 本轮验证：
  - `bun x vitest run tests/unit/viewport-gizmo-model.test.ts`
    - 结果：1 个测试文件通过，3 个测试通过。
  - `bun x vitest run tests/unit/camera-controls.test.ts --testNamePattern "defines six project-coordinate orthographic camera presets|fits camera presets without changing project-coordinate view directions"`
    - 结果：2 个相关测试通过。
  - `bun x vitest run tests/unit/viewport-gizmo-model.test.ts tests/unit/camera-controls.test.ts --testNamePattern "defines six project-coordinate orthographic camera presets|fits camera presets without changing project-coordinate view directions|viewport-gizmo-model"`
    - 结果：5 个相关测试通过，10 个跳过。
  - `bun x playwright test tests/playwright/canvas-interaction.spec.ts --grep "view pill switches active preset|ViewportGizmo click switches view"`
    - 结果：2 个浏览器用例通过。
  - `bun run typecheck`
    - 结果：通过。
- 独立 review：
  - Phase 2 review 无 blocker。
  - review 确认 gizmo 投影消费当前 `cameraState` 和项目坐标轴，六向按钮覆盖完整，未破坏 Phase 1 数值控件结构。
  - review 要求提交前隔离 Phase 3 相机交互改动；已通过 staged diff 确认本 Phase 提交不包含 `mesh-three.ts` 交互改动，也不包含 `zoomBy` / middle button hunk。
- 遗留问题：
  - `camera-controls.test.ts` 全量仍有 `orbitBy allows crossing over the top of the model` 失败，属于 Phase 3 相机拖拽 / orbit 行为。
  - `.viewport-gizmo__views button` 固定宽度可能让较长标签视觉偏紧，当前不阻塞 Phase 2；后续可结合浏览器截图确认布局表现。

## Phase 3：项目坐标系前端适配与相机交互

- 状态：已完成。
- 前序目标保护：
  - 未修改 STL / 3MF / `.scad` 生成预览 mesh 的 protocol payload。
  - Phase 1 参数与相机数值控件结构、测试标识和自动预览路径保持不变。
  - Phase 2 ViewportGizmo 三轴显示和六向点击入口保持可用。
- 本轮变更摘要：
  - Three.js 预览改为 Z-up 场景语义：camera `up` 使用 `+Z`，网格和底板落在项目坐标系 XY 平面，并按 mesh bounds 的 Z 下边界定位。
  - `visibleProjectPlaneForCamera` 改为根据相机观察方向判断 XY / XZ / YZ 可见项目平面。
  - `orbitBy` 和 Three.js pointer orbit 改为与 desktop 相同的可跨越顶部 / 底部模型行为，不再夹紧 elevation。
  - `sphericalFromCamera` 使用 position 与 `up` 共同反推 orbit 分支，避免跨越顶部 / 底部后通过相机数值面板更新 target / distance 时发生 180 度滚转。
  - 平移速度改为 `distance * 0.002`，缩放 factor 改为 `(1 - delta * 0.12).clamp(0.2, 5.0)`，与 desktop `OrbitalCamera` 对齐。
  - wheel delta 新增 pixel / line mode 归一化，浏览器 pixel delta 按 `/120`，line delta 直接使用，并按浏览器滚轮方向取反。
  - pointer 分类补齐 middle button pan，并让 Three.js 运行路径复用共享分类逻辑。
- 本轮验证：
  - `bun x vitest run tests/unit/camera-controls.test.ts`
    - 结果：1 个测试文件通过，16 个测试通过。
  - `bun x vitest run tests/unit/mesh-render-metrics.test.ts --testNamePattern "keeps renderer-visible project planes"`
    - 结果：1 个相关测试通过，5 个跳过。
  - `bun run typecheck`
    - 结果：通过。
  - `bun x playwright test tests/playwright/canvas-interaction.spec.ts --grep "three.js canvas renders and accepts pointer drag|view pill switches active preset|ViewportGizmo click switches view"`
    - 结果：3 个浏览器用例通过。
- 独立 review：
  - 第一轮 review 指出 top / bottom pole 处 `atan2(0, 0)` 会导致 `up` 丢失，且 `updateCameraFromSpherical` 未同步 `up`；已修复并补充 pole orientation 测试。
  - 第二轮 review 指出越过 pole 后重复 `orbitBy(state, 0, 0)` 会因为等价角度分支丢失而翻转；已修复 `orbitAngles` 分支恢复并补测试。
  - 第三轮 review 指出 spherical 往返会丢失越过 pole 后的 `up` 分支，影响相机数值控件路径；已改为通过 `state.up` 反推并补测试。
  - 第四轮 review 指出 wheel `deltaMode` 未区分 line / pixel；已新增 `wheelDeltaToZoomAmount` 并补测试。
  - 最终 review 无 blocker / important / minor，确认 Phase 3 可进入记录与提交。
- 遗留问题：
  - Playwright 的拖拽用例仍主要验证事件接收和状态可见，尚未精确断言拖拽后的方向和速度数值；最终 review 建议后续补充读取相机状态或 gizmo 投影变化的更强断言。

## Phase 4：预览可读性、加载状态与真实尺寸时机

- 状态：已完成。
- 前序目标保护：
  - Phase 1 的参数与相机数值控件结构、测试标识和自动预览路径保持不变。
  - Phase 2 的 ViewportGizmo 三轴显示、动态尺寸和六向点击入口保持可用。
  - Phase 3 的项目坐标系、Z-up 场景语义、相机 preset、orbit / pan / zoom 行为保持不变。
  - 同一文件刷新期间不清空上一帧，不重置用户手动相机；未手动调整的自动取景状态在 resize 后继续按当前 viewport 重算。
- 本轮变更摘要：
  - Three.js 预览背景调整为稍灰的 `#101114`，并通过 `data-preview-background` 暴露可验证标识。
  - `meshRenderInputsReady` 和 `meshSceneMetrics` 统一要求真实 mesh info、真实 viewport、有效 device pixel ratio 后再输出 plate、grid、axis、gizmo、fog、orthographic 和 clipping 相关指标。
  - `MeshViewer` 将初次加载和刷新期间都显示明显 overlay：初次为 `preview loading...`，已有上一帧时为 `preview updating...`。
  - `WasmClient` 增加仅用于浏览器测试的 preview / file read 延迟钩子，延迟真实 dispatch promise，覆盖远端异步等待路径；延迟实现会立即绑定 resolve / reject，避免未处理 rejection。
  - 同一 mesh 文件刷新走 `frame:false` 时保留上一帧和当前相机；自动取景状态不因同一路径刷新丢失，resize 后仍会根据真实 viewport 重算。
  - Orthographic 投影在真实 metrics 缺失时不输出占位 half-height；真实 mesh / viewport 到达后，按当前 camera 的 screen right / screen up 轴投影 bounds 计算 half-height，避免 right / left / front / back 在窄 viewport 下裁剪模型。
  - clipping near / far 同步应用到 perspective 和 orthographic camera，并通过 dataset 暴露回归标识。
  - CanvasZone 使用真实 stage viewport 计算 ViewportGizmo 尺寸。
  - ImageViewer 刷新同一路径时保留旧图，等待新图片 `onLoad` 后才结束 loading，并在新 URL 设置时清空旧 natural size。
- 本轮验证：
  - `bun run typecheck`
    - 结果：通过。
  - `bun x vitest run tests/unit/mesh-render-metrics.test.ts tests/unit/camera-controls.test.ts tests/unit/viewport-gizmo-model.test.ts`
    - 结果：3 个测试文件通过，26 个测试通过。
  - `bun x playwright test tests/playwright/canvas-interaction.spec.ts --grep "viewer toolbar drives render state|initial mesh preview exposes prominent loading|parameter preview exposes updating loading|image refresh exposes updating loading|status bar and chrome do not overlap|preview error card avoids chrome|three.js canvas renders and accepts pointer drag|view pill switches active preset|ViewportGizmo click switches view"`
    - 结果：13 个浏览器用例通过。
- 独立 review：
  - 第一轮 review 指出远端异步等待测试没有真实延迟，参数更新 loading 缺少覆盖；已让 preview dispatch promise 消费延迟钩子，并新增参数更新 overlay 测试。
  - 第二轮 review 指出延迟 promise 的 rejection handler 绑定过晚、背景色缺少可验证标识、初次 loading 文案断言过宽、同一文件刷新 pending 缺少覆盖；已修复延迟实现、暴露背景 dataset、精确断言 loading 文案，并通过文件 watch 刷新覆盖同一路径 pending。
  - 第三轮 review 指出 orthographic 在 metrics 缺失时仍输出占位 half-height，并建议补图片刷新覆盖；已改为 metrics 缺失时不输出 half-height，真实返回后再输出，并补充图片刷新 overlay 测试。
  - 第四轮 review 指出 orthographic half-height 未按当前相机方向计算；已新增按 screen right / screen up 投影 bounds 的计算，并补窄 viewport 下 long-Y mesh 的 Right / Top / Front 单元测试。
  - 第五轮 review 只剩 image load 时机和 helper 占位尺寸两个 minor；已让图片 loading 等新图 onLoad，并避免有 mesh 但 metrics 不可用时用占位 helper 尺寸重算。
  - 最终 review 无 blocker / important / minor，确认 Phase 4 可进入记录与提交。
- 遗留问题：
  - `MeshSceneMetrics.visiblePlane` 当前仍保留为通用字段，实际用户可见平面判断继续由 `visibleProjectPlaneForCamera` 和相机状态驱动；本 Phase 没有新增消费该字段。

## Phase 5：远距离相机与完整回归

- 状态：未完成。
- 待执行：
  - 让相机投影范围跟随当前视图和模型 bounds。
  - 使用针对性测试验证远距离场景。
  - 运行 `studio-web` typecheck、unit、e2e 与 build。
  - 调用独立 subagent 做只读完整 review；修复 review 发现的问题后重新回归。
  - 完成后记录最终验证结果。
