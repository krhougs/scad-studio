# Agent Tool Call 能力补全实施计划

## 背景

budn' CadQuery Agent 的 MVP 目标是跑通：

```text
多 Chat 讨论
→ Agent 生成 CAD Plan
→ 用户确认执行
→ CadQuery 生成 / 修改模型
→ Viewer 选择 component / part / assembly / face / edge / vertex
→ Agent 基于选择继续修改
```

当前主链路已经具备 LLM provider、streaming、基础 tool loop、Agent / Chat / Selection / CadQuery protocol command 和 CadQuery staging 执行能力。但真正注册给 LLM 主动调用的 Agent tools 只有 `read_file` 与 `list_directory`，且能力仍是最小版本。`cadquery.execute`、`cadquery.preview`、`cadquery.result.get` 目前是 app server protocol command，不等同于 LLM 可主动调用的 tool call。

本计划目标是补齐 Agent 按 MVP PRD 建模所需的 tool call 能力，并重新定义 Inform / Plan / Execute 三种操作级别下的权限模型。

## 用户强制约束识别

- 必须检查现有 tool 的能力范围，并明确需要补齐的能力。
- 必须补全用户补充的缺失工具：
  - `read_file()`
  - `write_file()`
  - `patch_file()`
  - `copy_file()`
  - `update_chat_summary()`
  - `save_cad_plan()`
- 必须重新评估 Inform / Plan / Execute 三种模式下的 tool 权限模型。
- CadQuery `.py` 模型文件不得通过普通 `write_file()` / `patch_file()` 直接改写；模型生成 / 修改必须走 confirmation + staging + CadQuery tool 边界。
- `save_cad_plan()` 与 `update_chat_summary()` 是产品语义工具，不能简单降级为普通文件写入。
- Ref 处理必须遵守 MVP 5 层 Ref：component / part / assembly、instance、feature、face / edge / vertex；selector 只作为内部查找手段。

## 当前目标工具集合

本轮目标工具集合按职责分为四类：

| 类别 | 工具 | 目标 |
|---|---|---|
| 只读上下文 | `read_file()`、`list_directory()`、`search_files()`、`get_project_context()`、`get_selection()`、`resolve_ref()` | 让 Agent 能自主定位文件、理解项目结构与 Viewer Ref |
| Plan / Chat 语义持久化 | `save_cad_plan()`、`update_chat_summary()` | 让 Plan 与 Chat 上下文可追溯，不靠普通文件写入伪造产品状态 |
| 受限文件写入 | `write_file()`、`patch_file()`、`copy_file()` | 写说明文档、Ref Map、实验版本文件，全部受权限和确认范围约束 |
| CAD 专用 | `cadquery_analyze_source()`、`cadquery_check_source()`、`cadquery_dry_run()`、`cadquery_execute()`、`cadquery_get_result()`、`cadquery_resolve_selection()` | 覆盖 CadQuery 源码理解、静态检查、staging 试运行、确认执行、结果摘要和选择映射 |

## 权限模型目标

本计划会在 Phase 0 中验证并固化最终权限表。初始建议如下，执行时如发现与最新源码或产品文档冲突，必须先修正权限文档和测试，再实现工具。

| Operation | 允许工具 | 禁止事项 |
|---|---|---|
| Inform | `read_file()`、`list_directory()`、`search_files()`、`get_project_context()`、`get_selection()`、`resolve_ref()`、`cadquery_analyze_source()`、`cadquery_get_result()`、`cadquery_resolve_selection()`；`update_chat_summary()` 仅在 Phase 0 明确调用主体后决定是否暴露给 LLM | 禁止修改设计源文件；禁止执行 CadQuery；禁止生成 outputs |
| Plan | Inform 允许的只读工具 + `cadquery_check_source()` + `save_cad_plan()`；`cadquery_dry_run()` 只允许用户显式触发的预览产品动作使用，不允许 LLM 自动调用 | 禁止修改 `.py` 模型源；禁止修改对象说明 `.md`；禁止自动执行 CadQuery；禁止生成正式 outputs |
| Execute | 只读上下文工具、`cadquery_analyze_source()`、`cadquery_check_source()`、`cadquery_dry_run()`、`cadquery_execute()`、`cadquery_get_result()`、`cadquery_resolve_selection()`、受 confirmation 限制的 `write_file()` / `patch_file()` / `copy_file()`；`save_cad_plan()` 和 `update_chat_summary()` 是否可用必须由 Phase 0 逐工具权限表明确 | 禁止越过确认范围；禁止普通文件工具直接改 CadQuery `.py` 模型；禁止写 `outputs/` 之外的导出；禁止在一次 Execute run 中多次成功 commit |

`Auto` 不是独立权限级别。`Auto` 入口必须先产生本轮 operation decision，再按判定后的 Inform / Plan / Execute 暴露工具。判定完成前只允许只读上下文工具；不得在未判定状态下暴露 `save_cad_plan()`、普通写入工具或任何会触发 CadQuery runner 的工具。

关于 CadQuery 预览：必须区分“预览已有文件”和“试运行拟议代码”。前者是读取当前 workspace 中已有 `.py` 的只读预览产品动作；后者是 `cadquery_dry_run()`，会在 staging 中执行拟议代码但不提交真实文件。Plan 卡片上的“预览”必须明确使用哪一种路径，禁止用同一个工具名混合两种行为。

## CadQuery 专用工具行为合同

本节是 Phase 0 必须固化的 CadQuery 工具行为基线。后续实现如果需要调整字段名，必须先更新 canonical schema、测试和本节描述。

### `cadquery_analyze_source()`

用途：理解现有 CadQuery 源码和项目语义，不执行 Python。

输入：

```json
{
  "target_path": "parts/top_lid.py",
  "include_paired_doc": true,
  "include_dependencies": true
}
```

执行过程：

1. 校验 `target_path` 位于 workspace 内，且是 `components/`、`parts/` 或 `assemblies/` 下的 `.py` 文件。
2. 读取目标源码，提取 `build(params)` 是否存在、`REFS` 是否存在、声明对象类型、import 依赖和明显的 project-local import。
3. 查找同名 `.md`，提取标题、Ref Map 段落、可编辑区域和保护区域摘要。
4. 只做文本 / AST 级分析，不调用 `budn_cad_runner`，不生成 mesh。

输出：

```json
{
  "status": "ok",
  "target_path": "parts/top_lid.py",
  "target_type": "part",
  "has_build_function": true,
  "has_refs": true,
  "paired_doc_path": "parts/top_lid.md",
  "local_dependencies": ["components/pcb_main.py"],
  "ref_keys": ["outer_shell", "top_surface"],
  "warnings": []
}
```

权限：

- Inform / Plan / Execute 均可用。
- 只读，无 confirmation。

禁止事项：

- 禁止执行 Python。
- 禁止根据文件名凭空推断缺失的 `REFS`。
- 禁止把 selector 暴露为用户可见 Ref。

### `cadquery_check_source()`

用途：对 LLM 拟议的完整 CadQuery 源码做执行前静态合同检查。

输入：

```json
{
  "target_path": "parts/top_lid.py",
  "target_type": "part",
  "code": "complete Python CadQuery source"
}
```

执行过程：

1. 检查源码是完整文件，而不是局部片段。
2. 检查存在 `build(params=None)` 和 `REFS`。
3. 检查 target type 与 `REFS` 顶层对象类型一致。
4. 检查 project-local import 是否位于允许目录。
5. 检查明显危险调用和普通文件系统写入调用。
6. 不执行 Python，不写 staging。

输出：

```json
{
  "status": "ok",
  "contract": {
    "target_type_matches": true,
    "has_build_function": true,
    "has_refs": true,
    "unsafe_calls": []
  },
  "warnings": []
}
```

权限：

- Plan 可用于检查拟议代码是否满足合同，但不能执行。
- Execute 可在 `cadquery_dry_run()` 和 `cadquery_execute()` 前使用。

禁止事项：

- 禁止替代 `cadquery_dry_run()` 判断几何是否真的可构建。
- 禁止把静态检查通过解释为执行成功。

### `cadquery_dry_run()`

用途：在 staging 中执行拟议 CadQuery 代码，验证能否构建可预览模型，但不改真实 workspace，不写正式 outputs。

输入：

```json
{
  "target_path": "parts/top_lid.py",
  "target_type": "part",
  "code": "complete Python CadQuery source",
  "params_json": "{}",
  "selection_context": ["@feature[top_lid.top_surface]"],
  "preview_quality": "fast"
}
```

执行过程：

1. 校验 target、target type、代码合同和 workspace 路径。
2. 创建 run-scoped staging 目录并镜像 workspace。
3. 将拟议代码写入 staging 内的 `target_path`。
4. 调用 `budn_cad_runner` 执行 staging 文件。
5. 解析 runner 输出，校验 root object kind、manifest、feature map 和 topology summary。
6. 将 mesh 放入临时 result cache，生成 dry-run `result_id`，供 Viewer 预览。
7. 清理 staging，或仅在 run 生命周期内保留调试信息。
8. 不回写真实 `.py`，不写正式 `outputs/`。

成功输出：

```json
{
  "status": "ok",
  "result_id": "dry_cq_123",
  "build_id": "sha256:...",
  "root_object_kind": "part",
  "summary": {
    "part_count": 1,
    "face_count": 18,
    "edge_count": 42,
    "vertex_count": 24,
    "features": ["outer_shell", "top_surface"]
  },
  "warnings": []
}
```

失败输出：

```json
{
  "status": "error",
  "error_type": "cadquery_build_error",
  "message": "Workplane object has no attribute ...",
  "traceback": "...",
  "retry_allowed": true
}
```

权限：

- Inform 不允许。
- Plan 中不允许 LLM 自动调用；只允许用户显式点击 Plan 预览时由产品流触发。
- Execute 中允许，用于提交前自检和失败修复。

禁止事项：

- 禁止 commit 真实 workspace 文件。
- 禁止写正式 `outputs/`。
- 禁止把 dry-run `result_id` 当作长期设计真相。
- 禁止在没有完整源码时执行。

### `cadquery_execute()`

用途：在 Execute + confirmation 下完成 CadQuery 模型写入、执行、导出和 commit。

输入：

```json
{
  "target_path": "parts/top_lid.py",
  "target_type": "part",
  "code": "complete Python CadQuery source",
  "params_json": "{}",
  "export_formats": ["step"],
  "export_targets": ["outputs/top_lid.step"]
}
```

执行过程：

1. 校验存在 `AgentCadQueryConfirmation`。
2. 校验 `target_path` 位于 `affected_files` 或 `new_files`。
3. 校验 `export_targets` 位于 confirmed `export_targets`，且都在 `outputs/` 下。
4. 执行 `cadquery_check_source()`。
5. 使用现有 staging 写入代码、执行 runner、生成 mesh 和 exports。
6. runner 成功且冲突检测通过后，回写真实 target 和 confirmed outputs。
7. 缓存 result，推送 `agent.tool_result` 和 `agent.mesh_ready`。
8. 单次 Execute run 中第一次成功 commit 后结束 CadQuery tool loop。
9. 后续 `.md` / Ref Map 更新必须通过确认范围内的 `patch_file()` 完成。

成功输出：

```json
{
  "status": "ok",
  "result_id": "cq_123",
  "build_id": "sha256:...",
  "committed_files": ["parts/top_lid.py"],
  "exports": ["outputs/top_lid.step"],
  "summary": {
    "part_count": 1,
    "face_count": 18,
    "edge_count": 42,
    "vertex_count": 24
  }
}
```

失败输出：

```json
{
  "status": "error",
  "error_type": "file_conflict | python_import_error | cadquery_build_error | topology_mapping_error | export_error | timeout",
  "message": "human-readable failure",
  "traceback": "optional Python traceback",
  "retry_allowed": true
}
```

权限：

- 只允许 Execute。
- 必须有 confirmation。
- 必须受 exact output scope 限制。

禁止事项：

- 禁止在 confirmation 外写文件。
- 禁止一次 Execute run 多次成功 commit。
- 禁止用普通文件工具改写同一 `.py` 后再执行。
- 禁止写 `outputs/` 之外的导出。

### `cadquery_get_result()`

用途：按 `result_id` 读取 CadQuery 轻量结果摘要，供 Agent 理解模型结果，不向 LLM 传递完整 mesh 大数组。

输入：

```json
{
  "result_id": "cq_123",
  "include_feature_map": true,
  "include_exports": true
}
```

执行过程：

1. 从 CadQuery result cache 查找 result。
2. 返回 root ref、root object kind、part / instance 摘要、feature map 摘要、拓扑统计和 exports。
3. 不返回完整 positions / normals / polyline 大数组。

输出：

```json
{
  "status": "ok",
  "result_id": "cq_123",
  "build_id": "sha256:...",
  "root_ref_text": "@part[top_lid]",
  "root_object_kind": "part",
  "parts": [
    {
      "ref_text": "@part[top_lid]",
      "object_kind": "part",
      "instance_path": null,
      "features": ["outer_shell", "top_surface"],
      "face_count": 18,
      "edge_count": 42,
      "vertex_count": 24
    }
  ],
  "exports": ["outputs/top_lid.step"]
}
```

权限：

- Inform / Plan / Execute 均可用。
- 只读。

禁止事项：

- 禁止返回完整 mesh 大数组给 LLM。
- 禁止把不存在或已过期 result 当作当前 selection truth。

### `cadquery_resolve_selection()`

用途：把 Viewer selection、raw face / edge / vertex 或 `result_id` 上的临时几何位置映射到 owner、feature 和稳定性风险。

输入：

```json
{
  "result_id": "cq_123",
  "selection_ref": "@face[top_lid:f_123]"
}
```

执行过程：

1. 读取 result cache 中的 topology summary 和 feature map。
2. 识别 owner ref、owner object kind、instance path、candidate feature。
3. raw geometry 若能稳定映射到 feature，返回 `@feature[...]`。
4. 若无法稳定映射，保留 raw ref 并返回风险说明。
5. selector 只作为内部 metadata，不作为用户可见 Ref 输出。

输出：

```json
{
  "status": "ok",
  "selection_ref": "@face[top_lid:f_123]",
  "owner_ref_text": "@part[top_lid]",
  "owner_object_kind": "part",
  "candidate_feature_ref": "@feature[top_lid.top_surface]",
  "stable_ref": "@feature[top_lid.top_surface]",
  "ambiguous": false,
  "risks": []
}
```

权限：

- Inform / Plan / Execute 均可用。
- 只读。

禁止事项：

- 禁止把 raw face / edge / vertex 当作长期真相。
- 禁止输出 `@selector[...]` 作为用户可见 Ref。


## 执行通用要求

每个 Phase 都必须遵循以下循环：

```text
干活 → 独立 subagent review → 回归验证 → 修复 block 项 → commit → 更新 plan-00-result.md → 自动推进下一 Phase
```

Review 要求：

- 每个 Phase 完成编码后，必须调用独立 subagent 执行 review。
- Review subagent 上下文必须包含：
  1. 当前 Phase 的阶段目标与验收标准。
  2. 本完整 `plan-00.md`。
  3. 本次变更 diff 或涉及文件清单。
- Review 不写文件，不污染 plan 存档。

测试要求：

- Rust 优先运行聚焦测试，再运行相关 crate 测试。
- Web 优先运行相关 unit tests；涉及 UI 流程时补充 Playwright。
- 不新增 Python 辅助脚本；CadQuery runner 仍是唯一 Python 例外。

## Phase 0 — Tool 能力盘点与权限合同

### 输入

- `docs/cadquery-mvp/init.md`
- `docs/cadquery-mvp/ref_components_parts_assemblies.md`
- `docs/cadquery-mvp/agent-system-prompt.md`
- `prompt-archives/2026042700-cadquery-mvp-design/plan-00.md`
- `prompt-archives/2026042801-agent-chat-redesign/plan-00.md`
- 当前源码中的 Agent tool loop、protocol command、dispatcher、Chat store、Selection snapshot 和 CadQuery staging 实现。

### 前序目标保护

这是第一个 Phase，无前序实现目标需要保护。必须保护现有 CadQuery staging 安全边界、单 running agent session、LLM provider 配置方式和现有 Chat / Agent / Selection protocol 不被破坏。

### 操作步骤

1. 梳理当前“LLM 可主动调用 tool”与“app server protocol command”的区别，形成能力矩阵。
2. 检查现有 `read_file()` 与 `list_directory()`：
   - 参数是否足够表达路径、范围、分页、过滤。
   - 输出是否包含截断信息、文件大小、hash、错误类型。
   - 是否满足 PathHandle / workspace path policy。
3. 检查现有 Agent 运行路径：
   - Inform / Plan / Auto 是否使用同一 tool set。
   - Execute 是否完全绕过 tool loop。
   - LLM tool call 是否被记录到 Chat JSONL 和 Agent push events。
4. 输出最终权限合同：
   - 每个 operation 可用工具。
   - 每个工具允许读写的路径范围。
   - 每个工具是否需要 confirmation。
   - 每个工具是否允许在 LLM 自动 tool loop 中出现。
5. 输出目标工具的 canonical schema：
   - 输入 JSON schema。
   - 成功 tool result schema。
   - 错误 tool result schema。
   - permission denied / conflict / not found / cancelled 等通用错误字段。
6. 明确 `Auto` operation decision 流程：
   - 未判定前只能暴露只读上下文工具。
   - 判定为 Inform / Plan / Execute 后按对应权限表过滤工具。
   - 自然语言确认不能直接把未确认的 Auto turn 提升为 Execute。
7. 若发现 `docs/cadquery-mvp/agent-system-prompt.md`、历史计划和当前代码权限模型冲突，优先按 MVP 安全边界修正文档和测试预期。

### 验收标准

- 有明确的能力矩阵，覆盖所有当前工具、目标工具和相关 app server command。
- 有明确的 canonical tool schema，覆盖所有目标工具的输入、成功结果、错误结果和通用错误字段。
- 权限合同明确回答：
  - `save_cad_plan()` 为什么不是普通 `write_file()`。
  - `update_chat_summary()` 为什么不是普通 JSONL 文件写入。
  - `write_file()` / `patch_file()` 为什么不能直接改 CadQuery `.py` 模型。
  - Plan 卡片的“预览已有文件”和 `cadquery_dry_run()` 的“试运行拟议代码”如何与 Agent Plan 模式区分。
  - `Auto` 如何先判定 operation，再按判定结果暴露工具。
- 相关测试至少覆盖 tool registry 中各工具的 operation allow / deny 结果。
- `plan-00-result.md` 记录 Phase 0 的最终权限表与任何文档修正。

## Phase 1 — Tool Registry 与统一执行入口

### 输入

- Phase 0 的能力矩阵与权限合同。
- 现有 LLM tool definition、tool executor、tool loop。
- 现有 ChatStore、Selection snapshot、CadQuery result cache 和 dispatcher worker 上下文。

### 前序目标保护

实现 Phase 1 时必须保护 Phase 0 固化的权限合同。禁止为了快速接入工具，把所有工具无差别暴露给 Inform / Plan / Auto；禁止让工具执行器绕过 workspace path policy。

### 操作步骤

1. 将 Agent tools 从硬编码 `Vec<LlmToolDefinition>` 扩展为可描述权限、参数 schema、输出类型和是否需要 confirmation 的 registry。
2. 为工具执行引入统一上下文：
   - workspace root。
   - current session id。
   - current run id。
   - operation level。
   - selection snapshot。
   - optional confirmation。
   - push event sink。
3. 在 tool loop 入口按 operation 过滤 tool definitions。
4. 为 `Auto` 增加 operation decision 阶段，确保未判定前只读，判定后再刷新本轮可用工具集合。
5. 执行 tool 前做一次权限判定；即使 LLM 构造了未授权 tool call，也必须返回结构化拒绝结果，而不是执行。
6. 把 LLM tool call start / result 统一接入 Agent push event 和 Chat JSONL 记录。CadQuery tool 不能是唯一被记录的 tool。
7. 保留现有 `agent.token` streaming 行为，避免工具事件影响 token 输出。

### 验收标准

- Inform / Plan / Execute 获取到的 tool definitions 与 Phase 0 权限合同一致。
- Auto 未判定前只能获取只读上下文工具；判定后按目标 operation 刷新工具集合。
- 未授权 tool call 返回结构化 permission denied tool result，并记录到 Chat。
- 已授权 tool call 会产生 `agent.tool_start` / `agent.tool_result`。
- 现有 CadQuery Execute push event 与 Chat tool result 行为不回退。
- 相关测试覆盖：
  - tool definitions 按 operation 过滤。
  - 未授权工具不会执行副作用。
  - tool call 记录进入 Chat history。

## Phase 2 — 只读上下文工具补齐

### 输入

- Phase 1 的 tool registry 与执行上下文。
- 当前 workspace read/list 能力、Selection snapshot、CadQuery mesh payload / SelectionRef 数据结构。
- Ref PRD 中的 component / part / assembly / instance / feature / raw geometry 映射规则。

### 前序目标保护

实现 Phase 2 时必须保护 Phase 1 的权限入口与事件记录。所有新增工具必须先通过 registry 权限过滤，不能各自实现一套路径校验或事件记录。

### 操作步骤

1. 升级 `read_file()`：
   - 支持读取范围或大小限制。
   - 输出截断状态、文件大小和稳定 hash。
   - 拒绝 workspace 外路径和不适合文本读取的文件。
2. 升级 `list_directory()`：
   - 支持非递归与受限递归。
   - 支持 pattern / file kind 过滤。
   - 输出截断状态。
3. 新增 `search_files()`：
   - 支持 query、路径范围、文件 pattern。
   - 默认排除 `outputs/`、staging、二进制和过大文件。
4. 新增 `get_project_context()`：
   - 汇总 components / parts / assemblies / plans / chats 的可读概览。
   - 不读取大文件正文，只返回路径、对象类型、是否存在配对 `.md`、可能的入口文件。
5. 新增 `get_selection()`：
   - 返回当前 selection snapshot、active index、context refs 和 result/build id。
   - 输出格式与 `SelectionRef` 字段一致。
6. 新增 `resolve_ref()`：
   - 把 `@component[...]` / `@part[...]` / `@assembly[...]` 映射到目标 `.py` / `.md`。
   - 把 `@feature[...]` 映射到 owner 文件与 `REFS` 条目。
   - raw face / edge / vertex 优先返回 candidate feature；不能稳定映射时保留 raw ref 并标记风险。

### 验收标准

- Agent 在无 Viewer 选择时能通过 `get_project_context()` 与 `search_files()` 找到候选模型文件。
- Agent 在有 Viewer 选择时能通过 `get_selection()` 与 `resolve_ref()` 找到 owner 文件、candidate feature 与稳定性风险。
- `read_file()` 的输出足够后续 `patch_file()` 做冲突检测。
- 所有只读工具在 Inform / Plan / Execute 可用，且不产生文件写入。
- 测试覆盖路径逃逸、输出截断、raw geometry 无稳定 feature、feature ref 映射失败等边界。

## Phase 3 — CAD Plan 与 Chat 语义持久化工具

### 输入

- Phase 1 的 tool registry。
- Phase 2 的只读上下文工具。
- 当前 ChatStore、AgentPlanProposedEvent、AgentCadQueryConfirmation、Plan extraction 逻辑。
- 已知问题：`AgentCadQueryConfirmation.plan_ref` 尚未持久绑定 CAD Plan 文件。

### 前序目标保护

实现 Phase 3 时必须保护 Phase 2 的只读工具无副作用边界。Plan 持久化不能让 Agent 绕过 confirmation 改模型源文件；Chat summary 更新不能成为任意 JSONL 写入后门。

### 操作步骤

1. 新增 `save_cad_plan()`：
   - 只写入 `plans/` 下的 Markdown CAD Plan。
   - 输入包含标题、目标 Ref、resolved target、affected files、CadQuery strategy、风险、验收方式和 execution boundary。
   - 输出 `plan_ref`、展示路径、hash 和 summary。
2. 将 Plan proposal 流程改为使用 `save_cad_plan()` 的结果：
   - `agent.plan_proposed` 事件需要携带足够信息让前端确认时填充 `plan_ref`。
   - 前端确认不再重新猜测 affected files / export targets，而是优先使用 Plan proposal 的结构化结果。
3. 新增 `update_chat_summary()`：
   - 通过 ChatStore API 更新 session summary、goal、related files、open questions 或等价 meta 数据。
   - 不允许 LLM 直接写 `chats/*.jsonl` 文件。
4. 明确 `save_cad_plan()` 与普通 `write_file()` 的关系：
   - Plan 模式写计划只允许走 `save_cad_plan()`。
   - 普通 `write_file()` 不承担 Plan 持久化职责。
5. 更新 system prompt 或相关文档，明确 Plan 阶段可保存 Plan，也可做 `cadquery_check_source()` 静态合同检查，但仍不修改模型源、不执行 CadQuery runner。

### 验收标准

- 用户请求方案时，Agent 能生成并保存 `plans/*.md`。
- `agent.plan_proposed` 与后续 confirmation 能绑定同一个 `plan_ref`。
- Chat history 能展示或追溯 Plan 工具调用结果。
- `update_chat_summary()` 不允许修改任意项目文件。
- Plan 阶段依然不能执行 CadQuery runner，也不能修改 components / parts / assemblies 下的 `.py` 或对象说明 `.md`。

## Phase 4 — 受限文件写入工具

### 输入

- Phase 0 的权限合同。
- Phase 1 的权限执行入口。
- Phase 3 的 Plan / Chat 语义工具。
- 当前 workspace write path 校验与 CadQuery confirmation 数据结构。

### 前序目标保护

实现 Phase 4 时必须保护 Phase 3 的 Plan 可追溯目标：普通 `write_file()` / `patch_file()` 不能替代 `save_cad_plan()`，也不能写入 Chat JSONL。必须保护 `.py` 模型只能通过 CadQuery tool 修改的边界。

### 操作步骤

1. 新增 `write_file()`：
   - 只写文本文件。
   - Plan 模式默认不可用，避免与 `save_cad_plan()` 重叠。
   - Execute 模式仅允许写 confirmation 范围内的新文件或已确认 affected files 中的非模型文本文件。
2. 新增 `patch_file()`：
   - 使用确定性 patch 语义，要求基于现有内容或 hash 做冲突检测。
   - 默认用于 `.md` 说明、Ref Map、执行记录等文本更新。
   - 拒绝对 CadQuery 模型 `.py` 文件做普通 patch。
3. 新增 `copy_file()`：
   - 支持实验版本和 variant 文件复制。
   - 复制 `.py` 模型文件只能在 Execute 模式、目标在 confirmed `new_files` 内时允许。
   - 复制 `.py` 只能是 byte-for-byte 复制；复制后的 `.py` 如需修改，仍必须通过 `cadquery_execute()` 或后续等价的 CadQuery 执行工具完成。
   - 复制配对 `.md` 时必须保持路径和命名可追溯。
4. 所有写入工具共享 workspace path policy、symlink escape 校验、expected hash 或等价冲突检测。
5. 写入工具的 tool result 必须返回写入路径、hash、是否创建新文件、是否发生冲突。

### 验收标准

- Inform / Plan 中调用 `write_file()`、`patch_file()`、`copy_file()` 会被拒绝并记录 tool result。
- Execute 中未包含在 confirmation 范围内的写入会被拒绝。
- `write_file()` / `patch_file()` 不能修改 CadQuery `.py` 模型源。
- `copy_file()` 可以在已确认实验版本范围内复制 `.py` / `.md` 文件，并保留可追溯 result；复制 `.py` 不允许同时改写内容。
- 文件冲突、目标已存在、路径逃逸、二进制写入等边界均有测试覆盖。

## Phase 5 — CadQuery 专用工具与执行边界

### 输入

- Phase 0 固化的 CadQuery 专用工具行为合同。
- Phase 1 的 tool registry。
- Phase 4 的写入权限与 confirmation 校验。
- 当前 CadQuery staging、runner、result cache、`cadquery.execute` / `cadquery.preview` / `cadquery.result.get` protocol command。

### 前序目标保护

实现 Phase 5 时必须保护 Phase 4 的普通文件写入边界。CadQuery `.py` 模型源修改只能通过 `cadquery_execute()` 或后续等价的 CadQuery 执行工具完成；`cadquery_dry_run()` 不能写真实 workspace，也不能生成正式 outputs。

### 操作步骤

1. 新增 `cadquery_analyze_source()`：
   - 只读取现有 `.py`、配对 `.md` 和可解析的 project-local 依赖。
   - 返回对象类型、`build(params)`、`REFS`、import、配对文档和潜在问题摘要。
   - 不执行 `budn_cad_runner`，不生成 mesh，不写文件。
2. 新增 `cadquery_check_source()`：
   - 对 LLM 拟议的完整 CadQuery 源码做静态合同检查。
   - 校验必须的入口、可接受 import、用户可见 Ref 层级、目标路径和配对文档要求。
   - 不执行 Python，不写 staging，不生成 result cache。
3. 新增 `cadquery_dry_run()`：
   - 在 staging 中写入拟议代码并调用 CadQuery runner。
   - 不回写真实 workspace，不生成正式 outputs，不更新 `.md` 或 Ref Map。
   - 成功时返回 dry-run `result_id`、`build_id`、mesh summary、topology summary、feature map summary 和 warnings。
   - 失败时返回 `error_type`、`message`、`traceback`、`retry_allowed`、`diagnostics` 和不产生副作用的证明字段。
   - 仅 Execute 中允许 LLM 调用；Plan 卡片预览只能作为用户显式产品动作触发同等 staging 能力。
4. 新增 `cadquery_execute()`：
   - Execute 模式专用，必须要求 `AgentCadQueryConfirmation`。
   - 输入包含 target path、target type、完整 CadQuery code、params、export formats、expected outputs、plan_ref、reason。
   - 执行必须使用现有 staging、exact outputs scope、冲突检测和原子 commit。
   - 成功时回写确认范围内的 `.py`、正式 outputs、manifest、topology metadata、feature map 和 result cache。
5. 让 Execute 进入工具循环，而不是只做一次“LLM 输出 fenced code → host 执行”。
6. CadQuery 失败时，把 Python traceback、错误分类和允许重试的信息作为 tool result 回送给 LLM。
7. 设置重试上限，避免无限修正循环；失败重试只允许在尚未成功 commit 前发生。
8. 规定单次 Execute run 中 `cadquery_execute()` 最多允许一次成功 commit：
   - 第一次成功 commit 后必须结束 CadQuery tool loop。
   - 若用户需要第二次建模，必须重新生成 Plan 或重新确认范围。
   - 成功 commit 后再次调用 `cadquery_execute()` 必须返回 permission denied tool result。
9. Execute 成功后必须更新确认范围内对应 `.md` 说明或 Ref Map：
   - 通过 `patch_file()` 更新执行记录、Ref Map 或建模假设。
   - 相关 `.md` 或 Ref Map 必须在 confirmation 的 affected files 或 new files 中，除非 Phase 0 明确规定受控例外。
   - 如果 `.md` 或 Ref Map 更新失败，必须返回用户可见错误，不得静默宣称执行完整成功。
10. 新增 `cadquery_get_result()`：
   - 读取 dry-run 或 execute result cache 中的轻量结果摘要。
   - 只返回 result metadata、mesh summary、topology summary、feature map summary、exports 和 diagnostics。
   - 不向 LLM 返回完整 mesh 顶点、三角面数组或大体量二进制内容。
   - 评估现有 `cadquery.result.get` protocol command：若可复用则封装为 tool executor；若保留为 protocol-only，必须在能力矩阵中说明原因。
11. 新增 `cadquery_resolve_selection()`：
   - 将 Viewer selection、raw geometry 或 result-local id 映射为 owner、feature、stable ref candidate 和稳定性风险。
   - 用户可见输出只允许 component / part / assembly、instance、feature、face / edge / vertex 五类。
   - selector 只能作为内部诊断字段，不能作为 MVP 用户可见 Ref 层级。
12. 明确 `cadquery.preview` protocol command 的定位：
   - 预览已有 workspace 文件时可以继续走只读产品动作。
   - 试运行拟议代码必须走 `cadquery_dry_run()` 的 staging 语义。
   - 两者都不能绕过 Execute confirmation 写真实 `.py` 或正式 outputs。

### 验收标准

- `cadquery_analyze_source()`、`cadquery_check_source()`、`cadquery_dry_run()`、`cadquery_execute()`、`cadquery_get_result()`、`cadquery_resolve_selection()` 均有 canonical schema、成功结果、错误结果和权限测试。
- Execute 中 LLM 可以调用 `cadquery_dry_run()` 与 `cadquery_execute()`，工具执行成功后返回 result id / build id / mesh summary / exports。
- CadQuery build error 会作为 tool result 返回给 LLM，LLM 可在受限轮数内修正并重试。
- 单次 Execute run 最多一次成功 CadQuery commit；成功后再次调用 `cadquery_execute()` 被拒绝。
- 没有 confirmation 时 `cadquery_execute()` 必须拒绝。
- `cadquery_dry_run()` 不污染真实 workspace，不写 `outputs/`，不会绕过 Execute confirmation。
- `cadquery_get_result()` 不返回完整 mesh 大数组。
- `cadquery_resolve_selection()` 不把 selector 或 subshape 作为 MVP 用户可见 Ref 层级。
- 现有 `cadquery.result.get` 已被明确评估，能力矩阵记录它是 tool 包装、protocol-only，还是需要替换。
- Execute 成功后对应 `.md` 说明或 Ref Map 被更新；更新失败时产生用户可见错误。
- 现有 CadQuery staging 回滚、output exact scope、Python import error mapping 和 result cache 行为不被破坏。

## Phase 6 — 前端确认流与协议补强

### 输入

- Phase 3 的 Plan 持久化结果。
- Phase 5 的 CadQuery 专用工具、dry-run、execute result 和现有 preview command 定位。
- 当前 `ChatZone`、`chat-actions.ts`、`cadquery-agent-scope.ts` 和 wasm bridge client。

### 前序目标保护

实现 Phase 6 时必须保护 Phase 5 的后端安全边界。前端只能展示和确认后端结构化结果，不能重新从 prompt 或 selection 猜测目标范围。

### 操作步骤

1. 调整 Plan confirmation card 数据来源：
   - 优先使用后端 `agent.plan_proposed` 的 target、affected files、new files、export targets、plan_ref。
   - 不再在确认时用 prompt 或 selection 重新推断写入范围。
2. 调整 preview 按钮：
   - 预览已有文件时使用 Phase 5 定义的只读 preview 路径。
   - 预览拟议代码时使用 Phase 5 定义的 `cadquery_dry_run()` staging 路径。
   - UI 文案明确预览不提交。
3. 调整确认按钮：
   - 构造 `AgentCadQueryConfirmation` 时保留 plan_ref。
   - affected / new / export 范围与 Plan proposal 保持一致。
4. Chat UI 展示非 CadQuery 工具事件：
   - read/search/context tools 可以折叠显示。
   - write/patch/copy/save plan/cadquery 工具需要可追溯显示。
5. 如协议缺少字段，补充 protocol / generated TypeScript / wasm bridge 并更新快照测试。

### 验收标准

- Plan 卡片确认后的 `AgentCadQueryConfirmation` 包含 `plan_ref`。
- 前端不再通过 prompt 关键词或 selection 临时构造影响范围。
- 用户可以看到 Plan 保存、文件写入、CadQuery 执行和失败的结构化工具结果。
- 现有无选择聊天、context pills、Plan 卡片和 CadQuery viewer 流程不回退。

## Phase 7 — 权限模型回归、文档同步与端到端验证

### 输入

- Phase 0-6 的实现结果。
- `docs/cadquery-mvp/agent-system-prompt.md`
- `docs/known_issues.md`
- 相关 Rust / Web / Playwright 测试。

### 前序目标保护

实现 Phase 7 时必须保护前面所有 Phase 已收敛的权限、追溯和 staging 边界。禁止为了通过端到端测试放宽权限或恢复 prompt 关键词推断。

### 操作步骤

1. 更新 Agent system prompt 中的 tool permission rules，使其与最终权限合同一致。
2. 更新 CadQuery MVP 文档中关于 tool call、Plan 持久化、preview 和 confirmation 的描述。
3. 检查 `docs/known_issues.md`：
   - 若 `plan_ref` 持久绑定已解决，更新该记录的当前处理方式。
   - 若仍有真实 blocker，补充新记录。
4. 运行聚焦回归：
   - Rust core agent tool tests。
   - Rust host dispatcher roundtrip tests。
   - Protocol / wasm bridge tests。
   - Web chat action / chat zone / cadquery agent scope tests。
5. 运行端到端验收路径：
   - 无选择 Inform。
   - 生成并保存 Plan。
   - 用户确认 Execute。
   - CadQuery 执行成功并显示 Viewer。
   - 确认范围内的对象说明 `.md` 或 Ref Map 被更新。
   - Viewer selection 进入下一轮 Agent 上下文。
   - CadQuery 失败后 LLM 收到 tool result 并受限重试。
6. 更新 `plan-00-result.md`，记录所有 Phase 结果、测试命令、遗留风险和最终权限表。

### 验收标准

- 文档、system prompt、权限测试和运行时行为一致。
- `rg` 检查不再出现误导 MVP 的 selector / subshape 用户可见 Ref 描述。
- 所有新增工具都有 allow / deny 测试和至少一个成功路径测试。
- 端到端路径证明 Agent 可以按 MVP PRD 进行建模，而不是只返回文本或单次 codegen。
