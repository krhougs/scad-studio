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

## Phase 0c — 完整 runner + Rust CadQuery 集成

### 完成情况

- 扩展 `budn_cad_runner`，输出统一 `parts[]` schema，包含 face / edge / vertex topology、`feature_map`、exports、metadata、manifest、dependencies、`deps_hash`、`build_id`、root / part 的 `ref_text` 与 `object_kind`。
- 新增安全 selector parser、feature ref mapper 和 exporter；selector 不使用 `eval()`，导出前先确认输出路径真实位置仍在 project root 内。
- 扩展 Assembly 处理：递归展开 nested Assembly，保留完整 `instance_path`，组合父子 transform，并通过 child metadata / refs 保留 part 与 feature 映射。
- 扩展 `app-server-protocol`：`WIRE_VERSION` 升到 2，新增 CadQuery command / response、`CadQueryMeshPayload`、`CadQueryResultReady`、`CadQueryObjectKind`、edge / vertex topology、CadQuery capability 字段。
- 新增 Rust runner JSON 解析与校验：验证 `unit=millimeter`、`sha256:` build_id、manifest hash、dependencies、exports / `export_hashes`、topology 索引范围、有限 `f32` 和扁平数组长度。
- 在 `app-server-core/src/cadquery/` 实现 CadQuery 子进程调用与 staging 写入：执行成功后才回写目标文件和 outputs，执行失败或冲突时不污染真实 workspace。
- 修正 staging 时序：目标文件 baseline 在复制 workspace 前捕获；回写 target 和 outputs 前复查同一 baseline，复制期间或执行期间发生外部修改时返回 file conflict。
- 修正 outputs 回写路径安全：回写前逐级检查目标父目录，拒绝 workspace 内 `outputs` 符号链接逃逸；已覆盖外部目录不产生 artifact 的回归测试。
- 扩展 app-server-host dispatcher、`studio-common::ManagedClient` 和桌面协议 client，使 CadQuery preview / execute / result get 走同一 app server protocol 路径。
- 扩展 `studio-web-wasm`：新增按 `result_id` 存取的 CadQuery side buffer 和 `CadQueryMeshHandle`，JS 可见事件只暴露轻量 `CadQueryResultReady`，大数组通过 handle getter 读取。
- 更新 TypeScript protocol package、wasm generated 产物、Web handshake 版本和相关单元测试。

### Review 与修复记录

- 第一轮独立 review 发现多项阻断风险：nested Assembly transform、`commit_outputs()` baseline、protocol `build_id` / `unit` 校验、Web handshake 版本、TS protocol 类型、outside export path 绝对 fallback。均已修复并回归。
- 第二轮独立 review 发现 outside export path 写入前拒绝不充分、topology 自索引未校验、Assembly child `feature_map` 为空。均以红绿回归方式修复。
- 第三轮独立 review 发现 Rust 未校验 runner JSON 的 `exports` / `manifest.export_hashes`，以及 staging baseline 捕获时机在 workspace copy 之后。均已修复并补充测试。
- 第四轮独立 review 发现 CadQuery outputs 回写可通过 workspace 内 `outputs` 符号链接写出 workspace。已补充 `cadquery_staging_rejects_output_symlink_escape` 红灯测试，并修复为回写前拒绝符号链接父目录。
- 最终独立 review 结论：未发现阻塞 Phase 0c 提交的问题。残余风险仅为 prepare 与最终普通路径写入之间的本地并发 TOCTOU，已记录到 `docs/known_issues.md`。

### 验证记录

- `cargo test -p app-server-core --test cadquery_tests`：16 个测试通过。
- `cargo test --workspace`：通过。
- `cargo test -p app-server-protocol --test borsh_payload_roundtrip_tests`：12 个测试通过。
- `cargo test -p studio-web-wasm --test wasm_bridge_smoke --target wasm32-unknown-unknown --no-run`：通过。
- `cargo check -p app-server-protocol-wasm --target wasm32-unknown-unknown`：通过。
- `cargo check -p studio-web-wasm --target wasm32-unknown-unknown`：通过。
- `bun test tests/cadquery_runner.test.ts`：9 个测试、57 个断言通过。
- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run --cwd packages/studio-web test:unit`：20 个文件、98 个测试通过。
- `bun run protocol:build`：通过。
- `bun run check:wasm-bindgen`：通过。
- `bun run web:build`：构建成功，仍有既有 Vite 大 chunk warning。
- `rustfmt --edition 2024 --config skip_children=true --check crates/app-server-core/src/cadquery/runner_json.rs crates/app-server-core/src/cadquery/staging.rs crates/app-server-core/tests/cadquery_tests.rs`：通过。
- `git diff --check`：通过。

### 遗留问题

- `docs/known_issues.md` 新增记录：CadQuery output 回写在本地可信 workspace 假设下仍有普通路径写入 TOCTOU 残余风险；当前不阻断 MVP，后续若要把 workspace 当作不可信输入，需要基于目录句柄和 no-follow 语义重新设计写入 API。
- `bun run web:build` 的大 chunk warning 是既有问题，已有 `docs/known_issues.md` 记录。
- CadQuery Python 环境仍按 MVP 策略手动安装；分发与沙盒策略留到产品化阶段。

## Phase 1 — Protocol / ManagedClient / Agent / Chat

### 完成情况

- 评估并记录 `rig-core` 当前兼容版本 `0.35.0`：provider 抽象、tool calling、stream API 和自定义 agent 控制 hook 符合后续接入方向。由于当前仓库没有 provider 配置、密钥管理和 mock provider 测试夹具，本 Phase 先实现 `AgentBackend` trait 和本地 deterministic fallback，不硬编码供应商或凭据。
- 扩展 `app-server-protocol`：新增 Chat 生命周期命令、Agent invoke / cancel、SelectionUpdate、CadQueryResultGet、Agent push events、`agent_busy` 错误，以及 Chat tool call / tool result / mesh result 记录。
- 扩展 `studio-common::ManagedClient`：维护 Chat sessions / history、Agent run、current selection、CadQuery result ready 的 snapshot 与事件更新；`AgentCancelled` 只作为取消请求确认，真正清理 running 状态由后续 `AgentDone` 事件完成。
- 扩展 `studio-web-wasm`、`app-server-protocol-wasm` 和 generated packages：新增 Chat / Agent / Selection dispatch；`cadquery.result.get` 的 JS 可见响应只保留轻量 `CadQueryResultReady`，mesh 大数组通过 `client_take_cadquery_mesh(result_id)` 读取。
- 实现 `app-server-host` dispatcher 异步 agent registry：`agent.invoke` 立即返回 `AgentStarted`，后续通过 push event 输出 token、tool start、tool result、mesh ready 和 done；已有 running session 时返回 `agent_busy`。
- 实现 `agent.cancel`：取消请求设置共享取消标记，运行任务负责停止 CadQuery 子进程、清理 staging、释放 registry，并发送 `AgentDone { cancelled: true }`，避免 cancel ack 先释放 running 状态造成第二个 agent 提前进入。
- 实现 Chat JSONL 存储：支持 create / list / send / history / archive，消息记录可保存 tool call、tool result 和 mesh result；session id 在 path join 前校验，只允许 ASCII 字母数字和 `-`。
- 收紧 Chat 文件系统边界：workspace root 以下的 `chats`、`chats/archived` 和每个 JSONL 文件均拒绝符号链接；`create`、`history`、`send`、`archive`、`list` 统一通过 no-follow metadata 检查，防止 Chat JSONL 写出 workspace。
- 实现 Inform / Plan / Execute 权限模型：Execute 必须携带 `AgentCadQueryConfirmation`，目标文件、affected / new 文件和 export targets 均使用 `PathHandle`；Agent 只能写入确认范围内的目标和输出。
- 修正 CadQuery 执行边界：Agent Execute 使用 backend 生成的 CadQuery 代码，不执行前端传入的原始 prompt 或任意 raw code；Agent Execute、直接 `CadQueryExecute` 和 `CadQueryPreview` 都使用 exact output scope，禁止把 staging 中未确认或非默认的 outputs 回写到真实 workspace；runner 返回后到 commit 前、commit prepare 前、commit 文件写入前都会再次检查 cancel 标记，文件间取消会回滚已写入文件和本轮新建目录，commit 成功后不再把 late cancel 改判为 cancelled done。
- 实现 Web Chat UI：支持 session 列表、创建 / 切换、Inform / Plan / Execute 模式、发送消息、agent streaming、tool result 展示、mesh result 展示和 done 后刷新 history；Chat archive 已在协议和后端实现，UI 暂未暴露归档入口；Chat / Agent / Selection 业务状态不写入 Zustand。

### Review 与修复记录

- 第一轮独立 review 发现 Chat session id 路径逃逸、Agent 只有 placeholder、Web Execute confirmation 不完整、Chat JSONL 缺少 tool call / result 记录、缺少 CadQuery result get wrapper、Agent done 后未刷新 history。均已修复并补充对应测试。
- 第二轮独立 review 发现 Agent Execute 仍会执行前端 raw code / prompt，以及 CadQuery staging 会把未确认 outputs 一并回写。已改为 `AgentBackend::generate_cadquery_code()` 生成代码，并新增 exact output scope。
- 第三轮独立 review 发现 `agent.cancel` 会在 worker 完成前释放 registry 并发送 done，直接 `CadQueryExecute` 也仍可绕过 output scope。已改为 worker 统一释放 registry；直接 execute / preview 均使用默认 exact output scope。
- 第四轮独立 review 发现 ManagedClient 在 cancel ack 时提前清理 `agent_run`，以及 `CadQueryPreview` 缺少 output scope 回归测试。已修复 ack 语义，并补充 preview exact output scope 测试。
- 第五轮独立 review 发现 ChatStore 可通过 `chats` 或 `chats/archived` 符号链接写出 workspace，且本文件缺少 Phase 1 结果记录。已补充 symlink 拒绝逻辑、红绿回归测试和本结果记录。
- 第六轮独立 review 发现三项问题：ChatStore 仍未覆盖中间目录符号链接，Web `dispatchCadQueryResultGet` 会在 Promise 内提前取出 mesh handle，CadQuery runner 完成后到 commit 前收到取消仍可能写入 workspace。已分别修复为 workspace root 以下逐组件 no-follow 检查、`dispatchCadQueryResultGet` 只返回轻量 payload 且调用方显式 `takeCadQueryMesh(result_id)`、runner 后 commit 前再次检查 cancel 标记，并补充对应红绿回归测试。
- 第七轮独立 review 发现 commit 成功后 dispatcher 仍可能因 late cancel 发送 `AgentDone { cancelled: true }`，并指出 Chat symlink 回归覆盖缺少 `chats/archived` 和 JSONL 文件本身。已将 cancel 检查下沉到 staging commit 的文件写入前，文件间取消会 rollback；dispatcher 在 commit 成功后不再改判为 cancelled done；同时补齐 `archived` 目录 symlink 与 JSONL 文件 symlink 测试。
- 第八轮独立 review 发现 staging commit 在第一次 cancel 检查前会先执行 prepare 并创建输出父目录。已把 cancel 检查提前到 prepare 前，并记录 prepare 阶段新建目录；任何 prepare 后取消或 commit 错误都会删除本轮新建的空目录。
- 第九轮独立 review 发现两项风险：文件间取消测试实际在第一轮文件写入前取消，无法证明 rollback；`run_execute_agent`、`useChatController`、`ChatComposer` 超过 50 行项目约束。已调整测试为第一轮文件写入完成、第二轮文件写入前取消，并断言输出文件和本轮创建目录均被回滚；同时拆分 dispatcher helper 和 Web Chat hook / composer 子组件。
- 第十轮独立 review 未发现 Critical 或 Important。Minor 指出结果记录中 Web Chat UI “归档”表述过宽；已修正为协议和后端支持归档、UI 暂未暴露归档入口。

### 验证记录

- 红灯验证：新增 `chat_store_rejects_chats_symlink_escape` 后，修复前执行 `cargo test -p app-server-core --test chat_tests chat_store_rejects_chats_symlink_escape -- --nocapture` 失败，实际创建了 `escaped-chat`。
- 红灯验证：新增 `chat_store_rejects_archive_through_chats_symlink_escape` 后，修复前执行 `cargo test -p app-server-core --test chat_tests chat_store_rejects_archive_through_chats_symlink_escape -- --nocapture` 失败，实际把 outside `main.jsonl` 归档到 outside `archived/main.jsonl`。
- 红灯验证：更新 `wasm-client.test.ts` 要求 result-get Promise 只返回轻量 payload 后，修复前 `bun run --cwd packages/studio-web test:unit tests/unit/wasm-client.test.ts` 失败，实际返回 `{ payload, mesh }`。
- 红灯验证：新增 `cadquery_staging_rejects_cancel_after_runner_before_commit` 后，修复前执行 `cargo test -p app-server-core --test cadquery_tests cadquery_staging_rejects_cancel_after_runner_before_commit -- --nocapture` 失败，实际完成 commit 并返回 `CadQueryRunResult`。
- 红灯验证：新增 `cadquery_staging_rolls_back_when_cancelled_between_commit_files` 后，修复前执行 `cargo test -p app-server-core --test cadquery_tests cadquery_staging_rolls_back_when_cancelled_between_commit_files -- --nocapture` 编译失败，缺少 cancellable commit API；补齐后该测试验证文件间取消 rollback。
- 红灯验证：新增 `cadquery_staging_rejects_pre_commit_cancel_without_creating_outputs_dir` 后，修复前执行 `cargo test -p app-server-core --test cadquery_tests cadquery_staging_rejects_pre_commit_cancel_without_creating_outputs_dir -- --nocapture` 失败，实际仍创建了真实 `outputs/` 目录。
- 绿色验证：修正文件间取消覆盖后，`cargo test -p app-server-core --test cadquery_tests cadquery_staging_rolls_back_when_cancelled_between_commit_files -- --nocapture` 通过，1 个测试通过。
- 绿色验证：`rustfmt --edition 2024 crates/app-server-core/src/chat.rs crates/app-server-core/src/cadquery/staging.rs crates/app-server-core/tests/chat_tests.rs crates/app-server-core/tests/cadquery_tests.rs` 后，`cargo test -p app-server-core --test chat_tests -- --nocapture` 通过，5 个测试全部通过。
- 绿色验证：补充 `chats/archived` 目录 symlink 与 JSONL 文件 symlink 测试后，`cargo test -p app-server-core --test chat_tests -- --nocapture` 通过，7 个测试全部通过。
- 绿色验证：`cargo test -p app-server-core --test cadquery_tests -- --nocapture` 通过，21 个测试全部通过。
- 绿色验证：`cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests -- --nocapture` 通过，10 个测试全部通过。
- 绿色验证：拆分 Web Chat hook / composer 后，`bun run --cwd packages/studio-web test:unit tests/unit/chat-zone.test.tsx` 通过，1 个文件、2 个测试通过。
- 绿色验证：`bun run --cwd packages/studio-web test:unit tests/unit/wasm-client.test.ts tests/unit/chat-zone.test.tsx` 通过，2 个文件、3 个测试通过。
- `rustfmt --edition 2024 --check <Phase 1 触及的 Rust 文件>`：通过。
- `cargo test --workspace`：通过；仅有既有 `app-server-core/src/watch.rs` dead_code warning。
- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run --cwd packages/studio-web test:unit`：22 个文件、101 个测试通过。
- `cargo check -p app-server-protocol-wasm --target wasm32-unknown-unknown`：通过。
- `cargo check -p studio-web-wasm --target wasm32-unknown-unknown`：通过。
- `bun run protocol:build`：通过。
- `bun run check:wasm-bindgen`：通过。
- `bun scripts/build_studio_web.ts`：构建成功，仍有既有 Vite 大 chunk warning。
- `git diff --check`：通过。

### 遗留问题

- `docs/known_issues.md` 新增记录：全仓库 `cargo fmt --check` 当前受既有无关格式差异阻塞。本轮不格式化未触及的无关源码，Phase 1 触及的 Rust 文件已通过 `rustfmt --check`。
- `docs/known_issues.md` 新增记录：Agent 后端当前使用本地 CadQuery 代码生成 fallback，尚未接入真实 LLM provider 配置；这不阻断 Phase 1 的协议、Chat、ManagedClient、权限范围和 CadQuery staging 主链路验收，但后续复杂 Agent 能力必须接入真实 provider 或 provider mock。
- `bun run protocol:check-generated` 在 Phase 1 提交前会因为 intended generated files 仍处于未提交 diff 状态而失败；本 Phase 使用 `bun run protocol:build` 验证生成流程，提交后再执行 generated check。
- `bun scripts/build_studio_web.ts` 的大 chunk warning 是既有问题，已有 `docs/known_issues.md` 记录。

## Phase 2 — Viewer 增强

### 完成情况

- 新增 CadQuery mesh handle 到 Web scene payload 的转换层，前端通过 `cadquery.result.get` 获取轻量 ready，再显式调用 `takeCadQueryMesh(result_id)` 读取 side buffer handle，大数组不进入 Zustand。
- 扩展 Three.js viewer：渲染 CadQuery face group、edge `LineSegments` 和 vertex `Points`；支持 face / edge / vertex / part / assembly picking、Shift 多选、hover highlight 和 selected highlight。
- 实现 `CadQueryViewer`：加载 result、渲染 CadQuery mesh、选择模式 dock、当前 feature/ref 状态展示、歧义确认弹窗和 `selection.update` 分发。
- Workbench 在 `agent.mesh_ready` 事件后打开 UI-only CadQuery tab；Zustand 只保存 `{ kind: "cadquery", result_id }` 这类 UI descriptor，不保存 mesh 业务状态。
- SelectionRef 生成符合 MVP 5 层 Ref：整体选择来自 `root_ref_text` / `root_object_kind` 或 part metadata；raw face / edge / vertex 的 `owner_ref_text`、`owner_object_kind`、`instance_path`、`build_id` 和 `result_id` 均来自 payload 元数据，不从文件名、路径或 mesh name 拼接。
- 重复 Assembly instance 选择已纳入 `instance_path` 作为 key 维度，避免同一 part 的不同实例在 additive multi-select 或 highlight 中被合并。
- CadQuery transform 使用 runner 当前 row-major 输出顺序，通过 Three.js `Matrix4.set(...)` 传入，保证靠 transform 分离的 Assembly child 可正确渲染与拾取。
- edge / vertex pick tolerance 当前为 `Line.threshold = 2`、`Points.threshold = 4`；基础浏览器验证已覆盖，真实复杂模型校准风险已记录到 `docs/known_issues.md`。

### Review 与修复记录

- 第一轮独立 review 发现重复 Assembly instance 的 raw selection key 缺少 instance 维度、浏览器测试覆盖不足、未实现 hover highlight、ambiguous 默认色和 dialog 信息不足。已补充红灯测试并修复 key、mode-specific selection keys、hover highlight、ambiguous 默认色恢复和 dialog target 展示。
- 第二轮独立 review 发现 CadQuery transform row-major / column-major 约定错误，以及 whole-result selection 在根对象为 part/component 时仍返回 `kind: assembly`。已补充 transform-only repeated instance 浏览器红灯测试和 root object kind 单元测试，并修复为 `Matrix4.set(...)` 与 `rootObjectKind`。
- 第三轮独立 review 发现 hover 离开 canvas 或切换 mode 后可能残留，以及非歧义 face 点击后缺少用户可见 feature/ref 信息。已补充 hover 清理浏览器断言和非歧义 selection status 单元测试，并修复为 `pointerleave` / mode switch 清理 hover、展示当前 selection 的 feature/ref。
- 最终独立 review 未发现 Critical 或 Important。Minor 要求补充 Phase 2 结果记录和 edge / vertex tolerance 风险记录，已在本节和 `docs/known_issues.md` 中完成。

### 验证记录

- 红灯验证：新增重复 Assembly instance raw selection key 单元测试后，修复前 `bun run --cwd packages/studio-web test:unit tests/unit/cadquery-selection.test.ts` 失败，两个 instance 生成相同 key。
- 红灯验证：扩展 hover 清理断言后，修复前 `bun run --cwd packages/studio-web test:e2e tests/playwright/cadquery-viewer-selection.spec.ts -g hover` 失败，canvas 离开后仍保留 hover dataset。
- 红灯验证：新增 root object kind 单元测试后，修复前 `bun run --cwd packages/studio-web test:unit tests/unit/cadquery-selection.test.ts` 失败，`@part[top_lid]` 被序列化为 `kind: assembly`。
- 红灯验证：将 repeated instance 浏览器 fixture 改为只靠 `transform` 分离后，修复前 `bun run --cwd packages/studio-web test:e2e tests/playwright/cadquery-viewer-selection.spec.ts -g "repeated assembly"` 失败，点击无法命中实例。
- 红灯验证：新增非歧义 face selection status 单元测试后，修复前 `bun run --cwd packages/studio-web test:unit tests/unit/cadquery-viewer.test.tsx` 失败，页面没有 `cadquery-selection-status`。
- 绿色验证：`bun run --cwd packages/studio-web test:unit tests/unit/cadquery-selection.test.ts` 通过，4 个测试通过。
- 绿色验证：`bun run --cwd packages/studio-web test:unit tests/unit/cadquery-viewer.test.tsx` 通过，3 个测试通过。
- 绿色验证：`bun run --cwd packages/studio-web test:e2e tests/playwright/cadquery-viewer-selection.spec.ts` 通过，4 个 Chromium 测试覆盖 face / edge / vertex / part / assembly / repeated instance / hover。
- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run --cwd packages/studio-web test:unit`：25 个文件、109 个测试通过。
- `bun scripts/build_studio_web.ts`：构建成功，仍有既有 Vite large chunk warning。
- `git diff --check`：通过。

### 遗留问题

- `docs/known_issues.md` 新增记录：CadQuery edge / vertex pick tolerance 当前使用固定阈值，已满足 MVP 基础路径，但仍需后续基于真实复杂 CadQuery 模型、缩放比例、投影模式和高 DPI 设备做校准。
- `bun scripts/build_studio_web.ts` 的大 chunk warning 是既有问题，已有 `docs/known_issues.md` 记录。

## Phase 3 — 端到端集成

### 完成情况

- 打通 Viewer selection → Chat → Agent Plan / Execute 的选择上下文：Web Chat 从 ManagedClient snapshot 读取 `current_selection`，显示当前 CadQuery ref，并在 Execute confirmation 中按当前 prompt 与 active selection 生成结构化范围。
- 后端 Agent Plan 使用 active selection、history 和 prompt 生成 Markdown CAD Plan，声明 selection target、target path、edit goal、`affected_files` 和 export target。
- Agent Execute 不执行前端 raw code；dispatcher 把 active selection 与确认的 `target_type` 传给后端 Agent，由后端生成 CadQuery 代码，再走 Phase 0c 的 staging 执行与 exact output scope。
- 实现 Ref 业务规则核心路径：
  - part face 有 feature 映射时，上升为 `@feature[...]`，并作为 selection target 与代码元数据传递给 Agent fallback。
  - 本地 Agent fallback 不再根据自然语言关键词生成 selector-based cut / fillet，避免把临时拓扑 id 或硬编码词表当成稳定几何编辑语义。
  - instance move 以 assembly 文件为主写入目标。
  - instance replacement 以 assembly 文件为主写入目标，owner component 文件仅作为受影响文件参与确认，避免误改所有同源 component 实例。
  - component body edit 与普通 instance body edit 归类为 component geometry。
  - ambiguous selection 保留 raw ref，并在 Plan 中标记需要确认。
- Web 侧新增 `cadquery-agent-scope.ts`，统一 Execute confirmation 的 target path、target type、affected files 和 export target 推导；Chat UI 不新增 Zustand 业务状态。
- `docs/known_issues.md` 新增记录：`AgentCadQueryConfirmation.plan_ref` 目前仍为 `null`，尚未持久绑定 CAD Plan 文件；当前不扩大写入权限，影响的是 Plan / Execute 长期追溯能力。

### Review 与修复记录

- 第一轮独立 review 发现非 part Execute 会生成 part 代码、selection 未影响实际 CadQuery 生成、active selection 未生效、ambiguous selection 被错误上升为 feature、`plan_ref` 未绑定。已补充 active selection 与 target type 传递，按 target type 生成 part / component / assembly 代码，ambiguous 保留 raw ref；`plan_ref` 问题记录到 `docs/known_issues.md`。
- 第二轮独立 review 发现 selection geometry 仍固定为 `faces(">Z")`，且 replacement 多文件范围声明不足。已移除固定 selector 回退，并补充 replacement affected files。
- 第三轮独立 review 发现 raw face / edge / vertex 在无 feature 映射时仍回退到通用 selector，且 component edit goal 文案不准确。已移除 raw geometry selector 回退，并把 component / instance body edit 标为 component geometry。
- 第四轮独立 review 发现 instance replacement 主写入目标错误：写 component 会影响所有同源实例。已改为 assembly 主写入目标、component 仅作为 affected file；同时补充普通 instance body edit 的回归测试。
- 最终独立 review 未发现 Critical、Important 或 Minor finding，确认 instance move / replacement / component replacement / instance body edit 的 target path、target type、affected files、edit goal 规则一致。

### 验证记录

- 红灯验证：新增 `local_agent_backend_does_not_modify_raw_face_without_feature_mapping` 后，修复前 `cargo test -p app-server-core --test agent_tests` 失败，实际仍生成 `.workplane().rect` 和 `cutThruAll`。
- 红灯验证：更新 instance replacement 测试要求 assembly 主写入目标后，修复前 `cargo test -p app-server-core --test agent_tests plan_turn_declares_instance_replacement_multi_file_scope -- --exact` 失败，Plan 仍声明 component replacement / component target。
- 红灯验证：新增 `plan_turn_labels_instance_body_edit_as_component_geometry` 后，修复前 `cargo test -p app-server-core --test agent_tests plan_turn_labels_instance_body_edit_as_component_geometry -- --exact` 失败，Plan 仍显示 assembly coordination。
- 红灯验证：更新 Web replacement confirmation 测试后，修复前 `bun run --cwd packages/studio-web test:unit tests/unit/cadquery-agent-scope.test.ts` 失败，`replace` 被确认成 component target。
- 绿色验证：`cargo test -p app-server-core --test agent_tests` 通过，12 个测试通过。
- 绿色验证：`cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests` 通过，12 个测试通过。
- 绿色验证：`bun run --cwd packages/studio-web test:unit tests/unit/cadquery-agent-scope.test.ts tests/unit/chat-zone.test.tsx` 通过，2 个文件、10 个测试通过。
- `cargo test --workspace`：通过；仅有既有 `app-server-core/src/watch.rs` dead_code warning。
- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run --cwd packages/studio-web test:unit`：26 个文件、117 个测试通过。
- `rustfmt --edition 2024 --check crates/app-server-core/src/agent.rs crates/app-server-core/tests/agent_tests.rs crates/app-server-host/src/dispatcher.rs crates/app-server-host/tests/shared_dispatcher_roundtrip_tests.rs`：通过。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/cadquery-viewer-selection.spec.ts`：4 个 Chromium 测试通过。
- `bun scripts/build_studio_web.ts`：构建成功，仍有既有 Vite large chunk warning。
- `git diff --check`：通过。
- `rg "@selector\\[|@subshape\\[|candidate_selector_ref" crates packages docs/cadquery-mvp docs/architecture.md`：无命中。

### 遗留问题

- `docs/known_issues.md` 新增记录：CadQuery Execute confirmation 尚未持久绑定 CAD Plan 文件。当前 Execute 仍受 `target_path`、`affected_files` / `new_files`、`export_targets` 和 staging exact output scope 约束，不阻断 Phase 3；后续需要增加 Plan 文件持久化和 confirmation `plan_ref` 校验。
- `bun scripts/build_studio_web.ts` 的大 chunk warning 是既有问题，已有 `docs/known_issues.md` 记录。

## 整体独立 review 收敛 — 2026-04-28

### 完成情况

- 根据整体独立 review，修复 `cadquery.preview` 在非 Execute confirmation 路径允许 `export_formats` 并回写 outputs 的问题。现在 preview 明确拒绝非空 `export_formats`，且不调用 outputs commit。
- 为 host 侧 CadQuery mesh result 增加有界缓存，当前限制为 8 个 result，并按插入顺序移除最早结果，避免长时间使用时缓存无上限增长。
- 将 CadQuery runner 的 `ImportError` / `ModuleNotFoundError` 映射为 `CadQueryRunnerErrorKind::PythonImport`，Agent push event 对应输出 `AgentErrorType::PythonImportError`。
- 拆分超过 500 行的新增文件：`agent.rs` 拆出 `agent/codegen.rs` 与 `agent/selection.rs`，`staging.rs` 拆出 `staging/commit.rs`，`cadquery_tests.rs` 拆出 `cadquery_staging_tests.rs`。
- 响应用户关于硬编码 `开孔`、`槽` 等自然语言词表的质疑：移除本地 Agent fallback 中 prompt-driven cut / fillet 生成逻辑。fallback 只生成稳定基础 CadQuery 结构，并把 selection ref 保留为上下文元数据；复杂几何编辑后续应由结构化 tool schema 或真实 Agent 输出驱动。
- 最后一轮独立 review 未发现 Critical 或 Important，仅指出 `dispatcher_cadquery_result_cache_evicts_oldest_entries` 测试函数超过 50 行。已拆出 fixture、preview 断言和 cache get 断言 helper，测试主函数缩短到 15 行左右。
- 继续按用户要求检查其它硬编码：移除本地 Agent fallback 中的 `height/tall` 尺寸启发式、固定 `faces(">Z")` body selector 和 assembly 默认 `offset=5`。当前 fallback 只生成默认 1x1x1 基础结构，参数仍可通过 `params` 覆盖。
- 确认仍有两处 prompt 关键词用于 move / replace 确认范围推导：Rust Plan fallback 与 Web Execute confirmation。该逻辑不直接生成几何，但仍属于临时硬编码；已记录到 `docs/known_issues.md`，后续应以结构化 edit intent 替代。

### 回归记录

- 红灯验证：新增 `dispatcher_cadquery_preview_rejects_export_formats_without_writing_outputs` 后，修复前 preview 返回 `CadQueryResultReady` 且允许输出写入路径。
- 红灯验证：新增 `dispatcher_cadquery_result_cache_evicts_oldest_entries` 后，修复前第 9 个 preview 后仍可读取最早的 `cq_part_0`。
- 红灯验证：新增 `cadquery_runner_maps_python_import_failure_to_error_kind` 后，修复前 `CadQueryRunnerErrorKind::PythonImport` 不存在。
- 红灯验证：新增 `dispatcher_execute_agent_maps_python_import_failure` 后，修复前 Agent error type 为 `CadQueryBuildError`。
- 绿色验证：`cargo test -p app-server-core --test agent_tests` 通过，12 个测试通过。
- 绿色验证：`cargo test -p app-server-core --test cadquery_tests --test cadquery_staging_tests` 通过，22 个测试通过。
- 绿色验证：`cargo test --workspace` 通过；仅有既有 `app-server-core/src/watch.rs` dead_code warning。
- 绿色验证：`bun run --cwd packages/studio-web test:unit` 通过，26 个文件、117 个测试通过。
- 绿色验证：`bun run --cwd packages/studio-web typecheck` 通过。
- 绿色验证：`bun test tests/cadquery_runner.test.ts` 通过，9 个测试、57 个断言通过。
- 绿色验证：`bun run protocol:check-generated` 通过。
- 绿色验证：`bun run web:build` 构建成功，仍有既有 Vite large chunk warning。
- 绿色验证：启动本地 Vite 后执行 `STUDIO_WEB_BASE_URL=http://127.0.0.1:5173 bun run --cwd packages/studio-web test:e2e tests/playwright/cadquery-viewer-selection.spec.ts`，4 个 Chromium 测试通过；随后已停止本地服务。
- 绿色验证：拆分测试 helper 后，`cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests dispatcher_cadquery_result_cache_evicts_oldest_entries -- --exact` 通过。
- 绿色验证：移除 codegen 尺寸 / selector / offset 启发式后，`cargo test -p app-server-core --test agent_tests` 通过，12 个测试通过。
- `cargo fmt --check -p app-server-core`：通过。
- `cargo fmt --check -p app-server-host`：通过。
- `git diff --check`：通过。
- 最终独立 review 结论：未发现 Critical、Important 或 Minor finding；确认运行时代码不再根据 `开孔` / `槽` / `cut` / `fillet` / `height` / `tall` 等 prompt 词直接生成几何修改，move / replace 词表只影响确认范围推断且已记录到 `docs/known_issues.md`。
- 行数复核：`agent.rs` 232 行、`agent/codegen.rs` 119 行、`agent/selection.rs` 207 行、`staging.rs` 419 行、`staging/commit.rs` 234 行、`cadquery_tests.rs` 289 行、`cadquery_staging_tests.rs` 436 行。
- 函数长度复核：上一轮 review 指出的 cache eviction 测试函数已拆分；新增 helper 均低于 50 行。

### 遗留问题

- `docs/known_issues.md` 新增记录：CadQuery Agent 确认范围仍使用 prompt 关键词推断 move / replace 意图。当前不直接生成几何、不扩大写入权限，但后续应以结构化 edit intent 替代。
- `cargo test --workspace` 的 `watch.rs` dead code warning 与 `bun run web:build` 的 Vite large chunk warning 均为既有问题，不阻断本 plan 验收。
