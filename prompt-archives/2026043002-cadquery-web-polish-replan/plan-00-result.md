# CadQuery Web Polish Replan Result

## 当前状态

- 计划已创建并经过独立 reviewer 按 `AGENTS.md` 和 `plan-prompt.md` 作为规则与前提审查。
- 已根据 reviewer 结论局部重写 `plan-00.md`。
- 已根据 2026-04-30 用户最新反馈再次修订 `plan-00.md`：PRD 示例暂不处理，system prompt 既有示例块暂不处理，Rust 代码里的 LLM 可见 feature 示例或占位命名纳入 Phase -1，system prompt 只补充 feature 命名责任指引。
- Phase -1 已完成执行、验证与独立 review。后续按计划进入 Phase 0。

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
- Phase 0：未开始。
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
