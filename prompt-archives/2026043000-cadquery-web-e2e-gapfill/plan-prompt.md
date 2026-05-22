# Plan prompt 存档

本目录对应任务：**CadQuery Web 端到端补缺验收**。

## 背景

`prompt-archives/2026042700-cadquery-mvp-design/plan-00.md` 已完成 CadQuery MVP 主体设计与多阶段执行。后续结果记录显示 CadQuery runner、Agent tool call、Web Chat、Viewer selection 和 Agent / Plan 双模式已逐步补齐，但仍需要以真实 Web 操作为准做端到端查漏补缺。

## 用户当前请求

用户要求基于 `prompt-archives/2026042700-cadquery-mvp-design/plan-00.md` 的执行结果继续补缺，并明确以下验收目标：

1. 自行启动 Web dev server。
2. 自己在网页中新建 Chat。
3. 在 Chat 中以“我想做一个放在车里的无线充电板上的给AirPods用的垫子”为起点，让该 Chat 成功使用 CadQuery 建模。
4. 让产生的模型可以在 Web 端文件列表中打开、预览。
5. 在预览区域中可以交互选择 Ref，并用于后续修改。
6. 自行与 Agent 对话完成目标。
7. 过程中出现的前端问题、LLM 消息中断、tool call 出错等问题需要自行解决。
8. 发现前端体验不佳的地方也要自行解决。
9. 全部调通后再通知用户，中间不要中断，也不要询问意见。

## 强制约束

- 对外产品名使用 `budn'`；代码标识符继续使用 `budn`。
- CadQuery `.py` 模型文件只能通过 App Server / CadQuery tool / staging 边界生成或修改。
- Web 与桌面必须走同一 app server protocol；本轮不引入绕过 protocol 的 Web 本地文件系统读写。
- 本轮默认使用 `bun` 启动和验证 Web 工具链。
- 不新增项目内 Python 辅助脚本；`budn_cad_runner` 仍是唯一 Python 例外。
- 当前运行环境没有暴露 Browser 插件要求的 Node REPL `js` 工具；浏览器验证允许使用仓库内 Playwright 或等价本地 Web 验证链路。
- 当前开发者工具约束要求未得到用户显式 subagent 授权时不能启动 subagent；本轮不使用 subagent review，改用可复现测试、浏览器验证和结果归档记录风险。

## 相关资料

- `prompt-archives/2026042700-cadquery-mvp-design/plan-00.md`
- `prompt-archives/2026042700-cadquery-mvp-design/plan-00-result.md`
- `prompt-archives/2026042900-agent-tool-calls/plan-00.md`
- `prompt-archives/2026042902-agent-plan-workspace-flow/plan-00.md`
- `docs/cadquery-mvp/agent-system-prompt.md`
- `docs/cadquery-mvp/agent-tool-contract.md`
- `docs/known_issues.md`
