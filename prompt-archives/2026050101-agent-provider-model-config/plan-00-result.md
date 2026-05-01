# Agent Provider / Model 配置与 Web 切换执行结果

## 当前状态

- 计划已创建，尚未开始实现。
- 执行前已检查：当前计划无 `TBD`、`TODO`、待确认项、未选择方案或缺失验收标准。
- 约束来源已核对：原始用户需求、后续补充需求、根 `AGENTS.md` 的 Plan Mode / 工具链 / app server / protocol / Web 边界要求。

## 独立 Review 结论

- 已启动独立只读 reviewer 审查 `plan-prompt.md`、`plan-00.md`、`plan-00-result.md` 和根 `AGENTS.md`。
- 最终结论：未发现阻塞项、高风险或需要修改计划的普通问题。
- reviewer 确认计划覆盖以下强制要求：
  - OpenAI Responses API 与 Anthropic API。
  - 多 provider / 多模型 `agents.toml`，且 `agents.toml` 被 ignore。
  - provider 模型发现默认开启，发现结果与手动配置同时生效，同 id 手动配置是字段级 override。
  - 配置文件与 Web UI 支持 `reasoning_effort` 和 `service_label`，Anthropic 也支持 reasoning effort。
  - Web UI 真实支持 provider/model 列表、模型切换、reasoning effort 和 service label 控件。
  - wire protocol、WASM bridge、TypeScript package 和前端接口支持读取 provider/model 列表。
  - `agent.invoke` / 发消息 API 携带 provider、model、reasoning effort 和 service label。
  - native web search 默认开启；`web_search_supported` 是布尔值；provider 实际调用失败按 Agent error 暴露。
  - `bun run web` 按新 env / config 格式验证。
  - 现有 Web、CadQuery、protocol、workspace tree、preview、Agent run 等功能受保护。

## 执行阶段需重点验证

- `web_search_supported` 默认按 provider kind 推导为 `true` 时，新发现模型如果实际不支持 web search，错误必须按 Agent error 暴露，且不得影响 Web 工作台启动和模型切换。
- Anthropic `reasoning_effort` 到 thinking / budget 的映射必须在实现阶段核对当前 Rig 与官方 API 行为。
- 生产 Web 发消息路径必须始终携带 provider/model/reasoning/service 参数；缺参 fallback 只能用于测试。

## Phase 记录

阶段完成时统一记录：完成情况、变更摘要、验证证据、独立 review 结果、阶段提交 SHA、遗留问题；Phase 6 额外记录 Plan 级 review 结果。

| Phase | 名称 | 状态 |
| --- | --- | --- |
| 1 | 配置格式与 ignore 基线 | 未执行 |
| 2 | Provider registry 与 Agent 执行分发 | 未执行 |
| 3 | Protocol 与 Studio common capability 扩展 | 未执行 |
| 4 | Web 模型选择 UI 与状态管理 | 未执行 |
| 5 | Host 切换命令持久状态与 `bun run web` 验证 | 未执行 |
| 6 | 文档、已知问题与最终验证 | 未执行 |
