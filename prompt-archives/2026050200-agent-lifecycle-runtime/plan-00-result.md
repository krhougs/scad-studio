# Agent 生命周期与 WebSocket 观察架构执行结果

## 当前状态

- 计划已创建，尚未执行实现。
- 当前仅完成需求整理、设计文档、计划存档和已知问题记录。
- 按用户要求，本轮不修改产品代码。
- 已按用户最新约束修正范围：provider `base_url` 属于本次产品整改范围，根目录 `llm.toml` 迁移不属于本次范围。
- 已根据独立 review 修正 plan，但按用户要求忽略 provider `base_url` 运行路径验证增强项。
- 已根据用户补充约束修正 plan：chat id 由后端随机生成，`chats.json` 管理显示顺序和 metadata，chat 在产品语义上等同于 agent。

## 已完成事项

- 提交根 `AGENTS.md` 规则变更：`a2526e9 Document complete feature implementation rule`。
- 新增设计文档：`docs/2026050200-agent-lifecycle-runtime/architecture.md`。
- 新增 prompt 存档：`prompt-archives/2026050200-agent-lifecycle-runtime/plan-prompt.md`。
- 新增实施计划：`prompt-archives/2026050200-agent-lifecycle-runtime/plan-00.md`。
- 记录已知问题：`scad-scene` 系统字体探测仍使用同步外部命令。
- 修正计划中的 Phase review 流程、`Option<String>` reasoning 语义、多 WebSocket 交互验收、idle 运行对象释放语义、分层边界验收和 Phase 2 行为验收。
- 新增 Chat identity 与 `chats.json` 前置 Phase，避免继续把 JSONL 文件名作为 chat id 来源。

## 尚未执行

- 未改 protocol。
- 未改 app server host runtime。
- 未改 studio-common / WASM bridge / Web UI。
- 未迁移 Agent 操作目标到 `agent_id`。
- 未实现 chat 模型绑定。
- 未实现多 WebSocket observer、event log、snapshot replay 或 idle 资源释放。
- 未实现 provider type 与 `base_url` 产品配置整改。
- 未实现 `chats.json` 驱动的 chat identity、显示顺序、当前 chat 和 metadata。
- 未迁移根目录 `llm.toml`，且本计划不要求迁移。

## 后续执行入口

执行前必须先检查 `plan-00.md` 中的执行前检查项。若发现 plan 与当前源码不一致，以当前源码和现有产品行为为准修正计划后再进入实现。
