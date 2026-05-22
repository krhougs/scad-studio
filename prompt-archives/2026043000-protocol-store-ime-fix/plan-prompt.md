# Protocol Store 重构 — 修复 CJK IME 输入卡死

## 背景

CJK 输入法在 chat input 中卡死，ASCII 输入正常。根因已确认：

```
client.pump() (每帧 rAF ~60Hz)
  → 任何 protocol 事件 → onSnapshotDirty()
  → setSnapshot(client.snapshot())          ← WASM 序列化，每次都是全新 JS 对象
  → WorkbenchLayout 整棵树 re-render
  → … → ComposerPrimitive.Input re-render
  → TextareaAutosize useLayoutEffect (无 deps，每次 render 都执行)
  → 同步 DOM 测量 → 破坏浏览器 IME 状态机
```

`@assistant-ui/react` 官网不存在此问题，因为它们没有外部状态以 60Hz 推送。

## 用户强制约束

- 方案 A：新建独立 Zustand protocol store，一次性到位
- 必须按 AGENTS.md 规范存档 plan

## 当前 snapshot 消费者分析

### WorkbenchLayout 直接使用
- `snapshot?.workspace_current?.root_name` → topbar / left panel 的 rootName
- `snapshot?.agent_run` → agentRun（控制 plan 执行按钮禁用状态）
- `snapshot?.chat_sessions` → handleRunMarkdownPlan
- `snapshot?.current_chat_session` → handleRunMarkdownPlan

### LeftPanel 透传
- `snapshot as ChatSnapshot` → 直接传给 ChatZone

### ChatZone 消费
- `snapshot?.chat_sessions`
- `snapshot?.current_chat_session`
- `snapshot?.current_chat_history`
- `snapshot?.agent_run`
- `snapshot?.agent_events`
- `snapshot?.current_selection`
- `snapshot?.llm_configured`

## 关键文件

- `src/state/ui-store.ts` — 现有 Zustand UI store（注释明确写 "No protocol business state"）
- `src/workbench/workbench-layout.tsx` — useState<Snapshot> + onSnapshotDirty
- `src/workbench/chat-zone.tsx` — ChatSnapshot 类型定义 + ChatZone 组件
- `src/workbench/left-panel.tsx` — 透传 snapshot
- `src/workbench/chat-runtime.tsx` — useChatRuntime
- `src/wasm-bridge/client.ts` — WasmClient.snapshot()
- `src/wasm-bridge/event-stream.ts` — onSnapshotDirty 调用点
