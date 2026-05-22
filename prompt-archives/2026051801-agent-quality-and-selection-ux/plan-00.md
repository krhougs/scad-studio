---
plan_id: "2026051801-agent-quality-and-selection-ux"
scope: "Phase 0（Agent 生成质量基础）+ Phase 1 选择→Ref→Agent 数据流与前端体验"
phases: 4
depends_on:
  - "docs/2026051800-competitive-research/roadmap.md"
  - "docs/cadquery-mvp/agent-system-prompt.md"
---

# Agent 生成质量基础 + 选择→Ref→Agent 前端体验

## 目标

1. 建立 Agent skill 机制，支撑 system prompt 之外的能力模块化扩展
2. 补齐 Agent 生成质量的三个核心缺失：失败分类修复循环、工程默认值、结构化 brief
3. 将 Chat pill、Viewer 选择、Ref Tree 选择统一为单一选择状态，修复前端体验关键断点
4. 建立分层验证手段，确认完整循环可工作

## 2026-05-18 Review 后修订原则

- Phase 2 不再描述为纯前端工作。统一选择状态需要同时收敛前端状态投影、Agent turn context 构建和 host 侧选择快照消费语义。
- 选择状态的唯一权威来源是 app server 当前 `SelectionUpdateRequest` snapshot；Chat pill 不再维护独立过滤状态。任何删除 pill、Ref Tree 取消勾选、Viewer 取消选择、清除全部选择都必须通过 `dispatchSelectionUpdate` 写回同一份选择快照。
- 失败修复 skill 必须在同一个 Agent turn 的首次 CadQuery 失败后仍能指导自动修复；不能只依赖”上一轮失败后下一轮注入”的机制。注入条件为 Agent mode + CadQuery 工具已注册即注入精简修复规则，确保首次失败时规则已在 preamble 中。
- 用户界面只显示友好错误摘要、错误类别和可恢复动作；traceback 等内部诊断信息只能进入可展开开发诊断区域或进程日志，不直接堆叠在业务区域。
- Agent 功能性验收必须记录模型可见上下文、工具调用轨迹、工具结果和最终回复，并由独立第三方 LLM 按 rubric 评估是否达成用户意图。
- Brief 是生成或修改前的可读摘要，不是阻塞确认步骤；本轮不新增确认协议、确认 UI 或等待用户修正假设的暂停流程。
- 本计划中的 Agent skill 指 budn' 产品 Agent 的动态 turn-context 指令模块，由 app server 注入产品 Agent prompt；不得写入 `AGENTS.md`、Codex skill 目录或工程协作规则来代替产品行为修复。
- 如果统一选择状态需要调整 WebSocket / app-server protocol 字段，可以修改 protocol；修改时必须同步更新 Phase 2-A protocol 变更范围表中列出的所有文件。
- Phase 0 → Phase 1 → Phase 2 → Phase 3 全部串行执行，不再尝试并行（Phase 0-1 和 Phase 2 在 `agent.rs` 的 `build_turn_context()` 上存在文件级冲突）。
- Skill 注入条件仅基于简单结构化信号（mode、工具注册状态、上一轮错误状态），语义判断（是否输出 brief、是否执行修复）交给 LLM 在 skill 文本指导下自行决定。
- Phase 1-D 基准框架使用 Rust 集成测试执行 agent turn，bun 负责编排和报告；不从零实现 WebSocket client。LLM rubric 评估延迟到 Phase 3。

## 当前代码关键入口

| 模块 | 位置 | 说明 |
|---|---|---|
| System prompt | `docs/cadquery-mvp/agent-system-prompt.md` | 291 行单文件，`include_str!` 编译时嵌入 |
| Prompt 加载 | `crates/app-server-core/src/agent.rs:78-79` | `cadquery_agent_system_prompt()` |
| Turn 上下文构建 | `crates/app-server-core/src/agent.rs:791-823` | `build_turn_context()` 构建每轮注入的上下文 |
| Turn preamble 拼接 | `crates/app-server-core/src/agent.rs:763-771` | `build_rig_prompt_and_history()` 把 context 前置到 user prompt |
| Tool 注册 | `crates/app-server-core/src/agent/tools/registry.rs:70-209` | 19 个工具的 name/description/schema |
| Context Pill | `packages/studio-web/src/workbench/chat-composer.tsx:37-58` | 有组件、无 CSS |
| Chat 发送 | `packages/studio-web/src/workbench/chat-actions.ts:137-211` | 当前仍传 `context_refs`，但后续应改为消费同一份选择快照的投影，避免与 `selections` 分叉 |
| Agent 事件展示 | `packages/studio-web/src/workbench/chat-messages.tsx:245-310` | `AgentEventRow` 统一展示 |
| 选择 Dock | `packages/studio-web/src/viewers/cadquery-viewer.tsx:393-416` | 8 按钮平铺 |
| Ref Tree | `packages/studio-web/src/workbench/cadquery-ref-tree.tsx` | Inspector 内层级树，无折叠/过滤 |
| 选择 CSS | `packages/studio-web/src/styles/workbench-zones.css:1305-1448` | dock/status/ref-tree/ambiguous |

---

## Phase 0: Agent Skill 基础架构

### 目标

建立 Agent skill 注入机制，使 system prompt 保持单文件不膨胀，新能力以 skill 形式注册并在满足条件时动态注入到 turn context 中。同时审视现有 tool description 的信息密度。

### 设计方向

**Skill 机制**：

当前 `build_turn_context()` 已经按条件注入选择快照、执行范围、搜索能力等上下文。Skill 是这个模式的泛化：每个 skill 是一段聚焦的指令文本，带触发条件，在相关 turn 被注入到 preamble 中，不相关时不占 token。

- 本计划中的 Agent skill 是 budn' 产品 Agent runtime 的内部指令模块，由 app server 根据 turn context 动态注入到产品 Agent 可见上下文；它不是 Codex / `.agents/skills` / `AGENTS.md` 工程协作 skill。
- 修改产品 Agent 行为时，必须改产品 prompt、runtime context、tool schema 或执行链路；禁止把产品 Agent 行为规则写进 `AGENTS.md` 或 Codex skill 当作产品修复。
- Skill 文本必须放在产品 Agent 可审计的代码或产品 prompt 附属路径中，例如 `crates/app-server-core/src/agent/` 下的专用模块 / 常量，或 `docs/cadquery-mvp/` 下明确标注为产品 Agent prompt 附属资料的文件。不得放入 Codex 工程 skill 目录。
- System prompt（`agent-system-prompt.md`）保持单文件，承载角色、核心原则、模式、文件契约、Ref 规则、权限表、响应规则——这些是**每个 turn 都需要**的基础契约
- Skill 承载**特定场景才需要**的扩展指令（失败修复处方、工程默认值、brief 模板等）
- Skill 的注入条件仅基于简单结构化信号（当前 mode、当前 turn 注册的工具能力、上一轮 CadQuery 工具是否返回错误），不做意图分类或自然语言分析。例如：Agent mode 且 CadQuery 执行/试运行工具已注册 → 注入失败修复 + 工程默认值 + brief skill；上一轮 CadQuery 返回错误 → 额外注入完整失败分类处方。
- Skill 文本内部包含适用条件说明（如"仅在新建模型时输出 brief"、"简单修改无需 brief"），语义判断由 LLM 自行完成，注入层不负责意图识别。

**Tool description 增强**：

当前 tool description 在 7-20 词之间。CadQuery 相关工具（`cadquery_dry_run`、`cadquery_execute`、`cadquery_analyze_source`、`cadquery_check_source`、`cadquery_get_result`、`cadquery_resolve_selection`）的 description 需要补充典型使用场景和与其他工具的配合关系，每个控制在 2-4 句。增强标准：一个不熟悉工具链的 Agent 读完 description 后能判断什么场景该用这个工具、用完之后下一步该做什么。

### 验收标准

1. Skill 注入机制可工作：至少一个 skill（如占位 skill）能根据条件出现在 turn preamble 中
2. Skill 不在时 preamble 与当前行为一致（回归验证）
3. System prompt 文件不变（行数、内容不改动）
4. 6 个 CadQuery 工具的 description 得到增强，满足上述增强标准
5. Skill 文本位置符合产品 Agent 边界，未写入 `AGENTS.md`、Codex skill 目录或工程协作规则
6. 现有编译 + 已有测试通过

### 前序目标保护

无前序 Phase。

---

## Phase 1: Agent 生成质量 Skills + 基准框架

### 目标

在 Phase 0 的 skill 架构基础上，实现三个 skill 并建立可重复执行的基准评估框架。

### Phase 1-A: 失败分类与修复循环 Skill

**背景**：当前 Agent 遇到 CadQuery 执行失败后直接报错，没有结构化的重试策略。

**Skill 内容**：

- 失败分类体系：语法错误、几何无效（空形状/自交/开壳）、倒角/圆角溢出、比例失调（尺寸偏离预期 >2x）、布尔运算失败（交集为空/相切退化）、装配定位失败
- 每类配最小修复处方（1-3 步）
- 修复循环规则：最多 2 次自动重试，每次重试前先 dry_run 验证
- 与 `cadquery_dry_run` / `cadquery_execute` 返回的 `error_type` 和 `diagnostics.traceback` 对齐

**注入条件**：

- Agent mode 且 `cadquery_dry_run` / `cadquery_execute` 在当前 turn 已注册 → 注入精简修复循环规则（确保本 turn 内首次失败后即可按规则修复）
- 上一轮 `cadquery_dry_run` / `cadquery_execute` 返回错误 → 额外注入完整失败分类与修复处方
- Plan mode 不注入（Plan mode 不允许运行 CadQuery runner）
- Skill 文本内部说明修复循环的适用条件和上限，LLM 自行判断是否执行修复

**验收标准**：

1. Agent 在同一个 Agent turn 内遇到倒角失败时，按处方缩小半径重试，而非直接报错停止（通过真实 LLM 功能测试验证）
2. 修复循环不超过 2 次 dry_run（通过工具调用轨迹验证）
3. Plan mode turn 的 preamble 不包含失败修复 skill 内容

### Phase 1-B: 工程默认值 Skill

**Skill 内容**：

- 3D 打印通用默认值：壁厚 2-3mm、倒角 0.5-1.5mm、最小特征 0.8mm
- 标准通孔：M2=2.4mm、M3=3.4mm、M4=4.5mm、M5=5.5mm、M6=6.6mm
- 坐标约定：Z-up，原点在底部几何中心
- 公差默认值：打印 ±0.2mm、CNC ±0.05mm
- 这些默认值是"假设"，Agent 必须在输出中声明使用了哪些默认值
- **不包含**特定硬件尺寸（Arduino/RPi 等）——Agent 需要时应使用 `web_search` 查询

**注入条件**：Agent mode 且 CadQuery 工具在当前 turn 已注册。Skill 文本内部说明何时适用（如"生成新模型或修改尺寸时使用这些默认值"），LLM 自行判断。

**验收标准**：

1. Agent 被要求"做个外壳"时不再因缺少壁厚信息而反复追问（使用默认值并声明）
2. 默认值出现在 Agent 输出中（可追溯）

### Phase 1-C: 结构化 Brief Skill

**Skill 内容**：

- Brief 模板：目标尺寸、关键特征清单、假设列表（含引用的默认值）、验证标准、REFS.features 规划
- Agent 在特定条件下生成代码前先输出 brief，帮助用户理解即将采用的目标、假设和验证标准
- Brief 是 Agent 消息的文本部分，不是工具调用
- Brief 不是阻塞确认步骤。Agent 输出 brief 后可以继续按当前 turn 执行，不新增等待用户确认、暂停执行或回滚假设的协议 / UI。

**注入条件**：Agent mode 且 CadQuery 工具在当前 turn 已注册（与工程默认值 skill 同条件注入）。

Skill 文本内部说明适用场景和不适用场景，LLM 自行判断是否输出 brief：
- 适用：新建模型（workspace 无结果或用户明确要求）、多参数未指定、大范围修改
- 不适用：简单修改（"把高度改为 50mm"）、查询描述操作、Plan 模式执行步骤

**验收标准**：

1. 新建模型的 turn 首条消息包含可读 brief，且该 brief 是执行前摘要而非等待确认状态
2. 简单修改操作不输出 brief，直接执行
3. Brief 中的假设列表引用了 engineering-defaults skill 中的具体值

### Phase 1-D: 基准评估框架

**目标**：建立可重复执行的基准评估框架。

**设计**：

- `benchmarks/` 目录下按场景组织
- 每个基准包含：prompt 文件、预期通过条件（JSON）、评分脚本（bun/TypeScript）
- 评分维度：执行成功率（`cadquery_execute` 无错误）、brief 存在性（新建场景）、REFS.features 完整性、导出文件生成
- **核心基准执行使用 Rust 集成测试**：在 `crates/app-server-core/tests/` 下编写基准测试，直接调用 `run_rig_agent_turn()` 或等价内部 API，复用已有完整 protocol 类型和 agent turn 调用能力，收集工具调用轨迹和最终回复
- **bun 脚本负责编排和报告**：启动 app-server、触发 Rust 测试二进制、收集测试输出、生成通过率表格
- 如需 WebSocket 层验证（如 benchmark smoke），复用 `packages/app-server-protocol` 的 TS 类型和现有 `tests/run_websocket_host.test.ts` 的连接基础设施，不从零实现 WS frame 解析
- LLM rubric 评估延迟到 Phase 3 实现；Phase 1-D 只记录工具调用轨迹、工具结果和最终回复，为 Phase 3 提供评估输入
- 先实现 5 个基准（简单几何体到中等复杂度零件）

**验收标准**：

1. `bun run bench` 一键跑完 5 个基准并输出通过率表格
2. 不依赖任何 Python 脚本（CadQuery 执行由 app-server 内部的 runner 完成）
3. 每个基准有明确的 pass/fail 判定
4. Rust 集成测试能完成 agent turn 生命周期并收集工具调用轨迹
5. 每次基准记录用户输入、工具调用轨迹、工具结果和最终回复
6. 若 Phase 2 修改 app-server protocol，Rust 集成测试中使用的 protocol 类型自然跟随 crate 更新；如有 WS 层 smoke 测试需同步验证

### 前序目标保护

- Phase 0 的 skill 注入机制不被破坏：新增 skill 内容文件，不改架构代码
- 现有 tool schema 和 registry 代码不改动（description 增强在 Phase 0 已完成）
- 现有前端代码不改动

---

## Phase 2: 选择状态统一 + 前端体验修复

### 目标

将 Chat pill、Viewer 选择高亮、Ref Tree 选择统一为单一选择状态源，修复前端体验的关键断点。该 Phase 会同时触碰 `packages/studio-web/` 和 Agent turn context 相关后端代码；不得再按“纯前端任务”并行执行与同一区域冲突的后端改动。

### Phase 2-A: 统一选择状态

**问题**：当前 `context_refs`（pill 筛选后的 ref_text 列表）和 `selections`（完整快照）是两套数据，pill 删除不影响 Viewer 高亮，Ref Tree 的勾选状态与 pill 也不同步。这导致用户在三个地方看到的选择状态不一致。

**目标**：

- 建立单一选择状态源，app server 当前 `SelectionUpdateRequest` snapshot 是权威状态，三个视图（Chat pill bar、Viewer 高亮、Ref Tree 勾选）都是这个状态的投影
- 删除 pill = 调用 `dispatchSelectionUpdate` 从选择快照中移除该 ref = Viewer 取消高亮 = Ref Tree 取消勾选
- 在 Ref Tree 中取消勾选 = pill 消失 = Viewer 取消高亮
- 在 Viewer 中取消选择 = pill 消失 = Ref Tree 取消勾选
- 清除全部选择 = 写回空 `selections` 和 `active_index: null`
- 发送消息时，Agent 收到的是统一后的 `Current selection`，不再区分用户可见的 `context_refs` 和后端 `selections`
- `build_turn_context()` 中 `User-attached context refs` 和 `Current Web preview selection` 合并为一个 section
- 发送 Agent turn 前必须确保最近一次选择更新已被 app server 接受，避免删除 pill 后立刻发送消息时 Agent 仍读取旧选择快照
- 如需删除或替换 `context_refs` 字段、让 `AgentStartTurnRequest` / `ChatCreateInitialTurn` 直接携带选择快照，或调整 `get_selection` 返回结构，可以修改 app-server protocol；变更必须保持旧 workspace 数据可读，并同步所有协议生成物和 roundtrip 测试

**Protocol 变更范围（如需修改）**：

以下文件在统一选择状态时预期需要变更，执行时逐个确认：

| 层 | 文件 / 目录 | 变更内容 |
|---|---|---|
| Rust protocol | `crates/app-server-protocol/src/protocol.rs` | 从 `AgentStartTurnRequest` 删除或废弃 `context_refs` 字段；Agent turn 改为从最近一次 `SelectionUpdateRequest` snapshot 派生选择 |
| Rust agent | `crates/app-server-core/src/agent.rs` | `AgentTurnInput` 移除 `context_refs`；`build_turn_context()` 合并 "User-attached context refs" 和 "Current Web preview selection" 为单一 "Current selection" section |
| Host | `crates/app-server-host/` | 构建 `AgentTurnInput` 时不再从 WS request 提取 `context_refs`，改为读取当前 selection snapshot |
| TS protocol | `packages/app-server-protocol/` | 同步 TS 类型定义 |
| WASM bridge | `packages/studio-web-wasm/generated/` | 重新生成 WASM 绑定 |
| Frontend dispatch | `packages/studio-web/src/workbench/chat-actions.ts` | 不再单独构造 `context_refs`；`sendChatMessageInner()` 和 `createChatSession()` 中移除 pill → context_refs 映射 |
| Frontend composer | `packages/studio-web/src/workbench/chat-composer.tsx` | Pill 改为从统一选择状态投影，删除 pill 调用 `dispatchSelectionUpdate` |
| Studio common | `packages/studio-common/` | 如消费 protocol 类型，同步更新 |
| Roundtrip 测试 | `crates/app-server-protocol/tests/` | 更新序列化 roundtrip 测试 |
| 前端单元测试 | `packages/studio-web/tests/unit/` | 更新 `chat-actions.test.ts`、`cadquery-selection.test.ts` |
| Rust 集成测试 | `crates/app-server-core/tests/` | Phase 1-D 基准测试使用 crate 内 protocol 类型，自然跟随更新 |

**Context Pill 样式**（同步完成，pill 可见是验证选择同步的前提）：

- `.context-pill-bar` 和 `.context-pill` 缺少 CSS 定义，需补齐
- Pill 展示当前选择的 `preferredRefText()`，带类型图标和删除按钮
- Pill bar 在 Composer textarea 上方，水平排列，溢出省略
- 视觉风格与 Composer 其他元素（operation-select、send 按钮）一致

**验收标准**：

1. 在 Viewer 中选择 → pill 出现（可见、可读）+ Ref Tree 勾选；三者同步
2. 删除 pill → Viewer 取消高亮 + Ref Tree 取消勾选；三者同步
3. 在 Ref Tree 中取消勾选 → pill 消失 + Viewer 取消高亮；三者同步
4. 最多显示 3 个 pill（`MAX_CONTEXT_PILLS` 限制）
5. Agent turn preamble 中只有一个选择 section，内容与用户在三个视图中看到的一致
6. 发送消息后，Agent 收到的选择列表与 pill bar 中显示的一致
7. 删除 pill 后立即发送消息，Agent 不会收到已删除的 ref
8. 若 protocol 发生变更，上表所列文件已同步更新并验证通过

### Phase 2-B: Agent CadQuery 工具调用展示（原 2-C）

**问题**：所有 agent 事件统一用 `AgentEventRow` 单行按钮展示，CadQuery 执行没有专门的进度和结果卡片。

**目标**：

- `agent.tool_start` 且 `tool_name` 为 CadQuery 工具时，展示专门的工具执行卡片（而非通用单行按钮）
- `cadquery_dry_run` / `cadquery_execute` 的卡片应显示：目标文件路径、执行状态（运行中/成功/失败）
- `cadquery_execute` 成功时的卡片显示：导出格式、导出路径
- `agent.mesh_ready` 事件在 chat 中显示为"模型已更新"提示，而非通用事件行
- 失败时显示 `friendlyErrorMessage`、错误类别和可恢复动作；traceback 摘要只允许放入可展开开发诊断区或进程日志，不直接展示在默认业务区域

**验收标准**：

1. Agent 执行 `cadquery_execute` 期间，chat 中显示带状态的执行卡片，而非空白等待
2. 执行成功后卡片显示导出路径，用户能看到模型已生成
3. 执行失败后卡片默认显示友好错误信息、错误类别和可恢复动作；开发诊断展开区最多显示 traceback 摘要前 3 行

### Phase 2-C: 选择 Dock 改进（原 2-D）

**问题**：8 个模式按钮平铺无分组，没有选择状态摘要，没有清除操作。

**目标**：

- 按钮分两组：object 级（component/part/assembly/instance）和 geometry 级（feature/face/edge/vertex），组间有视觉分隔
- Dock 显示当前选择数量（如 "2 selected"）
- 增加清除全部选择的按钮

**验收标准**：

1. 两组按钮之间有可见分隔
2. 有选择时显示数量，无选择时不显示
3. 清除按钮点击后所有选择被清除（pill + Viewer + Ref Tree 同步清除）

### 前序目标保护

- Phase 0 的 skill 架构不被触碰
- Phase 1 的 skill 内容文件不被修改
- 现有 tool registry/schema 代码不改动
- 现有 Viewer Three.js 拾取逻辑不改动（拾取结果仍产出 `SelectionRef`，变化在于状态管理层）
- protocol 可按统一选择状态需要调整；变更范围见 Phase 2-A protocol 变更范围表，所有列出文件必须同步更新

---

## Phase 3: 分层验证

### 目标

通过分层测试验证完整的选择→Ref→Agent 循环可工作。

### 第一层：组件级测试

验证前端组件在给定 props/state 下正确渲染，不需要 app-server。

**验证项**：

- Context Pill：给定选择状态渲染 pill，删除 pill 触发状态更新回调
- CadQuery 工具执行卡片：给定 tool_start/tool_result 事件正确渲染进度和结果
- 选择 Dock：按钮分组正确，选择数量显示正确，清除按钮触发回调
- 选择状态同步：修改选择状态后三个视图投影一致

### 第二层：后端与协议集成测试（需要 app-server）

验证前后端数据流打通，Agent 能收到正确的选择上下文。

**验证项**：

- Rust 侧直接验证 `build_turn_context()`：统一后的 `Current selection` 内容正确，且不再输出分叉的 `User-attached context refs`
- Rust 侧直接验证 skill 注入条件：Agent mode + CadQuery 工具已注册的 turn 包含失败修复 + 默认值 + brief skill；上一轮 CadQuery 失败后的 turn 额外包含完整失败分类处方；Plan mode turn 不包含 CadQuery skill
- WebSocket 集成测试只验证真实协议流程：handshake、chat / agent 初始化、选择更新、Agent turn 启动、agent snapshot / subscribe 和事件流收集，不直接断言不可观察的完整 preamble 字符串

### 第三层：端到端场景测试

**场景 1：选择→Chat→Agent 循环**

1. 启动 studio-web dev server + app-server
2. 在 Viewer 中切换到 face 模式，选择一个面
3. 验证：Chat Composer 中出现 Context Pill，Ref Tree 中对应项勾选
4. 发送消息"描述这个面的特征"
5. 验证：Agent 响应中引用了正确的面/特征

**场景 2：Agent CadQuery 执行循环**

1. 在空 workspace 中发送新建模型请求
2. 验证：Agent 输出 brief → 执行 `cadquery_execute` → chat 中显示执行卡片 → mesh_ready → Viewer 显示模型
3. 在 Viewer 中选择模型上的面
4. 发送基于选择的修改请求
5. 验证：Agent 读取选择上下文 → 修改模型 → 新模型中修改可见

**场景 3：Plan 模式循环**

1. 发送 `/plan` 开头的修改请求
2. 验证：Agent 创建 plan package → chat 中显示 PlanPackageCard
3. 点击 "Run Plan"
4. 验证：Agent 执行 plan → 模型更新

### 测试方式

- 第一层：组件级测试框架（与项目现有测试方式一致）
- 第二层：Rust 测试验证 turn context / skill 注入条件；复用 Phase 1-D 的 Rust 集成测试基础设施验证协议流程和事件流
- 第三层：Playwright 自动化；Viewer 中的 WebGL Canvas 拾取如果 Playwright 无法模拟，通过调用 WASM client 的 `dispatchSelectionUpdate` 注入选择状态
- Agent 功能性验收：测试程序可以使用本机配置启动真实开发环境模型，但不得把 `agents.toml` 内容、密钥或 provider 原始配置正文交给 LLM、写入日志、归档或输出；只允许记录 provider/model id、能力标记、工具调用轨迹、工具结果和最终回复。独立第三方 LLM 按 rubric 判断是否达成意图、是否调用合适工具、是否如实暴露限制、是否出现幻觉或无意义工作

### 验收标准

1. 第一层组件级测试全部通过
2. 第二层集成测试覆盖 skill 注入条件、选择上下文构建和可观察协议流
3. 第三层 3 个场景全部通过（Playwright 脚本或录屏）
4. Agent 功能性场景全部有第三方 LLM rubric 评估记录，且归档内容不包含 `agents.toml` 正文、密钥或 provider 原始配置正文

### 前序目标保护

- Phase 0-2 的所有修改已提交
- 测试文件放在测试目录，不修改产品代码
- 不为测试目的修改 system prompt 或 tool schema

---

## 依赖关系

```
Phase 0 ──→ Phase 1 ──→ Phase 2 ──→ Phase 3
(Skill 架构)  (Skill 实现    (选择统一     (分层验证)
               + 基准框架)    + 前端体验)
```

全部串行执行。原因：Phase 0-1 和 Phase 2 都会修改 `crates/app-server-core/src/agent.rs`（`build_turn_context()` 的 skill 注入和选择 section 合并），文件级冲突不可避免。

- Phase 0 → Phase 1：Phase 1 的 skill 依赖 Phase 0 的注入机制
- Phase 1 → Phase 2：Phase 2-A 合并 `build_turn_context()` 的选择 section 时，需要 Phase 0-1 的 skill 注入代码已稳定
- Phase 2 → Phase 3：Phase 3 验证 Phase 0-2 的完整交付
- Phase 2-A（统一选择状态 + Pill 样式）是 Phase 2 内部的前置依赖，必须先完成再做 2-B/2-C

## 关键风险

1. **Skill 注入粒度与 token 成本**：当前方案在 CadQuery 工具注册时统一注入所有 skill（失败修复 + 默认值 + brief），语义判断交给 LLM。如果 skill 文本过长，即使在简单查询 turn 中也会占用 token。需要控制每个 skill 的文本长度
2. **统一选择状态的前后端一致性**：三个视图同步要求 app server selection snapshot 作为唯一权威状态。如果 UI 只做本地过滤，Agent 仍会读取旧选择快照
3. **Playwright 对 WebGL 交互的覆盖能力**：如果无法在 Canvas 上模拟点击，第三层测试的 Viewer 交互部分需要退化到 WASM client API 层
4. **Protocol 变更的跨包同步**：Phase 2-A 统一选择状态预计涉及 11 个文件/目录的联动变更（见 Phase 2-A protocol 变更范围表）；遗漏任何一个都会导致编译失败或运行时不一致
5. **LLM 功能性验收成本**：真实模型和第三方 LLM 评估会增加运行时间和环境依赖。基准必须清晰区分确定性结构检查、真实模型功能测试和第三方评估，避免把模型波动误判为协议或 UI 回归

## 不做的事

- 不做渐进式参考加载（路线图 Phase 4）
- 不做标准件目录集成
- 不做 Ref Tree 折叠/搜索/过滤（本轮只修 Dock 和 Pill）
- 不做 Ambiguous 对话框的候选列表（仅保持现有 confirm/cancel）
- 不做 Viewer 右键上下文菜单
- 不保留与单一选择状态冲突的旧协议形态；如统一选择状态需要，允许修改 protocol 层数据结构
- 不改 Three.js 拾取逻辑
- 不新增 Python 脚本（CadQuery runner 是唯一例外）
- 不修改 system prompt 文件本身（新能力通过 skill 机制扩展）
