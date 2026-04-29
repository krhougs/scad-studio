# Phase 0b — API 验证与决策确认

## 已安装版本

- `@assistant-ui/react@0.12.27`
- 依赖链：`@radix-ui/*`、`zustand`、`react-textarea-autosize`、`assistant-stream`、`zod`
- **无 Tailwind 依赖**（peerDependencies 仅 react/react-dom）

## 已锁定设计决策验证

### 1. Borsh 序列化 — 通过

Borsh 仅用于内存传输（protocol.rs 的 BorshSerialize/BorshDeserialize derive），不涉及 assistant-ui。Phase 0a 已完成 run_id 字段的 Borsh 覆盖。assistant-ui 层面无需关注此决策。

### 2. isRunning 语义 — 通过

`useExternalStoreRuntime({ isRunning, messages, convertMessage, onNew })` 接受 `isRunning` 布尔值。文档确认其用途：
- 控制 Composer 的发送按钮 disabled 状态
- 控制 Cancel 按钮可见性
- **不用于**消息合并或去重判断

去重逻辑由外部 store 层（我们的 snapshot 转换函数）负责，通过稳定的 message id 实现。

### 3. 消息合并策略 — 通过

`ThreadPrimitive.Messages` 对每条消息独立调用 render function：
```tsx
<ThreadPrimitive.Messages>
  {({ message }) => <MyMessageComponent message={message} />}
</ThreadPrimitive.Messages>
```
每条消息产生独立的 DOM 节点，不存在相邻同 role 消息合并行为。`convertMessage` 逐条转换，每条消息保持独立身份（通过 `id` 字段）。

### 4. Markdown 渲染 — 通过

`MessagePrimitive.Parts` 支持自定义渲染：
```tsx
<MessagePrimitive.Parts>
  {({ part }) => {
    if (part.type === "text") return <ExistingMarkdownRenderer content={part.text} />;
    if (part.type === "data" && part.name === "agent-event") return <AgentEventCard data={part} />;
    return null;
  }}
</MessagePrimitive.Parts>
```
无需引入 `@assistant-ui/react-markdown`。现有 Markdown 渲染器通过 `part.type === "text"` 分支接入。

### 5. run_id 写入规则 — 通过

Phase 0a 已完成。只有最终 assistant 回答写入 run_id，工具调用/结果/用户消息/取消后的部分回答均为 None。

### 6. studio-web 边界 — 通过

@assistant-ui/react 是纯 React 库，仅影响 studio-web 包的 UI 层。共享状态机、协议扩展、WASM 类型同步均为 Phase 0a 的前置依赖，不在本迁移范围内。

### 7. 运行时选择 — 通过

`useExternalStoreRuntime` 的工作模式：
- 调用方持有消息数组的所有权（我们的 snapshot 驱动数据）
- 通过 `convertMessage` 回调将外部消息转换为 `ThreadMessageLike`
- assistant-ui 只做订阅和渲染
- 30Hz snapshot 更新下，通过稳定 `id` 实现增量 diff

`ThreadMessageLike` 支持 `id` 字段，可用于稳定身份标识。

### 8. 回退策略 — 通过

确认使用 assistant-ui 原语（ThreadPrimitive、MessagePrimitive、ComposerPrimitive），直接替换旧 UI。不使用 `@assistant-ui/react-ui`（Tailwind 版本），不引入额外样式包。

## 风险解除

| 风险 | 状态 | 说明 |
|------|------|------|
| assistant-ui 强依赖 Tailwind | **已解除** | @assistant-ui/react@0.12.27 无 Tailwind 依赖，使用 Radix UI 无样式原语 |
| 30Hz snapshot 导致过多 re-render | 待 Phase 1 验证 | 通过稳定 id + 增量缓存缓解 |
| 合成消息缺稳定身份 | 待 Phase 1 验证 | 禁止运行时生成 id，所有 id 来自 snapshot 数据 |

## 结论

全部 8 项已锁定设计决策均标注「通过」。可进入 Phase 1。
