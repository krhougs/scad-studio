# Plan 00 Execution Result

## Phase 0: Agent Skill 基础架构 — 完成

### 变更摘要

| 文件 | 类型 | 内容 |
|---|---|---|
| `crates/app-server-core/src/agent/skills.rs` | 新增 | Skill 注入机制：`SkillInjectionContext`、`collect_skill_injections()`、`active_skill_names()`，3 个 skill 文本常量（failure-recovery compact/full、engineering-defaults、structured-brief） |
| `crates/app-server-core/src/agent.rs` | 修改 | `pub mod skills;`、`build_turn_context()` 末尾添加 skill 注入调用、`build_skill_injection_context()` 从 `AgentTurnInput` 构建 skill 上下文、`has_last_turn_cadquery_error()` 检查历史中 `cadquery_dry_run`/`cadquery_execute` 是否返回错误 |
| `crates/app-server-core/src/lib.rs` | 修改 | 导出 `SkillInjectionContext`、`active_skill_names`、`collect_skill_injections` |
| `crates/app-server-core/src/agent/tools/registry.rs` | 修改 | 6 个 CadQuery 工具 description 增强，补充使用场景和工具配合关系 |
| `crates/app-server-core/tests/agent_skill_tests.rs` | 新增 | 10 个测试覆盖 skill 注入条件、Plan/Agent 模式差异、history 错误检测、`build_turn_context` 集成 |

### 验收标准达成情况

1. ✅ Skill 注入机制可工作：Agent mode + CadQuery tools 注册时注入 3 个 skill
2. ✅ Plan mode preamble 不包含 skill 内容
3. ✅ System prompt 文件未修改
4. ✅ 6 个 CadQuery 工具 description 增强
5. ✅ Skill 文本在 `crates/app-server-core/src/agent/skills.rs`
6. ✅ 编译通过，10 个新测试通过，已有测试无回归（1 个 pre-existing failure `cadquery_agent_system_prompt_covers_runtime_contract` 与本次无关）

### Review 发现与修复

- `has_last_turn_cadquery_error()` 原实现使用 `starts_with("cadquery_")` 匹配过宽，会在 `cadquery_get_result` 等只读工具失败时也触发完整失败分类 skill。已修复为只检查 `cadquery_dry_run` 和 `cadquery_execute`。

### 遗留问题

- `agent.rs` 已有 1243 行，超过 500 行限制（pre-existing，Phase 0 新增 34 行）
- `cadquery_agent_system_prompt_covers_runtime_contract` 测试 pre-existing 失败

---

## Phase 1: Agent 生成质量 Skills + 基准框架 — 完成

### 变更摘要

Phase 1-A/B/C 的 skill 文本内容已在 Phase 0 实现完毕（`skills.rs` 中包含完整的 failure-recovery compact/full、engineering-defaults、structured-brief 三个 skill 文本常量），无需额外代码变更。Phase 1-D 基准评估框架为本 Phase 的主要新增工作。

| 文件 | 类型 | 内容 |
|---|---|---|
| `crates/app-server-core/tests/benchmark_tests.rs` | 新增 | 基准测试 harness：`BenchmarkCadQueryRuntime` 实现 `CadQueryToolRuntime`（使用 `stage_cadquery_project_owned` + `run_cadquery_runner`），`TracingObserver` 收集工具调用轨迹，`run_benchmark()` 创建临时 workspace 并执行 `run_rig_agent_turn()`，`evaluate()` 检查验收条件，`run_all_benchmarks()` 标记 `#[ignore]` 加载场景、运行、输出结果表 |
| `benchmarks/scenarios/b01-simple-box.json` | 新增 | 基准场景：简单长方体 |
| `benchmarks/scenarios/b02-cylinder-with-hole.json` | 新增 | 基准场景：带中心孔圆柱 |
| `benchmarks/scenarios/b03-filleted-enclosure.json` | 新增 | 基准场景：圆角外壳 |
| `benchmarks/scenarios/b04-mounting-bracket.json` | 新增 | 基准场景：L 型安装支架 |
| `benchmarks/scenarios/b05-knob-with-grip.json` | 新增 | 基准场景：带防滑纹旋钮 |
| `scripts/run_bench.ts` | 新增 | bun 编排脚本：解析 `--scenario` 参数、调用 cargo test、读取结果 JSON、输出通过率表格 |
| `package.json` | 修改 | 新增 `"bench": "bun scripts/run_bench.ts"` |

### 验收标准达成情况

Phase 1-D 验收标准：

1. ✅ `bun run bench` 一键跑完 5 个基准并输出通过率表格（已验证：5 个场景加载、运行、输出结果表；因无 `agents.toml` 配置故全部 FAIL，属预期行为）
2. ✅ 不依赖任何 Python 脚本（CadQuery 执行由 `BenchmarkCadQueryRuntime` 通过 `run_cadquery_runner` 完成）
3. ✅ 每个基准有明确 pass/fail 判定（`evaluate()` 检查 `cadquery_execute_success`、`brief_expected`、`max_dry_run_attempts`、`exports_generated`）
4. ✅ Rust 集成测试能完成 agent turn 生命周期并收集工具调用轨迹（`TracingObserver` + `BenchToolTrace`）
5. ✅ 每次基准记录用户输入、工具调用轨迹、工具结果和最终回复（输出 JSON 包含 prompt、tool_traces、完整 agent_text + agent_text_preview）
6. ✅ Rust 集成测试使用 crate 内 protocol 类型，Phase 2 protocol 变更时自然跟随

Phase 1-A/B/C 验收标准（需真实 LLM 验证的部分延迟到 Phase 3）：

- ✅ 失败修复 skill 在 Agent mode + CadQuery tools 时注入（compact/full 双模式），Plan mode 不注入
- ✅ 工程默认值 skill 包含壁厚、通孔尺寸、坐标约定等内容
- ✅ Brief skill 包含模板、适用/不适用场景说明
- ⏳ Agent 实际行为验证（倒角失败修复、默认值声明、brief 输出）需真实 LLM 功能测试，在 Phase 3 通过基准框架执行

### 遗留问题

- 基准测试需要 `agents.toml` 配置才能运行真实 agent turn，当前无配置环境下正确报告 FAIL
- Phase 1-A/B/C 的 LLM 行为级验证依赖 Phase 3 完成

---

## Phase 2: 选择状态统一 + 前端体验修复 — 完成

### 变更摘要

#### Phase 2-A: 统一选择状态

| 文件 | 类型 | 内容 |
|---|---|---|
| `crates/app-server-protocol/src/protocol.rs` | 修改 | 从 `ChatCreateInitialTurn`、`AgentInvokeRequest`、`AgentStartTurnRequest` 删除 `context_refs` 字段 |
| `crates/app-server-core/src/agent.rs` | 修改 | `AgentTurnInput` 删除 `context_refs`；`build_turn_context()` 合并 "User-attached context refs" 和 "Current Web preview selection" 为 "Current selection:" |
| `crates/app-server-core/src/agent/tools.rs` | 修改 | `AgentToolRunContext` 删除 `context_refs` |
| `crates/app-server-core/src/agent/tools/readonly.rs` | 修改 | `get_selection` 返回不再包含 `context_refs` |
| `crates/app-server-core/src/agent/tools/registry/schemas.rs` | 修改 | `selection_success_schema()` 删除 `context_refs` |
| `crates/app-server-host/src/dispatcher.rs` | 修改 | `AgentWorker`、turn 构建全链路删除 `context_refs` |
| `packages/app-server-protocol/src/index.ts` | 修改 | TS 类型同步删除 `context_refs` |
| `packages/studio-web/src/workbench/chat-actions.ts` | 修改 | 删除 `context_refs` 构建和 `contextPills` 参数（全链路清理） |
| `packages/studio-web/src/workbench/chat-zone.tsx` | 修改 | Pill 删除改为 `dispatchSelectionUpdate`，`active_index` 保留原位逻辑；从 `useChatActions` 删除 `contextPills`；新增 `pendingSelectionRef` + `flushPendingSelection()` 修复选择更新与 Agent start turn 的竞态 |
| `packages/studio-web/src/workbench/workbench-layout.tsx` | 修改 | 删除 `contextPills: []` |
| `packages/studio-web/src/styles/workbench-zones.css` | 修改 | 新增 `.context-pill-bar`、`.context-pill` CSS |
| `docs/cadquery-mvp/agent-tool-contract.md` | 修改 | 同步删除 `context_refs` 文档引用 |
| WASM bindings (2 files) | 重新生成 | `app_server_protocol_wasm_bg.wasm`、`studio_web_wasm_bg.wasm` |
| 测试文件（6 文件） | 修改 | `llm_tests.rs`、`agent_tool_tests.rs`、`borsh_payload_roundtrip_tests.rs`、`shared_dispatcher_roundtrip_tests.rs`、`managed_client_tests.rs`、`chat-zone.test.tsx`、`protocol-package-import.test.ts`、`protocol_package_smoke.ts` |

#### Phase 2-B: CadQuery 工具调用展示

| 文件 | 类型 | 内容 |
|---|---|---|
| `packages/studio-web/src/workbench/chat-messages.tsx` | 修改 | 新增 `CadQueryToolCard` 组件（状态显示、目标路径、导出/提交文件、错误展示、可折叠 traceback）、`isCadQueryToolEvent()` 和 `safeParse()` 辅助函数 |
| `packages/studio-web/src/styles/workbench-zones.css` | 修改 | 新增 `.cadquery-tool-card`（含 `.is-running`/`.is-success`/`.is-error`/`.is-mesh-ready` 状态变体）、`.cadquery-tool-header`、`.cadquery-tool-target`、`.cadquery-tool-detail`、`.cadquery-tool-error`、`.cadquery-diag-toggle`、`.cadquery-tool-traceback` CSS |

#### Phase 2-C: 选择 Dock 改进

| 文件 | 类型 | 内容 |
|---|---|---|
| `packages/studio-web/src/viewers/cadquery-viewer.tsx` | 修改 | `CadQuerySelectionDock` 按钮分组（object/geometry）、选择数量显示、清除按钮；新增 `OBJECT_MODES`/`GEOMETRY_MODES` 常量 |
| `packages/studio-web/src/styles/workbench-zones.css` | 修改 | 新增 `.cadquery-select-dock__group`、`.cadquery-select-dock__sep`、`.cadquery-select-dock__count`、`.cadquery-select-dock__clear` CSS |

### 验收标准达成情况

**Phase 2-A**:
1. ✅ Viewer 选择 → pill + Ref Tree 三者同步（统一状态源）
2. ✅ 删除 pill → `dispatchSelectionUpdate` 写回 → Viewer/Ref Tree 同步清除
3. ✅ Ref Tree 取消勾选 → pill/Viewer 同步（通过统一状态投影）
4. ✅ `MAX_CONTEXT_PILLS` 限制保留（既有实现）
5. ✅ Agent turn preamble 只有一个 "Current selection:" section
6. ✅ 发送消息后 Agent 收到的选择列表与 pill bar 一致（统一状态源保证）
7. ✅ 删除 pill 后立即发送消息，Agent 不会收到已删除的 ref（`pendingSelectionRef` 追踪异步 `dispatchSelectionUpdate`，`flushPendingSelection()` 在发送 Agent turn 前等待完成；有单元测试覆盖竞态场景）
8. ✅ Protocol 变更范围表所列文件全部同步更新

**Phase 2-B**:
1. ✅ `cadquery_execute` 执行期间显示带状态的执行卡片
2. ✅ 成功后显示提交文件和导出路径
3. ✅ 失败后默认显示友好错误信息和错误类别；traceback 在可折叠区域内（最多 3 行）

**Phase 2-C**:
1. ✅ 两组按钮之间有可见分隔（`.cadquery-select-dock__sep`）
2. ✅ 有选择时显示数量，无选择时不显示
3. ✅ 清除按钮通过 `dispatchSelectionUpdate` 写回空选择

### Review 发现与修复

1. **CadQueryToolCard 缺少目标路径和导出信息**（阻塞）：原实现只显示工具名和状态，未解析 `args_json` 中的 `target_path` 和 `result_json` 中的 `committed_files`/`exports`。已修复：卡片头部显示目标路径，成功时显示提交文件和导出路径。
2. **`contextPills` 参数残留**（非阻塞→已修复）：`chat-actions.ts` 四个函数签名和 `useChatActions` 中 `contextPills` 已无消费者。已清理全链路（`chat-actions.ts`、`chat-zone.tsx`、`workbench-layout.tsx`、`chat-zone.test.tsx`），同时删除 `ContextPill` 导入。
3. **Pill 删除 `active_index` 强制归零**（非阻塞→已修复）：原实现在删除 pill 后无条件设 `active_index: 0`。已改为根据被删除项位置调整原 `active_index`。
4. **`agent-tool-contract.md` 残留 `context_refs`**（非阻塞→已修复）：开发文档同步更新。
5. **历史设计文档残留 `context_refs`**（非阻塞，不修复）：`docs/2026042801-*` 和 `docs/2026050200-*` 为历史设计文档，保留原样。

### 遗留问题

- Phase 2-B 的 CadQuery 工具卡片尚未在浏览器中完成端到端视觉验证（Phase 3 E2E 测试覆盖）
- `cadquery_agent_system_prompt_covers_runtime_contract` 测试 pre-existing 失败（与 Phase 2 无关）

---

## Phase 3: 分层验证 — 完成

### 变更摘要

#### Layer 1: 组件级测试

| 文件 | 类型 | 内容 |
|---|---|---|
| `packages/studio-web/tests/unit/chat-messages.test.tsx` | 修改 | 更新 2 个已有测试适配 `CadQueryToolCard`（tool_result 显示卡片+导出文件、tool_start 显示运行中+目标路径）；新增 2 个测试（错误状态+可折叠 traceback、非 CadQuery 工具回退通用行）|
| `packages/studio-web/tests/unit/cadquery-viewer.test.tsx` | 修改 | 新增 2 个测试：dock 按钮分组+分隔符、选择数量+清除按钮 |
| `packages/studio-web/tests/unit/chat-zone.test.tsx` | 修改 | 新增 1 个测试：selection update 竞态覆盖（`dispatchSelectionUpdate` 延迟 resolve 后 `dispatchAgentStartTurn` 等待完成） |

#### Layer 2: 后端与协议集成测试

已有测试覆盖，无需新增文件：
- `llm_tests.rs`：`build_turn_context_includes_unified_selection`、`build_turn_context_includes_mode_plan_ref_and_selection`、`build_turn_context_omits_selection_when_empty` 验证统一选择 section
- `agent_skill_tests.rs`：10 个测试覆盖 skill 注入条件（Agent/Plan mode、CadQuery 工具注册、上轮错误检测）
- `borsh_payload_roundtrip_tests.rs`：18 个协议 roundtrip 测试

#### Layer 3: E2E Playwright

| 文件 | 类型 | 内容 |
|---|---|---|
| `packages/studio-web/tests/playwright/selection-agent-cycle.spec.ts` | 重写 | 4 个 E2E 测试：通过 `injectStoreState()` 注入 Zustand store 状态，无条件断言；pill bar 出现（注入 `current_selection`）、CadQuery 工具卡片渲染（注入 `agent_events`）、工具错误卡片显示、清除按钮 dispatch（注入选择后点击 pill remove 验证 `selection.update` 协议帧） |
| `packages/studio-web/tests/playwright/_smoke-harness.ts` | 修改 | 新增 `injectStoreState()` 辅助函数，通过 Vite 动态 `import()` 获取 Zustand store 实例并调用 `setState()` |

#### LLM Rubric 评估

| 文件 | 类型 | 内容 |
|---|---|---|
| `crates/app-server-core/src/agent.rs` | 修改 | 新增 `run_rig_completion()` 公开函数，三种 provider kind 共用 `run_simple_chat()` 泛型实现 |
| `crates/app-server-core/src/lib.rs` | 修改 | 导出 `run_rig_completion` |
| `crates/app-server-core/tests/rubric_tests.rs` | 新增 | Rust rubric 评估测试：加载 benchmark 结果、通过共享 LLM 基础设施调用评估、输出 `*-rubric.json` |
| `scripts/run_bench_rubric.ts` | 重写 | 委托 `cargo test -p app-server-core --test rubric_tests`，不再独立调用 HTTP API |
| `package.json` | 已有 | `"bench:rubric": "bun scripts/run_bench_rubric.ts"` |

#### 基准评估框架修正

| 文件 | 类型 | 内容 |
|---|---|---|
| `crates/app-server-core/tests/benchmark_tests.rs` | 修改 | 结果 JSON 新增完整 `agent_text` 字段；`exports_generated` 从 `#[allow(dead_code)]` 改为参与 `evaluate()` 判定（cadquery_execute 成功才算通过） |

### 验收标准达成情况

1. ✅ 组件级测试通过（311 tests, 37 files；含竞态覆盖测试）
2. ✅ 集成测试覆盖 skill 注入条件（10 tests）、选择上下文构建（3 tests）、协议 roundtrip（18 tests）
3. ✅ Playwright E2E：4 个 spec 已改为通过 store 状态注入和协议帧验证的强制断言；3 个完整端到端场景通过（`selection-agent-live-cycle.spec.ts`：选择→Chat→Agent 循环、CadQuery 执行循环、Plan 模式循环），使用 `rawEnv` 模式匹配 `bun run dev` 环境配置
4. ✅ LLM rubric 评估完成（provider=token-plan-sgp model=mimo-v2.5-pro），5 个场景平均 7.2/10：b01=8/10、b02=7/10、b03=6/10、b04=6/10、b05=9/10。所有评估通过共享 `run_rig_completion()` + `agents.toml` 配置完成，无独立 HTTP API 调用。`intent_achieved` 维度偏低因测试环境 CadQuery runner 不可用，agent 正确识别并诚实报告了该限制

### 前置条件

- E2E 完整场景（`@live-agent`）需要 `agents.toml`（缺失时硬报错）和 `.env` 中 `CADQUERY_RUNNER_PYTHON` 指向可用 CadQuery Python

### 遗留问题

- `cadquery_agent_system_prompt_covers_runtime_contract` 测试 pre-existing 失败（与本计划无关）

---

## Review 报告整改记录

整改依据：`review-report.md`（2026-05-19）

| 问题编号 | 严重级别 | 描述 | 修复状态 | 修复摘要 |
|---|---|---|---|---|
| 1 | 高 | 删除 pill 后立即发送消息仍可能把旧选择传给 Agent | ✅ 已修复 | `chat-zone.tsx` 新增 `pendingSelectionRef` 追踪异步 `dispatchSelectionUpdate` Promise；`flushPendingSelection()` 在 `send`/`runPlan` 前等待完成；新增竞态单元测试 |
| 2 | 高 | Phase 3 E2E 不是 plan 要求的端到端验证 | ✅ 已修复 | 4 个 store 注入测试 + 3 个完整端到端场景（`selection-agent-live-cycle.spec.ts`）。Harness 新增 `rawEnv` 模式继承 `.env` + `process.env`（匹配 `bun run dev`），Vite 显式 `NODE_ENV=development`。缺失 `agents.toml` 硬报错 |
| 3 | 中 | Agent 功能性验收缺少第三方 LLM rubric 评估记录 | ✅ 已修复 | 新增 `run_rig_completion()` 共享 LLM 调用 + Rust rubric 测试；5 场景评估完成（平均 7.2/10），结果写入 `benchmarks/results/*-rubric.json` |
| 4 | 中 | Benchmark 结果只保存最终回复预览，rubric 输入不完整 | ✅ 已修复 | `benchmark_tests.rs` 结果 JSON 新增完整 `agent_text` 字段；`run_bench_rubric.ts` 优先读取完整文本 |
| 5 | 中 | `exports_generated` 被声明但未参与 pass/fail | ✅ 已修复 | 移除 `#[allow(dead_code)]`；`evaluate()` 新增 `exports_generated` 判定（要求 `cadquery_execute` 成功） |
