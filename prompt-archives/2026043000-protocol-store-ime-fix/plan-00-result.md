# Plan 00 Result: Protocol Store 重构

## 执行结果

全部 4 个 Phase 完成，Plan 级 review 通过。

### Phase 1: 创建 protocol-store 与 applySnapshot ✓

- 新建 `src/state/protocol-store.ts`，按 workspace / chat / transport 三域拆分
- `applySnapshot(raw)` 对每个域做结构比较，仅在字段真正变化时触发 Zustand set
- 导出 selector hooks：`useChatSnapshot`, `useWorkspaceName`, `useAgentRun`, `useChatSessions`, `useCurrentChatSession`, `useTransportStatus`
- 32 个单元测试覆盖 applySnapshot 和比较函数

### Phase 2: 接入 WorkbenchLayout ✓

- `onSnapshotDirty` 改为调用 `applySnapshot(client.snapshot())`，通过 ref 避免 stale closure
- 移除 `useState<Snapshot>` 和 `Snapshot` 类型定义
- WorkbenchLayout 通过 store selector 获取 `rootName`, `agentRun`, `chatSessions`, `currentChatSession`
- LeftPanel 移除 `snapshot` prop

### Phase 3: ChatZone 改用 store selector ✓

- ChatZone 移除 `snapshot` prop，通过 `useChatSnapshot()` 获取 chat 状态
- `chatSnapshotSelector` 显式构造仅含 chat 字段的对象
- `useChatSnapshot()` 使用 `useShallow` 做浅比较，确保非 chat 字段变更不触发 re-render
- 22 个 ChatZone 测试适配为通过 `useProtocolStore.setState()` 注入状态

### Phase 4: 验证与清理 ✓

- 删除 `tests/playwright/_debug-input.spec.ts`（调试用临时文件）
- 更新 `ui-store.ts` 注释
- typecheck 通过（仅保留预先存在的 chat-runtime.test.ts 错误）
- 全部 223 个单元测试通过

### 补充修复：React.memo 阻断 re-render 传播

首轮修复后用户验证 CJK 输入仍卡死。诊断发现 protocol store 只解决了 snapshot 引起的 re-render，但 WorkbenchLayout 中还有其他 useState 来源（特别是 `useLogBuffer`）持续触发 re-render。

根因链完整版：
1. watch 事件 / agent 事件 → `log.append()` → `setEntries()` → WorkbenchLayout re-render
2. 级联到 LeftPanel → ChatZone → ComposerPrimitive.Input
3. React controlled input 在 re-render 的 commit 阶段执行 `element.value = value`
4. IME composition 期间 `value`（旧值）与 `element.value`（含 composing text）不同
5. React 重置 textarea value → 破坏浏览器 IME 状态机

修复：ChatZone 包裹 `React.memo`，切断父级 re-render 的传播。ChatZone 的 props（client、onStatus、onOpenPlan）全部引用稳定（useState setter / useCallback），memo 比较不会穿透。ChatZone 内部状态变更（通过 Zustand selector）仍正常触发 re-render。

## Review 过程中发现并修复的问题

1. **`agentEventsEqual` 仅比较长度（Phase 1 review）** — 增加了最后一个事件类型名的比较。在 append-only 事件模型下足够正确。
2. **`chatSnapshotSelector` 返回整个 store state（Plan 级 review，阻塞项）** — 修复为显式构造仅含 chat 字段的对象 + `useShallow` 浅比较。
3. **`useLogBuffer` 等 useState 仍触发 re-render（用户验证后）** — ChatZone 加 `React.memo` 阻断传播。

## 变更文件清单

| 文件 | 变更类型 |
|------|---------|
| `src/state/protocol-store.ts` | 新建 |
| `src/state/ui-store.ts` | 注释更新 |
| `src/workbench/workbench-layout.tsx` | 移除 useState&lt;Snapshot&gt;, 改用 store |
| `src/workbench/left-panel.tsx` | 移除 snapshot prop |
| `src/workbench/chat-zone.tsx` | 移除 snapshot prop, 改用 useChatSnapshot() |
| `tests/unit/protocol-store.test.ts` | 新建 |
| `tests/unit/chat-zone.test.tsx` | 适配到 protocol store |
| `tests/playwright/_debug-input.spec.ts` | 删除 |

## 待手动验证

CJK IME 输入是否在浏览器中正常工作——需要启动 dev server 实际测试。
