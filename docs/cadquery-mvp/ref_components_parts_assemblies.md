# PRD: Ref 系统与 Components / Parts / Assemblies 逻辑关系

## 1. 目标

本 PRD 定义 MVP 中 **用户在 Viewer 选择对象后，Agent 如何理解并映射到 CadQuery Python 文件**，以及 `Components / Parts / Assemblies` 三类对象之间的关系。

核心目标：

```text
Viewer 选择对象
→ 生成 Ref
→ Agent 定位到对应文件和语义对象
→ Agent 讨论 / 出 Plan / 执行修改
→ CadQuery 重新生成模型
```

---

## 2. 核心原则

```text
1. 文件系统是 source of truth。
2. .py 负责模型生成。
3. .md 负责用途、装配、接口、可编辑范围和 Ref Map。
4. Viewer 选择结果必须能映射回 component / part / assembly / feature，raw face / edge / vertex 只作为当前 build 内的精细定位。
5. 优先选择 component / part / assembly / feature。
6. face / edge / vertex 只作为精细定位和兜底。
7. Agent 修改模型时只能改源文件，不直接改 artifact。
8. 用户只是讨论时不改文件；用户要方案时输出 Plan；用户确认后才执行。
```

---

## 3. Components / Parts / Assemblies 定义

### 3.1 Component

Component 是项目中被引用、被适配、被装配的对象。

典型包括：

```text
PCB
开发板
电池
屏幕
连接器
螺丝
铜螺母
传感器模块
用户已有的外部零件
标准件 / 外购件
```

Component 的作用：

```text
提供尺寸、接口、占位、装配约束。
```

Component 默认不是 Agent 要设计制造的对象。Agent 不应随意修改 Component 的真实尺寸，除非用户明确说明该 Component 是可设计对象。

文件：

```text
components/<component_id>.py
components/<component_id>.md
```

---

### 3.2 Part

Part 是 Agent 设计出来、用户可能制造的零件。

典型包括：

```text
上盖
底壳
支架
按钮帽
电池盖
夹具
安装板
转接件
```

Part 的作用：

```text
被制造、被打印、被加工。
```

Part 可以根据 Component 的尺寸和接口设计。

文件：

```text
parts/<part_id>.py
parts/<part_id>.md
```

---

### 3.3 Assembly

Assembly 是多个 Part 和 Component 的组合关系。

Assembly 的作用：

```text
描述谁和谁装在一起、怎么装、相对位置是什么、是否有干涉。
```

Assembly 主要负责：

```text
组合 Part
放置 Component
定义装配关系
记录装配顺序
检查干涉和间隙
输出整体预览
```

文件：

```text
assemblies/<assembly_id>.py
assemblies/<assembly_id>.md
```

---

## 4. 三者逻辑关系

```text
Component = 被适配 / 被引用的对象
Part      = 被设计 / 被制造的对象
Assembly  = 把 Component 和 Part 组合起来的对象
```

关系方向：

```text
Component 提供尺寸 / 接口 / 约束
Part 根据 Component 设计
Assembly 组合 Component + Part
```

示例：

```text
components/
  pcb_main
  usb_connector
  m2_5_screw

parts/
  bottom_case
  top_lid

assemblies/
  full_enclosure
```

对应关系：

```text
bottom_case 适配 pcb_main
bottom_case 提供 PCB 安装结构
top_lid 和 bottom_case 形成盖合关系
usb_connector 决定外壳开孔位置
m2_5_screw 决定螺丝孔和柱结构
full_enclosure 组合 bottom_case + top_lid + pcb_main + screws
```

---

## 5. 依赖方向规则

MVP 中依赖方向保持简单：

```text
Component 不依赖 Part
Component 不依赖 Assembly

Part 可以参考 Component
Part 不应该依赖 Assembly

Assembly 可以引用 Component
Assembly 可以引用 Part
```

允许的引用方向（A → B 表示 A 可以 import/参考 B）：

```text
Part → Component
Assembly → Component
Assembly → Part
```

避免：

```text
Assembly 里的逻辑反向污染 Part
Part 文件里写死某个 Assembly 的位置
Component 被某个 Part 私自改掉真实尺寸
```

---

## 6. 文件职责

### 6.1 Component `.md`

必须包含：

```md
# <component_id>

## Purpose
这个 component 是什么。

## Type
外购件 / 用户提供 / 参考件 / 可设计组件。

## Key Dimensions
关键尺寸。

## Interfaces
接口、孔位、连接器、安装面等。

## Used By
依赖它的 parts / assemblies。

## Edit Policy
Agent 是否允许修改真实尺寸。

## Ref Map
该 component 支持的 ref。
```

---

### 6.2 Part `.md`

必须包含：

```md
# <part_id>

## Purpose
这个 Part 的用途。

## Depends On
它依赖哪些 components。

## Used In
它属于哪些 assemblies。

## Interfaces
与其他 part / component 的配合关系。

## Editable Areas
允许修改的区域。

## Protected Areas
不应随意修改的区域。

## Manufacturing Assumptions
制造假设。

## Ref Map
该 part 支持的 ref。
```

---

### 6.3 Assembly `.md`

必须包含：

```md
# <assembly_id>

## Purpose
这个装配体的用途。

## Contains
包含哪些 parts 和 components。

## Assembly Relationships
相对位置、配合关系、约束。

## Assembly Order
装配顺序。

## Clearance / Interference Notes
间隙和干涉风险。

## Export Targets
导出目标。

## Ref Map
该 assembly 支持的 ref。
```

---

## 7. Ref 类型

> **MVP 范围决策（2026-04-27）**：MVP 实现 5 层 Ref：
> - component/part/assembly（§7.1-7.3）
> - instance（§7.4）
> - feature（§7.5）
> - face/edge/vertex（§7.6）
>
> **MVP 不实现**（后续按需加回）：
> - selector 独立 Ref 层 — Agent / runner 内部仍可用 CadQuery selector 查找 face，但不暴露给用户和协议
> - subshape 独立 Ref 层 — 与 feature 功能重叠，MVP 用 feature 覆盖

### 7.1 Component Ref

用于选择外部组件或可复用组件。

格式：

```text
@component[pcb_main]
@component[usb_connector]
```

映射：

```text
components/pcb_main.py
components/pcb_main.md
```

典型用户意图：

```text
移动 PCB
查看 USB 连接器位置
检查某个标准件是否干涉
```

注意：

```text
用户选中 component 后说“移动它”，通常是 Assembly placement 修改，不是修改 component 本体。
```

---

### 7.2 Part Ref

用于选择可制造零件。

格式：

```text
@part[top_lid]
@part[bottom_case]
```

映射：

```text
parts/top_lid.py
parts/top_lid.md
```

典型用户意图：

```text
加厚这个零件
改这个零件的外形
在这个零件上开孔
```

---

### 7.3 Assembly Ref

用于选择装配体。

格式：

```text
@assembly[full_enclosure]
```

映射：

```text
assemblies/full_enclosure.py
assemblies/full_enclosure.md
```

典型用户意图：

```text
让整体更紧凑
检查是否干涉
导出整个装配
移动某个组件相对位置
```

---

### 7.4 Feature Ref

用于选择语义特征。

格式：

```text
@feature[top_lid.outer_shell]
@feature[top_lid.top_surface]
@feature[bottom_case.pcb_mount_area]
```

Feature Ref 是精细修改的优先入口，比 raw face / edge 更稳定。

映射：

```text
parts/top_lid.py
REFS["features"]["top_surface"]
```

---

### 7.6 Raw Geometry Ref

用于 Viewer 直接选中的底层几何对象。

格式：

```text
@face[top_lid:f_123]
@edge[top_lid:e_456]
@vertex[top_lid:v_789]
```

使用规则：

```text
只作为当前 artifact 的精细定位。
可能随着重新生成失效。
Agent 应优先尝试映射到 feature；若不能稳定映射，保留 raw geometry ref 并要求用户确认风险。
```

---

## 8. Ref 处理优先级

Agent 收到 Ref 后按以下优先级处理：

```text
1. @component / @part / @assembly
2. @feature
3. @face / @edge / @vertex
```

原因：

```text
component / part / assembly / feature 最稳定。
face / edge / vertex 最容易失效。
selector 只作为 runner 内部查找手段，不作为用户可见 Ref。
```

---

## 9. CadQuery Python 文件约定

### 9.1 Part 文件约定

示例：

```python
# parts/top_lid.py

import cadquery as cq

REFS = {
    "part": "top_lid",
    "features": {
        "outer_shell": {
            "description": "Main outer shell of the top lid"
        },
        "top_surface": {
            "selector": 'faces(">Z")',
            "description": "Top planar face"
        },
        "outer_edges": {
            "selector": 'edges("|Z")',
            "description": "Vertical outer edges"
        }
    }
}

def build(params=None):
    params = params or {}

    width = params.get("width", 80)
    length = params.get("length", 60)
    height = params.get("height", 8)

    lid = (
        cq.Workplane("XY")
        .box(width, length, height)
        .tag("outer_shell")
    )

    return lid
```

要求：

```text
文件名就是 part id。
build() 返回 CadQuery 对象。
REFS 描述可选中的 feature。
重要 feature 必须有 tag 或 selector 描述。
```

---

### 9.2 Component 文件约定

示例：

```python
# components/pcb_main.py

import cadquery as cq

REFS = {
    "component": "pcb_main",
    "features": {
        "board_body": {
            "description": "PCB board body"
        },
        "mounting_holes": {
            "description": "PCB mounting hole positions"
        }
    }
}

def build(params=None):
    params = params or {}

    width = params.get("width", 80)
    length = params.get("length", 60)
    thickness = params.get("thickness", 1.6)

    pcb = (
        cq.Workplane("XY")
        .box(width, length, thickness)
        .tag("board_body")
    )

    return pcb
```

---

### 9.3 Assembly 文件约定

示例：

```python
# assemblies/full_enclosure.py

import cadquery as cq

from parts.top_lid import build as build_top_lid
from parts.bottom_case import build as build_bottom_case
from components.pcb_main import build as build_pcb

REFS = {
    "assembly": "full_enclosure",
    "children": [
        "top_lid",
        "bottom_case",
        "pcb_main"
    ]
}

def build(params=None):
    params = params or {}

    top_lid = build_top_lid(params.get("top_lid"))
    bottom_case = build_bottom_case(params.get("bottom_case"))
    pcb = build_pcb(params.get("pcb_main"))

    assembly = cq.Assembly(name="full_enclosure")

    assembly.add(
        bottom_case,
        name="bottom_case",
        metadata={"ref_text": "@part[bottom_case]", "object_kind": "part"}
    )

    assembly.add(
        top_lid,
        name="top_lid",
        metadata={"ref_text": "@part[top_lid]", "object_kind": "part"}
    )

    assembly.add(
        pcb,
        name="pcb_main",
        metadata={"ref_text": "@component[pcb_main]", "object_kind": "component"}
    )

    return assembly
```

要求：

```text
Assembly 负责组合，不负责修改 Part 内部几何。
Assembly 中每个 child 必须有 name、`ref_text` 和 `object_kind` metadata。若 CadQuery API 只能稳定保存 `ref` 这类短字段，该字段只能作为 Python metadata 输入别名；runner stdout、protocol payload、SelectionRef 一律归一为 `ref_text`。
```

---

## 10. Ref 到 Python 文件的映射规则

### 10.1 `@component[...]`

Ref：

```text
@component[pcb_main]
```

映射：

```text
components/pcb_main.py
components/pcb_main.md
```

Python 入口：

```python
from components.pcb_main import build
obj = build()
```

---

### 10.2 `@part[...]`

Ref：

```text
@part[top_lid]
```

映射：

```text
parts/top_lid.py
parts/top_lid.md
```

Python 入口：

```python
from parts.top_lid import build
obj = build()
```

---

### 10.3 `@assembly[...]`

Ref：

```text
@assembly[full_enclosure]
```

映射：

```text
assemblies/full_enclosure.py
assemblies/full_enclosure.md
```

Python 入口：

```python
from assemblies.full_enclosure import build
assy = build()
```

---

### 10.4 `@feature[...]`

Ref：

```text
@feature[top_lid.top_surface]
```

映射：

```text
parts/top_lid.py
REFS["features"]["top_surface"]
```

若 feature 有 selector：

```python
obj = build()
target = obj.faces(">Z")
```

若 feature 有 tag：

```python
obj = build()
target = obj._getTagged("top_surface")
```

---

### 10.5 内部 selector candidate（非 Ref）

CadQuery selector 是 Agent / runner 内部用于定位几何的执行手段，不作为 MVP 用户可见 Ref 层，也不写入 SelectionRef 的 `ref_text`。

示例：

```python
from parts.top_lid import build

obj = build()
target_faces = obj.faces(">Z")
target_edges = obj.edges("|Z")
```

raw geometry 选择若能推导出稳定 selector，runner 可以把它作为内部 candidate 附在 feature mapping 结果中；协议层仍返回 feature 或 raw geometry Ref。

---

### 10.6 `@face[...]`

Ref：

```text
@face[top_lid:f_123]
```

处理规则：

```text
1. 查 artifact 中的 face id。
2. 查该 face 是否绑定 feature。
3. 若绑定 feature，转成 @feature[...]。
4. 若不能绑定，生成内部 selector candidate。
5. 若 selector 不唯一，要求用户确认。
```

Agent 不应长期依赖 `f_123`。

---

### 10.7 `@edge[...]`

Ref：

```text
@edge[top_lid:e_456]
```

处理规则：

```text
1. 查 artifact 中的 edge id。
2. 查是否属于 named feature。
3. 若可以，转成 @feature[...]。
4. 若不确定，作为临时 geometry ref，并可携带内部 selector candidate 供 Agent 判断。
```

---

## 11. Viewer 选择返回格式

### 11.1 选择 Component

```json
{
  "kind": "component",
  "ref_text": "@component[pcb_main]",
  "file": "components/pcb_main.py"
}
```

---

### 11.2 选择 Part

```json
{
  "kind": "part",
  "ref_text": "@part[top_lid]",
  "file": "parts/top_lid.py"
}
```

---

### 11.3 选择 Assembly

```json
{
  "kind": "assembly",
  "ref_text": "@assembly[full_enclosure]",
  "file": "assemblies/full_enclosure.py"
}
```

---

### 11.4 选择 Feature

```json
{
  "kind": "feature",
  "ref_text": "@feature[top_lid.top_surface]",
  "file": "parts/top_lid.py",
  "selector": "faces(\">Z\")"
}
```

---

### 11.5 选择 Face

```json
{
  "kind": "face",
  "ref_text": "@face[top_lid:f_123]",
  "owner_ref_text": "@part[top_lid]",
  "owner_object_kind": "part",
  "candidate_feature_ref": "@feature[top_lid.top_surface]",
  "build_id": "sha256:source_params_deps",
  "ambiguous": false
}
```

---

## 12. Agent 判断规则

### 12.1 用户选中 Component

用户选择：

```text
@component[pcb_main]
```

用户说：

```text
把它往下移一点。
```

Agent 判断：

```text
这是 Assembly placement 修改，不是 PCB 本体修改。
```

应修改：

```text
assemblies/full_enclosure.py
assemblies/full_enclosure.md
```

不应修改：

```text
components/pcb_main.py
```

---

### 12.2 用户选中 Part

用户选择：

```text
@part[top_lid]
```

用户说：

```text
把它加厚。
```

Agent 判断：

```text
这是 Part 几何修改。
```

应修改：

```text
parts/top_lid.py
parts/top_lid.md
```

并重新生成相关 Assembly。

---

### 12.3 用户选中 Assembly

用户选择：

```text
@assembly[full_enclosure]
```

用户说：

```text
让整体更紧凑。
```

Agent 判断：

```text
这是装配级目标，可能影响多个 Part 和 Component 的相对关系。
```

应先输出 Plan，不应直接改单个文件。

---

### 12.4 用户选中 Face

用户选择：

```text
@face[top_lid:f_123]
```

用户说：

```text
在这个面上开孔。
```

Agent 判断：

```text
这是 Part 内的精细修改。
```

优先映射到：

```text
parts/top_lid.py
@feature[top_lid.top_surface]
```

---

### 12.5 用户选中 Edge

用户选择：

```text
@edge[top_lid:e_456]
```

用户说：

```text
这条边倒角。
```

Agent 判断：

```text
如果用户明确是一条边，则使用 raw edge，必要时辅以内部联系的 selector candidate。
如果用户想改一组边，则优先查 named feature；没有稳定 feature 时再使用内部 selector candidate 辅助判断。
```

若该边属于 named feature，应优先映射到对应 `@feature[...]`；否则保留 raw edge，并在 Plan 中说明 selector candidate 只是内部查找依据。

---

## 13. Agent Plan 要求

当用户基于 Ref 要求修改时，Plan 必须包含：

```md
## Target Ref
用户选择了什么。

## Resolved Target
该 ref 对应哪个文件、哪个 feature，以及是否需要内部 selector candidate 辅助定位。

## Modification Strategy
准备用 CadQuery 如何修改。

## Affected Files
会改哪些文件。

## Assembly Impact
是否影响 assembly 或其他 part / component。

## Risks
该选择是否稳定，是否可能影响装配。

## Confirmation Needed
是否需要用户确认。
```

---

## 14. MVP 验收标准

MVP 必须满足：

```text
1. Viewer 选择 component 后，Agent 能定位到 component .py 和 .md。
2. Viewer 选择 part 后，Agent 能定位到 part .py 和 .md。
3. Viewer 选择 assembly 后，Agent 能定位到 assembly .py 和 .md。
4. Viewer 选择 face 后，系统能给出 candidate feature；需要 selector 时只作为内部 candidate，不作为 Ref。
5. Viewer 选择 edge 后，系统能给出 raw edge 或 candidate feature；需要 selector 时只作为内部 candidate，不作为 Ref。
6. Agent 能把 @component[...] 映射到 components/<id>.py。
7. Agent 能把 @part[...] 映射到 parts/<id>.py。
8. Agent 能把 @assembly[...] 映射到 assemblies/<id>.py。
9. Agent 能把 @feature[...] 映射到 REFS 条目。
10. Agent 不应长期依赖 raw face / edge id。
11. 每个 .md 必须有 Ref Map。
12. 每个 .py 必须有 REFS。
13. 执行后必须更新对应 .md 的 Ref Map 或说明文档。
```

---

## 15. 最重要原则

```text
Component 描述被适配对象。
Part 描述被制造对象。
Assembly 描述组合关系。

用户优先选 component / part / assembly / feature。
face / edge / vertex 只是精细定位。
CadQuery selector 是执行桥梁。
.py 是模型真相。
.md 是语义说明。
Viewer ref 是用户和 Agent 的共同语言。
```
