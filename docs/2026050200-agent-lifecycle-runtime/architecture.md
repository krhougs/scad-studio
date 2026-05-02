# Agent 生命周期与 WebSocket 观察架构设计

## 背景

当前 Agent run 由 `HostRequestDispatcher` 启动，`HostRequestDispatcher` 又由每个 WebSocket connection 创建。Agent worker 持有当前 connection 的 `push_sink`，实时 token、reasoning、tool event 会直接推给该连接。这个结构能让一次连接内的 Agent run 工作，但无法满足刷新页面、多个 WebSocket 同时观察同一个 Agent、以及前端断开后重新接回实时状态的产品语义。

本设计将 Agent 生命周期从 WebSocket 生命周期中分离。WebSocket 只作为观察者和命令通道，Agent 生命周期由 workspace 级运行时管理。所有外部消费者通过 `agent_id` 查询和操作 Agent；`run_id` 或 `turn_id` 仅作为内部 turn 追踪字段，不作为外部操作目标。

## 当前行为核对

- 新建 chat 尚无消息时，前端可以通过后端返回的 `agent_model_registry` 选择模型。
- 当前 Chat session id 由 title 派生，并通过 `chats/<session_id>.jsonl` 文件名反推出 id 和 title；该身份模型不满足产品语义。
- 前端点击 New Chat 只应创建本地草稿窗口，不应在后端创建 chat、`chat_id` 或 `agent_id`。
- 当前模型选择是 WebSocket dispatcher 级运行时状态，不是 chat 级绑定状态。
- 发送消息时，前端会把当前模型快照随 `agent.invoke` 发送给后端；后端直接使用请求中的 provider/model/reasoning/service 参数。
- Chat history 当前没有持久化 chat 与模型的绑定关系。
- Reasoning 参数当前应保持 `Option<String>` 语义：`None` 表示不发送 reasoning 字段，`Some(String)` 表示把字符串原样发送给 LLM；不得引入嵌套 Option，也不得在后端生成默认 reasoning 字符串。
- WebSocket 断开不会主动 cancel 已启动的 worker，但旧 worker 仍持有旧连接的 `push_sink`，新连接无法接管旧 worker 的实时事件。
- 新连接会创建新的 `HostRequestDispatcher` 和新的 `AgentRunRegistry`，因此无法知道旧连接中仍在运行的 Agent run。

## 目标

- Agent 生命周期完全独立于 WebSocket 连接生命周期。
- WebSocket、用户、未来其他消费者只通过消息与 Agent runtime 共享状态。
- 多个 WebSocket connection 可以同时观察并操作同一个 Agent。
- Chat id 由后端随机生成，不能来自 title 或文件名。
- 产品语义中 chat 等同于 agent：每个 chat 拥有同一个稳定 `agent_id`，外部 Agent 操作以该 `agent_id` 为目标。
- Workspace 根目录 `chats.json` 是 chat 列表、显示顺序、当前 chat 和 chat metadata 的权威状态。
- 首次发送消息才创建后端 chat；创建参数必须包含首条用户消息和当前模型参数快照。
- 外部操作目标始终是稳定的 `agent_id`，不是 `run_id`。
- 一个 chat 首次发消息时绑定模型；绑定后同 chat 的后续消息必须使用同一模型，前端传入的模型变化不影响该 chat。
- 前端刷新或切换 chat 不影响 active Agent 的工作；重新连接后可以恢复当前 Agent 状态、历史事件和后续实时事件。
- 进程意外退出后，重启时必须把未完成 Agent turn 恢复为可解释的 interrupted 状态，不能让 UI 或 runtime 卡在 running。
- LLM idle 且无 WebSocket observer 时，drop Agent 运行对象，释放 LLM client、stream、tool executor、subscriber 等运行资源，只保留可恢复的持久状态。
- Provider 产品配置支持 `anthropic`、`openai_responses`、`openai_completions` 三类 provider type，并支持各类型的 `base_url` 规则。
- 不引入临时兼容路径、前端假状态或只覆盖演示用例的实现。

## 非目标

- 本计划不迁移根目录 `llm.toml`；该文件属于本地开发环境相关配置，不进入产品配置整改范围。
- 本计划不引入多 Agent 同时写 workspace 的并发写入语义。
- 本计划不更改 CadQuery staging、path policy、workspace tool contract、preview 或 file watch 的产品语义。
- 本计划不把 WebSocket 会话 token 作为 Agent 身份来源。

## 文件状态方案取舍

本设计比较三类基于文件系统的状态方案：

- SQLite：事务、WAL、锁和崩溃恢复成熟，官方文档明确说明 rollback journal / WAL 的 atomic commit 与隔离模型；但它会把 `chats.json` 产品约束转换为数据库表语义，并引入同步数据库调用边界。
- redb：纯 Rust 嵌入式 KV，支持 ACID transaction、单写多读和崩溃自动恢复，适合把 chat index、event log、turn state 放入一个数据库文件；但这会弱化 `chats.json` 作为 workspace 可检查状态文件的产品形态。
- 显式文件状态：保留 `chats.json`、Chat JSONL 和 Agent event JSONL。该方案最贴合当前产品约束，但必须显式定义单写入者、原子写入、event log 游标、重启恢复和损坏处理。

本计划采用第三种方案：由 app server runtime 作为 workspace 状态唯一写入者，`chats.json` 使用临时文件加 rename 或等价原子写入，Agent event log 使用 append-only JSONL。若未来需要跨进程同时写 workspace，再评估引入 SQLite 或 redb。

参考资料：

- SQLite atomic commit: https://www.sqlite.org/atomiccommit.html
- SQLite WAL: https://www.sqlite.org/wal.html
- SQLite isolation: https://sqlite.org/isolation.html
- redb database docs: https://docs.rs/redb/latest/redb/struct.Database.html
- redb project: https://github.com/cberner/redb
- fs4 file lock docs: https://docs.rs/crate/fs4/1.0.1

## 核心模型

```rust
pub struct AgentId(pub String);

pub struct ChatIndex {
    pub version: u32,
    pub active_chat_id: Option<ChatSessionId>,
    pub chats: Vec<ChatIndexEntry>,
}

pub struct ChatIndexEntry {
    pub chat_id: ChatSessionId,
    pub agent_id: AgentId,
    pub title: String,
    pub goal: Option<String>,
    pub summary: Option<String>,
    pub open_questions: Vec<String>,
    pub messages_path: PathHandle,
    pub events_path: PathHandle,
    pub archived: bool,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub related_files: Vec<PathHandle>,
    pub bound_model: Option<BoundAgentModel>,
}

pub struct AgentRuntimeInstance {
    pub agent_id: AgentId,
    pub chat_session_id: ChatSessionId,
    pub model: BoundAgentModel,
    pub state: StoredAgentState,
    pub active_turn: Option<ActiveAgentTurn>,
}

pub struct ActiveAgentTurn {
    pub turn_id: AgentTurnId,
    pub cancel: CancellationToken,
    pub task: JoinHandle<()>,
}

pub struct BoundAgentModel {
    pub provider_id: String,
    pub provider_type: AgentProviderType,
    pub model_id: String,
    pub reasoning_effort: Option<String>,
    pub service_label: Option<String>,
}
```

`AgentRuntimeInstance` 是后端 workspace 级运行对象，只在 Agent active 或仍有 subscriber 需要观察运行态时存在。`ActiveAgentTurn` 只在 LLM 正在输出、tool call 正在执行、或取消正在传播时存在。Agent idle 且没有 subscriber 后，runtime 必须 drop `AgentRuntimeInstance`，只通过 `chats.json`、event log 和 chat history 保留可恢复状态。

## Chat identity 与 chats.json

Chat id 必须由后端随机生成，作为 opaque id 使用。Title、显示顺序、当前 chat、归档状态、关联文件、更新时间和绑定模型都属于 metadata，不得参与 id 生成。

Workspace 根目录的 `chats.json` 是 chat 状态权威来源：

- `chats.json` 记录所有 chat 的显示顺序。
- 前端点击 New Chat 只创建本地草稿；草稿未发送消息前不写后端状态，刷新后无需恢复。
- 首次发送时，后端生成随机 `chat_id` 和对应 `agent_id`，把首条用户消息、当前模型参数快照和 chat metadata 作为创建参数写入 `chats.json`，再创建消息 JSONL 和 Agent event log。
- 切换 chat 时，后端更新 `chats.json.active_chat_id`；这是 workspace 级共享状态，所有已连接观察者都应收到当前 chat 变化事件或在 snapshot 中看到最新值。
- Chat 列表从 `chats.json` 读取，不能通过扫描 JSONL 文件名反推出 id 或 title。
- 消息 JSONL 路径只是内部存储路径，由 `chats.json` 的 `messages_path` 指向。
- Agent event log 路径由 `chats.json` 的 `events_path` 指向。
- Chat 与 Agent 是同一产品实体的两面：Chat 承载对话和 metadata，Agent 承载运行时与事件；二者通过 `chat_id` 和 `agent_id` 在 `chats.json` 中稳定关联。
- Chat 归档、重命名、排序调整不得改变 `chat_id` 或 `agent_id`。
- `chats.json` 必须承接现有 chat metadata，包括 title、goal、summary、open questions、related files、archived、created / updated 时间、messages path、events path、agent id 和 bound model。
- `chats.json` 写入必须采用临时文件加 rename 或等价原子策略；旧工作区迁移必须幂等，索引损坏时返回明确错误，不退回到文件名作为长期身份来源。

## Reasoning 参数语义

Reasoning 配置保持一层 `Option<String>`：

- `None` 表示不向 LLM provider request 写入 reasoning 字段。
- `Some(value)` 表示把 `value` 原样发送给 LLM。
- 后端不得为缺省 reasoning 自动生成默认字符串。
- 数据结构不得出现 `Option<Option<String>>` 或等价的嵌套缺省状态。

该语义已经覆盖当前业务场景：不传与原样发送是唯一需要表达的两种状态。

## Provider 与 base_url

产品配置以 `agents.toml` 和示例配置为准，不迁移根目录 `llm.toml`。

Provider type 固定支持三类产品语义：

- `anthropic`：Anthropic Messages API。
- `openai_responses`：OpenAI Responses API。
- `openai_completions`：OpenAI Chat Completions API 兼容形态。

`base_url` 解析规则由产品配置层统一处理，解析后的地址必须同时用于模型列表发现和 Agent turn 执行：

- 所有 provider type 的 `base_url` 若以 `#` 结尾，去掉末尾 `#` 后按输入地址原样使用，不做路径补全。
- Anthropic 不自动追加 `/v1`，因为 Rig Anthropic 调用路径已经包含 `/v1/messages` 和 `/v1/models`。
- OpenAI family provider 若未配置 `base_url`，使用 Rig 默认地址。
- OpenAI family provider 若配置的 `base_url` 不以 `/` 结尾且不以 `#` 强制原样结尾，则配置层补全为 `<base_url>/v1`。
- OpenAI family provider 若配置的 `base_url` 以 `/` 结尾，则保留该地址的路径语义，不追加 `/v1`。

已核对 Rig 行为：Rig 的 `Provider::build_uri` 只处理斜杠拼接，不会为 OpenAI-compatible provider 自动追加 `/v1`。因此 `/v1` 补全必须在 budn' 的配置解析层完成。

`BoundAgentModel` 不持久化 `base_url`。Chat 只绑定 provider / model / reasoning / service 等非敏感模型语义；每次执行 Agent turn 时，后端根据 `agents.toml` 中当前 provider 配置解析 `base_url`。如果配置中的 provider kind 与 chat 中保存的 `provider_type` 不一致，后端应返回明确的模型绑定错误。

## 运行时边界

```rust
pub struct WorkspaceAgentRuntime {
    active_agents: HashMap<AgentId, AgentRuntimeInstance>,
    chat_index: ChatIndexStore,
    subscribers: AgentSubscriberRegistry,
    event_log: AgentEventLog,
}
```

`WorkspaceAgentRuntime` 是唯一管理 Agent 生命周期的模块。WebSocket dispatcher 不再持有 `AgentRunRegistry`，也不直接创建 worker；它只向 runtime 发送命令，并订阅 runtime 输出的事件。

## 消息接口

```rust
pub enum AgentCommand {
    CreateChatAndStartTurn {
        title: String,
        prompt: String,
        requested_model: Option<BoundAgentModel>,
        related_files: Vec<PathHandle>,
        mode: AgentMode,
        context_refs: Vec<String>,
        plan_ref: Option<PathHandle>,
    },
    StartTurn {
        agent_id: AgentId,
        prompt: String,
        mode: AgentMode,
        context_refs: Vec<String>,
        plan_ref: Option<PathHandle>,
    },
    Cancel {
        agent_id: AgentId,
    },
    Snapshot {
        agent_id: AgentId,
        since_event_id: Option<AgentEventId>,
    },
    Subscribe {
        agent_id: AgentId,
        subscriber_id: AgentSubscriberId,
    },
}
```

所有外部命令都以 `agent_id` 为目标。`turn_id` 只存在于事件中，用于前端排序、去重和调试。

首次发送使用 `CreateChatAndStartTurn`，该命令是唯一会创建后端 chat 的入口。后端在同一个流程里生成 `chat_id` 和 `agent_id`、写入 `chats.json`、写入首条 user message、创建初始 event log，并启动第一个 Agent turn。已存在 chat 的后续发送使用 `StartTurn`，并且只读取 `chats.json.bound_model`。

## Chat 与模型绑定

- Chat 首次发送消息前没有绑定模型，前端可以修改当前候选模型。
- 首次 `CreateChatAndStartTurn` 时，后端把候选模型写入 `chats.json` 对应 chat metadata 的 `bound_model`。
- `bound_model` 必须持久化，保证刷新页面或重启 host 后仍可恢复。
- 已绑定 chat 的后续请求忽略前端传入的 provider/model/reasoning/service 参数，统一使用绑定模型。
- 前端切换到已绑定 chat 后，模型控件显示绑定模型并进入只读状态。

## Agent event log 与 Chat JSONL

文件状态职责固定如下：

- `agents.toml`：provider 与模型配置来源，包含 `base_url` 解析输入；不保存 chat 状态。
- `chats.json`：chat / agent metadata、显示顺序、当前 chat、消息路径、事件路径、绑定模型。
- `chats/<chat_id>.jsonl`：最终对话事实，包括 user message、最终 assistant message、tool call / tool result、search sources、mesh result 等。
- `agent-events/<agent_id>.jsonl`：runtime event log，用于 active turn 的 snapshot / replay / 多 WebSocket 观察。

Agent event log 使用统一事件 envelope，所有事件必须包含 `event_id`、`agent_id`、`turn_id`、`ts_ms` 和 payload。`event_id` 在单个 `agent_id` 范围内单调递增，`Snapshot.since_event_id` 使用该游标恢复断线期间事件。

Event log 只服务 runtime replay 和状态展示；Chat JSONL 只保存最终对话事实。每个 turn 的最终 assistant / tool 记录只能写入 Chat JSONL 一次，并用 `agent_id + turn_id` 关联。Turn terminal 后，只有在 Chat JSONL 对应最终记录写入成功后，event log 才能进入可压缩状态；active turn 的 event log 不得提前删除。

## 多 WebSocket 观察

每个 WebSocket connection 注册为 runtime subscriber。Agent 产生事件后先写入 runtime event log，再广播给当前所有 subscriber。多个 WebSocket connection 可以对同一个 `agent_id` 发起 snapshot、cancel 和 start turn 等命令；runtime 必须以 `agent_id` 和 `chats.json` 中的 chat metadata 为准处理命令，不能让不同 connection 的本地模型状态覆盖 Agent 状态。

```rust
pub enum AgentEvent {
    StateChanged {
        event_id: AgentEventId,
        agent_id: AgentId,
        turn_id: Option<AgentTurnId>,
        ts_ms: u64,
        state: AgentState,
    },
    Token {
        event_id: AgentEventId,
        agent_id: AgentId,
        turn_id: AgentTurnId,
        ts_ms: u64,
        text: String,
    },
    Reasoning {
        event_id: AgentEventId,
        agent_id: AgentId,
        turn_id: AgentTurnId,
        ts_ms: u64,
        text: String,
    },
    ToolStarted {
        event_id: AgentEventId,
        agent_id: AgentId,
        turn_id: AgentTurnId,
        ts_ms: u64,
        tool_call_id: String,
        tool_name: String,
        args_json: String,
    },
    ToolFinished {
        event_id: AgentEventId,
        agent_id: AgentId,
        turn_id: AgentTurnId,
        ts_ms: u64,
        tool_call_id: String,
        result_json: String,
    },
    Done {
        event_id: AgentEventId,
        agent_id: AgentId,
        turn_id: AgentTurnId,
        ts_ms: u64,
    },
    Error {
        event_id: AgentEventId,
        agent_id: AgentId,
        turn_id: Option<AgentTurnId>,
        ts_ms: u64,
        message: String,
    },
    Interrupted {
        event_id: AgentEventId,
        agent_id: AgentId,
        turn_id: AgentTurnId,
        ts_ms: u64,
        reason: AgentInterruptedReason,
    },
}
```

断开的 WebSocket 只移除 subscriber，不改变 Agent 状态。新 WebSocket 连接后通过 snapshot 读取当前 Agent 状态和可重放事件，然后订阅后续事件。

## 状态机设计

Chat 创建状态机：

```text
DraftChat（前端本地草稿）
  New Chat 点击
    -> DraftChat
  首次发送(prompt + model snapshot)
    -> CreateChatAndStartTurn

ChatRecord（后端持久状态）
  Absent
    CreateChatAndStartTurn -> Creating
  Creating
    chats.json + Chat JSONL + event log 写入成功 -> Ready
    任一写入失败 -> Absent，并清理本次创建的孤儿文件
  Ready
    首次 turn 创建并写入 bound_model -> Bound
    switch -> Ready，并广播 workspace 当前 chat 变化
    archive -> Archived
  Bound
    start turn -> ActiveTurn
    switch -> Bound，并广播 workspace 当前 chat 变化
    archive -> Archived
  ActiveTurn
    turn terminal + final history persisted -> Bound
    process restart recovery -> Interrupted
  Interrupted
    start new turn -> ActiveTurn
    archive -> Archived
  Archived
    unarchive -> Bound 或 Ready
```

Agent runtime 状态机：

```text
Dropped
  subscriber joins -> ObservedIdle
  start turn -> Running

ObservedIdle
  no subscribers -> Dropped
  start turn -> Running

Running
  websocket disconnect -> Running
  cancel -> Cancelling
  LLM/tool terminal -> PersistingTranscript
  process exits -> InterruptedOnRestart

Cancelling
  cancel acknowledged -> PersistingTranscript
  process exits -> InterruptedOnRestart

PersistingTranscript
  Chat JSONL final write succeeds -> ObservedIdle 或 Dropped
  final write fails -> FailedNeedsRecovery

FailedNeedsRecovery
  retry persistence -> PersistingTranscript

InterruptedOnRestart
  append interrupted event -> ObservedIdle 或 Dropped
```

Turn 状态机：

```text
Created
  -> Streaming

Streaming
  tool call -> ToolExecuting
  model done -> PersistingFinal
  cancel -> Cancelling
  error -> Failed
  process exits -> Interrupted

ToolExecuting
  tool result -> Streaming
  tool error -> Failed
  process exits -> Interrupted

PersistingFinal
  write Chat JSONL once -> Succeeded
  write fails -> FailedNeedsRecovery

Cancelling
  terminal event + final state persisted -> Cancelled
  process exits -> Interrupted

Failed
  error event + final state persisted -> Failed

Interrupted
  start next turn -> Created
```

## 进程意外退出恢复

进程意外退出后，不尝试恢复正在进行的 LLM stream 或 tool call。Provider stream、tool executor、cancel token 和进程内 task 已经丢失，强行恢复会引入重复 tool call、重复写文件或半截 assistant 内容进入上下文的风险。

重启恢复流程：

1. 启动 `WorkspaceAgentRuntime`。
2. 读取 `chats.json`。
3. 对每个 chat 对应的 `events_path` 读取最后一个 turn 状态。
4. 若 turn 没有 terminal event，则 append `AgentEvent::Interrupted { reason: HostRestarted }`。
5. 不创建 LLM client，不重新执行 tool call，不写正常完成的 assistant message。
6. Snapshot 返回 interrupted 状态，前端显示上一轮因后端重启中断。
7. 用户可以基于保留的 Chat JSONL 和 workspace 文件状态发起新的 turn。

Chat JSONL 恢复规则：

- 已完整写入的 user message 保留。
- 已完整写入的 tool call / tool result 保留。
- 未完成的 assistant stream 不写成最终 assistant message。
- 半截 token / reasoning 只保留在 event log 中用于说明中断前状态，不进入下次 LLM history。
- 未 terminal 的 tool call 不重新执行；新 turn 必须由用户再次触发。

## 资源释放

- Active turn 期间持有 LLM client、provider stream、tool executor、cancel token、task handle。
- WebSocket 断开不取消 active turn。
- Agent idle 且 subscriber 数为 0 时，drop Agent 运行对象，释放所有运行资源，仅保留 `chats.json`、event log、chat history 等持久状态。
- Agent done / failed / cancelled 后立即释放 active turn。
- 后续连接从持久状态恢复 UI，不重新创建 LLM stream。
- 进程重启恢复出的 interrupted turn 不创建 active turn，也不重建 LLM stream。

## 并发策略

本计划保持当前产品语义的保守版本：workspace 内同一时间只允许一个 active Agent turn。这个约束由 `WorkspaceAgentRuntime` 强制，不能只依赖前端禁用按钮。

原因：

- 当前 workspace tool、CadQuery staging、文件写入与 chat history 都会修改共享 workspace。
- 在没有 workspace 写入调度和冲突处理模型前，允许多个 Agent turn 同时写 workspace 会引入真实数据冲突。
- 多个 WebSocket 同时观察和操作同一个 Agent 是本计划目标；多个 Agent 同时执行不是本计划目标。

## Async / 线程现状

当前 Agent / WebSocket 主链路未发现手写系统线程、`spawn_blocking` 或 `block_in_place`：

- WebSocket accept、connection、Agent worker 使用 `tokio::spawn`。
- CadQuery runner、OpenSCAD、export、slicer、CadQuery env verify 使用 `tokio::process::Command`。
- 测试代码中存在 `Runtime::new().block_on(...)` 和 `thread::spawn`，不属于生产路径。

已确认一个非 Agent / WebSocket 主链路问题：`crates/scad-scene/src/system_fonts.rs` 使用同步 `std::process::Command` 调用 `fc-match`。该问题已记录到 `docs/known_issues.md`。

## 验收口径

- 两个 WebSocket connection 订阅同一个 `agent_id` 时，都能收到同一组 active Agent 事件。
- 一个 WebSocket connection 启动 Agent turn 后，另一个 WebSocket connection 可以用同一个 `agent_id` snapshot / cancel / start turn，并得到同一个后端状态结果。
- 一个 WebSocket 断开后，active Agent 继续运行；新 WebSocket 连接后可通过 snapshot 恢复当前状态并继续接收后续事件。
- 进程意外退出后重启，未 terminal 的 turn 会变为 interrupted；snapshot 显示 interrupted 状态，runtime 不持有 active LLM client 或 tool executor。
- 同一个 chat 首次发送后绑定模型；后续请求即使携带不同模型，后端仍使用绑定模型。
- Reasoning 参数保持一层 `Option<String>`：`None` 不发送字段，`Some(String)` 原样发送，不生成默认 reasoning 字符串。
- 外部 cancel / snapshot / subscribe 命令只接受 `agent_id`，不接受 `run_id` 作为目标。
- Agent idle 且无 subscriber 时，不再持有 Agent 运行对象、active LLM client、provider stream、tool executor 或 WebSocket push handle。
- Protocol 类型保持 transport-neutral；`studio-common` 不依赖 transport、浏览器 API 或平台事件循环；Web 只负责展示和连接接线。
- 现有 Chat history、Agent event、CadQuery staging、workspace tool policy、preview、watch 和 Web 工作台行为保持可用。
