# CadQuery Web Polish Replan Result

## 当前状态

- 计划已创建并经过独立 reviewer 按 `AGENTS.md` 和 `plan-prompt.md` 作为规则与前提审查。
- 已根据 reviewer 结论局部重写 `plan-00.md`。
- 已根据 2026-04-30 用户最新反馈再次修订 `plan-00.md`：PRD 示例暂不处理，system prompt 既有示例块暂不处理，Rust 代码里的 LLM 可见 feature 示例或占位命名纳入 Phase -1，system prompt 只补充 feature 命名责任指引。
- Phase -1 已完成执行、验证与独立 review。
- Phase 0 已完成当前状态审计、基线验证与独立 review。
- Phase 1 已完成 Ref 图层树、预览模式、RefKind 选择模式、验证与独立 review。
- Phase 2 已完成文件列表路由、artifact relation 与模型更新刷新。
- Phase 3 已完成 Agent 模型产物契约、验证与独立 review。
- Phase 4 已完成渲染模式与聊天流 UI、验证与独立 review。后续按计划进入 Phase 5。

## Review 结论处理

- 已补全每个 Phase 的独立 review 与收敛要求。
- 已补全 Plan 级最终独立 review 要求。
- 已修正 Phase 5：必须启动本轮可控 Web dev server，必须在真实网页中新建 Chat，必须用原始 AirPods 垫子 prompt 作为真实用户输入。
- 已修正 Phase 2：`.step` 与 `.py` 的预览映射只能来自 app-server/protocol/manifest 显式 artifact relation，前端不得通过路径、文件名或 runner 输出推断。
- 已补充 GUI 共享边界检查。
- 已补充完整需求覆盖清单和 Phase 6 覆盖矩阵要求。
- 已修正 Phase -1 范围：清理重点从泛化的 prompt / docs 清理改为 Rust tool schema / guidance / warning / error 中的具体 feature key 示例、占位 feature key 和验收任务耦合；system prompt 的本轮要求限定为补充 `REFS.features` 由 LLM 根据实际模型语义命名的指引。

## Phase 进度

- Phase -1：已完成。
- Phase 0：已完成。
- Phase 1：已完成。
- Phase 2：已完成。
- Phase 3：已完成。
- Phase 4：已完成。
- Phase 5：未开始。
- Phase 6：未开始。

## 验证记录

- `cargo test -p app-server-core cadquery_agent_system_prompt --test agent_tests`：3 passed，0 failed。
- `cargo test -p app-server-core cadquery_tool_schemas_do_not_suggest_placeholder_feature_keys --test agent_tool_registry_tests`：1 passed，0 failed。
- `cargo test -p app-server-core workspace_tool_executor_cadquery --test agent_tool_tests`：22 passed，0 failed。
- `rg -n "AirPods|airpods|wireless charging|charging_pad|airpods_recess|front_finger_notch|cable_relief|placement_pocket|access_notch|human_readable_feature_name|semantic_part_feature_name|semantic_component_feature_name|semantic_assembly_feature_name" crates/app-server-core/src docs/cadquery-mvp/agent-system-prompt.md packages/studio-web/src packages/app-server-protocol/src crates/app-server-protocol/src -g '!target' -g '!node_modules'`：无命中，exit 1。
- `bun run --cwd packages/studio-web test:unit tests/unit/cadquery-selection.test.ts tests/unit/cadquery-ref-tree.test.tsx tests/unit/cadquery-viewer.test.tsx tests/unit/cadquery-source-preview.test.tsx`：21 passed，0 failed。
- `bun run --cwd packages/studio-web typecheck`：`tsc --noEmit` 通过，exit 0。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/cadquery-viewer-selection.spec.ts`：8 passed，0 failed。
- `bun run --cwd packages/studio-web test:unit`：249 passed，1 failed；失败项为 `tests/unit/chat-zone.test.tsx > ChatZone > renders live agent tokens and tool events in arrival order`，原因是页面中出现两个 `read_file` 文本。该问题属于 Phase 4 聊天流 UI 范围，本 Phase 不修改聊天区实现。
- `git diff --check`：无输出，exit 0。

## Phase -1 结果

### 审计模块与结论

- 前端产品代码：独立审计覆盖 `packages/studio-web/src/workbench/`、`packages/studio-web/src/viewers/`、`packages/studio-web/src/state/`。未发现 AirPods、车载无线充电板或垫子语义进入前端运行路径。审计发现 `packages/studio-web/src/workbench/cadquery-source-path.ts` 存在 `outputs/<stem>.step -> parts/<stem>.py` 的源文件推断风险；该问题不是本 Phase 的验收 case 文案污染，已归入 Phase 2 的 artifact relation 修复范围。
- 后端 / app-server / protocol / transport / wasm：独立审计覆盖 `crates/app-server-core/src/**`、`crates/app-server-host/src/**`、`crates/app-server-protocol/src/**`、`crates/app-server-transport/src/**`、`crates/studio-web-wasm/src/**` 与相关生成类型。未发现当前验收任务语义进入通用产品路径或 protocol 契约。
- Rust CadQuery tool schema / guidance / warning / error：独立审计确认 `cadquery.rs` 的 `human_readable_feature_name` 和 `support.rs` 的 `semantic_part_feature_name` 属于 LLM 可见占位 feature key，必须清理；`args.rs` 的错误文案应保留 `REFS.type` 与 `"features"` 结构要求，但不得给出可复制 feature key。
- system prompt 指引：运行时 prompt 由 `crates/app-server-core/src/agent.rs` 通过 `include_str!` 读取 `docs/cadquery-mvp/agent-system-prompt.md`。已补充 feature 命名责任指引，明确 `REFS.features` key 由 LLM 根据当前请求、workspace 文件和实际模型语义命名；tool schemas、warnings、errors 只描述结构，不提供可复制名称；修改旧模型时优先保留已有稳定 key。
- 测试、prompt archive 与真实验收记录：AirPods / 车载无线充电板 / 垫子内容只出现在允许保留范围，包括 `prompt-archives/**`、测试 fixture、测试断言、真实验收记录和生成 workspace 模型。PRD 示例和 system prompt 既有示例块按计划边界暂不处理。

### 变更摘要

- 清理 `crates/app-server-core/src/agent/tools/registry/schemas/cadquery.rs` 中 LLM 可见的 `human_readable_feature_name` 占位 key，改为要求 `REFS` 包含匹配类型与非空 features，feature 名称来自实际模型语义。
- 清理 `crates/app-server-core/src/agent/tools/cadquery/support.rs` 中 LLM 可见的 `semantic_part_feature_name` 示例，保留 `REFS.features` 必填要求。
- 清理 `crates/app-server-core/src/agent/tools/cadquery/args.rs` 中具体 feature key 示例，保留 `REFS.type` 匹配与 `"features"` 字段要求，并修正模块内 helper 调用。
- 为 system prompt、tool schema、contract warning / error 增加防回归测试。
- 更新 `docs/cadquery-mvp/agent-system-prompt.md`，补充通用 feature 命名责任指引。

### 保留理由

- prompt archive 中的具体 case 是原始需求、计划背景、真实用户输入和验收记录，保留后便于后续真实网页验收复现。
- 测试 fixture 与断言中的具体 case 用于验证消息转换、文件扩展识别或防污染边界，不会作为产品默认 prompt、schema 或运行时分支被读取。
- 生成 workspace 模型属于真实验收产物，不是 app-server、protocol 或前端通用实现。
- PRD 示例和 system prompt 既有示例块是本计划明确暂不处理范围；本 Phase 只补充通用 feature 命名责任指引。

### 独立 Review 结论

- Phase -1 独立 review 结论：无阻塞项，无需返工。
- Reviewer 记录的非阻塞风险：`plan-00-result.md` 在 review 时尚未更新。本节已完成更新。
- 后续必须在 Phase 2 处理 Web 侧 `.step` 文件到 CadQuery source 的显式 artifact relation，不能继续依赖 `outputs/<stem>.step -> parts/<stem>.py` 推断。

## Phase 0 结果

### 当前工作树分类

- 已提交 checkpoint：`f4480b1 Phase -1 clean CadQuery LLM-visible feature guidance`。该提交只包含 Phase -1 的 LLM 可见 CadQuery 文案清理、system prompt 指引、防回归测试和本结果文档更新。
- 当前仍未提交的既有改动：
  - `README.md`、`docs/getting-started.md`：把 `STUDIO_WEB_WORKSPACE` 默认值文档从 `workspace/studio-web/` 改为 `workspace/budn-web/`。
  - `scripts/run_websocket_host.ts`：把默认 workspace 从 `workspace/studio-web` 改为 `workspace/budn-web`。
  - `tests/run_websocket_host.test.ts`：新增 Bun 测试，验证 websocket host 默认 workspace 使用 `workspace/budn-web`。
  - `prompt-archives/2026043002-cadquery-web-polish-replan/plan-00.md`：记录用户在 2026-04-30 补充的 Phase -1 边界，包括 PRD 示例暂不处理、system prompt 既有示例块暂不处理，以及 Rust LLM 可见 feature 示例或占位命名必须处理。
- 已完成且必须保护的能力：
  - 2026043000 结果记录显示 Web Chat 已通过真实 prompt 触发 CadQuery 建模，生成 `parts/airpods_charging_pad.py` 与 `outputs/airpods_charging_pad.step`。
  - LLM reasoning 已通过 `Thinking` 展示最新思考内容。
  - `.py` CadQuery 源文件能从文件列表进入模型预览。
  - Viewer 已能选择 face / edge / vertex / part / assembly，并把 selection 写入上下文。
  - CadQuery runner/staging 语义与失败不污染真实 workspace 的边界必须继续保护。
- 本轮半成品或待继续验证的范围：
  - Ref tree、选择模式、preview mode、render mode、done mark、tool event 单行状态等代码与测试已经存在，但还需要 Phase 1 和 Phase 4 按计划完成真实交互、边界和视觉验证。
  - `.step` 路由当前仍存在 `packages/studio-web/src/workbench/cadquery-source-path.ts` 的 `outputs/<stem>.step -> parts/<stem>.py` 推断，必须在 Phase 2 改为 app-server/protocol/manifest 显式 artifact relation。
  - `packages/studio-web/src/workbench/cadquery-agent-scope.ts` 仍有 `parts/agent_model.py` 默认目标；当前审计未确认它是本轮验收污染，但它属于旧 confirmation/scope 辅助路径，后续涉及 Agent scope 时需要避免恢复无上下文默认生成 part 的行为。
- 需要修正的错误边界：
  - 不能把 `.step` 源文件关系留在前端路径或文件名推断里。
  - 不能为了 Playwright 或真实验收加入测试专用运行时分支。
  - 不能回退 Phase -1 已清理的 LLM 可见占位 feature key。

### 进程与端口状态

- `ps -axo pid,ppid,etime,command | rg -i "bun|vite|playwright|run_websocket_host|studio-web|node|cargo run|app-server"`：未发现本项目 Vite、Playwright、`run_studio_web_dev` 或 `run_websocket_host` 进程；仅看到 Codex、Context7、Paseo、系统和其他桌面应用相关进程。
- `lsof -nP -iTCP -sTCP:LISTEN | rg "(5173|5174|1420|3000|4173|8787|9000|9001|8080|run_websocket|vite|bun|node)"`：未发现本项目 Web dev server 监听端口。命中项为其他本机服务。
- 重新执行 Playwright `cadquery-viewer-selection.spec.ts` 后，再次检查 `39193` 和 `5188`，未发现 harness 遗留监听进程。
- Phase 5 必须按计划启动本轮可控 Web dev server，并记录命令、端口和日志位置；不得复用旧 server。

### 被中断测试状态

- `packages/studio-web/test-results/.last-run.json` 当前为 `{"status":"passed","failedTests":[]}`。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/cadquery-viewer-selection.spec.ts` 重新执行通过：4 passed，0 failed。
- 该基线只证明 CadQuery Viewer 选择测试入口可用，不代表 Phase 1 到 Phase 5 的完整需求已完成。

### 污染复核

- Phase 0 复核命令：`rg -n "AirPods|airpods|wireless charging|charging_pad|airpods_recess|front_finger_notch|cable_relief|placement_pocket|access_notch|human_readable_feature_name|semantic_part_feature_name|semantic_component_feature_name|semantic_assembly_feature_name" crates/app-server-core/src docs/cadquery-mvp/agent-system-prompt.md packages/studio-web/src packages/app-server-protocol/src crates/app-server-protocol/src -g '!target' -g '!node_modules'`。
- 结果：无命中，exit 1。
- system prompt 既有示例块按 Phase -1 边界暂不处理；本轮新增 feature 命名责任指引不含当前验收对象专有命名。

### Phase 0 Review 状态

- Phase 0 独立 review 结论：无阻塞项，无需返工。
- Reviewer 记录的非阻塞风险：
  - `.step` 路由仍依赖 `outputs/<stem>.step -> parts/<stem>.py` 的前端路径推断；已归入 Phase 2 修复范围。
  - `agent_model` 默认目标仍存在于 Web 与 Rust Agent 辅助路径；后续涉及 Agent scope 或模型产物契约时需要继续确认它不会变成无上下文默认建模路径。

## Phase 1 结果

### GUI 边界判断

- 本 Phase 修改集中在 `packages/studio-web` 的浏览器壳层、Three.js Viewer 接线和 Web Inspector section。Ref tree、mode toolbar 与底部 selection dock 目前只服务 Web Workbench 的 CadQuery preview 交互，并且依赖 Web 侧 scene payload、DOM test id 和 Three.js canvas pick 接线；本轮未把它们抽到 `scad-ui`。
- 状态与行为仍通过 app-server protocol selection snapshot 同步：Ref tree 只构造 `SelectionRef` 并调用现有 `selection.update`；Viewer 只根据同一个 selection snapshot 生成 selected keys；Chat context 继续消费 protocol store 中的当前 selection。
- 本轮没有在 `studio-app` 与 `studio-web` 中复制语义相同的共享基础组件，也没有新增绕过 protocol 的前端 Ref 推断路径。可复用视觉基础层的抽离留给后续桌面端复用需求，不在本 Phase 增加跨 crate 改动。

### 变更摘要

- `cadquery-selection.ts` 增加 `component`、`instance`、`feature` 和独立 `preview` viewer mode，统一从 `CadQueryScenePayload` 生成可用 selection modes。
- Root 只作为 Ref tree 展示节点，不再生成可选 object row；`part.refText === scene.rootRefText` 的 object 不进入 Ref tree，也不能通过 canvas object mode 生成 selection。
- Ref tree 现在显示 component / part / assembly、instance、feature、face、edge、vertex，并支持跨层级自由多选，继续通过 `selection.update` 同步到 protocol snapshot。
- `mesh-three.ts` 的 canvas picking 支持 component、non-root part、non-root assembly、instance、feature、face、edge、vertex；RefKind 不匹配时不产生错误 selection。
- Feature mode 只使用 protocol `featureMap.faceIndices` 建立 face 到 feature 的关系，未命中时不从 face 的 `features[0]` 回退生成 tree 中不存在的 feature Ref。
- `CadQueryViewer`、`CadQuerySourcePreview` 和 `CanvasZone` 支持受控 `CadQueryViewerMode`，顶部 toolbar 提供 preview 与可用 RefKind mode，底部 `cadquery-select-dock` 只显示 selection modes，并把点击事件传回 Workbench。
- Preview mode 继续关闭 selection interaction、selection overlay、selection dock/status；axis、底板、gizmo、灯光、相机与 render settings 不走 selection 开关路径。

### 验证证据

- `bun run --cwd packages/studio-web test:unit tests/unit/cadquery-selection.test.ts tests/unit/cadquery-ref-tree.test.tsx tests/unit/cadquery-viewer.test.tsx tests/unit/cadquery-source-preview.test.tsx`：21 passed，0 failed。
- `bun run --cwd packages/studio-web typecheck`：`tsc --noEmit` 通过，exit 0。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/cadquery-viewer-selection.spec.ts`：8 passed，0 failed。
- `rg -n "AirPods|airpods|wireless charging|charging_pad|airpods_recess|front_finger_notch|cable_relief|placement_pocket|access_notch|human_readable_feature_name|semantic_part_feature_name|semantic_component_feature_name|semantic_assembly_feature_name" crates/app-server-core/src docs/cadquery-mvp/agent-system-prompt.md packages/studio-web/src packages/app-server-protocol/src crates/app-server-protocol/src -g '!target' -g '!node_modules'`：无命中，exit 1。
- `lsof -nP -iTCP:39193 -iTCP:5188 -sTCP:LISTEN`：无输出，exit 1，Playwright harness 未遗留监听进程。
- `git diff --check`：无输出，exit 0。
- 额外完整单元测试 `bun run --cwd packages/studio-web test:unit`：249 passed，1 failed；失败项仍为 `tests/unit/chat-zone.test.tsx > ChatZone > renders live agent tokens and tool events in arrival order`，属于 Phase 4 聊天流 UI 的既有待处理范围。

### 独立 Review 结论

- 第一轮 Phase 1 review 发现非 root assembly 被合并到 root、底部 dock 在 Workbench 受控模式下无效。已修复并补充 nested assembly 与 controlled dock mode 测试。
- 第二轮 Phase 1 review 发现 root object 仍可选、RefKind 不匹配时 canvas 会回退到错误 selection。已修复 root tree row、object mode fallback 和 nested assembly 行为。
- 第三轮 Phase 1 review 发现 feature mode 会从 `face.features[0]` 回退生成 tree 不存在的 Ref，且 root object 仍可能通过 object mode 进入 selection。已改为只使用 `featureMap.faceIndices` 和非 root object selection。
- 最终 Phase 1 re-review 结论：无阻塞项，无需返工。非阻塞缺口包括缺少 Workbench 级双向同步集成测试、preview mode 辅助元素的浏览器级断言、dock 位置截图或布局断言；这些在 Phase 5 真实网页验收和 Plan 级 review 中继续覆盖。

## Phase 2 结果

### 边界判断

- 本 Phase 涉及 app-server protocol、runner manifest 解析、managed client snapshot、wasm side buffer 和 Web Workbench 文件路由，属于跨 protocol 与 Web 壳层接线改动。
- `.py` 模型预览仍由文件列表直接打开 CadQuery source preview；`.step/.stp` 不再按扩展名进入 CadQuery，也不再由 `outputs/<stem>.step -> parts/<stem>.py` 推断 source。
- 生成的 STEP 只有在当前 protocol snapshot 的 CadQuery result artifact relation 中明确列为 export path 时，才会构造 `cadquery_artifact` tab，并把 preview target 指向 relation 中的 source path。
- Agent `mesh_ready` 事件不再创建临时 CadQuery result tab；只有当前 active CadQuery tab 与 ready relation 匹配时才刷新当前 tab。
- watch 刷新路径不再对 CadQuery 使用目录级 fallback；CadQuery 只在 watched path 与当前 tab path 精确匹配时刷新，避免 root watch 或无关 output 变更刷新错误模型。
- `.py` 模型写入仍由 CadQuery tool 和 staging commit 承接；本 Phase 没有新增普通文件写入 `.py` 或绕过 staging 的路径。

### 变更摘要

- `app-server-protocol` 新增 `CadQueryArtifactRelation` / `CadQueryArtifactExport`，并挂到 `CadQueryResultReady` 与 `CadQueryMeshPayload`；protocol version 升到 6。
- `app-server-core` runner JSON 解析保留 manifest 中的 `source_path`、`exports` 和 `export_hashes`，并通过 `cadquery_result_ready` 传递到轻量 ready payload。
- `studio-common` 与 `studio-web-wasm` 在从 mesh payload 生成 ready payload 时保留 artifact relation，wasm side buffer 不再丢失 relation。
- `packages/app-server-protocol/generated/app_server_protocol_wasm_bg.wasm` 已通过 `bun run protocol:build` 重新生成，匹配 protocol version 6。
- Web protocol store 记录 `cadquery_results`，文件列表打开 STEP 时只查询这些 result 的 artifact relation；无匹配 relation 时显示 unsupported。
- 删除 Web 运行路径中打开临时 `cadquery_result` tab 的 factory 使用；保留 legacy result path 识别，避免破坏已有 viewer 分支。
- 新增 `watch-refresh.ts`，把 watch 刷新策略变成可测试的纯函数，明确 CadQuery 不走目录级 fallback。

### 验证证据

- `bun run --cwd packages/studio-web test:unit tests/unit/watch-refresh.test.ts tests/unit/cadquery-source-path.test.ts tests/unit/tab-kind.test.ts tests/unit/cadquery-result-tab.test.ts tests/unit/protocol-package-import.test.ts tests/unit/protocol-store.test.ts tests/unit/cadquery-source-preview.test.tsx`：54 passed，0 failed。
- `bun run --cwd packages/studio-web typecheck`：`tsc --noEmit` 通过，exit 0。
- `cargo test -p app-server-core cadquery --test cadquery_tests`：11 passed，0 failed。
- `cargo test -p app-server-protocol cadquery_payload_roundtrips_and_ready_counts_are_lightweight --test borsh_payload_roundtrip_tests`：1 passed，0 failed。
- `cargo test -p app-server-host dispatcher_cadquery_result_get_preserves_artifact_relation --test shared_dispatcher_roundtrip_tests`：1 passed，0 failed。
- `cargo test -p app-server-core workspace_tool_executor_cadquery --test agent_tool_tests`：22 passed，0 failed。
- `cargo check -p studio-web-wasm --target wasm32-unknown-unknown`：通过，exit 0。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/cadquery-viewer-selection.spec.ts`：8 passed，0 failed。
- `lsof -nP -iTCP:39193 -iTCP:5188 -sTCP:LISTEN`：无输出，exit 1，Playwright harness 未遗留监听进程。
- `rg -n "AirPods|airpods|wireless charging|charging_pad|airpods_recess|front_finger_notch|cable_relief|placement_pocket|access_notch|human_readable_feature_name|semantic_part_feature_name|semantic_component_feature_name|semantic_assembly_feature_name" crates/app-server-core/src docs/cadquery-mvp/agent-system-prompt.md packages/studio-web/src packages/app-server-protocol/src crates/app-server-protocol/src -g '!target' -g '!node_modules'`：无命中，exit 1。
- Phase 2 提交后执行 `bun run protocol:check-generated`：通过，exit 0，generated protocol wasm 与当前 HEAD 一致。
- `git diff --check`：无输出，exit 0。

### 独立 Review 结论

- 第一轮 Phase 2 review 发现两个阻塞项：Agent `mesh_ready` 不匹配当前 tab 时仍会打开临时 result tab；watch 目录级 fallback 仍可能刷新无关 active CadQuery tab。已删除临时 result tab 打开路径，并让 CadQuery watch 刷新只接受精确路径命中。
- 第二轮 Phase 2 review 结论：无阻塞项，无高风险问题。Reviewer 确认 `.step/.stp` 只通过 `artifact_relation.exports` 找回 source path，普通 STEP 不再解析为 CadQuery tab，`cadQueryResultTab` factory 已删除，CadQuery watch 刷新不再走目录级 fallback。

## Phase 3 结果

### 边界判断

- 本 Phase 修改集中在 CadQuery Agent system prompt、`cadquery_execute` tool schema、Rust contract check / warning / error 和 app-server-core 测试。没有修改 Web 预览路由、artifact relation protocol 或 staging commit 机制。
- 模型说明硬校验只作用于 `cadquery_execute` 的新建或修改模型路径；`cadquery_analyze_source` 与 `cadquery_check_source` 仍返回 `ok`，并通过 contract / warning 告知缺少说明，不会阻断既有旧模型预览。
- `cadquery_execute` 的 scope、安全导入、unsafe 调用和 staging 相关校验顺序仍保留；本 Phase 只在 execution scope 通过后增加产品契约校验，并要求 `.step` 导出声明。
- system prompt 既有示例块按计划边界暂不清理；本 Phase 只强化新增或修改指引中的结构性要求。

### 变更摘要

- `docs/cadquery-mvp/agent-system-prompt.md` 明确 `MODEL_DETAILS` 必须包含 `purpose`、`key_dimensions`、`intended_use`、`assumptions`、`interaction_notes` 和 `manufacturing_or_placement_constraints`。
- `cadquery_execute` schema 将 `export_formats` 与 `export_targets` 设为必填，并在字段说明中要求包含 `.step` 导出目标，确保 `.py` 与 `.step` 同步。
- `cadquery_execute` 新增产品契约校验：缺少模型说明字段、缺少导出声明、未包含 `.step` 格式或 `.step` target 时直接拒绝执行。
- `MODEL_DESCRIPTION` / `MODEL_DETAILS` 检查从全文 substring 改为模块级赋值解析，支持普通字符串、三引号字符串和类型标注赋值，拒绝注释、字符串字面量、函数内赋值、空字段、集合值和空说明误通过。
- 成功路径测试 fixture 中的 `@feature[lid.top]`、`top_surface` 与 `"top": {}` 替换为语义化 feature 名称，避免把弱命名当作默认成功样例。
- 保留 `REFS.features` 必填、`REFS.type` 匹配、selection / feature map 映射、staging 和 artifact relation 已确认契约。

### 验证证据

- `cargo fmt`：通过。
- `cargo test -p app-server-core workspace_tool_executor_cadquery_execute_accepts_python_model_contract_variants --test agent_tool_tests`：修复前按预期失败于 `triple_quoted`；修复后 1 passed，0 failed。
- `cargo test -p app-server-core workspace_tool_executor_cadquery_execute --test agent_tool_tests`：19 passed，0 failed。
- `cargo test -p app-server-core --test agent_tool_tests`：138 passed，0 failed。
- `cargo test -p app-server-core --test agent_tool_registry_tests`：6 passed，0 failed。
- `cargo test -p app-server-core --test agent_tests`：15 passed，0 failed。
- `cargo test -p app-server-core --test llm_tests`：38 passed，0 failed。
- `cargo test -p app-server-core --test cadquery_staging_tests`：12 passed，0 failed。
- `cargo test -p app-server-core --test cadquery_tests -- --test-threads=1`：11 passed，0 failed。该文件中的大 stdout drain 用例在与其他 Cargo 测试并行时两次触发 2 秒超时；单独和串行均通过，失败模式与本 Phase 改动无关。
- `rg -n "AirPods|airpods|wireless charging|charging_pad|airpods_recess|front_finger_notch|cable_relief|placement_pocket|access_notch|earbud|earbuds|charging case|headphone|earpiece|充电盒|耳机|耳塞" crates/app-server-core/src docs/cadquery-mvp/agent-system-prompt.md packages/studio-web/src packages/app-server-protocol/src crates/app-server-protocol/src -g '!target' -g '!node_modules'`：无命中，exit 1。
- `rg -n "human_readable_feature_name|semantic_part_feature_name|semantic_component_feature_name|semantic_assembly_feature_name" crates/app-server-core/src docs/cadquery-mvp/agent-system-prompt.md packages/studio-web/src packages/app-server-protocol/src crates/app-server-protocol/src -g '!target' -g '!node_modules'`：无命中，exit 1。
- `rg -n '@feature\[[^\]]*\.top\]|@feature\[lid\.top\]|"top": \{\}|"top": \{"kind": "feature"\}|top_surface|valid_part_source\("top"\)' crates/app-server-core/tests crates/app-server-core/src/agent/tools`：无命中，exit 1。
- `git diff --check`：无输出，exit 0。

### 独立 Review 结论

- 第一轮 Phase 3 review 结论：无阻塞项；记录高风险非阻塞项，指出合法 Python 三引号字符串与类型标注赋值会被模型契约硬校验误拒绝。
- 已按 reviewer 结论补充失败用例并修复 `support.rs` 解析逻辑，覆盖三引号字符串和类型标注赋值成功路径，同时保留空字段、注释、字符串字面量和函数内赋值拒绝路径。
- 第二轮 Phase 3 review 结论：无阻塞项，无高风险非阻塞项，未发现验证缺口。

## Phase 4 结果

### GUI 边界判断

- 本 Phase 修改集中在 Web Workbench 的聊天事件呈现、Viewer toolbar 验证、CanvasZone 测试 harness 和 Web 侧 CSS 布局。当前实现依赖 `@assistant-ui/react` 消息结构、浏览器 DOM、Three.js canvas 和 Playwright 验证入口，属于 `studio-web` 壳层范围。
- 状态和行为仍沿用既有 `CanvasZone` viewer options、`CadQueryViewer` props、`mesh-three` options 与 chat runtime；本 Phase 没有新增跨端共享状态机，也没有在 `studio-app` 与 `studio-web` 之间复制共享基础组件。
- `tool modal`、chat event row、status bar 与错误卡片仅修正当前 Web 壳层呈现和验证缺口；没有触碰 app-server、protocol、artifact relation、CadQuery staging 或 Ref selection 契约。

### 变更摘要

- `AgentEventRow` 的 `agent.tool_start` 与 `agent.tool_result` 默认保持单行状态；`agent.tool_result` 单行摘要改为 `tool result ready`，避免同一工具名在 start/result 两行完全重复。
- tool event modal 保留完整 event payload，并额外把 `args_json` / `result_json` 解析为结构化 `arguments` / `result`，确保 `tool_name`、`tool_call_id`、`run_id` 和工具详情都可查看。
- 增加 Workbench 级 CadQuery canvas harness，Playwright 通过真实 `viewer-render-wireframe` / `viewer-render-xray` toolbar 按钮验证 `CanvasZone -> CadQueryViewer -> mesh-three` 渲染路径。
- STL 与 CadQuery 渲染模式 E2E 均补充同尺寸截图的 RGBA 像素差异比较，避免只看 DOM attribute。
- 补充 `agent.tool_start` modal 详情单元测试，以及连续 assistant 来源隐藏、用户来源不受影响的真实 CSS computed style 测试。
- 调整 1280x800 下 Web canvas status bar 和 preview error card 布局：FPS 区域允许收缩，错误卡片为换行 toolbar 留出上方空间。
- 将 Web CadQuery 测试 fixture 中的弱 feature 命名替换为 `lid_alignment_surface`，避免继续把弱语义命名作为成功样例。

### 验证证据

- `bun run --cwd packages/studio-web typecheck`：`tsc --noEmit` 通过，exit 0。
- `bun run --cwd packages/studio-web test:unit tests/unit/chat-messages.test.tsx tests/unit/chat-zone.test.tsx tests/unit/chat-runtime.test.ts tests/unit/cadquery-viewer.test.tsx`：70 passed，0 failed。
- `bun run --cwd packages/studio-web test:unit`：37 files / 260 tests passed，0 failed；保留既有 React act warning。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts --grep "cadquery toolbar drives render state"`：1 passed，0 failed。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts --grep "1280x800"`：2 passed，0 failed。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts`：16 passed，0 failed。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/cadquery-viewer-selection.spec.ts`：9 passed，0 failed。
- `git diff --check`：无输出，exit 0。
- `lsof -nP -iTCP:39182 -iTCP:5177 -iTCP:39193 -iTCP:5188 -sTCP:LISTEN`：无输出，exit 1，Playwright harness 未遗留监听进程。

### 独立 Review 结论

- 第一轮 Phase 4 review 发现阻塞项：STL 渲染模式截图比较被 viewport resize 干扰，且 PNG buffer 比较不足以证明像素变化；另记录 CadQuery canvas 路径缺少覆盖。已改为 resize 前完成截图与浏览器内 RGBA 像素比较，并新增 CadQuery canvas 像素变化验证。
- 第二轮 Phase 4 review 结论：无阻塞项；记录两个高风险问题：tool modal 对 start/result 只显示局部 JSON，缺少完整 payload 元数据；CadQuery 渲染模式验证只覆盖底层 viewer，未覆盖 Workbench toolbar 到 CadQuery canvas 的完整路径。已修复 modal payload，并新增 Workbench 级 toolbar E2E。
- 第三轮 Phase 4 review 结论：无阻塞项，无高风险问题；记录两个验证缺口：`agent.tool_start` modal 详情未直接测试，连续 assistant 来源隐藏与用户来源不受影响未有自动化断言。已补充对应单元测试。
- 第四轮 Phase 4 review 结论：功能 diff 无阻塞项、无高风险问题、无验证缺口；唯一阻塞项为本结果文档尚未记录 Phase 4 状态和 GUI 边界判断。本节已按该结论补齐。
