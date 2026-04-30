# CadQuery Web 端到端补缺验收计划

## Context

budn' 当前 CadQuery MVP 主链路已经具备 runner、protocol、Agent tool call、Web Chat、Viewer selection 和文件列表基础能力。本轮不重新设计架构，只以真实 Web 使用路径为验收标准，补齐阻止用户从 Chat 生成 CadQuery 模型、打开模型、预览模型和选择 Ref 的问题。

## 已确认强制约束

- 必须自行启动 Web dev server，并自行完成网页端新建 Chat、Agent 对话、CadQuery 建模、文件打开、预览和 Ref 选择验证。
- 不等待用户确认，不把过程中产生的新选择题交回给用户。
- CadQuery 模型写入与执行必须经过 app server / protocol / CadQuery tool / staging 边界。
- Web 端不得绕过 app server protocol 直接读取 runner 输出或本地文件。
- 发现并确认的非本轮可解决问题必须写入 `docs/known_issues.md`。
- 本轮不使用 subagent review，原因见 `plan-prompt.md` 的工具约束记录。

## Phase 0 — 执行前基线检查

### 输入

- 本计划与 `plan-prompt.md`。
- 既有 CadQuery MVP 计划和结果记录。
- 当前工作树 diff、已知问题记录和 Web 启动脚本。

### 操作步骤

1. 检查本计划、原计划和结果记录中是否存在会阻塞执行的占位内容、用户选择项、未完成方案决策或缺少验收口径。
2. 读取当前工作树 diff，确认是否存在上次中断留下的相关修改。
3. 确认 Web dev server、WebSocket host、LLM 配置、CadQuery runner 和浏览器验证链路的当前状态。

### 前序目标保护

这是本轮第一个 Phase，无前序 Phase 目标需要保护。必须保护既有 CadQuery staging 安全边界、单 running agent session、Chat JSONL 存储和 Web / app server protocol 边界。

### 验收标准

- 没有未补全的计划阻塞项。
- 当前工作树已有改动已被纳入本轮判断，未被误删或回退。
- 明确本轮端到端验证需要启动哪些本地进程与使用哪些环境变量。

## Phase 1 — Web Chat 到 CadQuery 建模链路补缺

### 输入

- Phase 0 的基线判断。
- 用户指定的起始消息：“我想做一个放在车里的无线充电板上的给AirPods用的垫子”。
- 当前 Agent tool call、LLM provider、CadQuery tool runtime 和 Chat history 实现。

### 操作步骤

1. 启动 Web 端所需本地服务。
2. 在网页中新建 Chat，发送用户指定起始消息。
3. 观察 Agent 事件、Chat history、tool call 记录、runner 行为和前端状态。
4. 对复现到的中断、错误、缺失记录或体验问题按 TDD 和根因分析修复。
5. 重复 Web 操作，直到 Chat 能成功触发 CadQuery 建模并产生可追踪结果。

### 前序目标保护

实现当前 Phase 时必须保护 Phase 0 确认的边界：不得通过 Web 直接写本地文件绕过 protocol，不得用普通文件工具改 CadQuery `.py` 模型，不得扩大 Agent 写入权限。

### 验收标准

- 新建 Chat 可以从指定中文需求进入 Agent 建模流程。
- Agent tool call 与 tool result 在实时消息和历史消息中都可见或可恢复。
- CadQuery runner 不因长 stdout、超时过短、LLM 错误未持久化等问题导致用户看不到可操作反馈。
- 若 LLM 或本机配置不可用，错误必须在 Chat 中以可理解消息持久化；若配置可用，必须完成 CadQuery 建模。

## Phase 2 — 文件列表打开与预览补缺

### 输入

- Phase 1 生成的 CadQuery 模型文件和 result。
- 当前 Web 文件列表、打开文件、CadQuery preview、Viewer tab 和 result cache 实现。

### 操作步骤

1. 在 Web 文件列表中定位 Agent 生成的模型文件。
2. 打开该文件并触发预览。
3. 观察文件列表刷新、tab 状态、preview 请求、mesh result 和 Viewer 渲染。
4. 修复阻止文件可见、打开、预览或反馈清晰的问题。

### 前序目标保护

实现当前 Phase 时必须保护 Phase 1 已达成的 Chat → Agent → CadQuery 建模链路，不能为了打开文件而绕过 Agent 结果、protocol 或 staging。

### 验收标准

- Agent 生成的模型文件能出现在 Web 文件列表中。
- 用户可以从文件列表打开该模型文件。
- Web 端能通过 app server protocol 预览该 CadQuery 模型。
- 预览失败时，UI 能显示可行动错误，不出现无反馈或消息丢失。

## Phase 3 — Viewer Ref 选择与后续修改入口补缺

### 输入

- Phase 2 中可预览的 CadQuery mesh。
- 当前 CadQuery Viewer selection、SelectionRef、Chat selection context 和 Agent 后续消息实现。

### 操作步骤

1. 在预览区域交互选择 face、edge、vertex、part 或 assembly Ref。
2. 确认选中 Ref 在 Viewer 状态和 Chat 上下文中可见。
3. 继续向 Chat 发送后续修改请求，观察 Agent 是否能读取当前 selection context 并使用 CadQuery 工具。
4. 修复 selection 丢失、Ref 显示不清晰、后续修改上下文缺失或前端交互不佳的问题。

### 前序目标保护

实现当前 Phase 时必须保护 Phase 1 的 Agent tool call 记录和 Phase 2 的文件打开 / 预览路径；不能让 Ref 选择逻辑依赖文件名、mesh 名或临时 DOM 状态推断业务 Ref。

### 验收标准

- Viewer 中可以交互选择 CadQuery Ref。
- 当前 selection 通过 protocol 更新到共享状态。
- Chat 后续 Agent 请求能带上当前 selection context。
- 后续修改请求可以继续走 CadQuery 工具链；若模型修改失败，Chat 中必须保存可理解错误和工具结果。

## Phase 4 — 回归验证与结果归档

### 输入

- Phase 1 至 Phase 3 的代码与行为变更。
- 单元测试、Rust 测试、Web typecheck、Web build、端到端浏览器验证。

### 操作步骤

1. 运行与本轮变更直接相关的最小测试集。
2. 运行 Web typecheck 和必要构建验证。
3. 运行端到端浏览器验证，覆盖新建 Chat、CadQuery 建模、文件打开、预览、Ref 选择和后续修改入口。
4. 更新 `plan-00-result.md`，记录每个 Phase 的执行结果、变更摘要、验证命令和遗留问题。
5. 对必须保留的已知问题更新 `docs/known_issues.md`。

### 前序目标保护

实现当前 Phase 时必须保护所有前序 Phase 的端到端目标，不能只让测试通过而破坏实际 Web 操作路径。

### 验收标准

- 相关测试与构建命令完成并记录结果。
- 端到端 Web 验证完成并记录关键证据。
- `plan-00-result.md` 记录完整。
- 若仍有非阻塞风险，已写入 `docs/known_issues.md`。
