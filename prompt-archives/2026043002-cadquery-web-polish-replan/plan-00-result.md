# CadQuery Web Polish Replan Result

## 当前状态

- 计划已创建并经过独立 reviewer 按 `AGENTS.md` 和 `plan-prompt.md` 作为规则与前提审查。
- 已根据 reviewer 结论局部重写 `plan-00.md`。
- 已根据 2026-04-30 用户最新反馈再次修订 `plan-00.md`：PRD 示例暂不处理，system prompt 既有示例块暂不处理，Rust 代码里的 LLM 可见 feature 示例或占位命名纳入 Phase -1，system prompt 只补充 feature 命名责任指引。
- Phase -1 已完成执行、验证与独立 review。
- Phase 0 已完成当前状态审计、基线验证与独立 review。后续按计划进入 Phase 1。

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
- Phase 1：未开始。
- Phase 2：未开始。
- Phase 3：未开始。
- Phase 4：未开始。
- Phase 5：未开始。
- Phase 6：未开始。

## 验证记录

- `cargo test -p app-server-core cadquery_agent_system_prompt --test agent_tests`：3 passed，0 failed。
- `cargo test -p app-server-core cadquery_tool_schemas_do_not_suggest_placeholder_feature_keys --test agent_tool_registry_tests`：1 passed，0 failed。
- `cargo test -p app-server-core workspace_tool_executor_cadquery --test agent_tool_tests`：22 passed，0 failed。
- `rg -n "AirPods|airpods|wireless charging|charging_pad|airpods_recess|front_finger_notch|cable_relief|placement_pocket|access_notch|human_readable_feature_name|semantic_part_feature_name|semantic_component_feature_name|semantic_assembly_feature_name" crates/app-server-core/src docs/cadquery-mvp/agent-system-prompt.md packages/studio-web/src packages/app-server-protocol/src crates/app-server-protocol/src -g '!target' -g '!node_modules'`：无命中，exit 1。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/cadquery-viewer-selection.spec.ts`：4 passed，0 failed。

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
