# Agent 生成质量基础 + 选择→Ref→Agent 前端体验

## 任务背景

基于 2026-05-18 竞品调研（`docs/2026051800-competitive-research/`）的产品路线图，规划 Phase 0（Agent 生成质量）和 Phase 1（端到端体验验证）中"选择→Ref→Agent 数据流验证"的联合实施方案。

## 当前状态

### 后端

- Agent 后端架构完备：19 个注册工具、原子暂存、Plan/Agent 模式、Web 搜索
- System prompt 位于 `docs/cadquery-mvp/agent-system-prompt.md`，291 行单文件
- 选择相关工具已实现：`get_selection`、`resolve_ref`、`cadquery_get_result`、`cadquery_resolve_selection`
- Agent turn 上下文注入已实现：每个 turn 开头注入格式化的选择快照（`agent.rs:817-822` via `selection_context()`）
- CadQuery Runner 输出完整拓扑 metadata + feature_map + 原子暂存
- 缺少：失败分类修复循环、工程默认值、结构化 brief 提取

### 前端

- studio-web 72 个文件，核心骨架已搭建
- 选择→Ref→Agent 管线的代码骨架完整
- **关键问题**（2026-05-18 排查）：
  1. Context Pill 完全没有 CSS 样式（`.context-pill-bar`、`.context-pill` 未定义），选择上下文在 Chat 中不可见
  2. Agent 工具调用统一用单行按钮展示，CadQuery 执行没有专门的进度和结果卡片
  3. 选择模式 Dock 8 个按钮平铺无分组，没有选择状态摘要
  4. Chat pill、Viewer 选择高亮、Ref Tree 选择三者之间不同步——当前实现不可用
  5. Ambiguous 选择对话框过于简陋，不显示候选列表
  6. Ref Tree 无折叠/过滤，复杂模型会有上百行
  7. Chat 中没有 CadQuery 结果的内联预览（缩略图/尺寸/特征列表）

### 用户强制约束

1. 除 CadQuery Runner 外的所有场景禁止新增 Python 脚本
2. UI 端到端测试优先级高于集成测试
3. System prompt 保持单个文件；模块化通过 skill 机制实现（动态注入 turn context，非 include_str 拼接）
4. 工程默认值保留 3D 打印通用数值，不硬编码特定硬件尺寸（Agent 需要时用 web_search 查）
5. Brief 不是每个 turn 强制输出，而是在满足特定条件时触发
6. Chat pill、Viewer 选择、Ref Tree 选择必须保持完全同步——单一选择状态源
7. Plan 中不包含时间估计
8. App server 只有 WebSocket transport，没有 HTTP REST API；基准框架必须使用 WebSocket client

## 讨论记录（2026-05-18）

### Review 要点与用户决策

1. **基准框架 transport**：plan 原本假设 HTTP API，实际 app-server 只有 WebSocket。用户确认使用 WS client。
2. **Prompt 模块化方式**：原方案是拆成 6 个 include_str! 文件拼接。用户否决，要求 system prompt 保持单文件，新能力通过 skill 机制（动态 turn context 注入）实现。
3. **工程默认值范围**：保留 3D 打印通用默认值（壁厚、公差、倒角），删除硬件尺寸表（Arduino/RPi 等），需要时用 web_search。
4. **Brief 触发条件**：不是每个 turn 强制输出，需要定义具体触发条件。
5. **选择状态同步**：用户明确要求 Chat pill、Viewer 选择高亮、Ref Tree 选择三者完全同步，共享单一选择状态。删除 pill = 取消选择 = Viewer 取消高亮 = Ref Tree 取消勾选。反之亦然。这意味着 Agent 只看到一个 `Current selection`，不再区分 `context_refs` 和 `selections`。
6. **并行执行**：Phase 0-1（后端 prompt/skill）和 Phase 2（前端）原判断为没有文件级交叉，可以并行；后续 review 已修正该判断，Phase 2 会触碰 Agent turn context，必须按文件边界重新判断是否能并行。
7. **E2E 测试分层**：Phase 3 应该分层验证（组件级 → 集成 → E2E），不要所有验证都压到 Playwright E2E 层。
8. **Brief 语义**：brief 是生成 / 修改前的可读摘要，不是阻塞确认步骤；本轮不新增确认协议、确认 UI 或等待用户修正假设的暂停流程。
9. **Agent skill 边界**：本计划中的 Agent skill 指 budn' 产品 Agent 的动态 turn-context 指令模块，由 app server 注入产品 Agent prompt；不得写入 `AGENTS.md`、Codex skill 目录或工程协作规则来代替产品行为修复。
10. **Protocol 调整**：统一选择状态优先于保留旧字段形态；如 WebSocket / app-server protocol 为此需要调整，可以修改，但必须同步 Rust protocol、TS protocol package、WASM bridge / generated package、host、studio-common、studio-web 和相关 roundtrip 测试。

## 参考文档

- `docs/2026051800-competitive-research/roadmap.md` — 产品路线图（Phase 0-5）
- `docs/2026051800-competitive-research/research-report.md` — 竞品调研报告
- `docs/cadquery-mvp/agent-system-prompt.md` — 当前 Agent system prompt
- `docs/cadquery-mvp/init.md` — MVP PRD

## 后续 Review 与修订指令（2026-05-18）

用户要求读取 `prompt-archives/2026051801-agent-quality-and-selection-ux` 的 plan 与依赖文档，执行 review 和 challenge，随后要求“改”。本轮修订结论：

1. Phase 2 不能再描述为纯前端任务，因为 `build_turn_context()` 的选择 section 合并属于后端 Agent turn context 改动。
2. Chat pill、Viewer 选择、Ref Tree 选择必须全部通过 app server 当前 `SelectionUpdateRequest` snapshot 表达，删除 pill 不能继续只维护前端本地过滤状态。
3. 失败修复 skill 不能只在“上一轮失败后的下一轮”触发；同一个 Agent turn 首次 CadQuery 失败后也必须有修复规则可用。
4. WebSocket 基准不能直接假设 `AgentStartTurn` 可独立调用；必须先创建或复用 chat / agent，获得 `agent_id` 后再启动 turn 并订阅事件流。
5. WebSocket 黑盒测试不能稳定断言完整 preamble 字符串；turn context 和 skill 触发应由 Rust 测试覆盖，WebSocket 层只验证可观察协议流和事件。
6. 用户界面默认区域不能直接展示 traceback；只显示友好错误摘要、错误类别和可恢复动作，内部诊断信息进入展开区或日志。
7. Agent 功能性验收必须包含独立第三方 LLM rubric 评估，不能只依赖固定文本匹配或工具轨迹。

## 二次独立 Review 后用户决策（2026-05-18）

1. Brief 采用“可读摘要”语义，不作为阻塞确认流程。
2. 工程默认值与 system prompt “不猜测”之间的优先级暂不作为本轮重点，仅保留默认值 skill 的当前方向。
3. 必须写清楚本计划中的 Agent skill 是 budn' 产品 Agent 的动态 turn-context 指令模块，不是 Codex / `.agents/skills` / `AGENTS.md` 工程协作 skill。
4. WebSocket / app-server protocol 细节如果为了统一选择状态需要调整，可以修改；计划不再强制保持旧 protocol 数据结构不变。

## 三次独立 Review 后用户决策（2026-05-18）

1. Phase 1-D benchmark client 与 Phase 2 protocol 调整的依赖关系需要写清楚：如果 Phase 2 修改 app-server protocol，必须同步更新 benchmark client。
2. Skill 触发器需要收敛为结构化信号优先，避免执行者只靠自然语言关键词判断。
3. 产品 Agent skill 的存放边界需要写清楚：必须放在产品 Agent 可审计的代码或产品 prompt 附属路径中，不得放进工程协作 skill 目录。
4. Protocol 变更时必须显式同步 generated package 和相关 roundtrip / benchmark 验证。
5. Agent 功能性测试程序可以读取本机配置以启动真实开发环境模型；限制是 LLM 不得读取 `agents.toml` 内容，测试输出、日志和归档不得包含配置正文、密钥或 provider 原始配置正文。

## 四次独立 Review 后用户决策（2026-05-18）

1. **Phase 0 Skill 机制泛化**：保持不变，不去掉 Phase 0。
2. **Skill 触发条件**：注入条件收敛为纯结构化信号（mode + 工具注册状态 + 上一轮错误状态），语义判断（是否输出 brief、是否执行修复）交给 LLM 在 skill 文本指导下自行决定。移除所有"用户意图"相关触发条件。
3. **基准框架实现路径**：核心基准执行改用 Rust 集成测试（直接调用 agent turn 内部 API），bun 只负责编排和报告。不从零构建 WebSocket client。如需 WS 层验证，复用现有 TS protocol package 和 `tests/run_websocket_host.test.ts` 基础设施。LLM rubric 评估延迟到 Phase 3。
4. **Phase 2-A protocol 变更范围**：在 plan 中显式列出所有预期受影响文件（11 个文件/目录），含变更内容说明。
5. **依赖图**：Phase 0 → Phase 1 → Phase 2 → Phase 3 全部串行，不再保留并行可能性。
6. **Phase 2-B（Context Pill CSS）**：合并到 Phase 2-A。Pill 可见是验证选择同步的前提。原 Phase 2-C/2-D 顺序重编为 2-B/2-C。
7. Phase 3 端到端测试环境依赖、selection_context() token 效率、回滚策略：暂不处理。
