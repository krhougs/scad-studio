# Studio Web Feature Parity 修复计划

## 背景

本计划基于 **2026-04-24 的重新审计**，不采信 `prompt-archives/2026042300-studio-web-feature-parity/plan-00-result.md` 的“已完成”自述。判定依据只有三类：

1. `studio-web` / `studio-web-wasm` 当前真实代码；
2. `studio-app` / `scad-viewer` / `studio-common` 当前真实实现；
3. 现有 smoke / unit / Playwright 测试是否真的覆盖了用户可见功能，而不是只覆盖占位路径。

审计结论表明，当前 `studio-web` 仍有几类问题会直接导致“smoke 通过，但验收不过”：

- `.scad` tab 不是桌面端那种真实 viewer，缺 3D canvas、导出、切片器动作；
- 设置页保存的 OpenSCAD / slicer 配置没有进入 workbench 的预览、导出、切片器请求；
- 参数编辑仍是手工 `name=value` 字符串，不是桌面端的 Customizer 参数模型；
- 预设路径与文件格式和桌面端不兼容；
- watch 刷新只覆盖目录树和激活 `.scad`，打开中的 Markdown / 图片 / mesh / 预设不会自动更新；
- 3D viewer 只具备基础 orbit / pan / zoom，桌面端已有的一整套工具栏、状态信息和切片器动作未接齐；
- 文档类型与 Markdown 渲染能力仍低于桌面端；
- 旧 result 里还有若干直接写错的实现项，例如 `use-camera-controller.ts`、`canvas-toolbar.tsx`、`canvas-statusbar.tsx`、`renderer-controller.ts` 当前并不存在。

## 目标

按“用户可验收的真实功能”而不是“占位 UI + smoke”来补齐 Web 端，并把 smoke / unit / Playwright 改造成能阻止同类误判再次发生。

## 非目标

- 不回退 `app-server-*`、`studio-common`、`studio-app` 已经成立的架构边界。
- 不为了让 smoke 通过继续增加新的占位文案或空壳按钮。
- 不在没有必要的前提下新增第二套参数、预设或 viewer 状态模型。
- 不默认修改 server 协议；优先复用现有共享代码和 wasm 能力，只有共享代码无法满足时才扩协议。

---

## Phase 1：先把错误的“完成判定”堵住

### 目标

先把当前 smoke / Playwright 的覆盖盲区补齐，避免后续修复过程中再次出现“测试绿了，但真实工作流仍缺一截”。

### 前序目标保护

- 不重写现有 workbench 架构。
- 不为通过测试引入新的假实现。
- 只把审计已确认的用户可见缺口转成回归用例。

### 输入

- 本次审计结论；
- `packages/studio-web/tests/playwright/*.spec.ts`；
- 旧 result 中声称“已完成”的功能点；
- `studio-app` 对应真实行为。

### 操作步骤

1. 为 `.scad` tab 增加真实验收条件，而不是只断言 `scad-preview-status` 有文本。
2. 为设置链路增加请求级校验：
   - 保存 `openscad_path` 后，`.scad` 预览请求要吃到该值；
   - 配置了 slicer 后，`SlicerList` / `ExportRun` 要吃到该值。
3. 为切片器动作增加用例，区分“列表展示”与“发送到切片器”。
4. 为预设兼容增加用例，至少覆盖桌面端 `*.scad.json` 文件能被 Web 识别。
5. 为 watch 增加文档级回归：
   - 已打开 Markdown 自动刷新；
   - 已打开图片自动刷新；
   - 已打开 mesh 自动刷新；
   - `.scad` 预设文件变化自动刷新。
6. 为 `.scad` viewer 增加 UI 级回归，明确要求存在真实 canvas 与导出 / 切片器入口。

### 验收标准

- 新增的回归用例能准确表达当前缺口，且不会被“有文本、有按钮、无真实功能”的实现误判通过。
- 用例命名和断言都直接对应用户可见行为，不再依赖“终态文字存在即可”这类弱断言。
- 回归矩阵至少显式覆盖以下审计结论：
  - `.scad` tab 不能再只用状态文本判定通过，必须验证真实 canvas 与导出 / 切片器入口；
  - 设置保存后的 `openscad_path` 必须被 `.scad` 预览请求消费；
  - 切片器不能只验证列表出现，还要验证“发送到切片器”动作；
  - 预设不能只验证 `<source>.presets.json` 自己回环，必须覆盖桌面端 `*.scad.json` 兼容；
  - watch 不能只验证目录树刷新，必须验证打开中的 Markdown / 图片 / mesh / 预设会刷新。
- 旧 result 中引用不存在文件的功能点不得再作为 smoke 通过的替代证据。

---

## Phase 2：把配置链路接到 workbench，而不是停留在 `/settings`

### 目标

让 Web workbench 在预览、导出、切片器三个请求面上真正消费 `AppConfig`，并把设置页补到能编辑桌面端已有的关键字段。

### 前序目标保护

- 保持 `studio-common` 管业务状态、React 只管壳层状态的边界。
- 不把 `AppConfig` 生搬进 Zustand 全局 UI store。
- 不破坏 Phase 1 新增回归用例的判定语义。

### 输入

- `packages/studio-web/src/routes/settings.tsx`
- `packages/studio-web/src/workbench/{workbench-layout,export-panel,slicer-panel}.tsx`
- `packages/studio-web/src/viewers/scad-split-viewer.tsx`
- `crates/studio-app/src/protocol_client.rs`
- `crates/scad-viewer/src/ui/settings_dialog.rs`

### 操作步骤

1. 在 workbench 建立共享配置快照，来源为首次进入时的 `ConfigLoad` 与设置保存后的更新。
2. 把配置快照注入：
   - `.scad` `PreviewRequest.configured_openscad_path`
   - `SlicerList.configured`
   - `ExportRun.configured_openscad_path`
   - `ExportRun.configured_slicers`
3. 把设置页补到至少覆盖桌面端已有的关键编辑项：
   - `openscad_path`
   - `slicers`
   - `floating_panel_opacity`
4. 清理当前“settings 自己有 client、workbench 自己有 client”的割裂体验，保证保存后 workbench 可见。
5. 为配置加载失败、保存失败、配置缺失三种状态补可观察的 UI 和日志。

### 验收标准

- 在 Web 设置页改动配置后，返回 workbench 不刷新页面也能影响后续预览、导出和切片器请求。
- `.scad` 预览请求与桌面端一样显式带上 `configured_openscad_path`。
- 切片器列表不再固定为空数组；`ExportRun` 不再写死空配置。
- `SlicerList`、`ExportRun`、`.scad` `PreviewRequest` 三条请求链都必须消费同一份共享配置快照，不能出现“设置页可保存、workbench 不生效”的孤岛状态。
- 设置页不再只是显示 slicer 数量，至少能编辑桌面端已有的 slicer 路径与 `floating_panel_opacity`。

---

## Phase 3：把 `.scad` tab 修成真正的 viewer 工作流

### 目标

让 `.scad` tab 与桌面端一样进入真实模型查看工作流，而不是继续停留在“源码 + 状态文本”的降级页面。

### 前序目标保护

- 保护 Phase 1 的回归用例，避免再次用占位文本混过验收。
- 保护 Phase 2 的配置透传，`.scad` viewer 改造后仍必须消费配置快照。
- 不破坏现有 `.stl` / `.3mf` mesh viewer 的基础 orbit / pan / zoom。

### 输入

- `packages/studio-web/src/workbench/{canvas-zone,scad-workbench,inspector}.tsx`
- `packages/studio-web/src/viewers/{scad-split-viewer,mesh-viewer,mesh-three}.tsx`
- `crates/studio-app/src/viewer_tab/mod.rs`
- `crates/scad-viewer/src/ui/{side_panel,toolbar,status_bar}.rs`

### 操作步骤

1. 决定 `.scad` tab 的最终承载形式：
   - 优先方案：让 `.scad` tab 进入与 `.stl` / `.3mf` 同一套真实 mesh canvas；
   - 参数/预设面板作为附加区域，而不是替代 viewer。
2. 让 `.scad` 页签显示真实 canvas、mesh 统计、相机预设结果。
3. 打通 `.scad` 页签上的导出面板与切片器动作。
4. 把当前对 `.scad` 无效的 view pills 改成真实驱动相机状态。
5. 若仍保留源码视图，明确它是附加信息，不再承担“预览”的主职责。

### 验收标准

- 打开 `.scad` 后能看到真实 mesh canvas，而不是只有 `preview ready | vertices ...` 文本。
- `.scad` 页签存在导出与切片器入口，且动作可用。
- `.scad` 页签上的相机预设按钮能影响真实 viewer，而不是只改标签文字。
- `.scad` 页签不再以 `ScadSplitViewer` 的源码区加状态文本承担主要预览职责；若保留源码视图，它只能是附加信息。
- `.scad` 与 `.stl` / `.3mf` 至少要共享同级别的 viewer 能力边界，不能再出现“只有 mesh tab 才能导出 / 切片”的分叉。

---

## Phase 4：参数与预设收敛到共享语义

### 目标

把 Web 参数编辑和预设读写收敛到与桌面端一致的共享模型，消除“手工 defines”和“自造预设文件格式”。

### 前序目标保护

- 保护 Phase 2 的配置透传和 Phase 3 的真实 `.scad` viewer。
- 不新增第二套并行参数模型。
- 不让预设迁移破坏已有文件读取；若存在历史 `.presets.json`，必须明确兼容或迁移策略。

### 输入

- `crates/studio-common/src/{document,params,presets}.rs`
- `crates/app-server-protocol/src/presets.rs`
- `packages/studio-web/src/workbench/{parameters-panel,preset-io,presets-panel,scad-workbench}.tsx`
- `packages/studio-web/src/viewers/scad-split-viewer.tsx`

### 操作步骤

1. 优先复用共享代码，而不是在 TS 再造一套参数解释器：
   - 评估把 `studio_common::parse_parameters`、`preset_path_for_source`、`PresetFile` 序列化能力经 `studio-web-wasm` 导出给 Web；
   - 只有这条走不通时，才考虑协议扩展。
2. 参数面板改为按共享模型渲染：
   - 数值、布尔、枚举控件；
   - 单项恢复默认值；
   - 与桌面端一致的 `current_defines` 语义。
3. 预设路径改为 `*.scad.json`，文件结构改为共享 `PresetFile`。
4. 明确历史 `<source>.presets.json` 的处理策略：
   - 兼容读取；
   - 一次性迁移；
   - 或显式不兼容并提供迁移提示。
5. 预设保存、加载、删除都改成共享语义，并与参数状态保持一致。

### 验收标准

- Web 能像桌面端一样直接显示 `.scad` 中可编辑参数，而不是依赖用户手输参数名。
- Web 读写的预设文件与桌面端互通。
- 参数编辑、恢复默认值、应用预设后三者产生的 `defines` 与桌面端一致。
- 参数面板不再只是字符串表单，必须体现共享参数模型的类型信息，至少覆盖数值、布尔、枚举和单项恢复默认值。
- 新实现不能继续把 `<source>.presets.json` 当成默认真相；若保留兼容读取，必须明确迁移或兼容规则。

---

## Phase 5：补齐文档级 watch 刷新，而不是只刷新目录树

### 目标

把 watch 从“目录树刷新”补成“打开中的文档会刷新”，覆盖 Markdown、图片、mesh、`.scad` 源文件和预设文件。

### 前序目标保护

- 保护 Phase 3 的 `.scad` viewer 和 Phase 4 的参数 / 预设状态。
- 不因为追求精确匹配而退化成完全不刷新；在协议粒度有限时，优先保证用户看到的内容正确。

### 输入

- `packages/studio-web/src/workbench/workbench-layout.tsx`
- `packages/studio-web/src/viewers/{markdown-viewer,image-viewer,mesh-viewer}.tsx`
- `packages/studio-web/src/workbench/scad-workbench.tsx`
- `crates/studio-app/src/{markdown_tab,image_tab,viewer_tab/io}.rs`

### 操作步骤

1. 给 viewer 层引入文档级 invalidation 输入，而不是只依赖 `path`。
2. 根据 watch 事件刷新：
   - Markdown 文本；
   - 图片内容；
   - mesh 预览；
   - `.scad` 源码与真实 viewer；
   - 预设文件列表。
3. 在协议仍只有目录级事件时，定义保守但可解释的刷新策略，并把日志写清楚。
4. 避免重复请求风暴，为当前活动文档与后台标签分别定义刷新策略。

### 验收标准

- 外部修改已打开文档后，用户看到的内容会刷新，不再只有目录树变化。
- `.scad.json` 预设文件被外部改写后，Web 参数 / 预设面板会同步更新。
- watch 回归用例覆盖 Markdown、图片、mesh、`.scad` 与预设文件。
- 验收时必须分别验证 Markdown、图片、`.stl` / `.3mf`、`.scad`、预设文件五类打开中的内容刷新，不能再用“目录树刷新成功”替代。

---

## Phase 6：补齐 viewer 控件、切片器动作与设置项

### 目标

在已有基础 viewer 之上，把桌面端已经存在、Web 仍缺失的操控面补齐，至少完成可直接影响建模和验收的部分。

### 前序目标保护

- 保护 Phase 3 的真实 viewer 和 Phase 5 的 watch 刷新。
- 不把当前基础 orbit / pan / zoom 改坏。
- 所有新增控件都必须真正接到 viewer 状态，禁止再次出现只改文案不改行为的按钮。

### 输入

- `packages/studio-web/src/viewers/mesh-three.ts`
- `packages/studio-web/src/workbench/{canvas-zone,inspector}.tsx`
- `crates/scad-viewer/src/ui/{toolbar,status_bar,side_panel,settings_dialog}.rs`

### 操作步骤

1. 以桌面端 toolbar / status bar 为基线，梳理 Web 端缺失项。
2. 优先补齐验收影响最大的控件：
   - render mode / projection / grid / axis / build plate / shadow 这类可见控制；
   - viewer 状态信息与 mesh 统计；
   - “发送到切片器”动作。
3. 设置页同步补齐与这些控件直接相关、桌面端已存在的关键配置。
4. 若某些桌面端能力在 Web 不可行，需在文档里明确标注，并从计划里单列为非本轮目标，不能伪装成已完成。

### 验收标准

- Web 端 viewer 控件不再只剩四个 preset 按钮。
- 切片器不再只是只读列表，而是能触发与桌面端同语义的动作。
- 已补的 viewer 控件都能通过自动化或可重复的人工步骤验证。
- viewer 上层能力必须明显超过当前“基础 orbit / pan / zoom + 4 个 preset”下限，至少补齐一组桌面端已经存在且用户可直接感知的控制项。
- “发送到切片器”必须形成真实工作流，而不是继续停留在只读信息展示。

---

## Phase 7：补齐低优先级文档能力，并清理旧 result 的误导项

### 目标

处理前面阶段之外的中低优先级差距，并把“旧 result 里写了但代码里不存在”的内容彻底清理干净。

### 前序目标保护

- 不为了追求文档能力覆盖而重新引入新的平行实现。
- 不把未完成项换个名字继续保留在 result 文档中。

### 输入

- `packages/studio-web/src/workbench/tab-kind.ts`
- `packages/studio-web/src/viewers/markdown-parser.ts`
- `crates/studio-app/src/{main,markdown_tab}.rs`
- `prompt-archives/2026042300-studio-web-feature-parity/plan-00-result.md`

### 操作步骤

1. 补齐图片类型支持差距：`gif` / `bmp` / `tif` / `tiff` / `ico`。
2. 决定 Markdown 策略：
   - 若继续保留极简 parser，明确它不是 parity 完成；
   - 若要对齐桌面端，就升级为更完整的 CommonMark / GFM 方案。
3. 清理或更正文档中对不存在文件、未完成功能的错误描述。
4. 把仍然不能在本轮完成的项目重新登记为真实遗留，而不是继续写成“已完成”。

### 验收标准

- 低优先级差距有明确处理结论，不再混在“已完成”列表里。
- 历史 result 中与现状冲突的描述被标记为失效或由新归档覆盖。
- 旧 result 中提到但当前不存在的文件与实现项必须被更正或显式判废，避免后续继续把它们当成既有能力。
- 图片类型支持范围与 Markdown 能力边界必须写成明确结论，不能继续停留在“看起来差不多”的模糊状态。

---

## 总体验收

完成全部 Phase 后，至少需要满足以下条件，才能把 `studio-web` 重新提交验收：

1. `.scad`、`.stl`、`.3mf` 三类模型页签都进入真实 viewer 工作流。
2. 设置页保存的配置会影响 workbench 的预览、导出、切片器行为。
3. 参数与预设与桌面端共享同一语义，而不是另一套字符串协议。
4. watch 事件能让打开中的文档刷新，而不是只更新目录树。
5. smoke / Playwright 的断言升级为真实功能断言，不能再被占位实现骗过。
6. `.scad` 页签具备真实 canvas、导出入口与“发送到切片器”动作，不再低于 mesh 页签一个等级。
7. 参数编辑不再依赖用户手写 `name=value`，预设文件默认格式与桌面端互通。
8. viewer 控件能力明显超过当前 4 个 preset 按钮的状态，并且每个新增控件都接了真实行为。
9. 图片类型与 Markdown 能力的差距要么补齐，要么在归档与文档中明确列为真实遗留，不能再写成“已完成”。
10. 本计划完成后的验收依据以源码、自动化回归和可重复的用户操作路径为准，不接受旧 result 的文字自述替代事实。
