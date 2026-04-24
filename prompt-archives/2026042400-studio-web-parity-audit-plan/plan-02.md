# Plan-02 讨论稿：Studio Web 预览与侧栏继续补齐

## 背景

Plan-01 已完成 `studio-web` 的左侧栏 Tab、设置 Tab、Markdown 预览、OpenSCAD CLI fallback、状态栏布局与产品命名等工作。本轮反馈集中在两类问题：

1. Web 端局部 UI 与设计稿、`studio-app` 仍存在差异。
2. 3D 预览能力仍缺少 `studio-app` 已有的相机、渲染模式、动态尺寸与参数自动预览能力。

补充反馈进一步明确了 dev server 外部访问、可拖动侧栏宽度、Tab 栏横向滚动、预览加载状态、参数重新渲染不重置相机、以及鼠标手势需要支持完整环绕调整。

本计划先作为讨论稿保存。执行前应以当前源码为准，不以旧 plan 或记忆替代代码核对。

## 已核对的当前事实

- `packages/studio-web/src/state/ui-store.ts` 的 `openTab` 已有按 `tab.id` 去重逻辑；重复打开仍异常时，应重点检查文件列表传入的 `id` 是否稳定，以及 UI 是否正确激活已有 Tab。
- `packages/studio-web/src/workbench/inspector-section.tsx` 当前使用 `CaretRight` / `CaretDown`，不符合本次要求的首行 `+/-` 状态展示。
- 左侧 `Files`、`Log`、`Settings` 仍大量使用 `.side-panel`、`.side-panel__body`、`.panel` 等嵌套边框与内边距，和“左侧内容直接占满宽度”的目标不一致。
- `packages/studio-web/src/workbench/log-panel.tsx` 当前仍有独立标题行和 `entries` 计数行，应改为列表内容组件，计数移动到上方标题区域。
- `packages/studio-web/src/workbench/rail.tsx` 当前 `history` 与 `log` 都使用 `ClockCounterClockwise`，需要为 Log 更换图标。
- `packages/studio-web/src/viewers/mesh-three.ts` 当前使用固定相机、固定底板、固定网格、较暗背景与灯光，缺少模型尺寸上报、动态 framing、fog、剖切与 mono/color 控制。
- `crates/scad-viewer/src/app.rs`、`crates/scad-viewer/src/ui/camera_overlay.rs`、`crates/scad-scene/src/camera.rs` 已有可参考的精确相机控制与按 bounds 调整视角逻辑。
- `packages/studio-web/src/workbench/parameters-panel.tsx` 当前仍显示 `apply` 按钮，数字参数只有 number input；需要改为节流自动预览与 slider。
- `packages/studio-web/src/config/app-config.ts` 与 `crates/studio-common/src/config.rs` 当前没有尺寸显示单位设置。
- `packages/studio-web/vite.config.ts` 当前 `server.host` 是 `127.0.0.1`，`scripts/run_studio_web_dev.ts` 也显式传入 `--host 127.0.0.1`，外部设备不能直接访问 dev server。
- Web 端当前通过 `VITE_WS_URL` 或 `?ws=` 直连 WebSocket；外部设备访问 dev server 时，`127.0.0.1` 会指向外部设备自身，不适合作为默认测试路径。
- `.app` 当前使用固定 `grid-template-columns: 52px 360px 1fr 320px`，左侧栏和右侧 Inspector 不能拖动，也没有跨会话宽度记忆。
- 当前 `MeshViewer` 进入 pending 时不会清空 canvas，但新 mesh 成功后 `setMesh` 很可能重新 framing；这会导致 parameters 重新渲染后相机状态丢失。
- 当前 `ImageViewer` 读取新图片时会清空旧 URL、尺寸和位移；需要区分“切换文件”和“同一文件刷新”的加载体验。

## 总体判断

本轮不应只改样式。预览区问题的根因是 Web 端缺少统一的模型 bounds 数据流，导致相机、底板、尺寸显示和预设视角都只能写固定值。Plan-02 应先补齐“mesh bounds → viewer info → inspector/settings”的数据通路，再实现相机和渲染模式，否则后续仍会出现固定尺寸模型可用、大尺寸或小尺寸模型不可用的问题。

## 假设

- 尺寸基础单位沿用当前预览协议的 `PreviewUnit::Millimeter`，设置只控制显示单位，不改变模型数据本身。
- 显示单位先支持 `mm`、`cm`、`in` 三种；英制单位默认使用 inch。
- 数字参数如果 OpenSCAD 注释或解析结果提供明确范围，则优先使用明确范围；没有范围时再根据当前值和默认值推导 slider 范围。
- 浏览器路由继续使用当前 `react-router-dom` 的 search params 机制，不手写 URL hash 解析器。
- `studio-app` 三线 handle 的具体交互以现有代码和运行效果为准；Web 端不凭印象新增不一致的行为。
- Parameters 触发的重新渲染属于同一文档刷新，不能自动 reset camera；只有切换活动文件、用户点击 reset、用户选择预设视角时才允许主动调整相机。
- Dev server 默认外部访问路径应尽量只暴露一个 HTTP 地址；WebSocket 默认走 Vite 反向代理，前端默认使用同源 WebSocket 路径。
- 左侧栏默认宽度沿用当前 `360px`，右侧栏默认宽度沿用当前 `320px`；最小宽度建议左侧 `280px`、右侧 `280px`，最大宽度由可用空间限制。
- 加载状态采用 stale-while-refresh：同一文件刷新时保留上一帧模型或图片，叠加非阻断加载提示；切换到不同文件时可以显示空加载态。

## 非目标

- 不重做 app server / protocol 架构。
- 不在 Web 端直接绕过 app server 调用文件系统或 OpenSCAD。
- 不改变 Markdown 预览库选择与安全策略，除非本轮修改引入回归。
- 不引入新的大型 3D 渲染框架；继续在当前 Three.js viewer 上补齐能力。

## Phase 0：Dev server 外部访问基线

### 输入

- `packages/studio-web/vite.config.ts`
- `scripts/run_studio_web_dev.ts`
- `scripts/run_websocket_host.ts`
- `packages/studio-web/src/workbench/workbench-layout.tsx`
- `README.md`
- `docs/getting-started.md`

### 要保护的前序目标

- 保留 `?ws=` 显式覆盖能力。
- 保留 `SCAD_STUDIO_WS_URL` 与 `STUDIO_WEB_PORT` 环境变量。
- 保留 app server WebSocket host 作为唯一后端能力入口，不让 Web 端绕过 protocol。

### 操作步骤

1. 将 Vite dev server 默认监听地址改为 `0.0.0.0`，`run_studio_web_dev.ts` 启动 Vite 时同步使用 `--host 0.0.0.0`。
2. 在 Vite dev server 配置 WebSocket 反向代理，例如同源路径 `/app-server/ws` 代理到 websocket-host。
3. 调整 `resolveWsUrl`：优先级保持 `?ws=` 高于环境变量；默认值改为根据 `window.location` 生成同源 WebSocket 代理 URL，避免外部设备拿到 `127.0.0.1`。
4. `run_studio_web_dev.ts` 默认注入同源代理配置；只有用户显式传入 `--ws-url` 或环境变量时才改用直接 WebSocket 地址。
5. 更新 README 与 getting-started，说明局域网访问方式、代理路径和覆盖变量。
6. 增加单元测试覆盖 `resolveWsUrl` 的 query、env、same-origin fallback 三种路径。

### 验收标准

- `bun run web` 默认输出可被局域网设备访问的 Vite 地址。
- 外部设备打开 dev server 地址后，不需要手动填写 `?ws=` 即可连接 app server。
- 显式 `?ws=` 和 `SCAD_STUDIO_WS_URL` 覆盖仍可用。
- 文档包含默认地址、代理路径和常见排错说明。

## Phase 1：侧栏与 Inspector 外观修正

### 输入

- 设计稿 `/Users/krhougs/LocalCodes/buddin/ui_kits/app/index.html`
- `packages/studio-web/src/workbench/inspector-section.tsx`
- `packages/studio-web/src/workbench/files-panel.tsx`
- `packages/studio-web/src/workbench/log-panel.tsx`
- `packages/studio-web/src/workbench/left-panel.tsx`
- `packages/studio-web/src/workbench/rail.tsx`
- `packages/studio-web/src/workbench/tabbar.tsx`
- `packages/studio-web/src/workbench/workbench-layout.tsx`
- `packages/studio-web/src/config/app-config.ts`
- `crates/studio-common/src/config.rs`
- `packages/studio-web/src/styles/workbench-zones.css`
- `packages/studio-web/src/styles/phase7.css`
- `packages/studio-web/src/styles/workbench.css`
- `packages/studio-web/tests/unit/app-config.test.ts`
- `crates/studio-common/tests/config_tests.rs`

### 要保护的前序目标

- 保留 Plan-01 已实现的左侧栏 Tab 结构。
- 保留设置作为左侧栏 Tab 的行为。
- 保留 `react-router-dom` search params 与左侧 Tab 联动，不改回手写 URL 解析。
- 保留 `@phosphor-icons/react`，侧栏图标继续使用 `weight="bold"`。

### 操作步骤

1. 将 `InspectorSection` 的展开状态展示改为标题首行的 `+` / `-` 文本按钮，移除 caret 图标依赖。
2. 提炼左侧标题区组件，允许标题区展示 `Files` 当前路径、`Log` 条数、`Settings` 状态等元信息。
3. 将 `LogPanel` 改成纯列表内容组件，删除内部标题行和 `xx entries` 行。
4. 将 `Files`、`Log`、`Settings` 的内容区改为满宽布局，移除嵌套 `.panel` 边框；必要时新增 `side-panel--flush` 或等价样式。
5. 为 Log 入口更换与 History 不同的图标，优先考虑 `TerminalWindow` 或 `ListDashes`，避免和历史记录语义冲突。
6. 将 `.app` 固定侧栏宽度改为 CSS 变量驱动：左侧默认 `360px`，右侧默认 `320px`。
7. 在左侧栏和右侧 Inspector 边界增加拖动条，拖动过程更新 CSS 变量，释放后按节流策略记忆宽度。
8. 在共享配置中增加 `left_panel_width` 与 `right_panel_width`，旧配置缺字段时使用默认宽度；拖动保存失败时仍保留当前会话内宽度。
9. Tab 栏启用横向 overflow；鼠标滚轮在 Tab 栏区域转为横向滚动，保留 Windows 上可见且深色适配的滚动条样式。
10. 为这些 UI 变化补充 Playwright 或 Testing Library 断言。

### 验收标准

- 右侧 section 第一行能直接看到 `+` 或 `-`，状态与展开行为一致。
- 左侧 Files、Log、Settings 内容宽度贴合左栏，不再出现双层卡片边框。
- Log 标题区包含条数信息，列表内不再显示独立 `xx entries` 行。
- Log 与 History 图标不同。
- 路由参数切换仍能正确激活左侧 Tab。
- 左右侧栏可拖动，有默认宽度、最小宽度，并能在刷新后恢复。
- Tab 数量超出宽度时可以通过鼠标滚轮横向滚动，Windows 下滚动条可见且不突兀。

## Phase 2：文件打开去重回归

### 输入

- `packages/studio-web/src/state/ui-store.ts`
- `packages/studio-web/src/workbench/files-panel.tsx`
- `packages/studio-web/src/workbench/workbench-layout.tsx`
- `packages/studio-web/tests/unit/ui-store.test.ts`
- `packages/studio-web/tests/playwright/workbench-layout.spec.ts`

### 要保护的前序目标

- 不改变当前 Tab 的数据结构，除非确认 `id` 本身不稳定。
- 保留当前文件类型识别与文件图标展示。

### 操作步骤

1. 先添加或补全 `ui-store` 单元测试，证明同一个 `tab.id` 重复打开只激活已有 Tab。
2. 添加浏览器层回归测试：从文件列表重复打开同一文件，只出现一个 Tab，并激活该 Tab。
3. 如果测试失败，沿文件列表点击路径查找 `tab.id` 是否使用了不稳定路径或显示名。
4. 只修复造成重复 Tab 的最小路径，不重写 Tab 管理。

### 验收标准

- 单元测试覆盖 store 去重。
- 浏览器测试覆盖文件列表重复打开。
- 用户重复打开同一文件时，不新增第二个 Tab。

## Phase 3：模型 bounds、尺寸与动态底板数据通路

### 输入

- `packages/studio-web/src/viewers/mesh-three.ts`
- `packages/studio-web/src/viewers/mesh-viewer.tsx`
- `packages/studio-web/src/viewers/scad-preview-viewer.tsx`
- `packages/studio-web/src/viewers/image-viewer.tsx`
- `packages/studio-web/src/viewers/viewer-options.ts`
- `packages/studio-web/src/workbench/canvas-zone.tsx`
- `packages/studio-web/src/workbench/inspector.tsx`

### 要保护的前序目标

- 不改变 app server 返回的 mesh payload 格式，除非确认前端无法可靠计算 bounds。
- 保留现有 preview status、错误展示与状态栏布局。
- 保留当前 `MeshViewerHandle.getStats()` 的 vertices / indices 能力，并向后兼容现有调用。

### 操作步骤

1. 在 Web viewer 内根据 positions 计算 `bounds`、`center`、`dimensions`、`radius`。
2. 将 `MeshStats` 扩展为 `MeshInfo`，通过回调传给 `CanvasZone` 与 `Inspector`。
3. 基于 `radius` 和最大边长计算 build plate 尺寸，最小值保护小模型，较大模型自动扩展。
4. 网格、坐标轴长度与 build plate 共用同一套动态尺寸输入。
5. 在无 mesh 或 mesh 为空时提供安全 fallback，避免相机和底板计算出现 `NaN`。
6. 为 mesh 加载状态引入非阻断 overlay：同一文件刷新时保留上一帧模型，pending 只显示加载提示；切换到不同文件时显示空加载态。
7. 为 image viewer 引入同样的加载状态：同一文件刷新时保留当前图片、缩放与位移，加载完成后替换；切换文件时重置为新文件加载态。

### 验收标准

- 不同尺寸模型能得到不同的底板、网格和坐标轴尺寸。
- Inspector 的 Preview section 可读取模型长宽高。
- 空模型或加载失败不会破坏预览区域。
- Parameters 修改触发重新渲染时，旧模型保持可见，加载提示不会清空 canvas。
- 图片刷新时旧图片保持可见，加载完成后再替换。

## Phase 4：相机控制与预设视角补齐

### 输入

- `crates/scad-viewer/src/ui/camera_overlay.rs`
- `crates/scad-scene/src/camera.rs`
- `packages/studio-web/src/canvas/camera-state.ts`
- `packages/studio-web/src/canvas/camera-controls.ts`
- `packages/studio-web/src/workbench/canvas-zone.tsx`
- `packages/studio-web/src/viewers/mesh-three.ts`
- `packages/studio-web/src/workbench/inspector.tsx`

### 要保护的前序目标

- 保留现有鼠标 orbit、pan、zoom 行为。
- 保留现有 perspective / orthographic 切换。
- 保留 toolbar 的基础布局，不让默认错误提示或状态栏遮挡预览内容。
- 保留同一文件 parameters 重新渲染前的相机状态，不因 mesh payload 更新自动 reset。

### 操作步骤

1. 参考 `OrbitalCamera::fit_bounds`，在 Web 端实现按 bounds 和 aspect ratio 计算相机距离。
2. 默认视角改为正面朝上斜 45 度，并确保完整模型可见。
3. 预设视角 `front/top/right/iso` 等全部通过当前 bounds 计算距离，不再使用固定距离。
4. 在右侧 Preview 或 Camera section 增加精确相机控制：target X/Y/Z、distance、azimuth、elevation、reset、预设视角按钮。
5. 在预览区域左下角增加三线 handle；先核对 `studio-app` 的实际行为，再按一致交互实现快速视角操作或相机面板入口。
6. 将 camera state 与 mesh payload lifecycle 分离：`setMesh` 只替换几何体，不在同一文件刷新时调用自动 framing。
7. 鼠标 orbit 支持绕模型完整环绕调整：azimuth 可连续变化，elevation 只在接近极点时限制，避免当前手势无法完成 360 度视角调整。
8. Canvas pointer 事件使用 pointer capture，并避免 toolbar、status bar、loading overlay 抢占正常拖动。
9. 添加相机纯函数单元测试，覆盖不同 bounds、不同 aspect ratio、空 bounds、同一文件重新渲染不 reset、连续 orbit。

### 验收标准

- 大模型、小模型、偏移模型都能在默认视角完整显示。
- 预设视角不会因为模型尺寸变化而过近或过远。
- 精确相机控制能实时改变预览相机。
- 左下角三线 handle 与 `studio-app` 行为一致。
- 修改 parameters 并重新生成预览后，相机 target、distance、azimuth、elevation 不被重置。
- 鼠标左键拖动可绕模型完成完整水平环绕，纵向拖动在接近极点时仍稳定。

## Phase 5：渲染模式、剖切与可见性修正

### 输入

- `crates/scad-viewer/src/ui/toolbar.rs`
- `crates/scad-scene/src/renderer.rs`
- `crates/scad-scene/src/pipeline.rs`
- `crates/scad-scene/src/shader.wgsl`
- `packages/studio-web/src/viewers/viewer-options.ts`
- `packages/studio-web/src/viewers/mesh-three.ts`
- `packages/studio-web/src/workbench/canvas-zone.tsx`
- `packages/studio-web/src/styles/viewers.css`

### 要保护的前序目标

- 保留现有 solid / wireframe / xray。
- 保留现有 grid / axis / build plate / shadow 开关。
- 保留黑色高端风格，但以模型可辨认为第一约束。

### 操作步骤

1. 在 `MeshViewerOptions` 增加 `colorMode: "mono" | "color"`、`fogEnabled`、`clipPlaneEnabled`，必要时增加剖切平面的 offset 与 normal。
2. 实现 mono/color：color 模式使用 vertex colors，mono 模式统一使用设计系统中的高可见材质色。
3. 实现 fog：在 Three.js scene 中配置与黑色背景匹配的低强度雾化，不遮蔽主体。
4. 实现剖切：启用 renderer local clipping，提供默认剖切平面，后续可在精确控制中扩展平面参数。
5. 调整背景色、环境光、主光、补光、轮廓光与材质 roughness/metalness，使黑色背景下模型可读。
6. 添加 Playwright 截图或 DOM 状态断言，确保模式按钮存在并能切换。

### 验收标准

- Toolbar 或右侧 section 可切换 mono/color、fog、剖切。
- mono 模式下无 vertex color 的模型仍清晰可见。
- 背景仍为深色，但模型轮廓、顶面、侧面能被正常识别。
- 剖切和 fog 开关不会破坏 wireframe/xray。

## Phase 6：参数自动预览、Preset 与数字 slider

### 输入

- `packages/studio-web/src/workbench/scad-workbench.tsx`
- `packages/studio-web/src/workbench/parameters-panel.tsx`
- `packages/studio-web/src/workbench/presets-panel.tsx`
- `packages/studio-web/src/workbench/parameter-model.ts`
- `packages/studio-web/tests/unit/parameter-model.test.ts`
- `packages/studio-web/tests/playwright/parameters-presets.spec.ts`

### 要保护的前序目标

- 保留参数解析失败时的错误展示。
- 保留恢复默认按钮。
- 保留已有 preset 读取、应用、删除能力。
- 不让参数输入时的每个键盘事件都立即触发 OpenSCAD 预览请求。
- 不让参数重新渲染破坏当前相机或上一帧预览。

### 操作步骤

1. 删除 `apply` 按钮与相关测试断言。
2. 将参数值变化改为立即更新表单状态，并通过防抖或节流更新实际 preview defines。
3. 将 save preset 表单移动到 Parameters section；Presets section 只保留已保存 preset 列表、加载与删除。
4. 数字参数显示 number input + slider。number input 保证精确输入，slider 保证快速调整。
5. slider 范围策略：
   - 有明确 min/max 时使用明确范围；
   - 无明确范围时，基于当前值和默认值推导对称范围；
   - 范围必须允许负数；
   - step 优先使用解析结果，否则根据当前值的小数位与数量级推导。
6. Parameters 触发 preview 时设置加载状态，但复用 Phase 3 的 stale-while-refresh 行为，避免清空已经打开的模型。
7. 参数节流逻辑增加单元测试或浏览器测试，验证快速连续输入只触发有限次数 preview 更新。

### 验收标准

- 参数修改后不需要点击 apply 即可更新预览。
- 快速拖动 slider 不会触发过量 OpenSCAD 请求。
- 恢复默认仍可用。
- save preset 位于 Parameters section。
- 数字参数 slider 支持负值范围。
- 参数更新期间有加载提示，但当前预览不消失，相机也不 reset。

## Phase 7：尺寸单位设置与 Preview section 信息展示

### 输入

- `crates/studio-common/src/config.rs`
- `crates/studio-common/tests/config_tests.rs`
- `packages/studio-web/src/config/app-config.ts`
- `packages/studio-web/src/workbench/settings-panel.tsx`
- `packages/studio-web/src/workbench/inspector.tsx`
- `packages/studio-web/tests/unit/app-config.test.ts`
- `packages/studio-web/tests/playwright/config-settings.spec.ts`

### 要保护的前序目标

- 保留现有配置兼容性，旧配置文件缺少新字段时必须使用默认值。
- 保留 app server 负责读写配置，Web 不直接写本地文件。
- 不改变 preview payload 的基础单位。

### 操作步骤

1. 在 `studio-common::AppConfig` 增加 `display_unit`，默认 `millimeter`。
2. 在 Web `AppConfigShape`、normalize 与 settings panel 增加对应字段。
3. Settings Tab 增加单位选择：`mm`、`cm`、`in`。
4. 在 Preview section 中展示模型 `width / depth / height`，按设置单位格式化。
5. 添加 Rust 配置序列化测试与 Web normalize 测试，确保旧配置兼容。
6. 添加浏览器测试，验证 settings 修改单位后 Preview section 的单位随之变化。

### 验收标准

- 旧配置能正常加载，新字段使用默认值。
- 设置中可以选择显示单位。
- Preview section 显示模型整体长宽高。
- 切换单位后展示数值和单位同步更新。

## Phase 8：回归验证与独立 review

### 输入

- 本 plan 全文。
- 本轮所有 diff。
- 相关测试输出。

### 要保护的前序目标

- 不为了通过单项测试而撤销前面 Phase 已实现的 UI、路由、预览或配置行为。
- 独立 review 不写文件，只输出问题清单。

### 操作步骤

1. 每个 Phase 完成后运行对应单元测试或浏览器测试。
2. 每个 Phase 完成后调用独立 subagent review，review 输入包含本 Phase 目标、验收标准、完整 plan 和本次 diff。
3. 修复 review 中的 blocker 与明确风险，再继续下一个 Phase。
4. 最后运行完整验证：
   - `bun --filter studio-web typecheck`
   - `bun --filter studio-web test`
   - `bun --filter studio-web playwright`
   - `bun --filter studio-web build`
   - `bun run web` 手动或自动 smoke，确认 dev server 监听 `0.0.0.0` 且 WebSocket 代理可连接
   - `cargo test -p studio-common`
   - `cargo check --workspace`
5. 更新 `plan-02-result.md`，记录每个 Phase 的结果、验证命令与遗留问题。

### 验收标准

- 全部本轮需求均有代码或测试覆盖。
- 完整验证通过；若存在既有 warning，需标明来源与影响。
- 独立 review 无 blocker 后才认为 Plan-02 完成。

## 需要确认的决策

1. 数字参数无明确范围时，建议 slider 使用 `base = max(abs(current), abs(default), 1)`，范围为 `[-2 * base, 2 * base]`；如果用户继续输入或拖动超出范围，再按新值重新计算范围。这个策略能满足负数和动态范围，但不会把 slider 拉得过大。
2. 显示单位建议固定为 `mm`、`cm`、`in`，不加入 feet。OpenSCAD 与 3MF 当前基础单位都是 millimeter，feet 对模型预览价值较低。
3. 剖切第一版建议先实现开关、默认平面和 offset；完整的法线向量编辑可以作为后续增强，除非本轮要求必须与 `studio-app` 完全一致。
4. 同一文件刷新时保留上一帧预览；切换文件时显示新文件加载态。这样可以避免 parameters 更新破坏当前预览，也避免用户切换文件时短暂误看旧文件。
