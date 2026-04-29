# Agent Chat 交互设计

## 背景

budn' CadQuery Agent Chat 的工程管线已搭建完整，但旧交互把用户带入四种 operation 和独立 plan confirmation 流，导致产品路径复杂，并且 Web 中直接输入执行请求时容易落入缺少结构化执行范围的错误。

本文档定义新的 Agent Chat 交互模型、用户流程和设计决策，作为后续实现的产品约束。

## 设计原则

| 原则 | 含义 |
|------|------|
| 对话优先 | 打开聊天就能直接输入，不需要任何前置操作 |
| 模式清晰 | 用户只需要理解 `Agent` 和 `Plan` 两个模式 |
| 上下文充实但不阻塞 | Selection、Ref 引用是可选上下文增强，不是前置条件 |
| 安全边界在服务端 | 写入和执行由 Agent mode path policy、CadQuery staging、`.py` 专用工具和 execution scope 约束 |
| 透明而非魔术 | Agent 读取的文件、操作范围、plan package 和执行结果都清晰可见 |

## 参考产品分析

详见 [competitive-analysis.md](./competitive-analysis.md)。

---

## 一、交互模型

### 1.1 核心变化

| 维度 | 旧设计 | 新设计 |
|------|--------|-------|
| 入口 | 选 operation → 选目标 → 输入 | 直接输入，默认 `Agent` mode |
| 模式 | 多个 operation 与独立执行流程 | `Agent` / `Plan` 双模式 |
| Selection | 执行前置条件 | 可移除的上下文胶囊 |
| 执行触发 | 独立确认流程 | `Agent` mode 直接工作，或 `Run Plan` 执行已有 plan |
| Plan 展示 | 确认卡片 | Workspace Plan Package 卡片 |
| 空状态 | "No active chat" | 欢迎引导 + 建议提示 |

### 1.2 Agent / Plan 双模式

| Mode | 适用场景 | 行为 |
|------|----------|------|
| `Agent` | 用户希望直接修改、创建、导出、运行已有 plan | 读取上下文，形成 execution scope，走 app server tool、path policy 和 CadQuery staging 执行 |
| `Plan` | 用户希望先分析方案、比较风险、保留计划档案 | 读取上下文，创建或更新 `plans/YYYYmmddnn-name/{request.md,plan.md,plan-result.md}`，不修改模型，不生成 outputs |

默认模式为 `Agent`。用户可以通过模式切换控件选择 `Plan`，也可以通过 `/plan` 快捷方式进入 Plan mode。`/agent` 明确切回 Agent mode。

删除 `/execute` 产品命令。执行已有 plan 的入口是 `Run Plan`，它发送 `agent.invoke { mode: Agent, plan_ref }`。

### 1.3 Workspace Plan Package 卡片

Plan mode 生成的卡片展示：

- `plan_id`
- `target_path`
- `target_type`
- `affected_files`
- `new_files`
- `export_targets`
- `status`

卡片动作：

- `Open Plan`：打开 `plans/<id>/plan.md`。
- `Run Plan`：触发 `agent.invoke { mode: Agent, plan_ref: plans/<id>/ }`。

`Run Plan` 只负责进入 Agent mode 主路径；服务端仍必须解析 plan front matter，并用 execution scope、path policy、CadQuery staging 和 outputs 策略约束实际写入。

### 1.4 直接 Agent 工作流

用户在 Agent mode 中可以直接说：

```text
把上盖高度改成 12mm，并导出 STEP。
```

Agent 的服务端流程：

1. 读取当前 Chat、context refs、selection 和项目结构。
2. 形成 execution scope。
3. 对普通文本写入应用 Agent mode path policy。
4. 对 CadQuery `.py` 修改使用 CadQuery 专用工具和 staging。
5. 将生成产物写入 `outputs/`，禁止前端绕过 protocol 写入。
6. 回复实际改动、outputs、风险和后续用户决策。

当请求范围复杂、目标不清楚或风险较高时，Agent 可以建议切换到 Plan mode 创建 plan package。

---

## 二、Selection 作为可选上下文

### 2.1 Context Pill 模型

Cursor 的 @-mention 是最佳参考。在 budn' 中，等价物是 Viewer 中的点击选择，即空间上下文引用。

交互：

1. 用户在 Viewer 中点击面、边、顶点、部件或装配实例。
2. 输入框上方出现可移除的上下文胶囊：`[×] @feature[top_lid.top_surface]`。
3. 可以点 × 移除。
4. 发消息时附加的上下文随消息发送。
5. 没有胶囊时照常发消息，Selection 永不阻塞发送。

多选：多次点击产生多个胶囊，协议支持 `Vec<SelectionRef>`。

### 2.2 胶囊生成策略

所有 Viewer 选择都生成胶囊，用户移除不需要的。最多保留最近 3 个，旧的自动移出。

### 2.3 无上下文时 Agent 的处理

当用户没有选择就说“把这个改高一点”时：

1. 对话历史推断：最近讨论了某个 part，Agent 推断指的是它。
2. 项目结构推断：workspace 只有一个 part，Agent 推断指的是它。
3. 主动追问：例如“你指的是哪个部件？项目里有 @part[top_lid] 和 @part[bottom_case]。”

Agent 追问比 UI 报错更符合对话式工作流。

### 2.4 @-mention 文本输入

用户可在消息中输入 `@` 触发自动补全，引用 workspace 中的 part、component 或 assembly。

MVP 可延后，Viewer 点击已覆盖主要场景，但协议中保留 `context_refs` 字段。

---

## 三、新用户体验

### 3.1 空状态

当没有聊天历史时显示欢迎界面：

```text
你好，我是你的 CAD 设计助手。

你可以直接描述想做的事情：

  ○ "设计一个手机壳"
  ○ "修改上盖的高度"
  ○ "解释 CadQuery 的 fillet 怎么用"

在 Viewer 中选择模型部件，
可以为对话添加精确上下文。
```

建议提示词可点击自动填入输入框。

### 3.2 关键状态引导

| 状态 | 用户看到什么 | 行为 |
|------|------------|------|
| 无 LLM 配置 | "需要配置 AI 服务才能开始" + [打开设置] | 引导，不报错 |
| 无 workspace | "打开一个项目文件夹开始设计" + [打开] | 引导打开 |
| 有 workspace 无 CadQuery 文件 | 正常聊天 | Agent 帮助创建项目结构 |
| LLM 连接失败 | Header 红色指示 + 具体错误 | 不阻塞 UI |

### 3.3 渐进发现路径

```text
打开聊天 → Agent mode 直接输入
    → Agent 回答、修改或建议 Plan mode
    → Viewer 显示模型 → 点击部件 → 发现 context pill
    → Plan mode 生成 workspace plan package
    → Open Plan 或在 Markdown preview 中 Run Plan
    → Agent mode 执行 plan，plan-result.md 记录结果
```

---

## 四、典型场景

### 场景 A：全新用户，从零开始

```text
用户: "我想设计一个 Arduino Uno 的外壳"
Agent [Agent]:
  "我需要几个参数：用途、开口需求、制造方式。"

用户切换到 Plan mode 后: "3D打印保护壳，需要USB和电源口，先给我方案"
Agent [Plan]:
  ┌ Plan Package ─────────────────────┐
  │ 2026050100-arduino-case           │
  │ 目标: parts/arduino_case.py       │
  │ 影响: parts/arduino_case.py/.md   │
  │ 导出: outputs/arduino_case.step   │
  │ [Open Plan] [Run Plan]            │
  └────────────────────────────────────┘

用户点击 [Run Plan]
Agent [Agent]:
  → 读取 plans/2026050100-arduino-case/plan.md
  → 创建 parts/arduino_case.py
  → 执行 CadQuery
  → 更新 plan-result.md
  "已创建。点击任意面来选择，告诉我需要什么修改。"
```

### 场景 B：Viewer 选择后修改

```text
[用户点击上盖顶面]
[Context pill: [×] @feature[arduino_case.top_surface]]

用户: "在这个面上加一个 logo 凹槽"
Agent [Agent]:
  → 基于 selection 和文件上下文形成 execution scope
  → 通过 CadQuery staging 修改模型
  → 输出实际改动和生成文件
```

### 场景 C：无选择，纯讨论

```text
用户: "CadQuery 里怎么做倒角？"
Agent [Agent]:
  "用 `.chamfer(distance)`。示例：
   result = cq.Workplane("XY").box(10,10,5).edges(">Z").chamfer(0.5)
   需要我在你的模型上演示吗？"
```

---

## 五、CAD 场景特有设计

### 5.1 Plan preview 后执行

Markdown preview 打开 `plans/<id>/plan.md` 时显示 `Run Plan`。点击后由前端发送 `agent.invoke { mode: Agent, plan_ref }`，不直接读写 workspace 文件，也不绕过 app server protocol。

### 5.2 结果记录

执行已有 plan 后，服务端更新 `plans/<id>/plan-result.md`：

- run 时间和 Chat session。
- 修改文件。
- 生成 outputs。
- CadQuery 诊断。
- 剩余风险和用户仍需决策的事项。

### 5.3 Assembly 影响范围透明

修改 component 会影响所有引用它的 assembly。Plan package 或 Agent 回复必须显示：

```text
修改 @component[pcb_main] 的尺寸
影响文件: components/pcb_main.py
此组件被以下 assembly 引用:
  - assemblies/full_enclosure.py (2 个实例)
```

---

## 六、协议变更摘要

| 变更 | 描述 |
|------|------|
| `AgentMode::{Agent, Plan}` | 替换旧 operation 语义 |
| `AgentInvokeRequest.mode` | 用户可见双模式 |
| `AgentInvokeRequest.plan_ref` | Agent mode 执行已有 plan package |
| `AgentInvokeRequest.context_refs` | 用户附加的上下文引用列表 |
| `ServerPushEvent::AgentPlanSaved` | Plan package 保存或更新事件 |
| `agent.plan.confirm` / `agent.plan.reject` | deprecated，仅返回迁移提示 |
| `ServerCapabilities.llm_configured` | 前端据此显示设置引导或正常 UI |

---

## 七、已知风险

| 风险 | 严重度 | 缓解 |
|------|--------|------|
| Agent mode 写入边界变宽 | 严重 | 服务端 path policy、CadQuery staging、`.py` 专用工具和 execution scope |
| Selection pill 探索性点击污染 | 中 | 可移除 + 最多 3 个 + 后续可加模式区分 |
| Agent mode 自由请求范围不清 | 中 | 目标不清时追问；复杂修改建议先切到 Plan mode |
| Markdown preview 误触执行 | 中 | 仅 `plans/<id>/plan.md` 显示 Run Plan，点击后仍走 app server protocol |
| Assembly 修改范围不透明 | 中 | Plan package 和 Agent 回复显示受影响 assembly |
