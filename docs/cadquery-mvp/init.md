# MVP PRD：CAD Agent Harness

## 1. 产品目标

构建一个网页端 CAD Agent Harness，让用户可以通过 Chat 与 Agent 协作完成 CAD 设计。

MVP 要跑通：

```text
多 Chat Session
→ CAD 方案讨论
→ Markdown CAD Plan package
→ Agent mode 执行 plan
→ CadQuery 生成 / 修改模型
→ Viewer 查看模型
→ Viewer 选择 component / part / assembly / 点 / 线 / 面
→ Agent 基于选择继续修改
```

---

## 2. 核心原则

```text
1. 文件系统作为 source of truth。
2. Chat 负责讨论上下文，不保存设计真相。
3. 每个 component / part / assembly 拆成独立文件。
4. 每个 component / part / assembly 都有对应 Markdown 文档。
5. 用户只是讨论时，Agent 不应直接动手。
6. 用户要方案时，Agent 输出 Markdown CAD Plan。
7. Agent mode 执行时才调用 CadQuery 工具；Plan mode 只创建计划档案。
8. Viewer 选择结果要能传给 Agent，作为后续修改目标。
9. 先不做复杂 Project State / Variant 系统。
10. 先不考虑沙盒和部署。
```

---

## 3. MVP 范围

### 3.1 必做

```text
1. 多 Chat Session
2. 文件系统项目结构
3. component / part / assembly 独立文件
4. 每个对象有同名 Markdown 说明文档
5. Agent 支持 Agent / Plan 两个模式
6. CadQuery 作为模型生成 / 修改工具
7. Rust 后端负责任务编排和产品状态
8. Python CadQuery 工具调用先跑起来
9. Viewer 支持 component 选择
10. Viewer 支持点 / 线 / 面选择
11. Viewer 选择结果能作为 ref 发送给 Agent
12. outputs 保存生成结果
```

### 3.2 暂不做

```text
1. 复杂 Project State
2. 复杂 Variant 系统
3. 多 Agent 系统
4. IR 中间层
5. 沙盒
6. 部署方案
7. 完整 DFM
8. 生产级验证
9. 高级装配仿真
10. OpenSCAD / CadQuery 双内核切换
```

---

## 4. 技术选型边界

```text
主产品后端：Rust
CAD 工具层：Python + CadQuery
模型查看：已有 Viewer / Renderer
状态来源：文件系统
Agent 执行：通过工具调用读写文件、生成模型、更新文档
```

MVP 不要求 CadQuery 运行在浏览器内。  
MVP 先把 Python CadQuery 工具调用跑通。

---

## 5. 项目文件结构

每个设计项目是一个文件夹。

```text
project/
├── README.md
├── chats/
│   ├── main.jsonl
│   ├── lid-discussion.jsonl
│   └── printability-review.jsonl
│
├── components/
│   ├── pcb_main.py
│   ├── pcb_main.md
│   ├── usb_connector.py
│   └── usb_connector.md
│
├── parts/
│   ├── top_lid.py
│   ├── top_lid.md
│   ├── bottom_case.py
│   └── bottom_case.md
│
├── assemblies/
│   ├── full_enclosure.py
│   ├── full_enclosure.md
│   ├── exploded_view.py
│   └── exploded_view.md
│
├── plans/
│   ├── 2026050100-slide-lid/
│   │   ├── request.md
│   │   ├── plan.md
│   │   └── plan-result.md
│   └── 2026050101-wall-mount/
│       ├── request.md
│       ├── plan.md
│       └── plan-result.md
│
├── refs/
│   └── current_selection.md
│
├── assumptions.md
└── outputs/
    ├── preview/
    ├── step/
    ├── stl/
    ├── 3mf/
    └── reports/
```

---

## 6. 文件职责

### 6.1 `README.md`

项目总说明。

包含：

```text
项目目标
当前设计方向
使用场景
主要组件
制造假设
当前风险
主要文件入口
```

---

### 6.2 `components/`

用于外部组件、参考件、可复用组件。

例如：

```text
PCB
开发板
连接器
电池
屏幕
螺丝
传感器模块
```

每个 component 包含：

```text
component_name.py
component_name.md
```

Markdown 说明包含：

```text
用途
尺寸
接口
和哪些 part / assembly 配合
是否为外购件
注意事项
```

---

### 6.3 `parts/`

用于可单独制造的零件。

例如：

```text
top_lid
bottom_case
wall_mount_bracket
button_cap
```

每个 part 包含：

```text
part_name.py
part_name.md
```

Markdown 说明包含：

```text
用途
关键参数
制造方式
和哪些 component 配合
和哪些 assembly 相关
可修改区域
不可随便修改区域
```

---

### 6.4 `assemblies/`

用于装配体。

Assembly 负责组合多个 component 和 part。

每个 assembly 包含：

```text
assembly_name.py
assembly_name.md
```

Markdown 说明包含：

```text
包含哪些 component / part
装配关系
装配顺序
接口关系
间隙要求
干涉风险
导出说明
```

---

### 6.5 `plans/`

保存 Agent 生成的 workspace plan package。

Plan package 使用 `plans/YYYYmmddnn-name/` 目录，每个目录包含：

```text
request.md
plan.md
plan-result.md
```

`Plan` mode 只允许创建或更新这三个计划档案文件，不修改 CAD 源文件，也不生成 outputs。`Agent` mode 可以读取已有 plan package 并执行，执行记录写入 `plan-result.md`。

---

### 6.6 `outputs/`

保存生成产物。

包括：

```text
预览图
STEP
STL
3MF
验证报告
导出包
```

---

## 7. 多 Chat Session

一个 Project 下支持多个 Chat。

每个 Chat 有独立讨论上下文。

### 7.1 Chat 类型

```text
主设计 Chat
某个 component Chat
某个 part Chat
某个 assembly Chat
某个方案探索 Chat
某个验证 Chat
```

### 7.2 Chat 存储格式

JSONL 格式（`chats/lid-discussion.jsonl`），每行一条消息记录：

```jsonl
{"ts":"2026-04-27T10:00:00Z","type":"meta","goal":"讨论 top_lid 的结构方案","related_files":["parts/top_lid.py","parts/top_lid.md","assemblies/full_enclosure.py"],"summary":"用户正在比较螺丝盖和滑盖","open_questions":["是否需要免工具拆装？","是否优先保证首版打样成功率？"]}
{"ts":"2026-04-27T10:01:00Z","role":"user","content":"滑盖和螺丝盖哪个好？"}
{"ts":"2026-04-27T10:01:05Z","role":"assistant","content":"从制造角度看...","tool_calls":[]}
{"ts":"2026-04-27T10:02:00Z","role":"assistant","content":"","tool_calls":[{"id":"tc_1","name":"cadquery","args":{"target":"top_lid","code":"..."}}]}
{"ts":"2026-04-27T10:02:10Z","role":"tool","tool_call_id":"tc_1","result":{"status":"success","mesh":{...}}}
```

JSONL 支持 tool calls / function results 的结构化存储，可追加、可重放、可截断。

### 7.3 Chat 必须支持

```text
新建
恢复
重命名
归档
记录摘要
关联文件
```

---

## 8. Agent 行为模型

Agent 使用两个产品模式：

### 8.1 Mode

| Mode | 含义 | 是否改模型 |
|---|---|---|
| Plan | 只读分析，创建 workspace plan package | 否 |
| Agent | 读写执行，可直接执行当前请求或已有 plan | 是 |

---

## 9. Agent Loop

```text
1. Resolve Context
   判断当前 Project / Chat / 相关文件 / Viewer 选择对象。

2. Resolve Mode
   使用用户选择的 `Agent` 或 `Plan` 模式。

3. Read Files
   读取相关 .py 和 .md 文件。

4. Act

   Plan:
     生成 workspace plan package，写入 plans/YYYYmmddnn-name/。
     不修改 CAD 源文件。
     不生成 outputs。

   Agent:
     如果带 plan_ref，读取 plans/<id>/request.md、plan.md 和 execution scope。
     如果不带 plan_ref，基于当前请求和 refs 形成 execution scope。
     修改相关 .py。
     更新相关 .md。
     调用 CadQuery 工具。
     生成 outputs。
     如果本次带 plan_ref，则更新对应 plan-result.md。
     返回结果。

5. Reply
   告诉用户：
   - 做了什么
   - 改了哪些文件
   - 生成了哪些产物
   - 有哪些假设
   - 有哪些风险
   - 下一步建议
```

---

## 10. Markdown CAD Plan

当用户需要方案或想先评估风险时，Plan mode 创建 workspace plan package。Agent mode 可以直接执行已有 plan，也可以在没有 plan_ref 时按当前请求工作。

### 10.1 Plan Package 格式

目录结构：

```text
plans/YYYYmmddnn-name/
├── request.md
├── plan.md
└── plan-result.md
```

`YYYYmmdd` 使用创建当天日期，`nn` 是当天第 n 个 plan，从 `00` 开始按已有同日期 plan 目录递增。`name` 使用小写 ASCII、数字和连字符。

`plan.md` 必须包含 machine-readable YAML front matter：

```yaml
---
plan_id: 2026050100-create-a-new-box
mode: plan
target_path: parts/box.py
target_type: part
affected_files:
  - parts/box.py
new_files: []
export_targets:
  - outputs/box.step
status: planned
created_at: 2026-05-01T09:12:00+08:00
source_chat_session: chat-1
---
```

正文格式：

```md
# CAD Plan: <title>

## Goal

这次设计要解决什么问题。

## Current Context

当前相关 component / part / assembly。

## Design Approach

设计思路和主要取舍。

## Affected Files

会影响哪些文件。

## CadQuery Strategy

准备用哪些 CadQuery 建模方式。

## Parameters and Assumptions

关键参数和假设。

## Risks

潜在风险。

## Validation Plan

执行后如何检查。

## Execution Boundary

会修改什么，不会修改什么。
```

`plan-result.md` 初始状态为 `pending`。Agent mode 执行该 plan 后追加 run、提交文件、生成 outputs、失败诊断和剩余风险。

Legacy `plans/*.md` 是历史计划文件，只读展示，不作为可直接执行的 plan package。

---

## 11. Viewer 选择与 Ref

Viewer 支持两类选择。

### 11.1 Component / Part / Assembly 选择

优先支持用户选择整体对象。

示例：

```text
@component[pcb_main]
@part[top_lid]
@part[bottom_case]
@assembly[full_enclosure]
```

用户体验：

```text
修改上盖
查看底壳
移动 PCB
检查整个装配
```

---

### 11.2 点 / 线 / 面选择

支持精细选择。

示例：

```text
@face[top_lid:top_surface]
@edge[bottom_case:front_edge]
@vertex[wall_mount:corner_1]
```

用户体验：

```text
在这个面上开孔
这条边倒角
这个点附近加支撑
```

---

### 11.3 Ref 给 Agent 的规则

Viewer 选择后，系统把 ref 写入当前 Chat 上下文。

Agent 收到 ref 后：

```text
1. 判断 ref 属于哪个文件
2. 读取对应 .py 和 .md
3. 如果只是讨论，返回分析
4. 如果用户在 Plan mode 中要方案，生成 workspace plan package
5. 如果用户在 Agent mode 中要求执行，修改对应文件并生成结果
```

---

## 12. CadQuery 使用定位

MVP 选择 CadQuery 的原因：

```text
1. 支持 B-rep / STEP 方向。
2. 支持 face / edge / vertex / solid / wire 选择。
3. Workplane / Selector 语义适合 Viewer 选择后继续修改。
4. 适合通过 Python 工具调用快速跑通。
```

MVP 不要求：

```text
复杂工程图
完整工业装配约束
高级曲面
全量生产级 STEP 工作流
```

---

## 13. 实验新想法的处理

暂时不做复杂 Variant 系统。

MVP 用文件复制和新 Chat 处理实验版本。

用户说：

```text
试试滑盖版本。
```

系统行为：

```text
1. 新建 Chat
2. 新建 workspace plan package
3. Agent mode 执行实验方案时复制相关 part / assembly 文件
4. 生成实验版本文件
```

示例：

```text
parts/top_lid_slide_experiment.py
parts/top_lid_slide_experiment.md
assemblies/full_enclosure_slide_experiment.py
assemblies/full_enclosure_slide_experiment.md
chats/slide-lid-experiment.md
plans/2026050100-slide-lid/plan.md
```

---

## 14. 用户关键流程

### 14.1 讨论

```text
用户：滑盖和螺丝盖哪个好？
Agent：只分析，不改文件。
```

---

### 14.2 出方案

```text
用户：给我一个滑盖方案，先别动模型。
Agent：生成 plans/2026050100-slide-lid/{request.md,plan.md,plan-result.md}。
```

---

### 14.3 执行

```text
用户：在 Agent mode 运行这个滑盖 plan。
Agent：
- 创建 / 修改对应 .py
- 更新对应 .md
- 调用 CadQuery
- 输出预览和模型文件
- 更新 plans/<id>/plan-result.md
```

---

### 14.4 Viewer 选择后修改

```text
用户在 Viewer 选择 top_lid 的一个面。
系统把 @face[top_lid:top_surface] 传给 Agent。

用户：在这个面上开一个孔。
Agent：基于该 ref 生成 Plan 或直接执行。
```

---

### 14.5 多 Chat 探索

```text
用户：我想另开一个更硬朗的版本。
系统：
- 新建 Chat
- 新建 Plan
- 如需执行，复制相关文件形成实验版本
```

---

## 15. MVP 验收标准

MVP 完成后应满足：

```text
1. 一个 Project 可以有多个 Chat。
2. Chat 可以关联相关 component / part / assembly 文件。
3. 每个 component / part / assembly 都有独立 .py 和 .md。
4. Agent 支持 Agent / Plan 两个模式。
5. Plan mode 不修改 CAD 源文件，不生成 outputs，只创建 plan package。
6. Plan 生成 `plans/YYYYmmddnn-name/{request.md,plan.md,plan-result.md}`。
7. Agent mode 能调用 CadQuery 工具生成模型。
8. Viewer 能显示生成模型。
9. Viewer 能选择 component / part / assembly。
10. Viewer 能选择 face / edge / vertex。
11. Viewer 选择结果能传给 Agent。
12. Agent 能基于 ref 继续讨论、计划或执行。
13. 输出产物进入 outputs/。
14. Agent 回复中说明改了哪些文件和生成了哪些产物。
```

---

## 16. MVP 成功定义

MVP 成功不是生成完美工业模型。

MVP 成功是跑通闭环：

```text
用户多 Chat 讨论
→ Agent 生成 workspace plan package
→ Agent mode 执行 plan
→ CadQuery 生成模型
→ Viewer 选择模型局部
→ Agent 基于选择继续修改
→ 文件系统记录模型、说明和产物
```

---

## 17. 一句话总结

```text
MVP 用文件系统做 source of truth：
.py 负责模型，
.md 负责用途和装配说明，
Chat 负责讨论上下文，
CadQuery 负责生成和修改，
Viewer Ref 负责把用户选择交给 Agent。
```
