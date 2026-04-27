# CAD Agent Harness MVP — plan-00 执行结果

## 执行上下文

- 执行分支：`cadquery-mvp-design-execution`
- 执行 worktree：`~/.config/superpowers/worktrees/scad-studio/cadquery-mvp-design-execution`
- 基线验证：`cargo test --workspace` 通过。

## Phase 0a — 规则与文档一致性前置

### 完成情况

- 在 `AGENTS.md` 增加 CadQuery Python 子进程豁免边界，明确仅允许 `budn_cad_runner` 作为 app server 外部 CAD 工具，不允许扩展为项目内任意 Python 辅助脚本。
- 在 `AGENTS.md` 增加 CAD Agent / CadQuery 架构约束，记录 CadQuery 方向、app server 归属、tool call 写入、staging 原子执行、protocol 数据边界和 MVP 5 层 Ref。
- 更新 `docs/cadquery-mvp/ref_components_parts_assemblies.md`，删除 selector / subshape 用户可见 Ref 描述，移除 `candidate_selector_ref` 示例，统一 Selection 示例为 `ref_text`、`owner_ref_text`、`owner_object_kind`。
- 更新 Ref PRD 的 Assembly metadata 要求，明确 child metadata 使用 `ref_text` / `object_kind`；若 CadQuery API 只能稳定保存短字段，它只能作为 Python metadata 输入别名，runner stdout、protocol payload、SelectionRef 一律归一为 `ref_text`。
- 更新 `docs/architecture.md`，把 WebSocket 线格式从旧 UTF-8 JSON 改为当前 Borsh binary frame，并保留 `app-server-protocol` 是唯一线格式来源的约束。
- 更新 `docs/cadquery-mvp/decisions.md`，把 Rig 评估改为 Phase 1 按 crates.io / docs.rs 当前版本验证，不固定旧版本号。
- 更新 `plan-prompt.md`，追加本轮“连续执行完整计划”的用户 prompt 存档。

### 验证记录

- `rg "### 7\\.5 Selector Ref|### 7\\.6 Subshape Ref|@selector\\[|@subshape\\[|candidate_selector_ref|Agent 能把 @selector|feature / selector / subshape|UTF-8 JSON|rig-core v0\\.31|metadata=\\{\"ref\"" docs/cadquery-mvp docs/architecture.md`：无命中。
- `git diff --check`：通过。
- 独立 review subagent：无阻断项。review 观察到一处“nearest selector”表述，我已改为“内部 selector candidate”。

### 遗留问题

- Phase 0a 未发现需要写入 `docs/known_issues.md` 的新问题。

## Phase 0b — 最小 CLI 跑通

### 完成情况

- 新增 `budn_cad_runner` Python 包，作为 app server 后续调用的外部 CadQuery runner 原型。
- 实现最小 CLI：`--script`、`--project-root`、`--output-dir`、`--exports`、`--params`。
- 实现 loader：把 project root 加入 `sys.path`，用 `importlib` 加载目标 `.py`，读取 `REFS` 和 `build(params)`。
- 实现 executor：执行 `build(params)`，把 build 内异常归类为 `build_error`。
- 实现 Workplane tessellation：通过 `val().wrapped`、`BRepMesh_IncrementalMesh`、`TopExp_Explorer` 和 `BRep_Tool.Triangulation_s` 输出 face mesh。
- 实现最小 Assembly 支持：遍历 `Assembly.objects`，跳过 root object，输出统一 `parts[]`、`instance_path`、`transform`、`ref_text` 和 `object_kind`。
- 实现 manifest / dependency hash：通过 AST 解析 project 内 import 依赖，输出 `dependencies`、`deps_hash`、`params_hash` 和 `build_id`。
- 新增 `docs/cadquery-mvp/python-runner.md`，记录 MVP 手动 Python + CadQuery 环境要求。
- 新增 `tests/cadquery_runner.test.ts` 和 CadQuery fixtures，通过 `bun test` 调用 `python3.11 -m budn_cad_runner` 验证 CLI 行为。

### CadQuery API 验证

当前验证环境：

- Python：`python3.11`
- CadQuery：`2.7.0`
- cadquery-ocp：`7.8.1.1.post1`

已验证 API 行为：

- `Workplane.val().wrapped` 返回 `TopoDS_Solid`。
- `BRepMesh_IncrementalMesh` 能生成 face triangulation。
- `TopExp_Explorer(shape, TopAbs_FACE)` 能遍历 face。
- `.tag("outer_shell")` 与 `_getTagged("outer_shell")` 可用，返回 `Workplane`。
- `Assembly.add(..., metadata={"ref_text": "...", "object_kind": "..."})` 可保存 child metadata。
- `Assembly.objects` 是 dict，包含 root assembly key 和 child key；root object 需要跳过。
- `Location.toTuple()` 可复查 child translation；runner 使用 `location.wrapped.Transformation()` 生成 4x4 transform。

API 验证命令：

```bash
perl -e 'alarm 45; exec @ARGV' python3.11 - <<'PY'
import cadquery as cq
from OCP.BRep import BRep_Tool
from OCP.BRepMesh import BRepMesh_IncrementalMesh
from OCP.TopAbs import TopAbs_FACE
from OCP.TopExp import TopExp_Explorer
from OCP.TopLoc import TopLoc_Location
from OCP.TopoDS import TopoDS

wp = cq.Workplane('XY').box(1, 2, 3).tag('outer_shell')
shape = wp.val().wrapped
BRepMesh_IncrementalMesh(shape, 0.1, False, 0.5, True)
explorer = TopExp_Explorer(shape, TopAbs_FACE)
face = TopoDS.Face_s(explorer.Current())
triangulation = BRep_Tool.Triangulation_s(face, TopLoc_Location())
assembly = cq.Assembly(name='full_enclosure')
assembly.add(
    wp,
    name='top_lid',
    loc=cq.Location(cq.Vector(1, 2, 3)),
    metadata={'ref_text': '@part[top_lid]', 'object_kind': 'part'},
)
child = assembly.objects['top_lid']
print('cadquery_version=' + cq.__version__)
print('val_wrapped=' + type(shape).__name__)
print('brep_mesh_faces=' + str(triangulation.NbTriangles() > 0))
print('top_exp_explorer=' + str(explorer.More()))
print('tag_getTagged=' + type(wp._getTagged('outer_shell')).__name__)
print('assembly_objects_keys=' + ','.join(assembly.objects.keys()))
print('child_metadata_ref_text=' + child.metadata['ref_text'])
print('child_object_kind=' + child.metadata['object_kind'])
print('child_location_tuple=' + str(child.loc.toTuple()))
PY
```

API 验证输出：

```text
cadquery_version=2.7.0
val_wrapped=TopoDS_Solid
brep_mesh_faces=True
top_exp_explorer=True
tag_getTagged=Workplane
assembly_objects_keys=full_enclosure,top_lid
child_metadata_ref_text=@part[top_lid]
child_object_kind=part
child_location_tuple=((1.0, 2.0, 3.0), (0.0, -0.0, 0.0))
```

### 验证记录

- 红灯验证：`bun test tests/cadquery_runner.test.ts` 初次失败，失败原因为 `No module named budn_cad_runner`。
- 绿色验证：`bun test tests/cadquery_runner.test.ts` 通过，4 个测试、26 个断言。
- 单体 CLI 验证：`python3.11 -m budn_cad_runner --script parts/top_lid.py --project-root tests/fixtures/cadquery-runner/simple --output-dir <tmp> --exports ''` 输出 `status=success`、`unit=millimeter`、`root_ref_text=@part[top_lid]`、`root_object_kind=part`、`face_count=6`、bounding box 为 `[-40,-30,-4]` 到 `[40,30,4]`。
- Assembly CLI 验证：`python3.11 -m budn_cad_runner --script assemblies/full_enclosure.py --project-root tests/fixtures/cadquery-runner/assembly --output-dir <tmp> --exports ''` 输出 `root_ref_text=@assembly[full_enclosure]`，`parts[]` 包含 `full_enclosure/bottom_case`、`full_enclosure/top_lid`、`full_enclosure/pcb_main`。
- 依赖 hash 验证：修改 fixture 中 `components/dimensions.py` 后，`deps_hash` 与 `build_id` 均变化。
- 错误分类验证：fixture build 抛出 `ValueError` 时 exit code 为 `1`，stdout `status=build_error`，stderr 包含 traceback。
- 环境错误验证：系统 `python3` 指向 Python 3.9 且未安装 CadQuery 时，runner 返回 `status=runner_error` 和 `ModuleNotFoundError` JSON；实际 CadQuery 验证使用 `python3.11`。

### 遗留问题

- Phase 0b 未发现需要写入 `docs/known_issues.md` 的新问题。
- `--exports` 参数在 Phase 0b 仅保留 CLI 形态，实际 STEP / STL / 3MF 导出按 plan 留到 Phase 0c。
