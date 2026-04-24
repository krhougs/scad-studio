# Plan-00：Studio Web 预览控制与坐标修正

## 背景

Plan-02 已补齐 Studio Web 侧栏、预览、参数、相机和配置的一批能力，但后续反馈指出控件、坐标、加载状态、视口时机和远距离相机仍未达到可用标准。本计划只处理这组反馈，不扩大到 app server、protocol 或跨端状态机重构。

## 强制约束

- `parameters` 和 `camera` 的数值编辑必须使用 `react-knob-headless` 绘制 knob，并使用 `@base-ui/react/number-field` 绘制输入框。
- 所有数值输入在输入过程中必须保持输入控件和相关 UI 排版稳定，不得出现宽高变化、位移或相邻元素跳动。
- 参数象限范围只根据模型参数初始值或显式范围变化，不根据 current value 变化。
- handle 指 `ViewportGizmo`；目标行为是点击切换视角。
- 相机拖拽、平移、缩放行为必须参考 `studio-app` OrbitControls，并在 Web 侧保持一致。
- 预览 XYZ 三轴必须符合 OpenSCAD 语义；实现前必须参考文档和当前源码。
- 初次渲染前、参数改变需要重新渲染时，以及任何等待远端异步加载的情况下，必须在 UI 的显眼处展示加载状态。
- 渲染链路必须严格检查所有依赖远端异步结果、真实 mesh bounds、真实图片尺寸、真实 viewport、projection 状态或 device pixel ratio 的计算；包括但不限于摄像机距离判断、near/far、orthographic 宽高、plate 尺寸、网格尺寸、gizmo 尺寸、fog 范围和裁切平面初始值。确认存在提前执行时，必须纳入本计划修复。

## 参考范围

- 当前 `studio-web` 参数、相机、预览区与测试代码。
- `studio-app` 中相机 OrbitControls 的交互语义。
- `scad-scene` 中 OpenSCAD 到 viewer 的坐标映射。
- Base UI NumberField、react-knob-headless 与 OpenSCAD 官方文档。

## Phase 1：失败测试与依据核对

背景：

- 当前问题覆盖参数范围、控件形态、数值输入稳定性、ViewportGizmo、坐标轴、显眼加载状态、视口尺寸、mesh bounds、渲染异步时机和远距离裁剪。
- 外部库 API、OpenSCAD 坐标规则和 desktop OrbitControls 行为必须以文档或源码为准。
- 渲染链路中可能还存在类似摄像机距离判断、plate 和网格大小的提前计算问题，需要在进入修复前系统检查。

症状：

- 现有测试不足以暴露这些问题，容易再次出现“看起来实现了但行为不对”的情况。
- 当前 Web 侧行为与 desktop viewer、OpenSCAD 语义和用户指定控件存在差异。

目标：

- 增加能先失败的单元测试和浏览器测试。
- 在测试设计前核对目标库 API、OpenSCAD 坐标映射和 desktop OrbitControls 行为。
- 检查渲染链路中所有依赖异步结果或真实尺寸的计算，把确认存在的提前执行问题纳入后续 Phase。
- 保护现有参数自动预览、canvas 不重叠、preview error、配置和布局测试。

解决思路：

- 用单元测试固定参数范围、相机几何计算和 OpenSCAD 语义轴。
- 用浏览器测试覆盖 knob、NumberField、输入过程布局稳定性、ViewportGizmo 点击切换、显眼加载状态、背景可读性、异步完成后的渲染尺寸计算和远距离相机显示。

验收方式：

- 新增测试先失败。
- 失败原因与目标行为一致，而不是测试写法错误或环境错误。
- 每个实现判断均能追溯到官方文档或当前源码。
- 渲染异步时机检查有明确覆盖范围，确认的问题不遗漏到后续任务之外。

## Phase 2：参数与相机数值控件

背景：

- 用户明确指定 `react-knob-headless` 与 `@base-ui/react/number-field`，这是本轮必要路径。
- 参数范围只应基于模型参数初始值或显式范围，不应随 current value 扩大。

症状：

- 当前控件不是目标库组件。
- 无显式范围时，current value 会影响推导范围，导致参数编辑范围不稳定。
- 数值输入过程中，控件宽度、按钮、标签和周边排版存在被动态内容影响的风险。

目标：

- 参数与相机数值编辑体验保持一致。
- 输入过程中控件和相关 UI 排版保持稳定。
- 参数推导范围只受初始参数定义或显式范围影响。
- 保护参数自动预览和相机状态共享，不新增端侧 protocol 行为。

解决思路：

- 在现有设计系统约束内引入用户指定控件库。
- 为参数和相机使用一致的数值编辑模式。
- 将范围推导从当前值变化中解耦。
- 为数值显示、输入框、按钮和周边布局设置稳定尺寸约束。

验收方式：

- 参数和相机可通过 knob、输入框与增减按钮编辑。
- 输入过程中相关 UI 不跳动、不变宽、不变高。
- current value 超出推导范围时，不会改变无显式范围参数的范围。
- 对应单元测试和浏览器测试通过。

## Phase 3：坐标轴、ViewportGizmo 与相机交互

背景：

- `scad-scene` 已定义 OpenSCAD 到 viewer 的坐标映射：OpenSCAD `[x, y, z]` 映射为 viewer `[x, z, -y]`。
- 用户澄清 handle 指 `ViewportGizmo`，且 ViewportGizmo 目标行为是点击切换视角。
- 相机实际拖拽交互需要参考 `studio-app` OrbitControls。

症状：

- Web 预览的 XYZ 轴方向和 OpenSCAD 语义不一致。
- ViewportGizmo 点击不可用或切换视角不符合预期。
- 相机拖拽、平移、缩放与 desktop viewer 行为存在差异。

目标：

- 坐标轴展示 OpenSCAD 语义，而不是误导用户的 viewer 内部轴。
- ViewportGizmo 点击后切换到对应视角。
- Web 相机拖拽、平移、缩放与 desktop OrbitControls 保持一致。
- 保护上一 Phase 的控件行为与 Plan-02 的 preview info。

解决思路：

- 以 `scad-scene` 坐标映射和 OpenSCAD 文档作为轴向依据。
- 以 desktop OrbitControls 作为鼠标交互依据。
- 将 ViewportGizmo 的验收范围限定为点击切换视角。

验收方式：

- 测试能断言 OpenSCAD X/Y/Z 与 viewer 方向的关系。
- 点击 ViewportGizmo 后视角发生预期变化。
- 鼠标拖拽、平移、滚轮缩放的方向和速度关系与 desktop 行为一致。

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

解决思路：

- 调整预览呈现，使背景稍灰且模型主体、辅助线和控件层级更清楚。
- 将视图、投影、plate、网格、gizmo、fog 和裁切相关计算与真实数据、真实 viewport 的可用状态绑定。
- 让加载状态覆盖初次渲染前、参数重新渲染、同一文件刷新和远端异步等待场景。

验收方式：

- 背景色和预览状态有可验证标识。
- 初次加载、resize 后和同一文件刷新都能完整显示模型。
- 摄像机距离、plate、网格和其他渲染辅助元素不会因为远端异步结果尚未到达而使用永久占位结果。
- 加载状态在初次渲染前、参数重新渲染和远端异步等待场景下有浏览器测试覆盖。
- 现有 preview error 和 stale preview 行为不回退。

## Phase 5：远距离相机与完整回归

背景：

- 用户报告摄影机距离太远时模型展示不完全。

症状：

- camera near/far 或投影范围可能没有覆盖当前 mesh bounds。

目标：

- 远距离相机仍能完整显示模型。
- 保护前面 Phase 已完成的控件、坐标、ViewportGizmo、相机交互、加载状态和尺寸时机行为。

解决思路：

- 让相机投影范围跟随当前视图和模型 bounds。
- 使用针对性测试验证远距离场景，再运行完整 Web 回归。

验收方式：

- 远距离相机测试通过。
- `studio-web` typecheck、unit、e2e 与 build 通过。
- `plan-00-result.md` 记录每个 Phase 的执行结果。
- 启动独立 subagent 做只读完整 review，且无 blocker 后结束本轮任务。
