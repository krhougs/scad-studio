# Web Agent Operation 下拉框 Prompt 存档

## 原始上下文

用户反馈 Web UI 中看不到任何和 Agent 操作模式有关的入口。当前代码调查结果：

- Web 端普通消息默认发送 `operation: "auto"`。
- 已存在隐藏 slash command：`/plan`、`/execute`、`/inform`。
- `packages/studio-web/src/styles/workbench-zones.css` 中存在 `.chat-input .tools button.mode` 样式，但 `ChatComposer` 没有渲染模式控件。
- Plan 卡片确认仍应继续走现有 `agent.plan.confirm`，避免普通文本绕过已保存 Plan 确认范围。

## 后续用户确认

用户明确要求：

> 在输入框做下拉框，而且要支持auto。

## 任务目标

在 Web Agent 输入框区域增加可见 operation 下拉框，支持 `auto`、`inform`、`plan`、`execute`，并让发送消息时使用当前下拉框选中的 operation。

## 约束

- 对外产品名仍使用 `budn'`。
- 只修改 Web Agent 输入与发送链路相关代码，不改 protocol 枚举、不改后端权限模型。
- `auto` 必须作为可选项保留。
- Slash command 作为既有兼容入口保留；若输入内容包含 slash command，应继续按已有 slash command 解析结果发送。
- 使用测试先覆盖行为，再改实现。
