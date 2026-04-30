# CadQuery Preview Ref Layer Polish Plan

## 背景

本计划延续 `prompt-archives/2026043000-cadquery-web-e2e-gapfill/plan-00-result.md` 的执行结果，目标是把 CadQuery Web 预览、Ref 选择、Agent 输出体验和文件列表路由继续补齐到可端到端验收的状态。

当前必须保护的前序成果：

- Web Chat 能以“我想做一个放在车里的无线充电板上的给 AirPods 用的垫子”为起点触发 Agent 调用 CadQuery 建模。
- CadQuery runner 能写入 `.py` 并导出 `.step`，失败时不污染真实 workspace。
- 文件列表打开 CadQuery `.py` 能显示模型预览。
- Viewer 选择 face / edge / vertex 后能把 Ref 写入当前 selection，并进入 Chat context。
- LLM reasoning 在前端显示 `Thinking` 和最新思考内容。

## 强制约束识别

- 右侧 Inspector 必须出现 Ref 层级树状结构，并支持在该区域自由多选任意用户可见 Ref。
- `.py` 和 `.step` 都必须从文件列表进入对应模型预览。
- Agent 写模型时必须保持 `.py` 与 `.step` 导出同步。
- 模型源代码必须包含可读用途说明和面向人类交互的稳定命名。
- solid / wireframe / xray 三种渲染模式必须可切换且生效。
- Agent 更新模型后不得打开新的临时结果 tab，而是刷新当前 `.py` / `.step` 预览 tab。
- Agent done 和 tool 状态 UI 必须更轻量。
- 同一段 Assistant 输出只显示一次 `ASSISTANT` 来源标签。
- `cadquery-select-dock` 必须位于预览区域底部正中间、status bar 上方。
- 必须提供选择模式与预览模式。选择模式保留现有所有选择模式；预览模式保留 axis、底板等预览外观，只隐藏选择用的线框、anchor、hover/selected 高亮和选择 dock/status。

## Phase 0 — 基线与测试入口

输入：

- 当前工作树与上一轮执行结果。
- 当前 dev server 状态。
- 现有 Web 单元测试与浏览器测试入口。

操作步骤：

1. 确认工作树已有改动，区分上一轮改动与本轮新增改动。
2. 确认 dev server 是否可复用；不可复用时按上一轮环境变量重新启动。
3. 读取现有 CadQuery viewer、Inspector、tab 路由、Chat message 和 Agent 事件渲染测试。
4. 为本轮关键行为补充失败测试。

验收标准：

- 能明确当前实现缺口。
- 新增测试在实现前能体现目标行为缺失。
- 不破坏前序成果。

前序目标保护：

- 不回退 protocol version、reasoning event、CadQuery source preview 和 Ref context 已有行为。

## Phase 1 — Ref 层级树与选择/预览模式

输入：

- CadQuery scene payload、feature map、topology metadata。
- 当前 selection protocol 状态。

操作步骤：

1. 在右侧 Inspector 增加 Ref tree section。
2. 以 protocol 中已有 scene / topology / feature map 数据渲染用户可见 Ref 层级。
3. 支持在 Ref tree 中多选 Ref，并通过现有 selection update protocol 同步。
4. 将 Viewer 当前选择状态同步回 Ref tree。
5. 增加选择模式与预览模式切换；选择模式保留现有所有选择模式，预览模式隐藏选择 dock、选择高亮、选择线框和 vertex anchor，但保留 axis、底板等预览外观。
6. 调整 `cadquery-select-dock` 到预览区域底部正中间、status bar 上方。

验收标准：

- Inspector 中可以看到 Ref 层级树。
- Ref tree 多选能更新 Chat context 和 Viewer selection。
- Canvas 点击选择也能同步反映到 Ref tree。
- 选择模式显示选择辅助界面；预览模式隐藏选择相关覆盖层，同时保留 axis、底板等预览外观。
- select dock 位置满足用户要求。
- 前序 Chat context 和 Viewer 选择能力保持可用。

前序目标保护：

- 不改变 app server / protocol 的 Ref 来源边界。
- 不让前端通过文件名、路径或 instance path 推断 Ref。

## Phase 2 — 文件列表路由与模型更新预览

输入：

- 文件列表打开 `.py` / `.step` 的交互。
- CadQuery Agent 执行完成事件。
- 当前 tab 状态。

操作步骤：

1. 让 `.step` 文件列表入口进入对应模型预览，而不是文本或未知文件处理。
2. 确认 `.py` / `.step` 打开的 tab 内刷新预览，不创建临时结果 tab。
3. Agent 执行完成后刷新当前模型预览 tab，并保持 `.py` 与 `.step` 导出同步。
4. 给 Agent / tool guidance 增加模型说明、稳定命名和导出同步约束。

验收标准：

- 文件列表打开 `.py` 和 `.step` 都显示 CadQuery 模型预览。
- Agent 后续修改后当前 `.py` / `.step` tab 直接刷新模型。
- Workspace 中 `.py` 与 `.step` 时间和内容符合一次成功 CadQuery 执行结果。
- 新生成模型源码包含文字说明和人类可读 Ref / feature 命名。

前序目标保护：

- 不允许普通文件写入工具绕过 CadQuery tool 改写 `.py` 模型。
- 不破坏 staging 成功后再回写的 CadQuery 执行语义。

## Phase 3 — 渲染模式和 Agent 输出体验

输入：

- 当前 Viewer toolbar。
- Chat stream / Agent event 渲染。

操作步骤：

1. 验证 solid / wireframe / xray 模式切换，修复不生效或状态不同步问题。
2. 将 done 事件渲染为轻量 logo 标识。
3. 将 tool start / running / result 等事件压缩为单行状态。
4. Tool 详细内容通过 modal 展开查看。
5. 同一 Assistant stream 的连续消息只在第一条显示 `ASSISTANT` 来源。

验收标准：

- 三种渲染模式在 DOM 状态与真实截图中均可确认切换。
- done 不再显示大 card。
- tool 状态默认只占一行，点击后可打开详情 modal。
- 连续 Assistant 消息只有第一条显示来源。
- 用户消息来源显示不受影响。

前序目标保护：

- 不丢失 Agent event 详细信息，只改变默认呈现方式。
- 不隐藏 LLM reasoning 的 `Thinking` 最新内容。

## Phase 4 — Playwright 端到端回归与归档

输入：

- 本轮所有修改。
- Web dev server。
- AirPods 充电垫模型 workspace。

操作步骤：

1. 运行相关 TypeScript 单元测试与类型检查。
2. 运行相关 Rust / protocol 验证命令。
3. 使用 Playwright 在真实 Web 页面中创建或复用 Chat，执行 CadQuery 建模与后续修改。
4. 从文件列表分别打开 `.py` 和 `.step`，验证预览、Ref tree、多选、模式切换、渲染模式、Agent 输出 UI。
5. 清理本轮产生的临时缓存。
6. 更新执行结果归档。

验收标准：

- 自动化测试通过或明确记录非阻塞既有 warning。
- Playwright 真实页面验收覆盖用户列出的 10 条要求。
- `plan-00-result.md` 记录完成情况、验证证据和遗留问题。

前序目标保护：

- 端到端回归必须覆盖上一轮已完成的 Chat → CadQuery → 文件预览 → Ref 后续修改链路。
