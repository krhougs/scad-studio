# Plan-00：Studio Web 预览控制与坐标修正

## 背景

Plan-02 已补齐 Studio Web 侧栏、预览、参数、相机和配置的一批能力，但后续反馈指出控件、坐标、加载状态、视口时机和远距离相机仍未达到可用标准。本计划只处理这组反馈，不扩大到 app server、protocol 或跨端状态机重构。

## 强制约束

- 本轮返工后的坐标目标为项目坐标系：右手系，`+X` 向右，`+Y` 向后 / 板面内第二方向，`+Z` 向上 / 层叠方向；`Top plane = XY`，`Front plane = XZ`，`Right plane = YZ`。
- 摄像机 preset 必须按本计划中的项目坐标系和平面定义解释：Top 从 `+Z` 侧看向原点，Front 从 `-Y` 侧看向原点，Right 从 `+X` 侧看向原点；Bottom / Back / Left 分别从 `-Z` / `+Y` / `-X` 侧看向原点。Top 视图屏幕上方对应 `+Y`，Bottom 视图屏幕上方对应 `-Y`，Front / Back / Left / Right 视图屏幕上方对应 `+Z`。
- OpenSCAD 已经符合项目坐标系；本轮不以修改 OpenSCAD 生成或解析得到的 STL / 3MF / protocol mesh payload 作为解决手段。
- 本轮坐标修正重点在前端预览架构适配：Web 预览的相机 preset、相机拖拽、平移、ViewportGizmo、网格、底板和坐标轴必须共同消费项目坐标系。
- 不要把预览区域的三轴辅助线改成另一套映射来掩盖前端场景/相机问题；如果 Three.js 内部需要渲染适配层，该适配不得改变用户可见坐标语义和后端 mesh 输出契约。
- `ViewportGizmo` 必须展示为三条轴线，并实时反映当前相机位置/朝向，同时保留点击切换视角。
- `ViewportGizmo` 点击切换视角必须覆盖 Top / Bottom / Front / Back / Right / Left 六个正交方向；iso 可以保留，但不能替代六向验收。
- `parameters` 和 `camera` 的数值编辑必须使用 `react-knob-headless` 绘制 knob，并使用 `@base-ui/react/number-field` 绘制输入框。
- 所有数值输入在输入过程中必须保持输入控件和相关 UI 排版稳定，不得出现宽高变化、位移或相邻元素跳动。
- 参数象限范围只根据模型参数初始值或显式范围变化，不根据 current value 变化。
- handle 指 `ViewportGizmo`；目标行为是点击切换视角。
- 相机拖拽、平移、缩放行为必须参考 `studio-app` OrbitControls，并在 Web 侧保持一致。
- 现有预览 mesh 输入、前端用户可见空间和交互语义必须按项目坐标系解释；实现前必须参考文档和当前源码。
- 初次渲染前、参数改变需要重新渲染时，以及任何等待远端异步加载的情况下，必须在 UI 的显眼处展示加载状态。
- 渲染链路必须严格检查所有依赖远端异步结果、真实 mesh bounds、真实图片尺寸、真实 viewport、projection 状态或 device pixel ratio 的计算；包括但不限于摄像机距离判断、near/far、orthographic 宽高、plate 尺寸、网格尺寸、gizmo 尺寸、fog 范围和裁切平面初始值。确认存在提前执行时，必须纳入本计划修复。

## 参考范围

- 当前 `studio-web` 参数、相机、预览区与测试代码。
- `studio-app` 中相机 OrbitControls 的交互语义。
- `scad-scene` 中 STL / 3MF 读入、mesh 数据模型、相机、网格、gizmo 与 renderer 的现有坐标约定。
- Three.js 中相机、`up`、对象变换和几何变换对坐标适配的影响。
- Base UI NumberField、react-knob-headless 与 OpenSCAD 官方文档。

## Phase 执行规则

每个 Phase 都必须按以下循环执行：

1. 干活：只处理当前 Phase 的目标，不顺手重构无关代码。
2. Review：调用独立 subagent 做只读 review，review 输入必须包含当前 Phase 目标与验收标准、完整 `plan-00.md`、本次变更 diff 或涉及文件清单。
3. 回归：按当前 Phase 验收方式运行针对性验证；review 发现 blocker 或 important 时先修复，再重新 review 和回归。
4. 记录：更新 `plan-00-result.md`，记录完成情况、变更摘要、验证结果和遗留问题。
5. 提交：当前 Phase 消除 blocker 且通过对应验证后提交；随后自动进入下一个 Phase，不等待用户确认。

## Phase 0：失败测试与依据核对

背景：

- 本轮返工同时涉及坐标、相机、ViewportGizmo、控件、加载状态、真实尺寸时机和远距离裁剪。
- 坐标目标已明确为前端预览架构适配项目坐标系，不通过修改 OpenSCAD 输出、STL / 3MF 解析结果或 protocol mesh payload 解决。
- 外部库 API、项目坐标系、摄像机 preset 方向、OpenSCAD 坐标规则、现有 STL / 3MF mesh 数据边界、Three.js 前端适配方式和 desktop OrbitControls 行为必须以文档或源码为准。

症状：

- 现有测试不足以暴露这些问题，容易再次出现“看起来实现了但行为不对”的情况。
- 当前 Web 侧行为与项目坐标系、desktop viewer 相机交互和用户指定控件存在差异。
- 已确认的现象包括：front 显示底部、back 显示顶部、left 显示旋转后的右视图、top 显示正视图。

目标：

- 先建立能失败的单元测试和浏览器测试，再进入任何实现 Phase。
- 在测试设计前核对目标库 API、现有 STL / 3MF 坐标语义、项目坐标系边界、摄像机 preset 方向、Three.js 前端适配方式和 desktop OrbitControls 行为。
- 检查渲染链路中所有依赖异步结果或真实尺寸的计算，把确认存在的提前执行问题纳入后续 Phase。
- 明确旧坐标轴测试和旧 review 结论已失效，不再作为完成依据。

输入：

- 当前 `plan-00.md` 与 `plan-prompt.md`。
- 当前 `studio-web` 参数、相机、预览区、ViewportGizmo、mesh viewer 和测试代码。
- `studio-app` OrbitControls 行为与现有 `scad-scene` / Three.js 坐标相关源码。

前序目标保护：

- 保护已保留的参数与相机数值控件实现，不因返工测试删除其现有行为。
- 保护不修改后端 STL / 3MF / protocol mesh payload 的边界。
- 保护现有参数自动预览、canvas 不重叠、preview error、配置和布局测试。

操作步骤：

1. 核对项目坐标系、camera preset 六向方向、Three.js 前端适配方式和 desktop OrbitControls 行为。
2. 重写旧坐标轴相关失败测试，改为覆盖前端 renderer 在现有 mesh payload 输入下呈现项目坐标系的用户可见空间。
3. 添加或更新 camera preset、ViewportGizmo 三轴投影、ViewportGizmo 六向点击、参数范围、数值控件稳定性、加载状态、真实尺寸时机和远距离裁剪相关失败测试。
4. 运行新增测试，确认失败原因与目标行为一致，而不是测试写法错误或环境错误。
5. 调用独立 subagent review 本 Phase 的测试设计与依据核对；修复 review 发现的问题后重新运行失败验证。
6. 更新 `plan-00-result.md` 并提交 Phase 0。

验收方式：

- 新增或重写测试先失败。
- 失败原因与目标行为一致。
- 每个实现判断均能追溯到官方文档或当前源码。
- 渲染异步时机检查有明确覆盖范围，确认的问题不遗漏到后续任务之外。
- 独立 subagent review 无 blocker。

## Phase 1：参数与相机数值控件回归确认

背景：

- 用户明确指定 `react-knob-headless` 与 `@base-ui/react/number-field`，这是本轮必要路径。
- 参数范围只应基于模型参数初始值或显式范围，不应随 current value 扩大。
- 本能力已有实现，但 Phase 0 返工后必须重新验证，不能只沿用旧结论。

症状：

- 当前 result 中旧的 Phase 2 结论基于返工前的测试状态，需要在新坐标目标下重新确认。
- 数值输入过程中，控件宽度、按钮、标签和周边排版仍需要保持稳定。

目标：

- 参数与相机数值编辑体验保持一致。
- 输入过程中控件和相关 UI 排版保持稳定。
- 参数推导范围只受初始参数定义或显式范围影响。
- 保护参数自动预览和相机状态共享，不新增端侧 protocol 行为。

输入：

- Phase 0 重写后的失败测试集合。
- 当前参数面板、相机面板和数值控件实现。

前序目标保护：

- 保护 Phase 0 建立的新测试和坐标边界，不删除或弱化后续 Phase 所需覆盖。
- 保护不修改后端 STL / 3MF / protocol mesh payload 的边界。

操作步骤：

1. 运行 Phase 0 中与参数范围、数值控件和相机数值编辑相关的测试。
2. 若测试失败，只修复参数范围、控件库接入、布局稳定性或相机数值状态共享相关问题。
3. 运行参数、预设和相机控件相关浏览器测试。
4. 调用独立 subagent review 本 Phase diff 或涉及文件清单；修复 review 发现的问题后重新回归。
5. 更新 `plan-00-result.md` 并提交 Phase 1。

验收方式：

- 参数和相机可通过 knob、输入框与增减按钮编辑。
- 输入过程中相关 UI 不跳动、不变宽、不变高。
- current value 超出推导范围时，不会改变无显式范围参数的范围。
- 对应单元测试和浏览器测试通过。
- 独立 subagent review 无 blocker。

## Phase 2：ViewportGizmo 当前相机指示

背景：

- `ViewportGizmo` 不能只是静态按钮；它还需要用三条轴线展示当前相机观察方向，帮助用户理解当前视角。
- 该 gizmo 的轴线展示应消费当前相机状态，不应引入独立于 mesh / camera 的第三套坐标映射。

症状：

- 当前 Web 侧没有能实时表达相机位置/朝向的三轴 gizmo。
- 只有按钮时，用户无法判断拖拽或视角切换后的当前空间方向。
- 旧目标只覆盖部分视角，缺少六向正交视角的完整验收。

目标：

- ViewportGizmo 展示 X/Y/Z 三条轴线，并随当前相机状态变化。
- ViewportGizmo 点击切换覆盖 Top / Bottom / Front / Back / Right / Left 六个正交方向；iso 可以保留。
- 保护参数和相机数值控件布局，不挤压现有 canvas chrome。

输入：

- Phase 0 中 ViewportGizmo 三轴投影和六向点击失败测试。
- 当前相机状态和 ViewportGizmo 入口。

前序目标保护：

- 保护 Phase 0 已固定的 camera preset 方向和前端 renderer 坐标边界。
- 保护 Phase 1 的数值控件结构、测试标识和稳定布局。
- 不通过改写预览区域三轴映射来掩盖前端场景/相机问题。

操作步骤：

1. 让 ViewportGizmo 三轴投影测试通过。
2. 让 ViewportGizmo 六向点击切换测试通过，并保留 iso 行为作为可选视角。
3. 验证三轴显示随当前相机状态实时变化。
4. 调用独立 subagent review 本 Phase diff 或涉及文件清单；修复 review 发现的问题后重新回归。
5. 更新 `plan-00-result.md` 并提交 Phase 2。

验收方式：

- 单元测试覆盖相机状态到三轴投影的关系。
- 浏览器测试能看到三条轴线，并验证视角切换后 gizmo 轴线发生变化。
- ViewportGizmo 点击可切换六个正交方向，且与本计划 camera preset 方向一致。
- 独立 subagent review 无 blocker。

## Phase 3：项目坐标系前端适配与相机交互

背景：

- 新目标确认 OpenSCAD 已经符合项目坐标系，本轮只修正 Web 前端预览架构，不通过修改 STL / 3MF / protocol mesh payload 解决视图问题。
- 前端需要按项目坐标系组织用户可见的渲染语义，其中 `+Z` 是垂直轴，`Top plane = XY`，`Front plane = XZ`，`Right plane = YZ`。
- 项目坐标系下的摄像机 preset 方向为：Top 从 `+Z` 侧看向原点，Bottom 从 `-Z` 侧看向原点，Front 从 `-Y` 侧看向原点，Back 从 `+Y` 侧看向原点，Right 从 `+X` 侧看向原点，Left 从 `-X` 侧看向原点。Top 视图屏幕上方对应 `+Y`，Bottom 视图屏幕上方对应 `-Y`，Front / Back / Left / Right 视图屏幕上方对应 `+Z`。
- 相机实际拖拽交互需要参考 `studio-app` OrbitControls。

症状：

- 当前 Web 前端相机/场景空间与项目坐标系混用，导致 front/back/left/top 视图串位。
- 当前 ViewportGizmo 点击切换视角不符合项目坐标系预期。
- 相机拖拽、平移、缩放与 desktop viewer 行为存在差异。

目标：

- 不修改 STL / 3MF / `.scad` 生成预览 mesh 的 protocol payload 作为本轮修复路径。
- Web 预览按项目坐标系展示 mesh、网格、底板、坐标轴和相机视角。
- front / back / left / right / top / bottom preset 按本计划规定的摄像机方向切换，且不出现滚转 90 度的错误朝向。
- Web 相机拖拽、平移、缩放与 desktop OrbitControls 的方向和速度关系保持一致。
- 保护上一 Phase 的控件行为、ViewportGizmo 三轴显示和 Plan-02 的 preview info。

输入：

- Phase 0 中前端 renderer 项目坐标系适配、camera preset 和 OrbitControls 失败测试。
- Phase 2 中 ViewportGizmo 三轴显示和六向点击能力。
- 当前 Three.js mesh viewer 和相机控制代码。

前序目标保护：

- 保护 Phase 0 的测试边界和不改后端 mesh payload 约束。
- 保护 Phase 1 的数值控件行为。
- 保护 Phase 2 的 ViewportGizmo 三轴实时指示和六向点击入口。

操作步骤：

1. 核对现有 mesh payload 形态，只读取其作为 renderer 输入，不把后端 mesh 输出改造作为任务。
2. 让前端 renderer 在现有 mesh payload 输入下呈现项目坐标系的用户可见空间。
3. 让六个 camera preset 的观察方向和屏幕上方方向符合本计划。
4. 对齐 Web 相机拖拽、平移、缩放与 desktop OrbitControls 的方向和速度关系。
5. 调用独立 subagent review 本 Phase diff 或涉及文件清单；修复 review 发现的问题后重新回归。
6. 更新 `plan-00-result.md` 并提交 Phase 3。

验收方式：

- 测试能断言前端 renderer 在现有 mesh payload 输入下呈现项目坐标系的用户可见空间。
- 测试能断言 front / back / left / right / top / bottom preset 的摄像机方向与本计划一致。
- 点击 ViewportGizmo 后视角发生预期变化。
- 鼠标拖拽、平移、滚轮缩放的方向和速度关系与 desktop 行为一致，并允许像 desktop 一样越过模型顶部/底部。
- 独立 subagent review 无 blocker。

## Phase 4：预览可读性、加载状态与真实尺寸时机

背景：

- 用户指出渲染区域颜色仍不清楚，背景需要稍灰。
- 用户指出距离与宽度计算发生在初次渲染或初次 loading 完成前，导致功能没有真正生效。
- 初次渲染前、参数改变触发重新渲染时，以及任何等待远端异步加载的情况下，都需要在 UI 的显眼处展示加载状态。
- 同一文件刷新时需要展示加载状态，同时不能清空上一帧。
- 渲染链路存在同类风险：依赖 mesh bounds、图片尺寸、viewport、projection 或 device pixel ratio 的计算可能在远端异步结果到达前使用占位值。

症状：

- 当前材质、网格、坐标轴、背景和 gizmo 在深色界面中不够清楚。
- 初次加载、慢加载或 resize 后可能使用占位尺寸计算相机。
- 初次渲染前、参数重新渲染、等待远端异步加载时，加载反馈不稳定或不够显眼。
- plate、网格、gizmo、fog、裁切和相机投影等渲染计算若使用占位 bounds 或占位 viewport，会导致模型显示范围、辅助线范围和交互反馈不可靠。

目标：

- 预览背景、模型、网格、坐标和 gizmo 在深色界面中可辨认。
- mesh bounds 与真实 viewport 都可用后再计算视图范围。
- 所有依赖真实渲染数据的尺寸、距离、投影和辅助元素范围计算都必须等待前置数据可用，并在远端异步结果、参数结果或 viewport 变化后重新计算。
- 初次渲染前、同一模型刷新、参数更新、图片刷新和等待远端异步加载时，都有显眼且明确的加载状态。
- 保护同一文件刷新不重置用户相机、不清空上一帧的行为。

输入：

- Phase 0 中加载状态、真实尺寸时机、辅助元素尺寸和远距离裁剪失败测试。
- Phase 3 中已经符合项目坐标系的前端 renderer 和相机行为。

前序目标保护：

- 保护 Phase 1 的数值控件行为。
- 保护 Phase 2 的 ViewportGizmo 三轴显示和六向点击。
- 保护 Phase 3 的项目坐标系和相机交互边界。
- 保护同一文件刷新不重置用户相机、不清空上一帧。

操作步骤：

1. 调整预览呈现，使背景稍灰且模型主体、辅助线和控件层级更清楚。
2. 将视图、投影、plate、网格、gizmo、fog 和裁切相关计算与真实数据、真实 viewport 的可用状态绑定。
3. 让加载状态覆盖初次渲染前、参数重新渲染、同一文件刷新和远端异步等待场景。
4. 调用独立 subagent review 本 Phase diff 或涉及文件清单；修复 review 发现的问题后重新回归。
5. 更新 `plan-00-result.md` 并提交 Phase 4。

验收方式：

- 背景色和预览状态有可验证标识。
- 初次加载、resize 后和同一文件刷新都能完整显示模型。
- 摄像机距离、plate、网格和其他渲染辅助元素不会因为远端异步结果尚未到达而使用永久占位结果。
- 加载状态在初次渲染前、参数重新渲染和远端异步等待场景下有浏览器测试覆盖。
- 现有 preview error 和 stale preview 行为不回退。
- 独立 subagent review 无 blocker。

## Phase 5：远距离相机与完整回归

背景：

- 用户报告摄影机距离太远时模型展示不完全。

症状：

- camera near/far 或投影范围可能没有覆盖当前 mesh bounds。

目标：

- 远距离相机仍能完整显示模型。
- 保护前面 Phase 已完成的控件、坐标、ViewportGizmo、相机交互、加载状态和尺寸时机行为。

输入：

- Phase 0 中远距离相机和裁剪相关失败测试。
- Phase 1-4 的全部已通过变更。

前序目标保护：

- 保护 Phase 1 的数值控件行为。
- 保护 Phase 2 的 ViewportGizmo 三轴显示和六向点击。
- 保护 Phase 3 的项目坐标系、camera preset 和 OrbitControls 行为。
- 保护 Phase 4 的加载状态、真实尺寸时机和可读性。

操作步骤：

1. 让相机投影范围跟随当前视图和模型 bounds。
2. 使用针对性测试验证远距离场景。
3. 运行 `studio-web` typecheck、unit、e2e 与 build。
4. 调用独立 subagent 做只读完整 review；修复 review 发现的问题后重新回归。
5. 更新 `plan-00-result.md` 并提交 Phase 5。

验收方式：

- 远距离相机测试通过。
- `studio-web` typecheck、unit、e2e 与 build 通过。
- `plan-00-result.md` 记录每个 Phase 的执行结果。
- 独立 subagent 完整 review 无 blocker 后结束本轮任务。
