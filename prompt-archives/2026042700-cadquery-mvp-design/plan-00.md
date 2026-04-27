# CAD Agent Harness MVP — 完整设计

## Context

budn' 产品方向转向 CadQuery Agent 协作式 CAD 设计。MVP 目标是跑通完整流程：

```
多 Chat 讨论 → Agent 生成 CAD Plan → 用户确认 → CadQuery 生成模型
→ Viewer 查看 → 精细选择 face/edge/vertex → Agent 基于选择继续修改
```

相关文档：
- `docs/cadquery-mvp/init.md` — 产品 MVP 定义
- `docs/cadquery-mvp/ref_components_parts_assemblies.md` — Ref 系统与对象关系
- `docs/cadquery-mvp/decisions.md` — 方向决策记录

当前工程审查结论（2026-04-27）：本 plan 是 CadQuery MVP 实施的最新约束来源。实施前必须先完成 Phase 0a 的规则与文档一致性修正；若 `docs/cadquery-mvp/*` 或 `docs/architecture.md` 中仍有旧表述，以本 plan 与根 `AGENTS.md` 的长期架构约束为准。

## 已确认决策

| 决策点 | 结论 |
|--------|------|
| Python 约束 | 豁免 CadQuery 子进程，视为外部工具（同 OpenSCAD CLI） |
| 产品方向 | CadQuery 替代 OpenSCAD，MVP 期间不删 OpenSCAD 但不再投入 |
| Agent 运行时 | Rust 自建 LLM 抽象层，后端运行，no vendor lock-in |
| B-rep 拓扑 | Python 端输出 topology metadata + mesh + feature mapping |
| MVP 范围 | 必须包含 face/edge/vertex 精细选择 |
| CadQuery 架构 | 复用现有外部工具子进程模式，与 OpenSCAD CLI 同一类 |
| CadQuery 是 tool call | Agent 直接通过 cadquery tool 建模，系统原子完成写入+执行+返回 |
| Python 环境 | MVP 手动安装，分发策略留到产品化阶段 |
| Chat UI 入口 | 复用现有 chat-zone |
| 拓扑稳定性 | CadQuery tag + selector 组合，Python 端做 feature→face 映射 |
| Chat 存储 | JSONL（完整消息日志） |
| Project 概念 | Project = workspace，同一个东西 |
| LLM/Agent 框架 | 先评估 Rig 当前最新兼容版本，贴合需求就用，否则 SDK 客户端自建 |
| Ref 层级 | MVP 5 层（砍 selector/subshape），后续按需加回 |
| venv 分发 | MVP 手动安装，分发策略产品化阶段再定 |
| mesh wire format | 基于现有 Borsh 协议扩展，不另起炉灶 |
| 前端架构 | 基于现有框架增量改造，保持当前 UI，不大改架构 |
| 并发模型 | 限制同时只有一个 running agent session |

---

## 1. 架构

```
Browser (React + Three.js)
    ↕ WebSocket (app-server-protocol)
app-server-host
    ↕ dispatch
app-server-core
    ├── workspace / file I/O          (existing)
    ├── external tool subprocess      (existing OpenSCAD → 扩展 CadQuery)
    ├── llm-provider                  (new) trait 抽象 + 多供应商
    ├── agent-orchestrator            (new) tool use 编排
    └── chat-session                  (new) 多会话管理
        
Python subprocess (budn_cad_runner)
    ← app-server-core 调用
    → JSON result (mesh + topology + feature_map + exports + metadata)
```

CadQuery 与 OpenSCAD 在架构上是同一类东西——外部工具子进程。但现有 `preview.rs` 是 OpenSCAD 专用分支（只支持 .stl/.3mf/.scad，dispatcher 直连 `preview_ready_response`），没有通用 external tool 抽象。CadQuery 需要在 `app-server-core/src/cadquery/` 新建子进程调用模块，参考但不复用 OpenSCAD 的代码路径。

---

## 2. Python 端执行框架 (budn_cad_runner)

### 2.1 定位

Python package，MVP 阶段需手动安装 Python + CadQuery 环境。app-server-core 通过子进程调用。分发策略（bundled venv / 自动安装）留到产品化阶段。

### 2.2 调用接口

```bash
python3 -m budn_cad_runner \
    --script parts/top_lid.py \
    --project-root /path/to/project \
    --output-dir outputs/ \
    --exports step,stl \
    --params '{"width": 80}'

# stdout: JSON result
# stderr: warnings / errors
# exit code: 0 success, 1 build error, 2 runner error
```

### 2.3 包结构

```
budn_cad_runner/
├── __main__.py        CLI 入口
├── loader.py          加载 .py 模块，提取 REFS dict + build()
├── executor.py        调用 build(params)，捕获异常
├── tessellator.py     带拓扑追踪的 tessellation
├── ref_mapper.py      REFS features → face/edge 映射 + 自动 selector 推导
├── selector_parser.py selector 字符串白名单解析（禁止 eval）
├── exporter.py        STEP / STL / 3MF 导出
├── metadata.py        bounding box / volume / surface area
├── manifest.py        输出 manifest 生成
└── schema.py          输出 JSON schema & 序列化
```

### 2.4 执行 pipeline

```
loader.load(script_path, project_root)
  → module, refs_dict, build_fn
  → 设置 sys.path = [project_root]，支持跨目录 import

executor.run(build_fn, params)
  → cq_object (Workplane | Assembly)

tessellator.tessellate(cq_object)
  → TessellationResult { faces[], edges[], vertices[] }
  → 根据 cq_object 类型分发：Workplane→val().wrapped, Assembly→递归 CadQuery child 对象，最终归一输出 parts[]

ref_mapper.map_features(cq_object, refs_dict, tessellation)
  → FeatureMap { features→face_indices, face→features, candidate_selectors }
  → 歧义检测：candidate_selector 匹配多个 face 时标记 ambiguous=true

exporter.export(cq_object, output_dir, formats)
  → export paths

metadata.compute(cq_object)
  → bounding_box, volume, surface_area

manifest.generate(script_path, params, exports, metadata, dependencies)
  → 输出 manifest（source hash, params hash, dependencies, deps hash, runner version, timestamp）

schema.serialize(all above)
  → JSON stdout
```

### 2.5 tessellator — 带拓扑追踪

OpenCASCADE 的 BRepMesh 给每个 TopoDS_Face 生成独立三角化。

**入口分发**（不同 CadQuery 对象类型）：

```python
def get_shape(cq_object):
    """从不同 CadQuery 对象类型提取 TopoDS_Shape"""
    if hasattr(cq_object, 'val'):
        # Workplane → 取最终 Shape
        return cq_object.val().wrapped
    elif hasattr(cq_object, 'wrapped'):
        # CadQuery Shape 对象
        return cq_object.wrapped
    elif hasattr(cq_object, 'toCompound'):
        # Assembly → 转为 Compound（不含 location，需要单独处理）
        raise ValueError("Assembly must use per-child tessellation")
    else:
        raise TypeError(f"Unsupported CadQuery object: {type(cq_object)}")
```

**单体 tessellation**：

```python
from OCP.BRepMesh import BRepMesh_IncrementalMesh
from OCP.TopExp import TopExp_Explorer
from OCP.TopAbs import TopAbs_FACE
from OCP.BRep import BRep_Tool
from OCP.TopLoc import TopLoc_Location

def tessellate_shape(shape, linear_deflection=0.1, angular_deflection=0.5):
    BRepMesh_IncrementalMesh(shape, linear_deflection, False, angular_deflection, True)
    
    faces = []
    explorer = TopExp_Explorer(shape, TopAbs_FACE)
    face_idx = 0
    while explorer.More():
        topo_face = explorer.Current()
        loc = TopLoc_Location()
        tri = BRep_Tool.Triangulation_s(topo_face, loc)
        if tri:
            faces.append(TessFace(
                face_idx=face_idx,
                triangles=extract_triangles(tri, loc),
                normal=compute_face_normal(topo_face),
                topo_face_ref=topo_face  # 保留引用，供 ref_mapper 做 IsSame 比较
            ))
        face_idx += 1
        explorer.Next()
    
    edges = extract_edges(shape)
    vertices = extract_vertices(shape)
    return TessellationResult(faces, edges, vertices)
```

face_idx 是单次 build 内确定的遍历序号（跨 build 不稳定），用于 feature_map 索引和前端 face group 渲染。

### 2.6 selector_parser — 白名单解析（禁止 eval）

selector 字符串不使用 `eval()`。使用白名单解析器：

```python
import re

ALLOWED_METHODS = {"faces", "edges", "vertices", "wires", "shells", "solids"}
SELECTOR_PATTERN = re.compile(r'^(faces|edges|vertices|wires|shells|solids)\("([^"]+)"\)$')

def parse_selector(selector_str):
    """解析 selector 字符串，返回 (method, expression) 或抛异常"""
    match = SELECTOR_PATTERN.match(selector_str)
    if not match:
        raise ValueError(f"Invalid selector: {selector_str}")
    method = match.group(1)
    expression = match.group(2)
    validate_selector_expression(expression)
    return method, expression

def evaluate_selector(cq_object, selector_str):
    """安全执行 selector，不使用 eval"""
    method, expression = parse_selector(selector_str)
    selector_fn = getattr(cq_object, method)
    return selector_fn(expression)
```

### 2.7 ref_mapper — feature 映射 + 歧义检测

两个职责：

**显式映射**：遍历 REFS dict 中声明的 features，用其 selector/tag 找到对应的 face_indices。

```python
def map_features(cq_object, refs_dict, tessellation):
    feature_map = {}
    face_to_features = {}
    
    for name, defn in refs_dict.get("features", {}).items():
        indices = []
        if defn.get("selector"):
            selected = evaluate_selector(cq_object, defn["selector"])
            indices = match_faces(selected, tessellation)
        elif defn.get("tag"):
            tagged = cq_object._getTagged(defn["tag"])
            indices = match_faces_from_workplane(tagged, tessellation)
        
        feature_map[name] = { "face_indices": indices, ... }
        for i in indices:
            face_to_features.setdefault(i, []).append(name)
    
    return FeatureMap(feature_map, face_to_features)
```

**自动推导 + 歧义检测**：

```python
def infer_candidate_selectors(face, tessellation, shape_bbox):
    candidates = []
    ambiguous = False
    
    # 方向推导
    if is_axis_aligned(face.normal, [0,0,1], tolerance=0.01):
        selector = ">Z" if is_at_max(face, shape_bbox, "Z") else "<Z"
        # 检查是否有其他 face 也匹配这个 selector
        matching_count = count_faces_matching(tessellation, selector)
        if matching_count > 1:
            ambiguous = True
        candidates.append({"selector": selector, "ambiguous": ambiguous})
    
    return candidates
```

当 `ambiguous=true` 时，前端 Viewer 应提示用户确认而不是直接使用该 selector。

### 2.8 Assembly 处理

Assembly 的 build() 返回 `cq.Assembly`。**逐 child 带 instance path 处理**：

```python
def tessellate_assembly(assembly, instance_path=""):
    parts = []
    for name, child in assembly.objects.items():
        child_path = f"{instance_path}/{name}" if instance_path else name
        
        if isinstance(child, cq.Assembly):
            # 递归子 assembly
            nested = tessellate_assembly(child, child_path)
            parts.extend(nested["parts"])
        else:
            shape = child.obj
            loc = child.loc
            transform = location_to_matrix(loc)  # 4x4 transform
            mesh = tessellate_shape(shape.wrapped)
            
            parts.append({
                "name": name,
                "instance_path": child_path,  # 如 "full_enclosure/top_lid"
                "object_kind": resolve_child_object_kind(child),
                "ref_text": resolve_child_ref_text(child, child_path),
                "transform": transform,
                "mesh": mesh,
                "feature_map": map_features(shape, get_child_refs(child), mesh)
            })
    
    return {
        "assembly": assembly.name,
        "root_object_kind": "assembly",
        "root_ref_text": resolve_assembly_ref_text(assembly),
        "parts": parts
    }
```

instance_path 解决同一 part 多次出现的问题（如多个螺丝）。

### 2.9 输出 JSON Schema

runner stdout 统一使用一个 schema：无论单体 Part 还是 Assembly，都输出 `parts[]`。单体模型是一个 part；Assembly 是多个带 `instance_path` / `transform` 的 part。Rust 端只维护这一条解析路径。

```json
{
  "status": "success | build_error | runner_error",
  "error": null,
  "error_type": null,
  "result_id": "cq_abc123",
  "build_id": "sha256:source_params_deps...",
  "unit": "millimeter",
  "root_ref_text": "@part[top_lid]",
  "root_object_kind": "part",

  "parts": [
    {
      "name": "top_lid",
      "object_kind": "part",
      "ref_text": "@part[top_lid]",
      "instance_path": null,
      "transform": null,
      "refs": {
        "part": "top_lid",
        "features": {
          "outer_shell": { "description": "...", "tag": "outer_shell" },
          "top_surface": { "description": "...", "selector": "faces(\">Z\")" }
        }
      },
      "mesh": {
        "faces": [
          {
            "face_idx": 0,
            "positions": [/* Float32 flat xyz */],
            "normals": [/* Float32 flat xyz */],
            "normal": [0.0, 0.0, 1.0],
            "features": ["top_surface"],
            "ambiguous": false,
            "candidate_selectors": [{"selector": ">Z", "ambiguous": false}]
          }
        ],
        "edges": [
          { "edge_idx": 0, "polyline": [/* Float32 flat xyz */], "adjacent_faces": [0, 1] }
        ],
        "vertices": [
          { "vertex_idx": 0, "position": [0.0, 0.0, 0.0], "adjacent_edges": [0, 1] }
        ]
      },
      "feature_map": {
        "top_surface": { "face_indices": [0], "selector": "faces(\">Z\")" },
        "outer_shell": { "face_indices": [0, 1, 2, 3, 4, 5], "tag": "outer_shell" }
      }
    }
  ],

  "exports": { "step": "path", "stl": "path" },

  "metadata": {
    "bounding_box": { "min": [0,0,0], "max": [80,60,8] },
    "volume": 38400.0,
    "surface_area": 12960.0
  },

  "manifest": {
    "source_path": "parts/top_lid.py",
    "source_hash": "sha256:abc123...",
    "params": {"width": 80},
    "params_hash": "sha256:params123...",
    "dependencies": [
      { "path": "parts/top_lid.py", "hash": "sha256:abc123..." },
      { "path": "components/pcb_main.py", "hash": "sha256:dep456..." }
    ],
    "deps_hash": "sha256:deps789...",
    "runner_version": "0.1.0",
    "timestamp": "2026-04-27T12:00:00Z",
    "export_hashes": {
      "step": "sha256:def456...",
      "stl": "sha256:ghi789..."
    }
  }
}
```

Assembly 也使用同一 schema，只是 `parts[]` 中的每个输出 part 带 `instance_path` 和 `transform`：

```json
{
  "status": "success",
  "result_id": "cq_full_enclosure",
  "build_id": "sha256:source_params_deps...",
  "unit": "millimeter",
  "root_ref_text": "@assembly[full_enclosure]",
  "root_object_kind": "assembly",
  "parts": [
    {
      "name": "bottom_case",
      "instance_path": "full_enclosure/bottom_case",
      "object_kind": "part",
      "ref_text": "@part[bottom_case]",
      "transform": [4x4],
      "mesh": {...},
      "feature_map": {...}
    },
    {
      "name": "m2_5_screw_1",
      "instance_path": "full_enclosure/m2_5_screw_1",
      "object_kind": "component",
      "ref_text": "@component[m2_5_screw]",
      "transform": [4x4],
      "mesh": {...},
      "feature_map": {...}
    },
    {
      "name": "m2_5_screw_2",
      "instance_path": "full_enclosure/m2_5_screw_2",
      "object_kind": "component",
      "ref_text": "@component[m2_5_screw]",
      "transform": [4x4],
      "mesh": {...},
      "feature_map": {...}
    }
  ],
  "exports": {...},
  "metadata": {...},
  "manifest": {...}
}
```

`build_id` 必须由 `source_hash + params_hash + deps_hash` 组合生成；任一 import 依赖文件变化都必须生成新的 `build_id`，用于让旧 selection 失效。
`root_ref_text` / `root_object_kind` 描述本次 build 的根对象，用于 assembly / part 整体选择；每个 `parts[]` 元素必须携带 `object_kind` / `ref_text`，用于 part / component 实例整体选择和 raw geometry 选择的上级归属。前端不得从 `name` 或 `instance_path` 反推对象 Ref。
runner JSON 中的 `source_path`、`dependencies[].path`、`exports` 是 project-root 相对展示路径。进入 app-server protocol response 或后续工具调用前，Rust 端必须把它们解析为 `PathHandle` 并重新校验。

### 2.10 mesh 传输

Python runner stdout 输出普通 JSON；数组字段使用 JSON number array，不使用 base64 typed array。理由是 MVP 优先保证可调试性和 schema 可读性；大 payload 优化留到后续。Rust 端解析 JSON 后校验：

- 所有浮点数必须是 finite `f32`。
- `positions` / `normals` / `polyline` 等扁平数组长度必须满足 3 的倍数。
- face / edge / vertex 索引必须在当前 part 范围内。
- `unit` 必须是 `"millimeter"`，坐标系必须符合 `docs/architecture.md` 的右手系、Top plane = `XY` 契约。

校验通过后，Rust 端转为 Borsh 编码的 `CadQueryMeshPayload`，通过现有 WebSocket binary frame 传到前端。wire format 基于现有 Borsh 协议扩展，与 `PreviewMeshPayload` 并列，不另起炉灶。

### 2.11 Python 模块导入策略

```python
# loader.py
import sys

def load(script_path, project_root):
    # project_root 加入 sys.path 头部，支持跨目录 import
    # 如 parts/top_lid.py 可以 from components.pcb_main import build
    if project_root not in sys.path:
        sys.path.insert(0, project_root)
    
    # 用 importlib 加载，避免模块名冲突
    spec = importlib.util.spec_from_file_location(
        f"budn_project.{script_path.stem}",  # 命名空间隔离
        script_path
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    
    refs_dict = getattr(module, "REFS", {})
    build_fn = getattr(module, "build", None)
    if build_fn is None:
        raise RunnerError("module has no build() function")
    
    return module, refs_dict, build_fn
```

每次子进程调用是独立 Python 进程，不存在模块缓存污染。project_root 在 sys.path 头部确保项目内 import 优先。

---

## 3. CadQuery Python 文件约定

每个 .py 文件遵循统一约定（来自 Ref PRD）：

```python
import cadquery as cq

REFS = {
    "part": "top_lid",  # 或 "component" / "assembly"
    "features": {
        "outer_shell": { "description": "Main shell", "tag": "outer_shell" },
        "top_surface": { "description": "Top face", "selector": 'faces(">Z")' }
    }
}

def build(params=None):
    params = params or {}
    width = params.get("width", 80)
    height = params.get("height", 8)
    
    result = (
        cq.Workplane("XY")
        .box(width, 60, height)
        .tag("outer_shell")
    )
    return result
```

- `REFS` dict 声明可选中的 features（tag 或 selector）
- `build(params)` 返回 CadQuery Workplane 或 Assembly
- 文件名 = 对象 id（top_lid.py → @part[top_lid]）
- Agent 生成/修改代码时负责维护 REFS 和 tag

---

## 4. Ref 系统

MVP 实现 5 层 Ref（砍掉 selector 和 subshape），按稳定性递减：

| 层级 | 格式 | 稳定性 | 用途 |
|------|------|--------|------|
| component/part/assembly | `@part[top_lid]` | 最高 | 对象级选择 |
| instance | `@instance[full_enclosure/m2_5_screw_1]` | 最高 | assembly 内实例（同 part 多次出现） |
| feature | `@feature[top_lid.top_surface]` | 高 | 语义特征，映射 REFS dict |
| face/edge/vertex | `@face[top_lid:f_123]` | 低 | raw geometry，可能跨 build 失效 |

MVP 不实现的层级（后续按需加回）：
- ~~selector (`@selector[top_lid@faces@>Z]`)~~ — Agent 内部用 selector 查找 face，但不暴露为独立 Ref 层
- ~~subshape (`@subshape[top_lid.top_surface]`)~~ — 与 feature 功能重叠，MVP 用 feature 覆盖

**instance ref**：解决同一 part 在 assembly 中多次出现的身份问题。用户选中某个螺丝时，ref 为 `@instance[full_enclosure/m2_5_screw_1]`，Agent 据此知道要修改 assembly placement（而非源文件）。

**处理优先级**：Agent 收到 raw ref 时主动向上映射到 feature ref。Viewer 选择返回携带 `candidate_feature_ref`。当 `ambiguous=true` 时，Agent 应要求用户确认。

**拓扑稳定性方案**：CadQuery `.tag()` + selector 组合。tag 在代码中，selector 是几何语义（如 ">Z" = 顶面），两者跨模型修改都稳定。Python 端 ref_mapper 负责建立 feature→face 映射，随 mesh 一起返回给前端。

### 4.1 Ref 解析与修改目标的业务规则

Agent 收到 Ref 后，必须根据 Ref 类型和用户意图确定**修改哪个文件**：

| 用户选择 | 用户意图 | 修改目标 | 不应修改 |
|----------|---------|---------|---------|
| @component[pcb] + "移动它" | Assembly placement | assemblies/*.py | components/pcb.py |
| @component[pcb] + "改尺寸" | Component 本体 | components/pcb.py（需确认 edit policy） | — |
| @part[top_lid] + "加厚" | Part 几何 | parts/top_lid.py + .md | — |
| @instance[assy/screw_1] + "移动" | Assembly placement | assemblies/assy.py | components/screw.py |
| @instance[assy/screw_1] + "换型号" | Component 替换 | assemblies/assy.py + components/ | — |
| @assembly[full] + "更紧凑" | 多文件协调 | 先输出 Plan | 不直接改单文件 |
| @face[lid:f_0] + "开孔" | Part 精细修改 | parts/lid.py | — |

Agent 在 Execute 前必须声明修改目标文件列表，用户确认后才执行。

---

## 5. Agent Tool Call

核心原则：**CadQuery 本身是 tool call**，系统原子完成「写入 .py → 执行 → 收集结果 → 返回」。

### 5.1 工具列表

**建模**

| 工具 | 参数 | 说明 |
|------|------|------|
| `cadquery` | target_path: PathHandle, target_type, code, export_formats | 写入 + 执行 + 返回 mesh/feature_map/exports |
| `cadquery_preview` | target_path: PathHandle, target_type | 只执行已有 .py，不修改代码 |

**文件**

| 工具 | 参数 | 说明 |
|------|------|------|
| `read_file` | path: PathHandle, line_range? | 读取项目文件 |
| `write_file` | path: PathHandle, content | 写入 .md / plans（不用于 .py） |
| `edit_file` | path: PathHandle, edits[] | 编辑 .md 文档 |
| `list_directory` | path: Option<PathHandle>, recursive?, pattern? | 列目录 |
| `search_files` | query, path: Option<PathHandle>, file_pattern? | 搜索文件内容 |

**上下文**

| 工具 | 参数 | 说明 |
|------|------|------|
| `get_selection` | — | 获取 Viewer 当前选择（含 candidate refs） |
| `get_project_context` | — | 项目全貌：components/parts/assemblies/plans/selection |

工具参数中的 `target_path` / `path` 在协议层必须是 `PathHandle` / `WorkspacePortablePath`，不能是未校验字符串。Agent 文本、Chat JSONL、Plan 文档中可以保存展示路径（如 `parts/top_lid.py`），但任何会触发 I/O、写入、CadQuery 执行或导出的 command 都必须由 app server 使用协议路径模型解析和校验后执行。新增 CadQuery / Chat / Selection 命令不得绕过 `docs/2026042500-cross-platform-path-policy/README.md` 的路径策略。

### 5.2 Operation Level 权限

| 级别 | 允许的工具 | 触发条件 |
|------|-----------|---------|
| Inform | read_file, list_directory, search_files, get_selection, get_project_context, cadquery_preview | 用户只是讨论/提问 |
| Plan | 同 Inform + write_file（仅 plans/） | 用户要方案 |
| Execute | 全部 | 用户确认执行 |

### 5.3 Execute 确认机制

Execute 级别需要结构化确认，不是模糊的"执行吧"：

```
Agent 输出 Plan 后，用户确认时系统记录：
- plan_ref: 对应 plans/*.md 的 PathHandle + 展示路径
- affected_files: 将被修改文件的 PathHandle 列表 + 展示路径
- new_files: 将被创建文件的 PathHandle 列表 + 展示路径
- export_targets: 将生成输出文件的 PathHandle 列表 + 展示路径
```

Agent 执行时只能在确认范围内操作。超出范围需要重新确认。

### 5.4 cadquery tool 原子性保障

cadquery tool call 的原子性通过 staging 目录保障（避免 .tmp 后缀导致 `__file__`/import 路径不一致）：

```
1. `CadQueryExecute` request 携带目标文件 `PathHandle`，app-server-core 先解析到 workspace 内真实路径，并执行路径合法性与 symlink escape 校验。
2. 记录目标文件当前 hash/mtime（冲突检测基线）
3. Agent 提交 code → 系统在 .budn_staging/<uuid>/ 内镜像目标路径写入文件
   例：.budn_staging/abc123/parts/top_lid.py
4. 执行 budn_cad_runner --script parts/top_lid.py --project-root .budn_staging/abc123/
   （staging 目录包含原 project 文件的 symlink/copy，确保 import 和 __file__ 正确）
5. 执行成功 → 回写前重新检查目标文件 hash/mtime，不一致则返回 file_conflict 错误
6. 检查通过 → 将 staging 中修改的文件 copy 回真实路径，写入 outputs
7. 执行失败 → 清理 staging，返回错误（含 Python traceback），原文件不变
8. 进程超时 → kill 子进程，清理 staging，返回 timeout 错误
9. 清理 staging 目录
```

超时默认 60 秒（可配置）。输出目录也先写临时位置再移动。

### 5.5 关键约定

- .py 代码只通过 `cadquery` tool 写入，不走 write_file
- 修改现有模型前必须 read_file 读当前 .py
- CadQuery 执行错误含 Python traceback，Agent 可修正重试
- Agent 文本流式传输，工具调用作为结构化事件插入流中
- CadQuery 执行结果触发 Viewer 自动刷新

---

## 6. Rust 后端新增模块

新模块全部放在 `app-server-core` 内（作为子模块），不新增 crate。理由：MVP 阶段模块边界不清晰，过早拆 crate 会增加 workspace 管理负担。稳定后再根据依赖关系考虑拆分。

```
crates/app-server-core/src/
├── cadquery/          (new) CadQuery 子进程调用 + staging
├── llm/               (new) LLM provider trait + 实现
├── agent/             (new) orchestrator + tool dispatch
└── chat/              (new) session 管理
```

`crates/app-server-protocol/` 新增 CadQuery 和 Agent 相关的 command/event 类型。

### 6.1 llm-provider + agent-orchestrator

**优先方案：评估 Rig 当前最新兼容版本**

Rig 是 Rust 生态较成熟的 LLM 框架，内置：
- 20+ model provider（含 Anthropic、OpenAI）
- tool use / function calling
- streaming
- Agent 抽象

Phase 1 开始前先按 crates.io / docs.rs 当前版本验证 Rig 的 tool use API 和 streaming 是否满足需求（特别是自定义 tool 注册和 Agent loop 控制）。不固定旧版本号；除非评估发现最新版存在阻断问题，否则优先使用当前最新兼容版本。如果 Rig 抽象贴合，直接用；如果太受限，退回 SDK 客户端（anthropic-sdk-rust + async-openai）+ 自建薄 provider trait。

**备选方案：SDK + 自建**

```
LlmProvider trait
├── complete(messages, tools, config) -> Stream<CompletionChunk>
├── 支持 tool use / function calling
└── 流式响应

实现：
├── anthropic-sdk-rust → AnthropicProvider
└── async-openai → OpenAiCompatibleProvider
```

### 6.2 agent-orchestrator

实现 Inform / Plan / Execute 三级行为模型。

```
Agent Loop:
1. Resolve Context — get_project_context + get_selection
2. Classify Operation — 判断 Inform / Plan / Execute
3. Read Files — 读取相关 .py 和 .md
4. Act — 根据级别调用工具
5. Reply — 流式返回结果 + 说明改了什么 + 下一步建议
```

无状态，上下文由 Chat session 提供。

### 6.3 chat-session

多 Chat 会话管理，JSONL 存储（文件系统 source of truth）。

```
chats/main.jsonl
chats/lid-discussion.jsonl

每行一条消息记录：
{"ts":"...","role":"user","content":"..."}
{"ts":"...","role":"assistant","content":"...","tool_calls":[...]}
{"ts":"...","role":"tool","tool_call_id":"...","result":{...}}
{"ts":"...","type":"meta","goal":"...","related_files":["parts/top_lid.py"],"summary":"..."}

生命周期操作：
- 新建：创建 chats/<name>.jsonl，写入 meta 行
- 恢复：读取 .jsonl，重建消息上下文（只取最近 N 条 + meta 行）
- 重命名：重命名 .jsonl 文件
- 归档：移动到 chats/archived/
- 摘要更新：追加新 meta 行（覆盖上一条 meta）
- 关联文件：meta 行中的 related_files 字段
```

JSONL 解决 Markdown 不能可靠承载 tool calls/results 的问题。每行是完整 JSON，可追加、可重放、可截断。
JSONL 中的 `related_files` 是给人和 LLM 阅读的展示路径；执行工具调用前，app server 必须把这些展示路径解析为 `PathHandle` 并重新校验。不能把 JSONL 里的路径字符串直接当作 I/O authority。

Project = workspace，不引入新概念。现有 workspace 机制直接承载 CAD project 的 components/ parts/ assemblies/ chats/ plans/ outputs/ 目录结构。

### 6.4 Protocol 扩展

**协议版本策略**：bump `WIRE_VERSION` (当前 =1) 到 2。前后端必须同步升级（MVP 阶段不需要跨版本兼容）。`ServerCapabilities` 新字段随版本 bump 一起加入，不需要单独兼容处理。

**wire envelope 接入点**（具体挂在现有枚举的哪些位置）：

```rust
// ClientCommand 新增 variant：
CadQueryExecute { ... }         // → CommandSuccess::CadQueryResultReady(CadQueryResultReady)
CadQueryPreview { ... }         // → CommandSuccess::CadQueryResultReady(CadQueryResultReady)
CadQueryResultGet { result_id } // → CommandSuccess::CadQueryMesh(CadQueryMeshPayload)
ChatCreate { ... }           // → CommandSuccess::ChatCreated { session_id }
ChatList { ... }             // → CommandSuccess::ChatList { sessions }
ChatSend { ... }             // → CommandSuccess::ChatAck { session_id }
ChatHistory { ... }          // → CommandSuccess::ChatHistory { messages }
ChatArchive { ... }          // → CommandSuccess::ChatArchived { session_id }
AgentInvoke { ... }          // → CommandSuccess::AgentStarted { session_id }
AgentCancel { ... }          // → CommandSuccess::AgentCancelled
SelectionUpdate { ... }      // → CommandSuccess::SelectionUpdated(SelectionUpdateResponse)

// ServerPushEvent 新增 variant：
AgentToken { session_id, text }
AgentToolStart { session_id, tool_name, args }
AgentToolResult { session_id, tool_call_id, result }
AgentMeshReady { session_id, result: CadQueryResultReady }
AgentError { session_id, error_type, message }
AgentDone { session_id, cancelled: bool }
```

所有新增 command 中的 workspace 文件路径字段必须使用 `PathHandle` / `WorkspacePortablePath`，包括 `CadQueryExecute.target_path`、`CadQueryPreview.target_path`、Chat 相关 `related_files`、Plan 确认范围里的 `affected_files` / `new_files` / `export_targets`。协议 payload 不接受未校验的相对路径字符串；展示字符串只用于 Chat / UI 文案。

`AgentMeshReady` 不直接携带 `CadQueryMeshPayload`。它只携带 `CadQueryResultReady`，其中包含 `result_id`、`build_id` 和轻量统计信息；Web 端收到后再发 `cadquery.result.get { result_id }` 拉取 mesh。

`result_id` 是 CadQuery result cache、`CadQueryMeshPayload` 和 wasm side buffer 的唯一 key。`CadQueryMeshPayload` 必须包含同一个 `result_id`，`studio-web-wasm` 在 `client_drain_events` 前截获 `CommandSuccess::CadQueryMesh(payload)`，按 `payload.result_id` 存入 `CadQuerySideBuffer`，并把 JS 可见的 response payload 缩减为 `CommandSuccess::CadQueryResultReady(CadQueryResultReady)`。JS 侧只通过 `client_take_cadquery_mesh(result_id)` 获取 `CadQueryMeshHandle`；不得使用 `request_id` 作为 CadQuery mesh 的长期 key。原因是 `cadquery.result.get` 的 request_id 只是一次取数请求，而同一个 result 可能被不同 UI 流程重复读取。

**新增 Chat / Agent 命令**：

```
chat.create / list / send / history / archive  — Chat 会话生命周期
agent.invoke                      — 触发 Agent（流式响应，见下方事件模型）
agent.cancel                      — 取消当前 Agent 执行
```

**Agent streaming 事件模型**（server→client push，基于现有 watch 事件扩展）：

```
agent.token         — 文本 token chunk（流式输出）
agent.tool_start    — 工具调用开始（tool name + args）
agent.tool_result   — 工具调用完成（result payload）
agent.mesh_ready    — CadQuery 执行完成，mesh 可渲染
agent.error         — 结构化错误（见 §6.5 错误分类）
agent.done          — Agent 轮次完成
```

**invoke/cancel 语义**：
- `agent.invoke` 是 request/response：server 立即返回 `CommandSuccess`（含 session_id），后续通过 push events 流式传输。如果已有 running session，`agent.invoke` 返回 `ProtocolErrorCode` 错误（`agent_busy`，需扩展 enum）。
- `agent.cancel` 取消当前 session：(1) 中断 LLM stream (2) 停止 tool loop (3) kill CadQuery 子进程 (4) 清理 staging 目录 (5) 发送 `agent.done` 事件（标记 cancelled）。
- 同时只有一个 running agent session。

**新增 Viewer 选择同步**：

```
selection.update                  — Viewer → Server，同步当前选择状态
```

Selection payload 使用协议内显式结构，不用临时 JSON：

```rust
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
enum SelectionKind {
    Component,
    Part,
    Assembly,
    Instance,
    Feature,
    Face,
    Edge,
    Vertex,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
struct SelectionRef {
    kind: SelectionKind,
    ref_text: String,                    // 如 "@face[top_lid:f_0]"
    owner_ref_text: Option<String>,      // raw geometry 选择时的上级 @part/@component/@assembly
    owner_object_kind: Option<CadQueryObjectKind>,
    instance_path: Option<String>,       // assembly child 选择时的 instance path
    candidate_feature_ref: Option<String>,
    build_id: Option<String>,            // raw geometry 必须携带
    result_id: Option<String>,           // 对应 CadQuery result cache
    ambiguous: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
struct SelectionUpdateRequest {
    selections: Vec<SelectionRef>,
    active_index: Option<u32>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
struct SelectionUpdateResponse {
    accepted_count: u32,
}
```

`SelectionRef.owner_object_kind` 与 `CadQueryMeshPayload` 共用 §7.2 的 `CadQueryObjectKind`。协议实现时应把该 enum 放在共享位置，避免 Selection 与 CadQuery payload 定义出两套含义相同的对象类型。

server 端保存当前 session / workspace 的最新 selection snapshot；`studio-common::ManagedClient` 在 `SelectionUpdated` response 后更新本地 snapshot，后续 `agent.invoke` 读取同一份 selection 状态。多选时 `active_index` 指向用户最后点击的选择项。

不需要 project.* 命令——Project = workspace，复用现有 workspace.open / workspace.list。

CadQuery 和 OpenSCAD 在协议层是独立的命令类型，不复用 `PreviewRequest`（其语义绑定 openscad_path）。

**协议扩展策略**：现有协议是固定 Borsh enum（`ClientCommand`/`CommandSuccess`/`ServerPushEvent` 各有显式 discriminant）。新增命令和事件通过追加 enum variant 实现，旧 discriminant 不变以减少无关漂移；MVP 阶段不承诺新旧客户端混跑。同时需要更新：
- `app-server-protocol` — 新增 variant
- `app-server-protocol-wasm` — wasm encoder/decoder
- `studio-common/managed_client` — pending kind、inbound push event handler、ClientEvent、ClientSnapshot、timeout 配置
- `studio-web-wasm/wasm_bridge` — client command dispatch、CadQuery side buffer、`client_take_cadquery_mesh`
- `packages/studio-web/src/wasm-bridge/event-stream.ts` — Agent / Chat / Selection 事件派发，不保存业务状态
- `packages/studio-web-wasm/generated/` — 新增 wasm-bindgen 导出后必须同步生成产物

**Capability 协商**：`ServerCapabilities` 新增字段标记 CadQuery/Agent 支持：
```rust
struct ServerCapabilities {
    // existing...
    cadquery: bool,        // 是否支持 cadquery.execute/preview
    agent: bool,           // 是否支持 agent.invoke/cancel + streaming events
    selection_sync: bool,  // 是否支持 selection.update
}
```
前端根据 capability 决定是否显示 Chat/Agent UI 和 CadQuery 相关功能。

**studio-common 状态归属**：
- Chat session 列表、当前 session、Agent running 状态、Agent streaming 事件摘要、当前 Viewer selection、CadQuery result ready 状态必须进入 `studio-common::ManagedClient` 的 snapshot / event 模型。
- `packages/studio-web` 只能通过 wasm bridge snapshot、事件回调和组件局部 `useState` 消费这些协议数据；禁止把 Chat / Agent / Selection 的业务状态放入 Zustand UI store。
- `studio-app` 后续接入同一份 `studio-common` 状态机；不得为桌面端另写一套 Chat / Agent / Selection 状态。

### 6.5 错误分类

Agent 和子系统的执行期错误通过 `agent.error` 事件传递。`ProtocolErrorCode` 只承接 request admission / protocol 层错误：例如 `agent.invoke` 进入前发现已有 running session 时，直接返回 `agent_busy`，因为此时没有 Agent stream，也不会发送 `agent.error`。invoke 被接受之后发生的 LLM、工具、权限、CadQuery、导出错误都作为 `agent.error` 事件的 payload：

| 错误类型 | 来源 | 用户应看到什么 |
|----------|------|---------------|
| `llm_error` | LLM API 调用失败 | "AI 服务暂时不可用，请稍后重试" |
| `llm_refused` | LLM 拒绝执行 | 显示拒绝原因 |
| `permission_denied` | 用户未确认 Execute | "需要确认后才能执行" |
| `file_conflict` | 目标文件被外部修改 | "文件已变更，请刷新后重试" |
| `python_import_error` | CadQuery 脚本 import 失败 | 显示 import 错误 + 缺失模块 |
| `cadquery_build_error` | build() 执行异常 | 显示 Python traceback |
| `tessellation_error` | mesh 生成失败 | "模型生成成功但无法渲染" |
| `topology_mapping_error` | REFS feature 映射失败 | "特征映射不完整，部分选择可能不可用" |
| `export_error` | STEP/STL/3MF 导出失败 | 显示导出错误 |
| `timeout` | 子进程超时 | "模型生成超时，请简化模型或增加超时" |

---

## 7. Viewer 增强

**前提**：基于现有前端框架增量改造，保持当前 UI 和架构。网页端已有 TS Three.js 渲染路径（`packages/studio-web/src/viewers/mesh-three.ts` + `mesh-viewer.tsx`），wasm `renderer_create` 桩不是当前 active path。CadQuery 渲染应扩展现有 TS Three.js renderer，不走 wasm renderer。

### 7.1 渲染

- 扩展 `packages/studio-web/src/viewers/mesh-three.ts`：新增 CadQuery face group 渲染模式
- 按 face group 创建独立 BufferGeometry（每个 B-rep face 一个 group）
- Edge 渲染为 LineSegments
- Vertex 渲染为 Points（可选显示）
- Assembly 每个 child 作为独立 Group，应用 transform
- 复用现有场景管理、相机控制、材质系统

### 7.2 Protocol mesh payload

现有 `PreviewMeshPayload`（positions/normals/colors/indices + Borsh）不够。CadQuery 结果需要新的 payload 类型，**基于现有 Borsh 协议扩展**：

```rust
// app-server-protocol 新增，与 PreviewMeshPayload 并列
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
struct CadQueryResultReady {
    result_id: String, // server 端 CadQuery result cache / wasm side buffer key
    build_id: String,
    part_count: u32,
    face_count: u32,
    edge_count: u32,
    vertex_count: u32,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
enum CadQueryObjectKind {
    Part,
    Component,
    Assembly,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
struct CadQueryMeshPayload {
    result_id: String,
    build_id: String,  // 复合 hash：source_hash + params_hash + deps_hash
    unit: PreviewUnit, // MVP 固定为 Millimeter，展示换算复用现有 Web display_unit
    root_ref_text: String,                  // 本次 build 根对象，如 "@assembly[full_enclosure]"
    root_object_kind: CadQueryObjectKind,   // 根对象类型，用于整体选择
    // 单体 Part 或 Assembly parts
    parts: Vec<CadQueryPartMesh>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
struct CadQueryPartMesh {
    name: String,                              // part/component name
    object_kind: CadQueryObjectKind,            // part/component 语义类型，不从 name 推断
    ref_text: String,                           // 如 "@part[top_lid]" / "@component[m2_5_screw]"
    instance_path: Option<String>,             // assembly 内路径，如 "full_enclosure/top_lid"
    transform: Option<[f32; 16]>,              // 4x4 transform（assembly child 才有）
    faces: Vec<FaceGroup>,
    edges: Vec<EdgeGroup>,
    vertices: Vec<VertexPoint>,
    feature_map: HashMap<String, Vec<u32>>,    // feature name → face_indices
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
struct FaceGroup {
    face_idx: u32,
    positions: Vec<f32>,
    normals: Vec<f32>,
    features: Vec<String>,
    ambiguous: bool,  // 该 face 的 feature 映射是否有歧义
}
```

```rust
#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
struct EdgeGroup {
    edge_idx: u32,
    polyline: Vec<f32>,          // flat [x,y,z,x,y,z,...] 折线顶点
    adjacent_faces: Vec<u32>,    // 相邻 face_idx
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
struct VertexPoint {
    vertex_idx: u32,
    position: [f32; 3],
    adjacent_edges: Vec<u32>,    // 相邻 edge_idx
}
```

单体 Part：`parts` 只有一个元素，`instance_path` 和 `transform` 为 None。
Assembly：`parts` 包含每个可渲染 part / component，带 `instance_path` 和 `transform`。
`build_id` 是 source_hash + params_hash + 所有依赖文件 hash 的复合值，确保任何变更都使旧选择失效。
`unit` 在协议 payload 中使用现有 `PreviewUnit::Millimeter`，CadQuery runner 输出也必须归一为毫米。Web 端已有 `display_unit`（millimeter / centimeter / inch）负责尺寸展示换算；CadQuery MVP 不新增另一套单位设置。
`root_ref_text` / `root_object_kind` 用于整次结果的 assembly / part 整体选择；`CadQueryPartMesh.ref_text` / `object_kind` 用于 part / component 实例整体选择。raw face / edge / vertex 选择的 `owner_ref_text` / `owner_object_kind` 必须来自这份 payload 元数据，不得从文件名、mesh name 或 instance_path 临时拼接。

`CadQueryMeshPayload` 允许出现在 Borsh wire frame 中，但不得直接出现在 JS 层 `ClientEvent` payload 中。Web bridge 必须在 `client_drain_events` 前把 mesh payload 移入 side buffer，并把 JS 可见事件缩减为 `CadQueryResultReady`。

CadQuery 不能复用现有 `MeshHandle`。当前 `MeshHandle` 只暴露单个 mesh 的 positions / normals / colors / indices，会丢失 CadQuery 必需的 face group、edge group、vertex、feature_map、instance_path 和 transform。需要新增 `CadQueryMeshHandle`：

- `client_take_cadquery_mesh(result_id) -> CadQueryMeshHandle | undefined`。
- handle 内部持有完整 `CadQueryMeshPayload`。
- metadata（result_id、build_id、root_ref_text、root_object_kind、part ref_text、object_kind、parts、face_idx、features、ambiguous、instance_path、transform）可以作为小对象序列化给 JS。
- 大数组通过专用 getter 返回 typed array，例如按 part index + face index 取 `positions` / `normals`，按 edge index 取 `polyline`。
- TS Three.js renderer 只消费 `CadQueryMeshHandle` 或由它转换出的轻量 view model，不经过 `serde_wasm_bindgen` 展开大数组。

### 7.3 选择

- **Raycasting**：Raycaster 对每个 FaceGroup 做 intersection test
- **hover highlight**：鼠标悬浮时高亮对应 face group（修改 material emissive）
- **click select**：点击后读取 face group 的 features 和 ambiguous 标记
- **multi-select**：Shift+click 支持多选
- **selection serialization**：选择结果序列化为 Ref 格式
- **ambiguous 处理**：当 face 的 `ambiguous=true` 时，显示确认弹窗让用户确认 feature 归属

选择返回格式（MVP 5 层 Ref，不含 selector 层）：

```json
{
  "kind": "face",
  "ref_text": "@face[top_lid:f_0]",
  "owner_ref_text": "@part[top_lid]",
  "owner_object_kind": "part",
  "instance_path": "full_enclosure/top_lid",
  "candidate_feature_ref": "@feature[top_lid.top_surface]",
  "build_id": "abc123",
  "result_id": "cq_abc123",
  "ambiguous": false
}
```

前端直接读取 mesh 中的 features，不需要额外后端请求。当 `ambiguous=true` 时显示确认弹窗。UI 层可以把 `ref_text` 显示为 `ref` 文案，但发送 `selection.update` 时必须使用协议字段 `ref_text`。`build_id` 绑定 manifest source_hash + params_hash + deps_hash，防止选择结果用到过期模型上。
component / part / assembly 整体选择必须使用 `CadQueryMeshPayload.root_ref_text` 或 `CadQueryPartMesh.ref_text` 生成 Ref；raw face / edge / vertex 选择的 `owner_ref_text` / `owner_object_kind` 也必须来自对应 `CadQueryPartMesh.ref_text` / `object_kind`。前端不得用 `name`、`instance_path` 或文件路径自行拼接 Ref。

### 7.4 选择 → Agent

选择结果作为 ref 插入 Chat 上下文，Agent 按 Ref 处理优先级解析。

### 7.5 前端工作量评估

Viewer 增强是前端工作量最大的部分：

| 任务 | 复杂度 | 依赖 |
|------|--------|------|
| CadQueryMeshHandle 接入 | 中 | protocol 定义 + wasm bridge |
| face group BufferGeometry 构建 | 中 | payload 格式 |
| Raycaster per-face-group picking | 中 | face group 渲染 |
| hover highlight | 低 | picking |
| click → ref 生成 | 中 | feature_map + ambiguous 标记 |
| multi-select + selection panel | 中 | ref 格式 |
| selection → Chat context | 低 | Chat UI |
| Assembly instance picking | 高 | instance_path + nested groups |
| Edge / Vertex picking | 高 | LineSegments/Points raycasting 精度 |

Edge/Vertex picking 是技术风险最高的前端任务——Three.js 对线和点的 raycasting 精度不如面。可能需要扩大 pick tolerance 或用 proximity-based picking。

---

## 8. 实施分期

每个 Phase 都必须按“实现 → 独立 review → 回归验证 → 修复 → 记录结果 → commit”的循环执行。独立 review 必须使用 subagent，输入至少包含：当前 Phase 目标与验收标准、完整 `plan-00.md`、本次 diff 或涉及文件清单。每个 Phase 完成后实时更新 `plan-00-result.md`，不得等到全部完成后再补写。

### Phase 0a — 规则与文档一致性前置

**输入**：
- 本 plan。
- 根 `AGENTS.md`。
- `docs/cadquery-mvp/init.md`、`docs/cadquery-mvp/ref_components_parts_assemblies.md`、`docs/cadquery-mvp/decisions.md`。
- `docs/architecture.md` 与 `docs/2026042500-cross-platform-path-policy/README.md`。

**前序目标保护**：
- 保护已确认的 CadQuery 方向、MVP 5 层 Ref、JSONL Chat、Borsh wire format、单 running agent session 决策。
- 不引入产品代码实现，只修正文档和实施规则。

**操作步骤**：
- 在 `AGENTS.md` 明确 CadQuery Python 子进程豁免：仅限 `budn_cad_runner` 作为外部工具被 app server 调用，不允许项目内任意新增 Python 辅助脚本。
- 同步 `docs/cadquery-mvp/ref_components_parts_assemblies.md`：MVP 不暴露 `@selector` / `@subshape` Ref；selector 只作为 Agent / runner 内部查找手段；删除 `candidate_selector_ref`；验收标准不得要求 Agent 产出 `@selector[...]` Ref。
- 同步 `docs/cadquery-mvp/ref_components_parts_assemblies.md` 的 Assembly metadata 示例：统一使用 `ref_text` / `object_kind`；若 CadQuery API 只能稳定保存 `ref` 这类短字段，必须在文档中明确它只是 Python metadata 输入别名，runner stdout、protocol payload、SelectionRef 一律归一为 `ref_text`。
- 同步 `docs/architecture.md`：协议线格式以当前 Borsh binary frame 为准，不再描述 UTF-8 JSON WebSocket frame；保留 `app-server-protocol` 是唯一线格式来源的约束。
- 更新 `docs/cadquery-mvp/decisions.md`：Rig 评估改为当前最新兼容版本，不固定旧版本号。
- 在 `plan-00-result.md` 记录 Phase 0a 结果和仍需后续处理的问题。

**验收标准**：
- 运行 `rg "### 7\\.5 Selector Ref|### 7\\.6 Subshape Ref|@selector\\[|@subshape\\[|candidate_selector_ref|Agent 能把 @selector|feature / selector / subshape|UTF-8 JSON|rig-core v0\\.31|metadata=\\{\"ref\"" docs/cadquery-mvp docs/architecture.md`；若有命中，必须逐条归类为“仅历史说明 / 仍误导 MVP 实现”，并删除或改写所有“仍误导 MVP 实现”的正文、示例和验收项。
- `AGENTS.md` 明确 CadQuery 子进程豁免边界。
- `docs/cadquery-mvp/decisions.md` 不再固定过期 Rig 版本。
- 文档 diff 经过独立 review，无阻断项后 commit。

### Phase 0b — 最小 CLI 跑通

**输入**：
- Phase 0a 已通过的文档和规则。
- 手动安装的 Python + CadQuery 环境。
- 一个最小 `parts/top_lid.py` 示例。

**前序目标保护**：
- 不绕过 Phase 0a 中定义的 Python 豁免边界。
- 不修改 protocol、Web UI、Agent 行为；只验证 CadQuery runner 最小竖切。

**操作步骤**：
- 编写手动安装 Python + CadQuery 环境的文档。
- 实现 `budn_cad_runner` 最小 CLI：loader、executor、单 Workplane tessellator。
- 运行 `python3 -m budn_cad_runner --script parts/top_lid.py --project-root <fixture> --output-dir <tmp>`，输出 JSON。
- 增加最小 Assembly fixture，验证目标 CadQuery 版本中的 `Assembly.add(...)`、child 遍历 API、child name、location/transform、metadata/ref_text/object_kind 附加方式。
- 增加带 import 依赖的最小 fixture，验证 runner 能记录 `dependencies`、生成 `deps_hash`，且修改被 import 文件后 `build_id` 必须变化。
- 验证目标 CadQuery 版本中的 `val().wrapped`、`BRepMesh_IncrementalMesh`、`TopExp_Explorer`、`.tag()`、`_getTagged` 行为；验证结果写入 Phase 结果文档。若实际 API 与 plan 示例不同，先修订 plan 和 runner 设计，再进入 Phase 0c。
- 验证 runner 输出单位固定为毫米，坐标符合 `docs/architecture.md` 的右手系和 Top plane = `XY` 契约；Web 尺寸展示继续复用现有 `display_unit` 配置。

**验收标准**：
- 单个 `.py` 文件 CLI 执行成功，stdout 输出 mesh faces、bounding box、root_ref_text、root_object_kind。
- 最小 Assembly CLI 执行成功，stdout 能输出统一 `parts[]`、instance_path、transform 和每个 part 的 ref_text / object_kind。
- 修改 import 依赖文件后重新执行，stdout 中 `deps_hash` 与 `build_id` 必须变化。
- stdout 中 `unit` 必须为 `millimeter`，基础 box fixture 的 bounding box 方向与尺寸符合项目坐标系。
- CLI 失败时 stderr / JSON error 能区分 build error 与 runner error。
- API 原型验证结论可复查，不依赖猜测。
- 独立 review 无阻断项，相关测试 / CLI 验证通过后 commit。

### Phase 0c — 完整 runner + Rust CadQuery 集成

**输入**：
- Phase 0b 的最小 runner。
- 当前 `app-server-protocol`、`app-server-core`、`studio-common`、`studio-web-wasm` 代码。
- `PreviewSideBuffer` 现有实现作为重载荷 bridge 参考。

**前序目标保护**：
- 保持 app server 是唯一 I/O 和外部工具能力层。
- 不把 CadQuery mesh 重载荷直接暴露给 JS `ClientEvent`。
- 不把 selector / subshape 作为 MVP 用户可见 Ref 层。

**操作步骤**：
- 定义 CadQuery 输出 JSON schema：统一 `parts[]`、topology、feature_map、exports、metadata、manifest、dependencies、deps_hash，并保留 root / part 的 `ref_text` 与 `object_kind`。
- 定义 `CadQueryMeshPayload`、`CadQueryResultReady`、CadQuery command / response 类型和校验逻辑。
- 在 `studio-web-wasm` 设计并实现 CadQuery side buffer：drain 事件前按 `result_id` 截获 mesh payload，JS 通过 `client_take_cadquery_mesh(result_id)` 获取专用 `CadQueryMeshHandle`。
- 扩展 runner：ref_mapper、selector_parser、exporter、manifest、Assembly parts 处理。
- 在 `app-server-core/src/cadquery/` 实现子进程调用和 staging 原子写入。

**验收标准**：
- 命令行执行 `.py` 输出完整 JSON。
- Rust 端可解析统一 `parts[]` JSON，并生成 Borsh `CadQueryMeshPayload`。
- Rust 端校验 `dependencies` / `deps_hash` / `build_id`，被 import 文件变化时旧 selection 失效。
- `CadQueryMeshPayload` 保留 `root_ref_text` / `root_object_kind` 和每个 `CadQueryPartMesh` 的 `ref_text` / `object_kind`，整体选择与 raw geometry 上级归属不依赖前端反推。
- Rust / Web 端不新增独立单位配置；`CadQueryMeshPayload.unit` 使用 `PreviewUnit::Millimeter`，展示换算使用现有 Web `display_unit`。
- `studio-web-wasm` 不通过 `serde_wasm_bindgen` 展开 CadQuery mesh 大数组。
- `CadQueryMeshHandle` 能让 JS 读取 result_id、build_id、root_ref_text、root_object_kind、part ref_text、object_kind、face group、edge group、vertex、feature_map、instance_path 和 transform。
- protocol、core、wasm bridge 相关测试通过；独立 review 无阻断项后 commit。

### Phase 1 — Protocol / ManagedClient / Agent / Chat

**输入**：
- Phase 0c 的 CadQuery runner 与 protocol payload。
- 当前 `ManagedClient` snapshot / event 模型。
- 当前 `packages/studio-web/src/workbench/chat-zone.tsx` 占位实现。

**前序目标保护**：
- 保护 `studio-common` 作为跨端协议状态机的唯一归属。
- 保护 Zustand 只保存 UI 壳状态的边界。
- 保护单 running agent session 限制。

**操作步骤**：
- 评估 Rig 当前最新兼容版本的 tool use、streaming、自定义 Agent loop；记录采用 Rig 或 SDK 自建的判定依据。
- 扩展 `app-server-protocol`：Chat 生命周期命令、Agent invoke/cancel、SelectionUpdate、CadQueryResultGet、Agent push events、`agent_busy` 错误。
- 扩展 `studio-common::ManagedClient`：Chat session、Agent run、current selection、CadQuery result ready 的 snapshot / event / timeout / reconnect 语义。
- 扩展 `studio-web-wasm` 和 generated package：Chat / Agent / Selection dispatch、push event 派发、`CadQueryMeshHandle` 与 CadQuery side buffer take API。
- 实现 app-server-host / dispatcher 异步任务 registry：`agent.invoke` 立即返回 `AgentStarted`，后续用 push event 输出 token、tool 状态、result ready、done；`agent.cancel` 中断 LLM stream、tool loop、CadQuery 子进程并清理 staging。
- 实现 chat-session JSONL 存储和 Inform / Plan / Execute 权限模型。
- 实现 Chat Web UI 的 session 列表、send flow、streaming 展示、tool result 展示；协议业务状态只来自 ManagedClient snapshot / events。

**验收标准**：
- Chat 可以创建、切换、发送、读取历史，JSONL 可重放。
- `agent.invoke` 在已有 running session 时返回 `agent_busy`。
- `agent.cancel` 能停止运行并发送 cancelled done event。
- Web UI 不向 Zustand 写入 Chat / Agent / Selection 业务状态。
- `cadquery.result.get` 的 JS Promise 只暴露轻量 `CadQueryResultReady`，大数组必须通过 `client_take_cadquery_mesh(result_id)` 读取。
- 通过 Chat 对话触发 Agent 生成 CadQuery 代码并执行；多 session 切换正常。
- 独立 review 无阻断项，协议 / wasm / Web smoke 相关验证通过后 commit。

### Phase 2 — Viewer 增强

**输入**：
- Phase 1 可获取的 `CadQueryMeshHandle` / typed array 数据。
- 当前 TS Three.js renderer：`packages/studio-web/src/viewers/mesh-three.ts` 和 `mesh-viewer.tsx`。
- MVP 5 层 Ref 规则。

**前序目标保护**：
- 不启用 wasm `renderer_create` 桩作为真实渲染路径。
- 不把 selector / subshape 暴露为用户可见 Ref。
- 不绕过 `SelectionUpdate` 与 ManagedClient 状态。

**操作步骤**：
- 扩展 `mesh-three.ts`：新增 CadQuery face group 渲染模式。
- 将 `CadQueryMeshHandle` 转为 face group BufferGeometry、LineSegments、Points。
- 实现 component / part / assembly 整体选择；Ref 必须来自 `CadQueryMeshPayload` / `CadQueryPartMesh` 元数据。
- 实现 face / edge / vertex 精细选择、hover highlight、multi-select、Assembly instance picking。
- 选择结果格式化为 MVP 5 层 Ref，并通过 `selection.update` 同步给 server。
- 实现歧义确认弹窗：当 `ambiguous=true` 时让用户确认 feature 归属。

**验收标准**：
- 点击模型面能显示对应 feature。
- component / part / assembly 整体选择能生成协议一致的 Ref，且不依赖前端按名称或路径拼接。
- face / edge / vertex 选择能从 payload 元数据生成 `owner_ref_text` / `owner_object_kind`、`build_id` 和 `result_id`，并通过 `selection.update` 同步给 server。
- 歧义时弹出确认。
- Assembly 内能区分同一 part 的不同实例。
- edge / vertex 选择有可接受的 pick tolerance，并记录剩余精度风险。
- Playwright 或等价浏览器验证覆盖基本选择路径；独立 review 无阻断项后 commit。

### Phase 3 — 端到端集成

**输入**：
- Phase 2 的 Viewer selection。
- Phase 1 的 Agent / Chat / Execute 权限。
- Ref 业务规则表。

**前序目标保护**：
- 不回退到 raw face / edge id 作为长期修改目标。
- 不让 Agent 超出用户确认的 affected_files / new_files / export_targets。
- 不破坏 Phase 0c 的 staging 原子写入。

**操作步骤**：
- 打通 Viewer selection → Chat ref → Agent 解析 → 修改目标判定。
- 实现 §4.1 Ref 业务规则：区分 component placement、part 几何修改、assembly 多文件协调等场景。
- 生成并展示 Markdown CAD Plan；Execute 前记录结构化确认范围。
- 管理 outputs/ 与 manifest 追溯。
- 执行端到端验证：讨论 → Plan → 确认 → 生成 → 选择 → 再修改。

**验收标准**：
- PRD 定义的完整流程可演示。
- Agent Execute 前声明目标文件列表，执行时不越界。
- CadQuery 执行失败不污染真实文件。
- Viewer 新选择可以驱动下一轮 Agent 修改。
- 独立 review 无阻断项，端到端验证通过后 commit。

---

## 9. 已关闭的讨论项

全部开放问题已关闭：

- ~~Chat 存储格式~~：JSONL（原 Markdown 不能承载 tool calls）
- ~~Rig 评估~~：Phase 1 开始时调研，不阻塞 plan
- ~~mesh wire format~~：基于现有 Borsh 协议扩展
- ~~Ref 层级 MVP 简化~~：确认 5 层（砍 selector/subshape）
- ~~bundled venv 平台分发~~：MVP 手动安装，分发策略留到产品化阶段
- ~~并发模型~~：限制同时只有一个 running agent session
- ~~前端架构~~：基于现有框架增量改造，保持当前 UI
- ~~crate 组织~~：新模块放 app-server-core 子模块，不新增 crate

### Codex Review 修复记录

以下 Codex findings 已在此版本 plan 中修复：

1. decisions.md 未同步 → 需要在实施开始时同步（§10 AGENTS.md 更新 + decisions.md）
2. Ref 层级/selector 语义矛盾 → 移除 candidate_selector_ref（§7.3）
3. 前端 renderer 是桩 → 明确标注需补全（§7.1）
4. OpenSCAD 子进程不是通用抽象 → 明确新建 cadquery/ 模块（§1）
5. 错误模型接入 → 通过 agent.error 事件传递（§6.5）
6. Agent streaming 协议缺失 → 新增事件模型（§6.4）
7. Markdown Chat 不能承载 tool calls → 改用 JSONL（§6.3）
8. 原子写入路径问题 → staging 目录方案（§5.4）
9. 缺少并发控制 → 限制单 running session（§6.4）
10. mesh wire format 矛盾 → Borsh 基准（§7.2, §2.10）
11. Phase 0 过大 → 拆为 0a/0b（§8）
12. crate 组织未定 → app-server-core 子模块（§6）
13. 拓扑稳定性/build_id → selection payload 增加 build_id（§7.3）

### Codex Review Round 2 修复记录

以下 findings 已在此版本 plan 中修复：

1. CadQueryMeshPayload/Viewer 矛盾 → 重新设计 payload 含 ambiguous 标记（§7.2）
2. Assembly schema 与 wire payload 不对齐 → 引入 CadQueryPartMesh 嵌套结构（§7.2）
3. build_id 不够 → 改为 source_hash + params_hash + deps_hash 复合值（§7.2）
4. 协议扩展成本 → 新增协议扩展策略和更新清单（§6.4）
5. Capability 缺失 → ServerCapabilities 新增 cadquery/agent/selection_sync（§6.4）
6. agent_busy 错误模型 → invoke 返回 ProtocolErrorCode，后续走 push events（§6.4）
7. agent.cancel 不足 → 分解 5 步取消流程（§6.4）
8. staging 冲突检测 → 回写前比较 hash/mtime（§5.4）

### 文档同步状态（工程审查后修订）

- [x] `docs/cadquery-mvp/init.md`：项目结构 chats/*.md → chats/*.jsonl，Chat 格式改 JSONL 示例。
- [ ] `docs/cadquery-mvp/decisions.md`：Phase 0a 需把 Rig 评估改为当前最新兼容版本，并保留 CadQuery 子进程豁免需同步 `AGENTS.md` 的前置要求。
- [ ] `docs/cadquery-mvp/ref_components_parts_assemblies.md`：Phase 0a 需删除会误导 MVP 实现的 `@selector` / `@subshape` 用户可见 Ref、`candidate_selector_ref` 和对应验收项。
- [ ] `AGENTS.md`：Phase 0a 需新增 CadQuery Python 子进程豁免边界。
- [ ] `docs/architecture.md`：Phase 0a 需把协议线格式从旧 UTF-8 JSON 表述修正为当前 Borsh binary frame。

### Codex Review Round 3 修复记录

1. 残留 candidate_selector_ref → 从 §4 处理优先级中移除
2. CadQueryMeshPayload wire envelope → 明确 ClientCommand/CommandSuccess/ServerPushEvent variant 映射（§6.4）
3. ServerCapabilities Borsh 兼容 → bump WIRE_VERSION=2，同步升级（§6.4）
4. dispatcher 异步化 → Phase 1 新增 async task registry + running session 管理
5. selection.update payload → 定义 SelectionRef 结构和 server 端存储
6. EdgeGroup/VertexPoint 未定义 → 补充 Borsh struct 定义（§7.2）
7. Web renderer 路径错误 → 修正为 mesh-three.ts（TS Three.js），不走 wasm 桩
8. chat-zone placeholder → 明确 Phase 1 需实现完整 session/send/streaming UI

### 工程审查修复记录（2026-04-27）

1. Ref PRD 同步声明不成立 → 文档同步状态改为待办，并把 Ref PRD 修正列为 Phase 0a 阻断项。
2. CadQuery mesh 重载荷不能直接进入 JS `ClientEvent` → 新增 `CadQueryResultReady`、`CadQueryResultGet` 和 `CadQuerySideBuffer` 约束。
3. 缺少 `studio-common` 的 Chat / Agent / Selection 状态设计 → §6.4 新增 ManagedClient snapshot / event 归属约束，Phase 1 明确实现。
4. Phase 分期不满足 AGENTS 结构要求 → §8 重写为含输入、前序目标保护、操作步骤、验收标准、独立 review / 回归要求的执行计划。
5. CadQuery Python 豁免没有作为前置步骤 → Phase 0a 第一项要求更新 `AGENTS.md`。
6. `docs/architecture.md` 线格式仍写 UTF-8 JSON → Phase 0a 纳入 Borsh binary frame 文档同步。
7. Rig 版本引用过期 → 改为评估当前最新兼容版本，不固定旧版本号。

### 独立 review 修复记录（2026-04-28）

1. 工具参数路径绕过协议路径模型风险 → §5.1 / §5.3 / §5.4 / §6.4 明确所有 workspace I/O command 使用 `PathHandle` / `WorkspacePortablePath`，Chat JSONL 中的展示路径不能作为 I/O authority。
2. `build_id` 缺少依赖 hash 来源 → §2.9 增加 `dependencies`、`params_hash`、`deps_hash`，Phase 0b / 0c 增加依赖变更导致 `build_id` 变化的验收。
3. runner 单体 / Assembly 输出 schema 不统一 → §2.9 改为统一 `parts[]` schema，单体模型也是一个 part。
4. CadQuery mesh 单位与坐标系不明确 → §2.10 / §7.2 明确 payload 使用 `PreviewUnit::Millimeter`，runner 输出固定毫米并遵循项目坐标系；Web 展示换算复用现有 `display_unit`。
5. Selection 示例字段与协议字段不一致 → §7.3 统一使用 `ref_text`，UI 仅可把它显示为 `ref` 文案。
6. Phase 0a `rg` 验收可能误判 → §8 Phase 0a 改为对命中结果逐条归类，删除或改写仍会误导 MVP 实现的正文、示例和验收项。

### 二次独立 review 修复记录（2026-04-28）

1. `CadQueryPartMesh` 缺少对象级 Ref 元数据 → §2.9 / §7.2 / §7.3 / Phase 0c / Phase 2 增加 `root_ref_text`、`root_object_kind`、part `ref_text`、`object_kind`，整体选择和 raw geometry 上级归属不得由前端按名称或路径反推。
2. Assembly 处理措辞可能误读为输出 `children` schema → §2.4 / §2.9 / §7.2 改为明确递归 CadQuery child 对象后归一输出 `parts[]`。

### 三次独立 review 修复记录（2026-04-28）

1. 旧 raw geometry 上级字段对 component 归属有歧义 → §6.4 / §7.2 / §7.3 / Phase 2 改为 `owner_ref_text` + `owner_object_kind`，并要求二者来自 payload 元数据。
2. `CadQueryMeshHandle` 未明确向 JS 暴露 `result_id` / `build_id` → §7.2 / Phase 0c 明确 metadata/getter 和验收必须覆盖这两个字段。
3. `docs/cadquery-mvp/ref_components_parts_assemblies.md` 中 Assembly metadata 示例仍使用 `ref` → Phase 0a 增加 `ref_text` / `object_kind` 文档同步要求和对应 `rg` 检查项。

### 已知风险（接受，实施时解决）

- CadQuery API 假设（Phase 0b 验证）
- selector parser 覆盖面（迭代扩展）
- 安全边界（MVP 本地信任模型，不做沙盒）
- .md 文档维护（Agent 行为约束，不纳入原子事务）
- 权限校验（Phase 1 tool dispatcher 层实现）
- Edge/Vertex picking 精度（Phase 2 技术风险）
- CadQuery result cache 生命周期、过期清理和 side buffer 容量（Phase 0c/1 实现时处理）

## 10. AGENTS.md 更新

以下内容必须在 Phase 0a 完成，不得推迟到产品代码实施之后：

- 新增 CadQuery 子进程豁免条款
- 记录产品方向：CadQuery 替代 OpenSCAD
- 新增 CAD Agent 架构约束

## 审查记录

经过多轮独立 Codex 审查和一次工程审查：

- **Round 1**：20 findings，13 fixed，6 deferred as known risks，1 false positive。
- **Round 2**：13 findings，8 fixed in plan，4 doc sync（当时标记为完成；工程审查后发现 Ref PRD 和架构文档仍需 Phase 0a 复核修正）。
- **Round 3**：8 findings，全部 fixed（wire envelope 映射、WIRE_VERSION bump 策略、dispatcher 异步化、selection.update payload、EdgeGroup/VertexPoint 定义、TS renderer 路径修正、chat-zone placeholder、残留 candidate_selector_ref 移除）。
- **工程审查（2026-04-27）**：7 项 findings，已修订进本 plan；其中 Ref PRD、AGENTS、架构文档同步作为 Phase 0a 阻断项。
- **独立 review（2026-04-28）**：无 Critical；1 项 Important 和 1 项 Minor 已修订进本 plan（对象级 Ref 元数据、Assembly children 措辞）。
- **独立 review（2026-04-28，Avicenna）**：无 Critical；2 项 Important 和 1 项 Minor 已修订进本 plan（raw geometry owner 字段、handle 暴露 result_id/build_id、Assembly metadata 文档同步）。
- **未解决**：7 项已知风险（已接受，实施时解决）。
- **结论**：本 plan 可作为实施输入，但必须先完成 Phase 0a，再开始产品代码实现。
