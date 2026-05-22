# Web Agent Chat UI 问题修复 Prompt 存档

## 用户输入

用户指出 Web 前端存在以下问题：

1. 冷启动时 Chat 列表没有被正确加载；新建 Chat 后，旧记录还显示在上面。
2. 文件列表缺少刷新按钮。
3. Agent 工作时输出排版混乱；文字输出、思考状态和 tool call 应按先后顺序排列。
4. 输出 thinking 或 tool call 后，正在恢复的动画丢失，最终返回的文字内容会被丢弃。
5. 消息不会自动滚动到最新。

## 当前定位

- `packages/studio-web/src/workbench/chat-zone.tsx` 冷启动只发起 `chat.list`，列表返回后没有自动加载首个 session 的 history。
- `crates/studio-common/src/managed_client/inbound.rs` 收到 `ChatCreated` 时只切换 `current_chat_session`，没有清空旧 `current_chat_history`，因此新建 Chat 后可能继续显示旧 session 记录。
- `packages/studio-web/src/workbench/chat-messages.tsx` 当前按“历史消息 → agent events → streaming text → thinking”固定分段渲染，无法保持 token、tool call、done 的实时顺序。
- `packages/studio-web/src/workbench/chat-zone.tsx` 的 `useStreamAccumulator()` 在 `agent.done` 时立即清空 `streamText`，若 history 刷新尚未完成，最终文字会暂时或永久消失。
- `packages/studio-web/src/workbench/files-panel.tsx` 没有暴露用户可点击的刷新入口；`workbench-layout.tsx` 已有 root 和 expanded directory 刷新函数，可复用。
- `.chat-body` 是滚动容器，但 Chat 渲染没有在消息、事件或流式 token 更新后滚动到底部。

## 约束

- 不改变 Agent / Plan 双模式协议。
- 不绕过 app server protocol 读取或写入 workspace。
- 不引入 Python 脚本。
- 优先保持现有 Web 组件结构，只做必要修复。
- 修复必须先增加失败测试，再实现。
