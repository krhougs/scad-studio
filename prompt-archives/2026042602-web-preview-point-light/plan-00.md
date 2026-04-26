# Plan-00：Studio Web 预览额外点光源控制

## 背景

上一轮 Web 预览外观能力已经完成：背景颜色、网格颜色和光照强度可以在右侧 Inspector 中实时调整，并按 `.scad` 文件持久化到现有 `<stem>.scad.json`。当前需求是在这套外观配置上继续增加额外点光源控制，使用户能在不开启 shadow 时按需补光，并在开启 shadow 时保证运行时有点光源参与阴影效果。

本计划只处理 Web 端预览渲染与右侧 Inspector 控制，不修改 app server、protocol、transport、OpenSCAD 请求、mesh payload 或后端能力。

## 目标

- 增加额外点光源模式：`off`、`auto`、`manual`。
- 点光源模式按 `.scad` 文件持久化到现有 `<stem>.scad.json`。
- 手动点光源位置 `X / Y / Z` 按 `.scad` 文件持久化。
- 手动位置默认使用当前自动位置。
- manual 位置区域提供 `reset` 按钮，点击后把手动位置写成当前自动位置，并保持 manual 模式。
- 开启 shadow 时运行时强制启用额外点光源，但不修改持久化配置。
- 自动点光源位置使用摄像机 `front` framing 的同一套 distance 计算方式。
- 整理并用测试固定按钮状态切换流程，避免 appearance 控制触发 `.scad` 重新预览请求。

## 非目标

- 不新增全局偏好设置。
- 不修改现有 shadow 开关的持久化语义；shadow 仍属于 viewer toolbar 状态，不写入 `.scad.json`。
- 不新增光源颜色、光源强度、衰减半径或 helper 可视化控制。
- 不改变基础环境光、半球光和多方向补光策略。
- 不改变相机交互、ViewportGizmo、裁切、导出或切片器能力。

## 强制约束识别

- 用户指定额外点光源开关状态为 `off / auto / manual`。
- 用户指定点光源模式和手动位置都是 per `.scad` file 配置，继续写入现有 `<stem>.scad.json`。
- 用户指定 shadow 开启时强制启用额外点光源，但不能修改配置。
- 用户指定自动位置默认为摄像机 `front` 面右上角 45 度方向，distance 计算方式同摄像机位置计算方式。
- 用户指定 manual 位置设置需要 `reset` 按钮，reset 到自动位置。
- 前序主线要求仍然有效：修改 appearance 不应触发 `.scad` 重新渲染请求。
- 每个 Phase 必须保护前面 Phase 已达成的目标和边界。
- 每个 Phase 完成编码后必须调用独立 subagent review，review 输入包含当前 Phase 目标与验收标准、完整 `plan-00.md`、本次变更 diff 或涉及文件清单。

## 状态模型

### 持久化配置

`previewAppearance` 扩展以下字段：

- `pointLightMode`: `"off" | "auto" | "manual"`
- `pointLightPosition`: `[number, number, number]`

默认配置：

- `pointLightMode = "off"`
- `pointLightPosition = 当前自动位置`

旧 `.scad.json` 缺少上述字段时必须兼容读取，运行时使用默认值；写回时输出归一化后的字段。

`pointLightPosition` 的写入时机必须遵守以下规则：

- 缺失 `pointLightPosition` 时，运行时可以用当前自动位置补齐展示和渲染，但不能在 mesh bounds 或 viewport 未就绪时把 fallback 写入文件。
- 只有用户切换到 manual、编辑 X / Y / Z、点击 reset，或已有真实自动位置且需要保存 manual 默认位置时，才写入 `pointLightPosition`。
- 从旧文件读取并重新保存其他 appearance 字段时，如果用户没有触发 manual 相关操作，不应因为缺少 `pointLightPosition` 而写入不可靠 fallback。

### 自动位置定义

自动位置基于当前 mesh bounds、当前 viewport aspect ratio 和摄像机 `front` framing 规则计算：

1. 先使用与 `fitCameraToBounds(bounds, "front", aspectRatio)` 同源的 distance 计算方式，得到当前模型中心和 framing distance。
2. `front` 视角方向为 `[0, -1, 0]`。
3. 右上 45 度方向定义为 `normalize([1, -1, 1])`。
4. `autoPosition = center + normalize([1, -1, 1]) * distance`。

这里的“右上 45 度”指从 `front` 视角投影到屏幕平面后的右上方向；不是要求三维空间中与每个轴都形成 45 度夹角。

如果当前还没有 mesh bounds 或 viewport 未就绪，自动位置使用默认相机目标附近的稳定 fallback，并在 mesh 信息可用后重新计算。

### 按钮状态流

配置状态与运行时状态分离：

| 配置模式 | shadow | 按钮选中 | 运行时点光源 | 配置是否改变 |
| --- | --- | --- | --- | --- |
| `off` | 关闭 | `off` | 关闭 | 否 |
| `off` | 开启 | `off`，显示 forced 标记 | 自动位置开启 | 否 |
| `auto` | 关闭 | `auto` | 自动位置开启 | 否 |
| `auto` | 开启 | `auto` | 自动位置开启 | 否 |
| `manual` | 关闭 | `manual` | 手动位置开启 | 否 |
| `manual` | 开启 | `manual` | 手动位置开启 | 否 |

用户操作流程：

- `off -> auto`：保存 `pointLightMode = "auto"`，不修改 `pointLightPosition`。
- `auto -> manual`：保存 `pointLightMode = "manual"`；如果当前没有合法 `pointLightPosition`，写入当前自动位置。
- `manual -> auto`：保存 `pointLightMode = "auto"`，保留 `pointLightPosition` 作为下次 manual 的记忆值。
- `manual -> off`：保存 `pointLightMode = "off"`，保留 `pointLightPosition`。
- manual 下编辑 X / Y / Z：保存 `pointLightPosition`，保持 `pointLightMode = "manual"`。
- manual 下点击 `reset`：保存 `pointLightPosition = 当前自动位置`，保持 `pointLightMode = "manual"`。
- 开关 shadow：只影响运行时 `effectivePointLightMode`，不写入 `pointLightMode` 或 `pointLightPosition`。

## Phase 执行规则

每个 Phase 都必须按以下循环执行：

1. 干活：只处理当前 Phase 的目标，不顺手重构无关代码。
2. Review：调用独立 subagent 做只读 review，review 输入必须包含当前 Phase 目标与验收标准、完整 `plan-00.md`、本次变更 diff 或涉及文件清单。
3. 回归：按当前 Phase 验收方式运行针对性验证；review 发现 blocker 或 important 时先修复，再重新 review 和回归。
4. 记录：更新 `plan-00-result.md`，记录完成情况、变更摘要、验证结果和遗留问题。
5. 提交：当前 Phase 消除 blocker 且通过对应验证后提交；随后自动进入下一个 Phase，不等待用户确认。

## Phase 1：配置模型与失败测试

### 输入

- 上一轮 Web 预览 appearance 配置与测试。
- 现有 `.scad.json` 读写测试。
- 现有 `MeshViewerOptions`、`PreviewAppearancePanel`、`mesh-three` 和 `canvas-interaction` Playwright 用例。

### 前序目标保护

- 保护上一轮已完成的背景颜色、网格颜色、光照强度持久化。
- 保护 presets 与 `previewAppearance` 共存，不丢旧 presets。
- 保护修改 appearance 不触发 `.scad` preview request 的主线要求。
- 保护 shadow 开关现有 toolbar 行为。

### 操作步骤

1. 扩展单元测试，覆盖点光源模式和手动位置的默认值、读取、写回和非法值归一化。
2. 扩展纯函数测试，覆盖自动位置使用 `front` framing distance，且方向为 front 右上 45 度。
3. 扩展 Playwright 配置读写 / 状态流测试，覆盖右侧栏点光源模式切换、manual X/Y/Z 编辑、reset、per file 持久化和切换文件隔离。
4. 扩展 Playwright 运行时测试，覆盖 Three.js effective mode、点光源位置 dataset、shadow forced dataset 和点光源是否实际加入渲染状态。
5. 扩展 preview request dedup 测试，覆盖点光源模式、manual 位置和 reset 均不触发 `.scad` preview request。
6. 扩展 shadow 强制启用测试，覆盖 configured `off` + shadow on 时运行时点光源启用，但 `.scad.json` 仍保存 `off`，且 UI 选中态仍停留在 `off`。

### 验收标准

- 新增或扩展的单元测试在实现前失败，失败原因对应缺失的点光源配置和自动位置能力。
- 新增 Playwright 配置读写 / 状态流测试在实现前失败，失败原因对应缺失的右侧栏点光源 UI、reset 或持久化能力。
- 新增 Playwright 运行时测试在实现前失败，失败原因对应缺失的 Three.js 点光源、effective mode 或 shadow forced 渲染状态。
- preview request dedup 新断言在实现前失败，失败原因对应点光源 appearance 操作尚未接入。
- 现有外观配置、presets 和 dedup 测试不被删除或弱化。
- 独立 subagent review 无 blocker。

## Phase 2：点光源配置读写与右侧栏状态流

### 输入

- Phase 1 的失败测试。
- 现有 `PreviewAppearance` 和 `.scad.json` 读写能力。
- 现有右侧 Inspector appearance 面板。

### 前序目标保护

- 保护 Phase 1 测试语义，不通过降低断言让测试通过。
- 保护上一轮 appearance 写回队列、dirty/version 和 settings refresh 行为。
- 保护 presets round-trip，新增点光源字段时不能丢失 presets。
- 保护非 `.scad` mesh 直接预览不要求 per file 持久化。

### 操作步骤

1. 扩展 `PreviewAppearance` 运行时模型，加入点光源模式和手动位置。
2. 扩展 `.scad.json` parse / stringify，兼容旧文件并归一化非法字段。
3. 扩展右侧 appearance 面板，加入 `off / auto / manual` 状态控制。
4. manual 模式显示 X / Y / Z 数值输入和 `reset` 按钮；非 manual 模式隐藏或禁用手动位置编辑。
5. 实现按钮状态流：
   - 配置模式按用户点击写回。
   - shadow 强制启用只影响运行时状态，不写回配置。
   - reset 写入当前自动位置并保持 manual。
6. 保证所有点光源配置变化只更新 viewer options 和 `.scad.json`，不触发 `.scad` preview request。

### 验收标准

- `.scad.json` 单元测试通过。
- 右侧栏能看到点光源模式控制。
- manual 模式能编辑 X / Y / Z 并持久化。
- reset 后 X / Y / Z 变为当前自动位置，模式仍为 manual。
- 切换 `.scad` 文件时点光源配置按文件隔离。
- configured `off` + shadow on 不改 `.scad.json`。
- preview request dedup 测试通过。
- 独立 subagent review 无 blocker。

## Phase 3：Three.js 点光源运行时与自动位置

### 输入

- Phase 2 中进入 `MeshViewerOptions` 的点光源配置。
- Three.js `PointLight`、`castShadow`、shadow map 配置能力。
- 当前 mesh bounds、viewport、camera framing 和 shadow 开关状态。

### 前序目标保护

- 保护 Phase 1 和 Phase 2 建立的配置模型、持久化、状态流和 dedup 行为。
- 保护现有基础打光、背景、网格、相机、ViewportGizmo、裁切和 vertex color。
- 保护 shadow 开关现有可见行为，新增点光源只补足额外运行时光源。
- 保护现有 renderer `shadowMap.enabled`、mesh `castShadow / receiveShadow`、DirectionalLight shadow camera 和 toolbar shadow 开关链路；本 Phase 不重新定义整条 shadow pipeline。

### 操作步骤

1. 在 Three.js viewer 中创建额外 `PointLight`，默认不启用。
2. 计算 `effectivePointLightMode`：
   - `shadowsEnabled && pointLightMode === "off"` 时运行时视为 `auto`。
   - 其他情况沿用配置模式。
3. `effectivePointLightMode = "off"` 时隐藏或禁用点光源。
4. `effectivePointLightMode = "auto"` 时使用自动位置。
5. `effectivePointLightMode = "manual"` 时使用持久化手动位置。
6. 配置点光源 shadow 属性，使 shadow on 时点光源可投射阴影；shadow off 时点光源可补光但不投射阴影。该步骤只接入额外点光源，不改写已有 shadow map 开关、mesh cast/receive 规则或 DirectionalLight shadow 设置。
7. 通过 canvas dataset 暴露配置模式、运行时模式、点光源位置、是否 forced、点光源是否在场景中启用和点光源是否 castShadow，供 Playwright 验证。

### 验收标准

- 自动位置测试通过，distance 与 `front` framing 规则一致。
- Playwright 能验证 `off / auto / manual` 的运行时 dataset。
- Playwright 能验证 shadow on + configured off 时 `effectiveMode = auto`，点光源实际启用，`PointLight.castShadow = true`，UI 仍选中 `off`，且 `.scad.json` 仍为 `off`。
- Playwright 能验证 manual reset 后运行时位置变为当前自动位置。
- 现有 viewer toolbar、相机、加载状态和外观持久化测试通过。
- 独立 subagent review 无 blocker。

## Phase 4：完整回归与结果归档

### 输入

- Phase 1-3 的全部变更、review 结果和提交。

### 前序目标保护

- 保护 Phase 1 的配置模型和测试覆盖。
- 保护 Phase 2 的 per `.scad` 持久化、reset 状态流和 dedup 行为。
- 保护 Phase 3 的 Three.js 点光源运行时效果和 shadow 强制启用语义。
- 保护上一轮已完成的背景、网格、基础光照、presets、相机、watch refresh 和 `.scad.json` settings refresh 行为。

### 操作步骤

1. 运行前端 typecheck。
2. 运行相关 unit 测试。
3. 运行相关 Playwright 测试：
   - appearance / point light 持久化。
   - preview request dedup。
   - parameters presets round-trip。
   - canvas toolbar / shadow 交互。
4. 生成或检查 Playwright screenshot，并用最小可量化口径确认画面非空白、模型像素与背景像素存在可检测对比、shadow on/off 不导致画面全黑或全白、主要 UI 不遮挡模型。
5. 检查 `git diff`，确认没有后端协议、transport 或 OpenSCAD 请求契约改动。
6. 更新 `plan-00-result.md`，记录每个 Phase 的完成情况、验证命令、review 结论和遗留风险。
7. 提交最终结果。

### 验收标准

- `bun run --cwd packages/studio-web typecheck` 通过。
- 相关 unit 测试通过。
- 相关 Playwright 测试通过。
- 截图检查非空白，模型和网格可见；像素检查能证明模型与背景存在对比，shadow on/off 不导致全黑或全白。
- `plan-00-result.md` 完整记录 Phase 1-4。
- 工作树只包含本计划范围内的变更。
- 独立 subagent 完整 review 无 blocker。

## 执行完成判定

整个计划只有在以下条件全部满足时才算完成：

- 额外点光源模式 `off / auto / manual` 可在右侧栏切换。
- 点光源模式和手动位置按 `.scad` 文件持久化到现有 `<stem>.scad.json`。
- manual 模式 X / Y / Z 可编辑，reset 可写回当前自动位置且保持 manual。
- 自动位置使用 `front` framing distance 和 front 右上 45 度方向。
- shadow on + configured off 时运行时强制启用自动点光源，但不修改配置。
- 点光源相关 appearance 操作不触发新的 `.scad` preview request。
- 自动化测试和 Playwright 验证覆盖关键状态流。
- 每个 Phase 均完成独立 subagent review，并已写入 `plan-00-result.md`。
