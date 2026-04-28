# Agent Chat 交互设计

## 背景

budn' CadQuery Agent Chat 的工程管线已搭建完整，但产品层面存在致命可用性问题：

1. 没有接入 LLM，Agent 无法产出有意义的回答
2. 没有选择目标就无法有效使用 Execute 模式
3. Inform/Plan/Execute 三种模式的切换让新手困惑
4. Selection 与 Agent 强耦合——是执行前置条件而非可选上下文

本文档定义 Agent Chat 的交互模型、用户流程和设计决策，作为后续实现的产品约束。

## 设计原则

| 原则 | 含义 |
|------|------|
| 对话优先 | 打开聊天就能直接输入，不需要任何前置操作 |
| 上下文充实但不阻塞 | Selection、Ref 引用是可选的上下文增强，不是前置条件 |
| Agent 主动，用户掌控 | Agent 判断操作级别并提出方案，用户确认后才执行 |
| 复杂度渐进展现 | 新手看到简单聊天界面，高级能力通过使用自然发现 |
| 透明而非魔术 | Agent 的判断、读取的文件、操作的范围都清晰可见 |

## 参考产品分析

详见 [competitive-analysis.md](./competitive-analysis.md)。

---

## 一、交互模型

### 1.1 核心变化

| 维度 | 旧设计 | 新设计 |
|------|--------|-------|
| 入口 | 选模式 → 选目标 → 输入 | 直接输入 |
| 操作级别 | 用户手动选 Inform/Plan/Execute | Agent 自动判断（保留高级覆盖） |
| Selection | Execute 的前置条件 | 可移除的"上下文胶囊" |
| Execute 触发 | 用户预选 Execute 模式 | Agent 输出 Plan 确认卡片 → 用户确认 |
| 空状态 | "No active chat" | 欢迎引导 + 建议提示 |

### 1.2 Agent 自动模式判断

`AgentOperationLevel` 保留在协议中，语义从"用户选择"变为"Agent 判定输出"。

| 用户意图信号 | Agent 判定 | 行为 |
|-------------|-----------|------|
| 提问/比较/解释 | Inform | 回答，不碰文件 |
| "给方案"/"怎么改"/"plan" | Plan | 生成 CAD Plan 确认卡片 |
| "做"/"执行"/"确认" + 已有 Plan 卡片 | Execute | 按确认范围执行 |
| 直接修改指令（"高度改成12mm"） | Plan → 等确认 → Execute | 先展示计划，确认后执行 |
| 简单快捷操作（"fillet 2mm"）+ 有明确选择 | 快捷确认 → Execute | 轻量确认卡片 |
| 模糊/不明确 | Inform + 追问 | 先理解，再推进 |

**高级覆盖**：专业用户可通过斜杠命令强制模式（`/plan ...`、`/execute`），默认界面不显示模式选择按钮。

**透明性**：Agent 回复消息上显示操作级别标签（"Inform"/"Plan"/"Execute"），判定不对时用户可在下一句纠正。

### 1.3 Execute 确认机制

任何文件修改都必须经过结构化确认。

**状态机**：

```
对话中 → Agent 判定需要执行
    → Agent 输出 Plan 确认卡片（目标文件 + 影响范围 + 变更描述）
    → 等待确认
        → [确认执行] 按钮 或 自然语言("做吧") → Execute
        → "改一下..." → Agent 调整 Plan，重新展示卡片
        → "算了"/"取消" → 回到对话
```

**安全约束**：
- 协议层 `AgentCadQueryConfirmation` 保持不变——硬门禁
- 没有 Plan 卡片在前时，自然语言"确认"不触发 Execute
- Plan 卡片的结构化数据由前端自动构建为确认 payload

### 1.4 快捷操作路径

简单操作（fillet、chamfer、修改尺寸）不需要完整 CAD Plan。

当 Agent 判定操作简单且上下文明确时，展示轻量确认卡片：

```
对 @feature[top_lid.top_surface] 的边缘应用 2mm fillet
修改: parts/top_lid.py
[执行] [调整参数] [取消]
```

判断条件：操作单一、只影响一个文件、有明确的选择上下文。

---

## 二、Selection 作为可选上下文

### 2.1 Context Pill 模型

Cursor 的 @-mention 是最佳参考。在 budn' 中，等价物是 Viewer 中的点击选择——"空间 @-mention"。

**交互**：
1. 用户在 Viewer 中点击面/边/顶点/部件
2. 输入框上方出现可移除的上下文胶囊：`[×] @feature[top_lid.top_surface]`
3. 可以点 × 移除
4. 发消息时附加的上下文随消息发送
5. 没有胶囊时照常发消息——Selection 永不阻塞发送

**多选**：多次点击产生多个胶囊，协议已支持 `Vec<SelectionRef>`。

### 2.2 胶囊生成策略

所有 Viewer 选择都生成胶囊，用户移除不需要的。最多保留最近 3 个，旧的自动滑出。

### 2.3 无上下文时 Agent 的处理

当用户没有选择就说"把这个改高一点"时：

1. **对话历史推断**——最近讨论了某个 part，Agent 推断指的是它
2. **项目结构推断**——workspace 只有一个 part，Agent 推断指的是它
3. **主动追问**——"你指的是哪个部件？项目里有 @part[top_lid] 和 @part[bottom_case]。"

Agent 追问比 UI 报错好得多。

### 2.4 @-mention 文本输入（延伸能力）

用户可在消息中输入 `@` 触发自动补全，引用 workspace 中的 part/component/assembly。

MVP 可延后，Viewer 点击已覆盖主要场景，但协议中预留 `context_refs` 字段。

---

## 三、新用户体验

### 3.1 空状态

当没有聊天历史时显示欢迎界面：

```
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

```
打开聊天 → 输入问题 → Agent 回答 / 生成模型
    → Viewer 显示模型 → 点击部件 → 发现 context pill
    → Agent 提出 Plan 卡片 → 发现确认机制
    → 执行确认 → 发现自己掌控执行时机
    → 开新 session → 发现多对话
    → 输入 /plan → 发现高级控制
```

---

## 四、典型场景

### 场景 A：全新用户，从零开始

```
用户: "我想设计一个 Arduino Uno 的外壳"
Agent [Inform]: 
  "好的。几个问题：
   1. 用途？（保护/展示/安装？）
   2. 开口需求？（USB、电源、引脚？）
   3. 制造方式？（3D打印/CNC？）"

用户: "3D打印保护壳，需要USB和电源口"
Agent [Plan]: 
  ┌ CAD Plan ─────────────────────────┐
  │ 基于 Uno 标准尺寸 (68.6×53.4mm)   │
  │ 两件式外壳，壁厚 2mm              │
  │ 包含 USB-B 和 DC 电源开口         │
  │ 目标: parts/arduino_case.py       │
  │ [预览] [确认执行] [取消]           │
  └────────────────────────────────────┘

用户: "做吧"
Agent [Execute]: 
  → 创建 parts/arduino_case.py → 执行 CadQuery → Viewer 显示
  "已创建。点击任意面来选择，告诉我需要什么修改。"
```

### 场景 B：Viewer 选择后修改

```
[用户点击上盖顶面]
[Context pill: [×] @feature[arduino_case.top_surface]]

用户: "加一个 logo 凹槽"
Agent [Plan]:
  ┌ Quick Action ─────────────────────┐
  │ 在 top_surface 添加 20×10mm 凹槽  │
  │ 修改: parts/arduino_case.py       │
  │ [执行] [调整参数] [取消]           │
  └────────────────────────────────────┘
```

### 场景 C：无选择，纯讨论

```
用户: "CadQuery 里怎么做倒角？"
Agent [Inform]:
  "用 `.chamfer(distance)`。示例：
   result = cq.Workplane("XY").box(10,10,5).edges(">Z").chamfer(0.5)
   需要我在你的模型上演示吗？"
```

---

## 五、CAD 场景特有设计

### 5.1 预览后再提交

Plan 确认卡片包含 [预览] 按钮：执行 CadQuery 但不写入文件，Viewer 显示效果，用户满意后再 [确认执行]。

### 5.2 撤销

MVP 支持撤销最后一次 Agent 执行：
- 执行前自动备份目标文件
- Execute 结果卡片包含 [撤销] 按钮
- 撤销 = 恢复备份 + 重新渲染旧版本

### 5.3 Assembly 影响范围透明

修改 component 会影响所有引用它的 assembly。确认卡片必须显示：

```
修改 @component[pcb_main] 的尺寸
影响文件: components/pcb_main.py
⚠ 此组件被以下 assembly 引用:
  - assemblies/full_enclosure.py (2 个实例)
```

---

## 六、协议变更摘要

| 变更 | 描述 |
|------|------|
| `AgentOperationLevel::Auto` | 新增默认值，Agent 自动判断操作级别 |
| `AgentInvokeRequest.context_refs` | 用户附加的上下文引用列表 |
| `ServerPushEvent::AgentPlanProposed` | Plan 确认卡片数据推送 |
| `ClientCommand::AgentPlanConfirm` | 用户确认 Plan |
| `ClientCommand::AgentPlanReject` | 用户拒绝 Plan |
| `ClientCommand::AgentExecuteRevert` | 撤销最后一次执行 |
| `ServerCapabilities.llm_configured` | 前端据此显示设置引导或正常 UI |

---

## 七、已知风险

| 风险 | 严重度 | 缓解 |
|------|--------|------|
| LLM 误判 Execute 导致意外修改 | 严重 | 协议层 `AgentCadQueryConfirmation` 硬门禁，Auto 永不直接产出 Execute |
| Selection pill 探索性点击污染 | 中 | 可移除 + 最多 3 个 + 后续可加模式区分 |
| 无 undo 导致 Execute 焦虑 | 中 | 实现单步撤销 |
| 自然语言"确认"误判 | 中 | 必须有 Plan 卡片在前才接受 |
| Assembly 修改范围不透明 | 中 | 确认卡片显示影响的 assembly |
