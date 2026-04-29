# Plan 00: Protocol Store 重构

## 目标

用 Zustand store 替代 WorkbenchLayout 中的 `useState<Snapshot>`，实现按字段粒度的订阅。chat 组件只在 chat 相关字段真正变化时 re-render，从根本上解决 CJK IME 输入卡死问题。

## 验收标准

1. CJK IME 输入在 chat input 中正常工作（手动验证）
2. 非 chat 事件（watch_event、transport 状态变化等）不触发 ChatZone 及其子树 re-render
3. 所有现有功能不回归：workspace 文件列表、chat 会话切换、agent 运行/取消、topbar 状态
4. `applySnapshot` 的结构比较逻辑有单元测试覆盖
5. typecheck 通过
6. 现有单元测试通过

---

## Phase 1: 创建 protocol-store 与 applySnapshot

**目标**: 新建 `src/state/protocol-store.ts`，定义按域拆分的 store 状态，实现带结构比较的 `applySnapshot`。

**前序目标保护**: 无（首个 Phase）。

**验收标准**:
- store 按域拆分为 workspace / chat / transport 三个区域
- `applySnapshot(raw)` 对每个域做浅结构比较，字段未变化时不触发该域的 Zustand set
- chat 域的字段包含：`chat_sessions`, `current_chat_session`, `current_chat_history`, `agent_run`, `agent_events`, `current_selection`, `llm_configured`
- workspace 域的字段包含：`workspace_current`
- transport 域的字段包含：`transport_status`
- 提供 selector hooks：`useChatSnapshot()`, `useWorkspaceName()`, `useAgentRun()`, `useTransportStatus()` 等
- `applySnapshot` 的单元测试覆盖：完全相同数据不触发更新、单个域变化只更新该域、null/undefined 边界
- typecheck 通过

---

## Phase 2: 接入 WorkbenchLayout

**目标**: WorkbenchLayout 改用 protocol store，移除 `useState<Snapshot>`。

**前序目标保护**: Phase 1 的 store 接口和类型不被破坏。

**验收标准**:
- `onSnapshotDirty` 调用 `applySnapshot(client.snapshot())` 而不是 `setSnapshot`
- WorkbenchLayout 中原来读 `snapshot.xxx` 的地方改用 store selector
- `useState<Snapshot>` 和 Snapshot 类型定义从 WorkbenchLayout 中移除
- LeftPanel 不再接收 `snapshot` prop
- typecheck 通过

---

## Phase 3: ChatZone 改用 store selector

**目标**: ChatZone 从 store 订阅 chat 相关状态，而不是接收 `snapshot` prop。

**前序目标保护**: Phase 1 store 接口稳定、Phase 2 的 WorkbenchLayout 改动不回归。

**验收标准**:
- ChatZone 的 `snapshot` prop 移除
- ChatZone 通过 `useChatSnapshot()` 或更细粒度的 selector 获取 chat 状态
- ChatSnapshot 类型保留（作为 store chat 域的类型）
- 现有单元测试（chat-runtime.test.ts）通过
- typecheck 通过

---

## Phase 4: 验证与清理

**目标**: 端到端验证 IME 修复效果，清理遗留调试文件。

**前序目标保护**: Phase 1-3 的所有变更不回归。

**验收标准**:
- typecheck 通过
- 单元测试全部通过
- `tests/playwright/_debug-input.spec.ts` 已删除（调试用临时文件）
- ui-store.ts 的注释更新（protocol 业务状态现在在 protocol-store 中）
