# scad preview request dedup plan

## 背景

打开 `.scad` 预览时，WebSocket 中出现两次 mesh。独立 dev server 实测已确认：前端对同一个 `examples/cube.scad` 发出了两个不同 `request_id` 的 `preview.request`，服务端分别正常返回两个 `preview_ready`。STL 路径没有复现重复 mesh。

根因集中在 Web 前端 `.scad` 预览状态链路：源码解析完成后设置一次 `appliedDefines`，随后参数 debounce 又设置一次内容相同但引用不同的 `appliedDefines`，触发 `MeshViewer` 预览 effect 再次发送请求。

## 目标

修复 `.scad` 预览重复发送等价 `PreviewRequest` 的问题，并用自动化测试固定行为。

## 非目标

- 不移除 React `StrictMode`。
- 不调整后端 `app-server-host`、transport、protocol 的一请求一响应语义。
- 不重构整个 workbench、参数面板或 viewer 架构。
- 不新增与重复预览请求无关的功能。

## 强制约束识别

- 用户要求“石锤”和“确定结果”：执行计划前已有 request id 级别的实测证据，实施阶段必须保留或转化为可重复验证的测试。
- 项目要求 Phase review 必须使用独立 subagent。每个 Phase 完成编码和验证后，必须将当前 Phase 目标、完整计划和 diff 或文件清单交给 subagent review。
- 计划执行过程必须自动推进，只有遇到真实 blocker 才暂停。
- 新增测试与辅助脚本优先使用 `bun` 生态与现有 Playwright / Vitest 基础设施。

## Phase 1: 固化复现与回归测试

### 输入

- 已实测的 `.scad` 重复请求现象：`request_id=10` 与 `request_id=12` 均为 `preview.request`，source 和 defines 等价。
- 现有 Playwright smoke harness 已能记录 outgoing WebSocket frame，并可用 protocol wasm 解码 client frame。

### 操作步骤

1. 扩展或复用现有 Playwright recorder，使测试能够拿到所有 outgoing `preview.request` 的 `request_id`、source、defines、configured OpenSCAD path。
2. 增加一个面向 `.scad` 预览的失败回归测试：打开 `examples/cube.scad`，等待首次 preview response 可观测后，继续等待超过参数 debounce 的稳定窗口，再统计 decoded outgoing frame。稳定窗口不得短于 350ms，避免第二个 `preview.request` 尚未发出时测试提前结束。
3. 测试必须明确区分 `file.read`、`slicer.list` 与 `preview.request`，避免把其他协议请求误判为 mesh 请求。
4. 测试必须用协议层字段构造重复请求 key：source 使用 decoded `preview.request.source` 的稳定 JSON 表示，defines 使用按内容序列化后的数组，configured OpenSCAD path 使用 decoded payload 中的值；禁止用对象引用或 UI 文案判断重复。
5. 失败信息必须输出重复 key、全部 request id、每个请求的发送时间或相对时间差、configured OpenSCAD path，确保可以直接判断是否为两个不同请求。
6. 先在修复前运行该测试，确认它能稳定暴露当前重复请求问题。

### 验收标准

- 修复前测试能观测到至少两个等价 `preview.request`，并失败。
- 测试输出包含足够信息定位重复请求：source+defines+configured OpenSCAD path key、request id 列表、发送时间差。
- 不依赖真实用户配置，不写入用户真实 HOME 或系统配置目录。

### 保护前序目标/边界

这是第一个 Phase。执行时必须保护现有 smoke harness 对其他 spec 的兼容性，不能破坏已有 `installProtocolRecorder` 调用方。

### Phase review

完成后调用独立 subagent review，重点检查：

- 测试是否真的验证 `.scad` 重复 `PreviewRequest`，而不是验证 UI 文案。
- recorder 扩展是否会污染其他 Playwright spec。
- 失败信息是否足够支撑后续修复判断。

## Phase 2: 收敛 `.scad` appliedDefines 等价更新

### 输入

- Phase 1 中可失败的重复请求测试。
- `.scad` 状态链路中存在多处 `setAppliedDefines(formatCurrentDefines(...))` 或等价直接设置。

### 操作步骤

1. 在 `.scad` workbench 状态更新链路中加入内容相等判断：当下一次 defines 与当前 defines 按顺序完全一致时，保持旧数组引用。
2. 将源码解析、参数 debounce、恢复默认、预设应用等会更新 `appliedDefines` 的路径统一接入该判断。
3. 保持参数面板的用户交互语义不变：当用户实际改变参数值时，仍然必须触发一次新的预览请求。
4. 增加或扩展正向验证：初次打开 `.scad` 后只发送一次等价 `preview.request`；随后实际修改一个参数，必须出现新的 `preview.request`，且 request id 不同、decoded defines 内容不同。
5. 运行 Phase 1 的回归测试，确认同一个 `.scad` 初次打开只发送一次等价 `preview.request`。

### 验收标准

- Phase 1 测试通过。
- 打开 `.scad` 后，同一 source 与同一 defines 内容不会产生两个不同 request id 的 `preview.request`。
- 用户实际修改参数后仍会发送新的 `preview.request`；该请求 request id 与初次请求不同，decoded defines 内容也不同。
- 不影响 STL / 3MF 直接预览。

### 保护前序目标/边界

必须保护 Phase 1 建立的测试语义。不得通过放宽测试、缩短等待时间或过滤掉真实请求来让测试通过。

### Phase review

完成后调用独立 subagent review，重点检查：

- 是否只消除了等价更新，没有吞掉真实参数变化。
- 是否存在 stale closure 或状态更新顺序问题。
- 是否引入了与 React 渲染周期不匹配的副作用。

## Phase 3: 评估并补齐预览请求层幂等保护

### 输入

- Phase 2 后 `.scad` 初次打开重复请求已消除。
- `MeshViewer` 仍然直接根据 effect 依赖发送 `dispatchPreviewRequest`。

### 操作步骤

1. 评估是否仍存在其他上游等价状态引用变化会触发重复预览请求，例如配置路径等价变化、refresh signal 非目标文件变化、父组件重复传入等价 viewer options。
2. 只有在 Phase 2 修复后仍通过 decoded WebSocket frame 观察到等价重复请求，或能明确指出一个具体上游路径会稳定产生等价重复请求时，才允许在预览请求层加入 request key 级别保护。
3. 若加入 request key 保护，同一 source、defines、configured OpenSCAD path、refresh 语义对应的 in-flight 请求不得重复发送。
4. 若评估后没有必要增加幂等层，必须在 result 文档中说明不增加的理由，并保留 Phase 2 的更小修复；不得用防御层掩盖未修正的 `.scad` 状态等价更新。
5. 运行 `.scad` 回归测试、STL 预览相关 smoke、前端类型检查或最小相关单元测试。

### 验收标准

- 不再出现 `.scad` 初次打开双 `preview.request`。
- 若加入 request key 保护，旧请求完成后不会阻止后续真实刷新或参数变化。
- 若不加入 request key 保护，必须有清晰证据说明 Phase 2 已覆盖当前问题，且额外保护会增加不必要复杂度。

### 保护前序目标/边界

必须保护 Phase 2 的用户参数变化语义：真实参数变化和显式刷新仍应触发新预览。不得用全局节流、固定时间窗口或吞掉所有重复 source 的方式掩盖问题。

### Phase review

完成后调用独立 subagent review，重点检查：

- request key 或不增加 request key 的判断是否合理。
- 是否会误伤显式刷新、参数修改、切换文件后的预览。
- 是否保持 `MeshViewer` 与 `ScadWorkbench` 的职责边界。

## Phase 4: 全量回归与结果归档

### 输入

- Phase 1-3 的代码与测试变更。
- 每个 Phase 的 subagent review 结果。

### 操作步骤

1. 运行与本问题相关的前端验证命令，至少包含新增 Playwright 回归测试和前端 typecheck。
2. 如时间和环境允许，运行相关 smoke 集合，覆盖 STL 和 `.scad` 预览。
3. 检查 `git diff`，确认没有无关格式化、无关文案或后端 protocol 改动。
4. 更新 `plan-00-result.md`，记录每个 Phase 的完成情况、验证命令、review 结论和遗留风险。
5. 提交并推送变更，commit message 聚焦 `.scad` preview request dedup。

### 验收标准

- 新增回归测试通过。
- 前端类型检查通过。
- 相关 smoke 验证通过，或记录无法运行的具体原因。
- `plan-00-result.md` 完整记录每个 Phase 的执行结果。
- 工作树只包含本计划范围内的变更。

### 保护前序目标/边界

必须保护所有前序 Phase 已达成目标。若最终回归失败，不得通过删除测试、降低断言或移除用户可见能力通过验证；必须回到对应 Phase 修正。

### Phase review

完成后调用独立 subagent review，重点检查：

- 计划目标是否全部完成。
- 测试与实测证据是否一致。
- 是否存在未记录的风险或已知问题。

## 执行完成判定

整个计划只有在以下条件全部满足时才算收敛：

- `.scad` 初次打开不再发送两个等价 `preview.request`。
- 后端一请求一响应语义未被修改。
- 真实参数变化与显式刷新仍能触发新预览。
- 新增或调整的测试能够在自动化环境中复现并防止回归。
- 每个 Phase 均完成 subagent review，并已写入 `plan-00-result.md`。
