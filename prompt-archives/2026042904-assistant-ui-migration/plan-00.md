# assistant-ui 替代 Agent Chat 实施计划

## 背景

当前 Agent Chat UI 是完全手写的 React 组件，约 1000 行代码，包含自定义流式消息拼接、timeline 构建、snapshot 轮询驱动渲染等逻辑。目标是用 `@assistant-ui/react` 替代，获得更好的流式处理、自动滚动、无障碍支持，同时确保最终成品符合当前设计系统。

## 用户强制约束

- 使用 `@assistant-ui/react`（不安装 `@assistant-ui/react-ui`，那是 Tailwind 版本）
- **不新增** `@assistant-ui/react-markdown`，继续复用现有 Markdown 渲染器
- Agent events 采用**合成消息方案**——将 agent events 转换为 assistant-ui 消息模型，完全走消息渲染路径
- **先单会话**——保留现有 `<select>` 会话切换器，多会话后续单独做
- 引入新库不改变功能的样式，最终成品必须符合当前设计系统
- 安装依赖使用 `bun`，遵循项目工具链约束

## 已锁定设计决策

以下决策在计划制定阶段已确认，执行时不得重新讨论：

1. **Borsh 序列化**：仅用于内存传输，不涉及持久化。
2. **isRunning 语义**：仅控制发送按钮状态和取消操作可见性，不用于消息合并或去重判断。去重完全依赖 run_id 加历史数量基线。
3. **消息合并策略**：相邻消息不做合并。每条消息（含合成消息）保持独立，不按 role 合并连续消息。
4. **Markdown 渲染**：不引入 assistant-ui 的 Markdown 组件，继续使用现有渲染器，通过 assistant-ui 的自定义渲染插槽接入。
5. **run_id 写入规则**：只有 agent run 产生的最终 assistant 回答写入 run_id；工具调用、工具结果、用户消息、取消后的部分回答均为 None。
6. **studio-web 边界**：本次迁移仅涉及 studio-web 包内的聊天 UI 组件。共享状态机、协议扩展、WASM 类型同步均为前置依赖，不属于本迁移的 UI 层职责。
7. **运行时选择**：使用 external store 运行时模式——当前数据流是 snapshot 驱动的 30Hz 重渲染，external store 模式允许持有消息数组的所有权，assistant-ui 只做订阅和渲染。
8. **回退策略**：不存在降级备选路径。必须使用 assistant-ui 原语；新旧 UI 直接替换，不做渐进式切换。

## 核心解决思路

### 消息转换

将 snapshot 中的历史消息和 agent events 合成为 assistant-ui 消息模型。需要解决：
- **消息稳定身份**：每条消息必须有稳定 id（来自历史消息 id、run_id、合成 id），禁止运行时生成，避免 30Hz 下 diff 抖动
- **去重逻辑**：保留现有的流式 token 与历史消息去重能力，不依赖 isRunning 状态
- **增量缓存**：30Hz snapshot 更新下，按 id 复用未变化的消息引用，避免全量重建

### Agent event 渲染

Agent events（plan_saved、error、tool 调用等）不是真实工具调用，不使用工具调用类型，使用自定义数据类型渲染，每个 event 合成为一条独立消息。

### 提交流程

从现有提交函数中抽出无 React 状态副作用的核心逻辑，负责会话创建、命令 dispatch。UI 层管理 draft 清空、busy 状态、错误恢复。

### 会话隔离

按当前 session id 隔离运行时实例，会话切换时重建运行时。isRunning 只在当前 session 有活跃 agent run 时为 true。

## 前置依赖：run_id 协议扩展

当前消息记录没有 run_id，现有去重靠运行开始时的历史数量基线。需要在协议层、存储层和 WASM 类型同步中增加 run_id 字段。旧记录无 run_id 时保留现有 baseline 去重能力。Borsh 仅用于内存传输，无需持久化兼容。

此项作为前置依赖独立完成，确保不影响现有功能。

## Phase 划分

### Phase 0a — run_id 协议迁移（前置依赖）

**输入**：
- 当前消息记录的数据结构定义
- JSONL 存储层的读写路径
- WASM bindgen 的类型生成配置
- 现有去重逻辑的基线行为

**目标**：完成 run_id 全量改动，确保不影响现有功能。此 Phase 属于前置依赖交付，改动范围覆盖协议层、存储层和 WASM 类型同步，不在本次 UI 迁移的 studio-web 包内。

**操作步骤**：
- 在消息记录中增加 run_id 字段
- 存储模型同步更新
- 服务端写入路径限定为最终 assistant 回答专用
- Rust 单元测试覆盖新字段的序列化、反序列化和旧数据兼容
- WASM 和 TS 类型同步更新
- 测试 fixture 更新

**验收标准**：
- `cargo test` 全通过，无新增失败
- 旧 JSONL 文件可正常读取，run_id 字段为 None
- `wasm-pack build` 编译通过，TS 类型定义包含 run_id 字段

**前序目标保护**：无（首个 Phase）

---

### Phase 0b — API 验证与决策确认

**输入**：
- Phase 0a 产出的 run_id 协议改动
- 已锁定设计决策（运行时选择、消息合并、Markdown 渲染等共 8 项）
- `@assistant-ui/react` 的公开文档和类型定义

**目标**：通过最小原型验证已锁定设计决策在实际 API 中可行，输出验证结果文档。此 Phase 不做决策——所有决策已在「已锁定设计决策」中确认；此 Phase 只验证决策可落地。

**操作步骤**：
- 安装 `@assistant-ui/react`
- 按已锁定设计决策逐项构建最小原型，验证每项决策在实际 API 中的行为
- 验证最终成品能符合当前设计系统（样式、布局、交互）
- 输出验证结果文档，记录每个已锁定决策的验证结论

**验收标准**：
- 验证结果文档覆盖全部 8 项已锁定决策，每项标注「通过」
- 最小可运行原型编译通过（`bun run typecheck` 零错误）
- 消息不合并行为在原型中验证：连续同 role 消息渲染为独立 DOM 节点
- DOM 结构截图与现有 UI 对比，记录需要调整的具体清单
- **进入 Phase 1 的硬性前提**：全部 8 项已锁定决策均标注「通过」；任何一项未通过则 Phase 0b 未完成，不得进入 Phase 1

**前序目标保护**：Phase 0a 的 run_id 协议改动不得回退

---

### Phase 1 — 替换核心 UI

**输入**：
- Phase 0a 产出的 run_id 协议改动
- Phase 0b 产出的验证结果文档（全部 8 项已通过）和样式差异清单
- 以下功能清单（每项均为必须覆盖的旧功能）：
  1. 发送消息并接收 agent 流式回复
  2. 停止正在进行的 agent run
  3. 切换 agent 模式（plan / act 等）
  4. 首条消息自动创建会话
  5. 历史消息加载与展示
  6. 流式 token 去重（不重复显示已完成的历史消息）
  7. 发送失败后的错误恢复与重试
  8. 空输入校验（不允许发送空消息）
  9. 长消息（> 2000 字符）正常发送和渲染
  10. 连续多次运行不冲突
  11. 会话切换中发送不产生竞态
  12. 历史加载后继续运行不中断
  13. Cmd/Ctrl+Enter 发送、Enter 换行
  14. IME 输入法组合态不误提交
  15. Agent event 渲染（plan_saved 卡片、error 卡片、agent event 行）
  16. 相邻合成消息不被合并
  17. 无障碍：textarea label、按钮 aria-label、焦点顺序、错误卡片可读性

**目标**：用 assistant-ui 原语直接替换现有聊天 UI，新旧直接替换，不做渐进式切换。

**操作步骤**：
- 实现 snapshot 到 assistant-ui 消息模型的转换（含稳定 id、去重、增量缓存、消息不合并）
- 实现 snapshot 数据源与 assistant-ui 运行时的桥接
- 替换输入区域：保持现有快捷键和提交行为，IME 组合态不误提交
- 替换消息渲染：支持自定义渲染插槽接入现有 Markdown 渲染器和 agent event 卡片
- 移除旧 UI 组件，直接使用新实现

**验收标准**：
- 上述功能清单 17 项全部通过——每项有对应测试用例或手动验证记录
- run_id 去重：最终 assistant 的 run_id 覆盖 live stream——单元测试覆盖
- tool call 带/不带 run_id 都不会误判完成——单元测试覆盖
- 30Hz 下消息 id 稳定：连续 100 次 snapshot 更新，消息 id 零变化——自动化测试
- 静态历史消息不因 snapshot 更新而重渲染：通过 React Profiler hook 记录渲染次数，无变化消息渲染次数为 0——自动化测试断言
- 验证命令：`bun run typecheck` 零错误，`bun test` 全通过
- 回归测试通过：其他 session 运行中不显示当前 thread thinking、cancel 后保留已收到 token

**前序目标保护**：Phase 0a 的协议改动和 Phase 0b 的验证结论不得回退

---

### Phase 2 — 清理与集成

**输入**：
- Phase 1 产出的完整聊天功能（已直接替换旧 UI）
- Phase 0b 产出的样式差异清单
- 现有测试套件（unit tests + playwright tests）
- 当前 lint 警告基线（执行前记录 `bun run lint` 输出作为 baseline）

**目标**：清理替换过程中残留的旧代码和未使用的引用，确保完整功能可用。

**操作步骤**：
- 清理因替换而变为未使用的旧代码和引用
- 基于 Phase 0b 的样式差异清单修正样式
- 回归验证全部功能

**验收标准**：
- 以下旧功能清单 6 项全部被新路径覆盖——每项有对应验证记录：
  1. timeline 去重逻辑
  2. 自动滚动到底部
  3. 空状态展示
  4. disabled 状态（发送中禁用输入）
  5. 上下文 pill 展示
  6. operation select 展示与切换
- `bun test` 全通过，零新增失败
- `bunx playwright test` 全通过，零新增失败
- `bun run typecheck` 零错误
- `bun run lint` 警告数不超过 Phase 输入中记录的基线数量

**前序目标保护**：Phase 0b 的验证结论和 Phase 1 的完整聊天功能不得回退

---

## 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| assistant-ui 运行时强依赖 Tailwind | 高 | Phase 0b 编译级验证；若阻塞则在本 Phase 内解决后才能进入 Phase 1 |
| 30Hz snapshot 更新导致过多 re-render | 中 | 增量缓存 + 稳定 id；Phase 1 验收标准已包含 Profiler 量化指标 |
| 合成消息缺稳定身份导致 diff 抖动 | 高 | 禁止运行时生成 id，所有 id 来自 snapshot 数据 |
| run_id 旧历史回退 | 中 | 无 run_id 时回退到数量基线去重 |
| DOM 结构变化导致样式失效 | 中 | Phase 0b 记录样式差异清单，Phase 2 按清单修正 |

## 依赖图

```
Phase 0a (run_id 协议迁移) ── 前置依赖
    │
    └── Phase 0b (API 验证与决策确认) ── 验证已锁定决策，全部通过才能继续
        │
        └── Phase 1 (替换核心 UI) ── 直接替换旧 UI
            │
            └── Phase 2 (清理与集成) ── 清理残留代码
```
