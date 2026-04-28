# Agent Chat 产品流程重设计

## Context

budn' CadQuery Agent Chat 的工程管线（协议/服务端/前端）已搭建完整，但产品层面存在三个致命问题导致完全不可用：

1. **没有接入 LLM** — `LocalAgentBackend` 是 stub，Inform/Plan 返回硬编码文本，Execute 直接报错（`agent.rs:117-121`）
2. **没有选择目标就无法有效使用** — Execute 模式强制要求选择/目标路径，且三种模式的切换让新手困惑
3. **Selection 和 Agent 强耦合** — `buildCadQueryConfirmation()` 从 Selection 推导目标文件和影响范围，是 Execute 的前置条件而非可选上下文

本 plan 的目标：从产品经理视角重新设计 Agent Chat 交互流程，让所有目标用户（尤其是新手）都能快速上手。

### 参考产品分析

**Codex App 关键启发：**
- 任务导向而非选择导向——用户描述意图，Agent 自行探索和规划
- 确认是内联的——Agent 提出变更，用户逐步审批，不需要提前选择模式
- 每个任务独立上下文，可并行工作

**Cursor 关键启发：**
- @-mention 上下文注入——@file、@codebase、@docs 作为可选精确上下文
- Agent Mode 直接工作——描述高级目标，Agent 规划并执行多文件变更
- 自动上下文增强——系统自动附加当前文件、最近编辑、活跃错误等
- 模式是策略而非门禁——Agent/Plan/Debug/Ask 改变行为方式，不限制访问

**budn' 的独特优势：**
- 3D Viewer 选择是我们独有的"空间 @-mention"——比代码编辑器的文本引用更直观
- CAD 用户习惯"选择 → 操作"的直接交互模式，这和 Cursor 的 @-mention 天然契合
- Ref 系统（5 层）已经为精确的上下文引用打好了基础

---

## 一、核心交互模型重设计

### 1.1 从"工具优先"到"对话优先"

| 维度 | 现有设计 | 新设计 | 理由 |
|------|---------|-------|------|
| 入口 | 选择模式 → 选择目标 → 输入 | 直接输入 | 消除前置步骤，降低认知负担 |
| 操作级别 | 用户手动选 Inform/Plan/Execute | Agent 自动判断（保留高级覆盖） | 新手不理解三种模式的区别 |
| Selection | Execute 的前置条件 | 可选的"上下文胶囊" | 无选择也要能聊天 |
| Execute 触发 | 用户预选 Execute 模式 | Agent 输出 Plan → 用户确认 → 执行 | 始终先看计划再动手 |
| 空状态 | "No active chat" | 欢迎引导 + 建议提示 | 新手第一眼要知道能做什么 |

### 1.2 Agent 自动模式判断

`AgentOperationLevel` 保留在协议中，但语义从"用户选择"变为"Agent 判定输出"。

**判断规则：**

| 用户意图信号 | Agent 判定 | 行为 |
|-------------|-----------|------|
| 提问/比较/解释 | Inform | 回答，不碰文件 |
| "给方案"/"怎么改"/"plan" | Plan | 生成 CAD Plan 卡片 |
| "做"/"执行"/"确认" + 已有 Plan 卡片 | Execute | 按确认范围执行 |
| 直接修改指令（"高度改成12mm"） | Plan → 等确认 → Execute | 先展示计划，确认后执行 |
| 简单快捷操作（"fillet 2mm"）+ 有选择 | 快捷确认 → Execute | 轻量确认卡片，不出完整 Plan |
| 模糊/不明确 | Inform + 追问 | 先理解，再推进 |

**协议变更：** `AgentInvokeRequest.operation` 新增 `Auto` 值（默认）。保留 `Inform`/`Plan`/`Execute` 用于高级覆盖。

```rust
// AgentInvokeRequest 新增字段
pub operation: AgentOperationLevel,          // 新增 Auto 值
pub operation_override: Option<AgentOperationLevel>,  // 高级覆盖（/plan、/execute 命令）
pub context_refs: Vec<String>,              // 用户附加的上下文引用
```

**高级用户的显式控制：** 通过聊天输入的斜杠命令覆盖模式（`/plan 设计一个滑盖`、`/execute`），而不是 UI 按钮。这让默认界面保持简洁，同时给专业用户控制感。

**状态透明：** Agent 回复消息上显示小标签表明判定的操作级别（"Inform"/"Plan"/"Execute"），让用户知道 Agent 在做什么级别的操作，如果判定不对可以在下一句纠正。

### 1.3 Execute 确认机制

**核心原则：** 任何文件修改都必须经过结构化确认。自然语言"确认"和按钮确认都可以触发，但前提是 Agent 已经展示了 Plan 确认卡片。

**状态机：**
```
对话中 → Agent 判定需要执行
    → Agent 输出 Plan 确认卡片（目标文件 + 影响范围 + 变更描述）
    → 等待确认状态
        → 用户点击 [确认执行] 按钮 → Execute
        → 用户说"做吧"/"确认"/"go" → Execute
        → 用户说"改一下..."  → Agent 调整 Plan，重新展示卡片
        → 用户说"算了"/"取消" → 取消，回到对话
```

**Plan 确认卡片的结构化内容**（来自 Agent 回复中的 JSON block，前端渲染为卡片）：
- 目标文件和影响文件列表
- 变更描述（一句话总结）
- 导出目标
- [预览] [确认执行] [取消] 按钮

**安全约束：**
- 协议层的 `AgentCadQueryConfirmation` 保持不变——这是防止意外执行的最后一道门
- Plan 确认卡片的结构化数据由前端自动构建为 `AgentCadQueryConfirmation`，不再需要用户手动输入目标路径
- 没有 Plan 卡片在前的情况下，自然语言"确认"不触发 Execute

### 1.4 快捷操作路径

CAD 用户习惯"选择 → 操作 → 确认"的紧凑循环。对于简单操作（fillet、chamfer、修改尺寸），完整的 CAD Plan 流程太重。

**快捷确认卡片：** 当 Agent 判定操作简单且上下文明确时，跳过完整 Plan，展示轻量确认：
```
对 @feature[top_lid.top_surface] 的边缘应用 2mm fillet
修改: parts/top_lid.py
[执行] [调整参数] [取消]
```

判断条件：操作单一、只影响一个文件、有明确的选择上下文。否则走完整 Plan 流程。

---

## 二、Selection 作为可选上下文

### 2.1 "Context Pill"模型（类比 Cursor @-mention）

在 Cursor 中，用户通过 @file、@codebase 添加上下文。在 budn' 中，等价物是 **Viewer 中的点击选择**——"空间 @-mention"。

**交互：**
1. 用户在 Viewer 中点击一个面/边/顶点/部件
2. 输入框上方出现可移除的"上下文胶囊"：`[×] @feature[top_lid.top_surface]`
3. 用户可以移除胶囊（点 ×）
4. 发消息时，附加的上下文随消息一起发送
5. 没有胶囊时照常发消息——Selection 从未阻塞发送

**多选：** 多次点击产生多个胶囊，协议已支持 `Vec<SelectionRef>`。

**胶囊信息丰富度：** 除了 Ref 文本，可以在展开/悬停时显示该选择的关键属性（如尺寸、法向量），帮助用户确认选对了。

### 2.2 何时生成胶囊

**方案：** 所有 Viewer 选择都生成胶囊，用户移除不需要的。

理由：
- 简单一致——用户永远知道"点击了什么就附加了什么"
- 避免"为什么我点了没反应"的困惑（如果需要特殊手势来附加）
- Cursor 的 @-mention 也是"输入了就附加了，不想要就删"

**风险：探索性点击产生大量胶囊。**

缓解：
- 只保留最近 N 个（如 3 个），旧的自动滑出
- 或者区分"选择模式"和"导航模式"——按住 Shift 点击才生成胶囊（但这增加了学习成本，不推荐 MVP）
- MVP 先用"都生成、可移除"方案，根据用户反馈再优化

### 2.3 Agent 如何处理"无上下文"情况

当用户没有任何选择就说"把这个改高一点"时：

1. **对话历史推断**——如果最近讨论了某个 part，Agent 推断指的是它
2. **项目结构推断**——如果 workspace 只有一个 part，Agent 推断指的是它
3. **主动追问**——"你指的是哪个部件？项目里有 @part[top_lid] 和 @part[bottom_case]。"

这把"需要选择"从 UI 阻塞变成了对话中的自然引导。Agent 追问比 UI 报错好得多。

### 2.4 @-mention 文本输入（延伸能力）

除了 Viewer 点击，用户可以在消息中输入 `@` 触发自动补全：
- `@part[...]` — workspace 中的 parts
- `@component[...]` — workspace 中的 components
- `@assembly[...]` — workspace 中的 assemblies

这是 Cursor 模式在文本层面的延伸。**MVP 可以延后实现**，因为 Viewer 点击已经覆盖了主要场景，但设计上要为此预留空间（`context_refs` 字段已经在协议中）。

---

## 三、新用户体验

### 3.1 空状态欢迎界面

```
┌──────────────────────────────────────┐
│  budn' agent                         │
│                                      │
│  你好，我是你的 CAD 设计助手。        │
│                                      │
│  你可以直接描述想做的事情：            │
│                                      │
│  ○ "设计一个手机壳"                   │  ← 点击自动填入输入框
│  ○ "修改上盖的高度"                   │
│  ○ "解释 CadQuery 的 fillet 怎么用"  │
│                                      │
│  在 Viewer 中选择模型部件，            │
│  可以为对话添加精确上下文。            │
│                                      │
│  ┌──────────────────────────────────┐│
│  │ 描述你想做的事情...                ││
│  └──────────────────────────────────┘│
│                           [Send →]   │
└──────────────────────────────────────┘
```

### 3.2 关键状态引导

| 状态 | 用户看到什么 | 行为 |
|------|------------|------|
| 无 LLM 配置 | "需要配置 AI 服务才能开始。" + [打开设置] 按钮 | 引导设置，不是报错 |
| 无 workspace | "打开一个项目文件夹开始设计。" + [打开] 按钮 | 引导打开 workspace |
| 有 workspace，无 CadQuery 文件 | 正常聊天——Agent 帮助创建项目结构 | 用户说"设计一个XX"，Agent 创建 parts/xxx.py |
| LLM 配置但连接失败 | Header 区域红色指示 + 具体错误信息 | 不阻塞 UI，但发送消息时给出错误引导 |

### 3.3 渐进发现路径

```
打开聊天 → 输入问题
       ↓
Agent 帮你解答 / 生成第一个模型
       ↓
Viewer 显示模型 → 点击部件
       ↓
Chat 中出现 context pill → 发现"选择可以作为上下文"
       ↓
Agent 提出 Plan 卡片 → 发现"执行前可以先看计划"
       ↓
用确认按钮执行 → 发现"我可以控制什么时候动手"
       ↓
开新 session 讨论其他 → 发现"可以有多个对话主题"
       ↓
输入 /plan 命令 → 发现"可以显式控制 Agent 行为"
```

---

## 四、LLM 接入方案

### 4.1 配置层级

```
环境变量 (最高优先) → Workspace 配置 → 全局 AppConfig → 无配置（引导设置）
```

### 4.2 配置结构

Workspace 级别配置（不进入版本控制）：

```json
{
  "llm": {
    "provider": "openai_compatible",
    "base_url": "https://api.openai.com/v1",
    "model": "gpt-5.5",
    "api_key_source": "env:OPENAI_API_KEY",
    "timeout_ms": 60000,
    "max_tokens": 4096
  }
}
```

环境变量覆盖（开发调试用）：
- `BUDN_LLM_BASE_URL` → base_url
- `BUDN_LLM_MODEL` → model
- `BUDN_LLM_API_KEY` → 直接提供 key

### 4.3 调试阶段的配置

用户提供的 `workspace/studio-web/providers.txt` 转为环境变量或 workspace config：
- **绝对禁止将 API key 或 endpoint 提交到代码库**
- `.gitignore` 中添加 workspace 配置路径
- 优先使用环境变量方式传递敏感信息

### 4.4 实现选型

MVP 期间先用直接 HTTP 调用 OpenAI Compatible API（基于 `reqwest` + SSE 流式解析），不引入 rig-core 或其他框架。理由：
- 只需要一个 provider（OpenAI Compatible）和 streaming chat completion
- rig-core 引入的抽象层级在 MVP 阶段收益不大，且版本迭代较快
- 直接 HTTP 调用更可控、更容易调试、更少依赖

如果后续需要多 provider 支持或复杂 tool use 编排，再评估是否引入框架。

### 4.5 前端设置 UI

Settings 面板新增 LLM 配置区域：
- Provider 选择（OpenAI Compatible / Anthropic / 自定义）
- Base URL 输入
- API Key 输入（密码字段）
- Model 名称
- [测试连接] 按钮

---

## 五、CAD 场景特有考虑

### 5.1 预览后再提交

CAD 用户习惯在提交操作前看到预览效果。利用现有 `CadQueryPreview` 命令：

Plan 确认卡片中包含 [预览] 按钮 → 执行 CadQuery 代码但不写入文件 → Viewer 显示效果 → 用户满意后 [确认执行]。

这是传统 CAD 工具的核心交互模式，缺少它会让 CAD 用户感到不安。

### 5.2 撤销机制

当前架构没有 undo。CAD 用户高度依赖撤销。

MVP 最小可行方案：
- Agent 执行前自动备份目标文件（staging 目录中保留原始副本）
- Execute 结果卡片中包含 [撤销] 按钮
- 撤销 = 恢复备份 + 重新执行 CadQuery Preview 显示旧版本
- 不需要完整的 undo stack，只支持撤销最后一次 Agent 执行

### 5.3 Assembly 操作的范围透明

修改一个 component 会影响所有引用它的 assembly instance。确认卡片必须显示影响范围：

```
修改 @component[pcb_main] 的尺寸
影响文件: components/pcb_main.py
⚠ 此组件被以下 assembly 引用:
  - assemblies/full_enclosure.py (2 个实例)
  
[预览] [确认执行] [取消]
```

---

## 六、UI 组件变更清单

### 6.1 移除

| 组件 | 文件 | 理由 |
|------|------|------|
| `OperationSelector` | `chat-zone.tsx:396-412` | 模式选择由 Agent 自动判定 |
| `ExecuteTargetInput` | `chat-zone.tsx:430-444` | 目标文件由 Plan 确认卡片承载 |
| `OperationButton` (3个) | `chat-zone.tsx:446-460` | 同上 |

### 6.2 新增

| 组件 | 功能 | 位置 |
|------|------|------|
| `ContextPillBar` | 显示 Viewer 选择作为可移除胶囊 | ChatComposer 内，输入框上方 |
| `WelcomeEmptyState` | 欢迎消息 + 建议提示词 | ChatBody 空状态 |
| `PlanConfirmationCard` | Agent 输出的确认卡片（预览/确认/取消） | ChatBody 中作为特殊消息 |
| `QuickConfirmCard` | 简单操作的轻量确认 | ChatBody 中 |
| `AgentLevelBadge` | 显示 Agent 判定的操作级别 | Agent 消息气泡上 |
| `LlmStatusIndicator` | 连接状态指示（绿/红） | ChatHeader |
| `LlmSetupGuide` | 无 LLM 配置时的引导 | ChatBody 空状态 |

### 6.3 修改

| 组件 | 变更 | 原因 |
|------|------|------|
| `ChatComposer` | 移除 operation/targetPath props，新增 contextPills prop | 简化输入区域 |
| `ChatComposerTools` | 只保留 Send + 附件按钮 | 移除模式切换按钮 |
| `sendChatMessage()` | 不再构建 `confirmed_cadquery`，改为发送 `context_refs` | 确认由 Plan 卡片触发 |
| `ChatHeader` | 添加 LLM 状态指示 | 让用户知道连接状态 |
| `ChatBody` | 支持渲染 PlanConfirmationCard | 新的消息类型 |
| `cadquery-agent-scope.ts` | 简化——不再在发送时构建 confirmation，改为 context pill 数据 | 解耦 |

---

## 七、后端变更清单

### 7.1 LLM Provider

新增 `crates/app-server-core/src/llm/` 模块：
- `mod.rs` — `LlmProvider` trait + config 加载
- `openai_compatible.rs` — 直接 HTTP SSE 调用
- `config.rs` — 配置结构和环境变量读取

### 7.2 Agent 自动模式

修改 `crates/app-server-core/src/agent.rs`：
- 新增 `classify_operation(prompt, history, selections) -> AgentOperationLevel`
- 当 `operation == Auto` 时由 Agent（LLM）判断或使用规则引擎
- `draft_turn` 和 `generate_cadquery_code` 的入口逻辑不变，只是 operation 来源从用户输入变为 Agent 判定

### 7.3 Plan 确认流程

修改 `crates/app-server-host/src/dispatcher.rs`：
- Agent 输出 Plan 时，通过 `agent.plan_proposed` 事件推送结构化数据
- 用户确认时，前端构建 `AgentCadQueryConfirmation` 并发送新的 `agent.plan_confirm` 命令
- 原有的 `AgentInvokeRequest.confirmed_cadquery` 保留作为协议层安全门

### 7.4 Selection 解耦

修改 `crates/app-server-host/src/dispatcher.rs`：
- Selection 从 AgentWorker 的前置条件变为可选上下文
- 当 `context_refs` 非空时注入 LLM prompt
- 当为空时不注入，Agent 可通过 `get_selection` 工具主动查询

---

## 八、协议变更

| 变更 | 类型 | 描述 |
|------|------|------|
| `AgentOperationLevel::Auto` | 新增 enum variant | 默认值，Agent 自动判断 |
| `AgentInvokeRequest.context_refs` | 新增字段 | 用户附加的上下文引用列表 |
| `ServerPushEvent::AgentPlanProposed` | 新增 push event | 携带 Plan 确认卡片数据 |
| `ClientCommand::AgentPlanConfirm` | 新增 command | 用户确认 Plan，携带 confirmation payload |
| `ClientCommand::AgentPlanReject` | 新增 command | 用户拒绝 Plan |
| `ClientCommand::AgentExecuteRevert` | 新增 command | 撤销最后一次执行 |
| `ClientCommand::ConfigUpdateLlm` | 新增 command | 更新 LLM 配置 |
| `ClientCommand::ConfigTestLlm` | 新增 command | 测试 LLM 连接 |
| `ServerCapabilities.llm_configured` | 新增字段 | 前端据此显示设置引导或正常 UI |

---

## 九、实施分期

### Phase A — LLM 接入（Agent 能说话）

**目标：** 让 Agent 能给出有意义的回答。

- 实现 `LlmProvider` trait 和 OpenAI Compatible provider（直接 HTTP + SSE）
- 实现 workspace/环境变量 LLM 配置加载
- 将 `LocalAgentBackend.draft_turn()` 替换为真实 LLM 调用（Inform/Plan 模式）
- Execute 模式的 `generate_cadquery_code()` 也接入 LLM
- 流式 token 通过现有 `agent.token` push event 传输到前端

**验收：** 配置好 LLM 后，Inform 模式的对话能得到有意义的 AI 回答。

### Phase B — 解耦 Selection + 重设计 Composer

**目标：** 无选择也能聊天，Selection 变为可选上下文。

- 移除 `OperationSelector` 和 `ExecuteTargetInput`
- 实现 `ContextPillBar`（Viewer 选择 → 可移除胶囊）
- 实现 `WelcomeEmptyState` 空状态欢迎界面
- 协议新增 `AgentOperationLevel::Auto` 和 `context_refs`
- 后端 Selection 从前置条件变为可选上下文

**验收：** 无 Viewer 选择时可以正常聊天。有选择时以胶囊形式显示在输入区域。

### Phase C — Agent 自动模式 + Plan 确认卡片

**目标：** 完整的对话到执行流程。

- 实现 Agent 意图分类（Auto 模式下由 LLM 判断操作级别）
- 实现 `PlanConfirmationCard` 和 `QuickConfirmCard`
- 实现 Plan → 确认 → Execute 状态机
- 实现 `AgentPlanProposed`/`AgentPlanConfirm`/`AgentPlanReject` 协议流
- 实现 Agent 级别标签显示

**验收：** 用户描述修改 → Agent 输出 Plan 卡片 → 用户确认 → 执行 → Viewer 更新。

### Phase D — 体验打磨

**目标：** 新用户 5 分钟内完成从零到第一个模型。

- LLM 配置 Settings UI
- 预览按钮（Plan 卡片中的 [预览]）
- 撤销最后一次执行
- Assembly 影响范围提示
- 错误状态友好处理（LLM 断连、CadQuery 构建失败、选择失效等）
- 斜杠命令支持（/plan、/execute 等高级覆盖）

**验收：** 新用户从打开 budn' 到看到第一个生成的 3D 模型，全程无阻塞。

---

## 十、已知风险

| 风险 | 严重度 | 缓解 |
|------|--------|------|
| LLM 误判 Execute 导致意外修改 | 严重 | 协议层 `AgentCadQueryConfirmation` 是硬门禁，Auto 模式永远不直接产出 Execute |
| Selection pill 探索性点击污染 | 中 | 可移除 + 最多保留 N 个 + 后续可加导航/选择模式区分 |
| 无 undo 导致 Execute 焦虑 | 中 | Phase D 实现单步撤销 |
| 自然语言"确认"误判 | 中 | 必须有 Plan 卡片在前才接受 NL 确认 |
| Assembly 修改范围不清晰 | 中 | 确认卡片显示影响的 assembly 列表 |
| Inform/Plan 误分类 | 低 | 两者都不改文件，成本只是回复格式不太对 |

---

## 关键文件索引

| 文件 | 职责 | 涉及的变更 |
|------|------|-----------|
| `packages/studio-web/src/workbench/chat-zone.tsx` | Chat UI 主组件 | 移除 OperationSelector/ExecuteTargetInput，新增 ContextPillBar/PlanConfirmationCard/EmptyState |
| `packages/studio-web/src/workbench/cadquery-agent-scope.ts` | Selection → Agent scope | 简化为 context pill 数据提供者，不再在发送时构建 confirmation |
| `crates/app-server-core/src/agent.rs` | Agent 后端 | 接入 LLM、自动模式判断、Plan 确认流 |
| `crates/app-server-core/src/agent/selection.rs` | Selection → 目标路径 | 从前置条件改为可选上下文注入 |
| `crates/app-server-host/src/dispatcher.rs` | Agent Worker | Plan 确认状态机、Execute revert |
| `crates/app-server-protocol/src/protocol.rs` | 协议定义 | Auto 操作级别、context_refs、Plan 确认事件/命令 |
| `crates/app-server-core/src/llm/` (新增) | LLM Provider | OpenAI Compatible 实现、配置加载 |
| `docs/cadquery-mvp/agent-system-prompt.md` | Agent 系统 prompt | 无需大改——已经定义了三种级别的行为规则 |
