# Phase 0 契约 · wasm 桥接 API

本文件固定 `crates/studio-web-wasm` 对 TypeScript 侧暴露的 API 形态、错误模型、超时与节流策略、reconnect 语义、trait 适配方式，以及硬约束。后续 Phase 实现时必须原文对齐，如需变动先改本文件。

## 0. 背景约束摘要

- `studio-common::AppServerTransportPort` 现状（见 `crates/studio-common/src/app_server_client.rs`）：**所有方法都是同步签名**（`Result<(), AppServerTransportError>` 或 `Result<Option<Event>, _>`），不返回 `Future`。
- `studio-common::AppServerClient<T>` 现状：封装 `begin_handshake` / `reconnect` / `send_command` / `subscribe` / `unsubscribe` / `cancel` / `poll`，持有 `next_request_id`、`handshake`、`current_workspace`，不含 pending request registry、watch registry、timeout、重试。
- 结论：trait 本身无需改造成异步；但 timeout / reconnect 重发 / watch 重订阅 / 请求完成事件这些语义目前**不在 `studio-common` 中**，需要 Phase 2 在 `studio-common` 中补齐一层 `ManagedClient`——仍以同步 tick + poll 形式驱动，不引入 JS Promise。`ManagedClient` 是本计划固定名称，Phase 2 实现时禁止改名。

## 1. 命令派发函数（wasm → studio-common → outbound 队列）

每个命令对应一个 wasm export，TypeScript 禁止自行构造 `ClientRequestEnvelope`。参数/返回类型在 wasm 端统一定义并通过 `wasm_bindgen` 导出。

```
client_dispatch_workspace_current(handle: ClientHandle) -> Result<RequestId, ClientError>
client_dispatch_workspace_list(handle: ClientHandle, params: WorkspaceListParams) -> Result<RequestId, ClientError>
client_dispatch_preview_request(handle: ClientHandle, params: PreviewParams) -> Result<RequestId, ClientError>
client_dispatch_file_read(handle: ClientHandle, params: FileReadParams) -> Result<RequestId, ClientError>
client_dispatch_file_write_text(handle: ClientHandle, params: FileWriteParams) -> Result<RequestId, ClientError>
client_dispatch_config_load(handle: ClientHandle) -> Result<RequestId, ClientError>
client_dispatch_config_save(handle: ClientHandle, params: ConfigSaveParams) -> Result<RequestId, ClientError>
client_dispatch_slicer_list(handle: ClientHandle, params: SlicerListParams) -> Result<RequestId, ClientError>
client_dispatch_export_run(handle: ClientHandle, params: ExportParams) -> Result<RequestId, ClientError>
client_subscribe_directory_watch(handle: ClientHandle, params: WatchParams) -> Result<RequestId, ClientError>
```

设计约束：
- `RequestId` = `app_server_protocol::RequestId`（`pub struct RequestId(pub u64)` newtype，见 `crates/app-server-protocol/src/protocol.rs`）。通过 `#[wasm_bindgen]` 以 `bigint` 形式过桥到 JS，禁止在 JS 侧隐式截成 `number`。
- 所有派发函数**同步返回** `Result<RequestId, ClientError>`；不返回 `Promise`/`Future`。同步错误仅用于**本地不可入队**情况（`InvalidHandle` / `NotReady`），发生此类错误时不产生事件。能入队的请求之后的失败（超时、协议错误、取消、传输断开）一律通过 `client_drain_events` 的 `RequestFailed` / `RequestTimedOut` 事件回传。
- 派发实现内部调用 `studio-common::ManagedClient`（Phase 2 新增）对应方法，进而调用底层 `AppServerClient::send_command` / `subscribe`，并把产生的 envelope 序列化后 push 进 wasm-local 的 outbound 队列。
- 入参类型由 wasm 端集中在 `wasm_bridge/params.rs` 中定义，`#[wasm_bindgen]` 导出；不得让 React 侧自建 snake_case JSON 字段。

### 1.1 Params 类型与 `app-server-protocol` 的映射

Phase 2 实现时，下列 params 直接**复用** `app-server-protocol` 中同名 request 类型（`#[wasm_bindgen]` re-export 或 `serde-wasm-bindgen` 薄包装）；禁止在 wasm 侧复制字段定义。

| wasm bridge 类型 | 来源 |
|------------------|------|
| `WorkspaceListParams` | `app_server_protocol::WorkspaceListRequest` |
| `PreviewParams` | `app_server_protocol::PreviewRequest` |
| `FileReadParams` | `app_server_protocol::FileReadRequest` |
| `FileWriteParams` | `app_server_protocol::FileWriteTextRequest` |
| `ConfigSaveParams` | `app_server_protocol::ConfigSaveRequest` |
| `SlicerListParams` | `app_server_protocol::SlicerListRequest` |
| `ExportParams` | `app_server_protocol::ExportRunRequest` |
| `WatchParams` | wasm 特有，等同于 `{ request: app_server_protocol::WatchSubscribeRequest, throttle_ms: Option<u32> }` |
| `HandshakeParams` | `app_server_protocol::CapabilityHandshakeRequest` |

字段语义以 `app-server-protocol` 为唯一来源；若需变更，改 `app-server-protocol` 后同步更新本文件。

## 2. Transport 接缝 API（JS WebSocket ↔ wasm 状态机）

```
client_create() -> ClientHandle
client_begin_handshake(handle: ClientHandle, params: HandshakeParams) -> Result<(), ClientError>
client_next_outbound(handle: ClientHandle) -> Option<EnvelopeBytes>
client_receive_inbound(handle: ClientHandle, bytes: EnvelopeBytes) -> Result<(), ClientError>
client_mark_transport_closed(handle: ClientHandle, reason: TransportCloseReason)
client_cancel(handle: ClientHandle, request_id: RequestId) -> Result<RequestId, ClientError>
client_tick(handle: ClientHandle, now_ms: u64)
client_destroy(handle: ClientHandle)
```

说明：
- `client_create()` 仅分配 `ClientHandle` 与空 `ManagedClient` 状态；**不入队**任何 envelope。首次握手由 JS 在 WebSocket `onopen` 后显式调用 `client_begin_handshake(handle, params)` 触发；调用后 `client_next_outbound` 将输出第一条 handshake envelope。
- `HandshakeParams` = `app_server_protocol::CapabilityHandshakeRequest`；JS 侧通过 `@scad-studio/studio-web-wasm` 暴露的构造函数生成该结构，不允许 TS 侧手拼 JSON。
- `EnvelopeBytes`：二进制帧（`Uint8Array`）；序列化格式复用 `app-server-protocol` 定义的 JSON envelope（与 `websocket-host` 端到端一致）。
- `client_next_outbound` 返回 `None` 代表队列暂空；JS 侧在 WebSocket `onopen`、每次成功 send 之后，以及每次 `client_tick` 之后，循环 drain 到空。
- `client_receive_inbound` 内部调用 wasm transport 适配器的 inbound 缓冲，然后由 `ManagedClient::poll` 消费 → 产出事件进入事件环。
- `client_mark_transport_closed` 触发 reconnect 状态机：保留 pending 请求与 watch 注册表（**注册表位于 `ManagedClient`，不在 wasm transport 适配器内**），等待 JS 侧重新连上后先调用 `client_begin_handshake` 再循环 `client_next_outbound` 取走重发 envelope。
- `TransportCloseReason` 包含 `{ code: u16, reason: string, was_clean: bool }`。
- `client_cancel(handle, request_id)` 发起取消：将 target request 在 ManagedClient pending registry 中标记为 `Cancelled`，同时把 `CancelRequest` envelope 入队；返回的 `RequestId` 为 cancel 命令自身的 id。
- `client_destroy(handle)` 释放 `ManagedClient` 与所有 wasm 内部队列；调用后该 handle 上任何 `client_*` 调用返回 `ClientError::InvalidHandle`。`client_destroy` 可重入调用（第 2 次是 no-op，也不 panic）。销毁后不自动销毁 `MeshHandle` / `RendererHandle`（由调用方各自 destroy）。

## 3. 状态读取 API

```
client_drain_events(handle) -> Vec<ClientEvent>
client_snapshot(handle) -> ClientSnapshot
```

### 3.1 `ClientEvent` 枚举（最小集合）

| 变体 | 载荷 | 语义 |
|------|------|------|
| `HandshakeAccepted` | `{ session_token, server_capabilities, negotiated_version }` | 握手完成，可用 |
| `RequestSucceeded` | `{ request_id, payload: CommandSuccess }` | 请求完成 |
| `RequestFailed` | `{ request_id, error: ClientError }` | 请求失败（含 Cancelled / TransportClosed / ProtocolError 等） |
| `RequestTimedOut` | `{ request_id }` | 请求超时（由 `client_tick` 触发） |
| `WatchEvent` | `{ request_id, payload: WatchEventPayload }` | watch 推送（schema 见 §7） |
| `WatchResubscribed` | `{ request_id }` | reconnect 后自动重订阅成功 |
| `TransportOpen` | `{}` | reconnect 握手完成 |
| `TransportClosed` | `{ reason: TransportCloseReason }` | 连接丢失（供 UI 显示重连状态） |

`client_drain_events` 语义：**返回自上次调用以来累积的事件，并清空内部事件缓冲**；事件顺序与产生顺序一致。

### 3.2 `ClientSnapshot` 字段（最小集合）

React 侧**只从 snapshot 读业务状态**，不自行累积。

| 字段 | 类型 | 来源 |
|------|------|------|
| `workspace_current` | `Option<WorkspaceCurrentResponse>` | 由最新成功的 `WorkspaceCurrent` 响应填充 |
| `workspace_list` | `Option<WorkspaceListResponse>` | 由最新成功的 `WorkspaceList` 响应填充 |
| `current_directory_entries` | `Vec<DirectoryEntry>` | 派生自最新 workspace + watch 更新 |
| `preview_tasks` | `Vec<PreviewTaskState>` | 预览任务状态机 |
| `active_preview_target` | `Option<PreviewTargetId>` | 当前激活的预览目标 |
| `preview_error` | `Option<PreviewError>` | 最近一次预览错误 |
| `watch_lifecycle` | `WatchLifecycleSummary` | watch 订阅当前状态（订阅数 / 最近事件时间戳 / 重订阅计数） |
| `last_error` | `Option<ClientError>` | 便于 UI 显示最近一次错误 |
| `transport_status` | `TransportStatus` | `{ Connecting / Open / Reconnecting / Closed }` |

> 约束：snapshot **不含** UI 壳状态（route / 面板开关 / 输入草稿 / canvas ref）；这些归 React Zustand。

## 4. 渲染 API（mesh / renderer）

```
mesh_decode(bytes: Uint8Array) -> MeshHandle
mesh_destroy(handle: MeshHandle)

renderer_create(canvas_id: &str) -> RendererHandle
renderer_resize(handle: RendererHandle, width: u32, height: u32, device_pixel_ratio: f32)
renderer_render(handle: RendererHandle, mesh: MeshHandle, camera: CameraState)
renderer_destroy(handle: RendererHandle)
```

设计约束：
- `renderer_*` 必须幂等：同一 handle 的 `renderer_destroy` 可多次调用（第 2 次是 no-op）；`renderer_resize` 允许在未首次 render 前调用。
- renderer 不持有 protocol 状态，不调用任何 `client_*`。
- `CameraState = { position: [f32; 3], target: [f32; 3], up: [f32; 3], fov_y_deg: f32, near: f32, far: f32 }`。

## 5. 错误模型

```rust
pub enum ClientError {
    InvalidHandle,                                  // handle 不存在 / 已 destroy
    NotReady,                                       // 握手未完成却尝试派发业务命令
    DecodeError { context: String },                // envelope 或 payload 反序列化失败
    UnknownRequest { request_id: RequestId },       // inbound 响应找不到对应请求
    TransportClosed,                                // 请求在 transport 未就绪 / 已断开时被拒
    Cancelled,                                      // 已被 client_cancel 主动取消
    ProtocolError { code: String, message: String },// server 返回的业务错误
    Timeout,                                        // tick 触发的超时
}
```

硬约束：
- wasm 侧**禁止 panic**；所有失败路径通过 `ClientError` 经事件回传或同步返回。
- `client_dispatch_*` 同步返回的 `ClientError` 仅限 `InvalidHandle` / `NotReady`（本地不可入队）。一旦入队成功，后续失败（`TransportClosed` / `Cancelled` / `Timeout` / `ProtocolError` / `DecodeError` / `UnknownRequest`）一律走 `client_drain_events` 的事件流，不再同步抛回。
- `ClientError` 的 serde 表示形式在 Phase 0 契约冻结；Phase 2 的 S1a 必须覆盖该枚举的序列化稳定性。

## 6. 超时策略

- 每个请求在入队时记录 `issued_at_ms`（由最近一次 `client_tick(now_ms)` 得到的时间戳）。
- 默认超时：
  - 交互类命令（`WorkspaceCurrent` / `WorkspaceList` / `FileRead` / `ConfigLoad` / `ConfigSave` / `SlicerList`）：15000 ms。
  - 长任务命令（`PreviewRequest` / `ExportRun`）：不设超时（由服务端推送进度 / 完成事件收敛）。
  - `FileWriteText`：15000 ms。
  - watch 订阅：**无超时**（长连接订阅）。
- 超时检测纯粹由 `client_tick(now_ms)` 推进；wasm 内部不使用 `setTimeout` / `wasm-bindgen-futures`。
- 超时命中后：
  - 从 pending registry 移除。
  - 发射 `RequestTimedOut { request_id }`。
  - 后续若 server 再返回该 request_id 的响应，按 `UnknownRequest` 忽略（只进 `last_error`，不再重发事件）。
- 超时常量写入 `studio-common` 内部常量模块，允许后续通过 `ManagedClient::with_timeouts(...)` 注入覆盖。
- JS 端不维护 per-request 超时表；只需以 ≥ 30 Hz 或 `requestAnimationFrame` 频率调用 `client_tick`。

## 7. Watch 节流策略

- 节流**只在 studio-common / wasm 侧实现**；TS 不重复节流。
- watch 订阅命令 `WatchParams` 携带：
  ```rust
  pub struct WatchParams {
      pub request: app_server_protocol::WatchSubscribeRequest,
      pub throttle_ms: Option<u32>, // None 代表使用默认值
  }
  ```
  其中 `WatchSubscribeRequest { directory: Option<PathHandle> }` 为协议原字段，不再由 wasm 覆盖定义。
- 默认 `throttle_ms = 150`。
- 节流语义：同一 watch 订阅在 `throttle_ms` 窗口内的多次底层事件合并为一次 `WatchEvent`。`WatchEventPayload` schema：
  ```rust
  pub enum WatchEventPayload {
      Changed {
          subscription_id: SubscriptionId,
          window_start_ms: u64,
          window_end_ms: u64,
          changed_paths: Vec<PathHandle>, // 去重后的全量变更路径
      },
      Error {
          subscription_id: SubscriptionId,
          message: String,
      },
  }
  ```
  - `changed_paths` 保留窗口内出现过的全部路径（去重后），不区分 created / modified / deleted——上游 `WatchChangedEvent` 仅提供路径集合，不提供 kind；若未来 `app-server-protocol` 扩展，再同步更新本 schema。
  - `WatchError` 不合并：每次收到立刻产出，不受 throttle 约束。

## 8. Reconnect 策略

- 重连由 JS 端 WebSocket 层发起（指数退避：1s, 2s, 4s, 8s, 上限 15s），wasm 不主动触发。
- JS 每次建立新连接后按顺序：
  1. 调 `client_next_outbound` 直到 `None`，把内部 outbound 队列全部发送。
  2. 继续正常 `client_receive_inbound` / `client_tick` 循环。
- wasm `ManagedClient` 在 `client_mark_transport_closed` 后：
  - 保留 pending 请求 registry（按到达顺序重新进入 outbound 队列，等下一次 `client_next_outbound` 被 drain）。
  - 保留 watch 订阅 registry，reconnect 成功后自动重发 `WatchSubscribe`；收到 ack 发射 `WatchResubscribed { request_id }`。
  - 未发送过的请求（仍在 outbound 队列）保持原顺序。
- 握手重新成功后发射 `TransportOpen` 事件；失败走 `TransportClosed`。
- JS 端不维护与重发相关的业务状态，只负责 WebSocket 生命周期与 envelope 搬运。

## 9. `AppServerTransportPort` trait 适配审查

| trait 方法 | 签名（当前） | wasm 适配方式 |
|------------|--------------|---------------|
| `handshake(req)` | 同步 | 写入 outbound 队列（序列化为 handshake envelope），由 JS drain 后 send |
| `reconnect(req)` | 同步 | 同上（作为新的握手帧） |
| `request(env)` | 同步 | 序列化 envelope，push 进 outbound 队列 |
| `subscribe(id, req)` | 同步 | 同上，并在 wasm watch registry 中登记 `id → req` 以便 reconnect 重订阅 |
| `unsubscribe(id, req)` | 同步 | 同上，并从 watch registry 移除目标 |
| `cancel(id, target)` | 同步 | 同上，并将 target 标记为 `Cancelled`（若 pending 存在） |
| `poll_server_event()` | 同步 | 从 inbound 缓冲中取出 `client_receive_inbound` 写入的解码后事件 |
| `close()` | 同步 | 标记 `TransportStatus::Closed`，outbound 不再出队直到 `client_mark_transport_closed` → reopen |

结论：**不需要** 为 `AppServerTransportPort` 新增异步方法；wasm 侧实现为纯 in-memory `VecDeque` 队列即可。

补充（Phase 2 将要在 `studio-common` 中新增）：`ManagedClient`（暂定名）作为 `AppServerClient<T>` 的 pump/registry 外壳，至少提供：

```rust
impl<T: AppServerTransportPort> ManagedClient<T> {
    pub fn begin_handshake(&mut self, params: CapabilityHandshakeRequest) -> Result<(), ClientError>;
    pub fn tick(&mut self, now_ms: u64);              // 推进超时与节流
    pub fn drain_events(&mut self) -> Vec<ClientEvent>;// 产出上面表 3.1 的事件
    pub fn snapshot(&self) -> ClientSnapshot;         // 产出表 3.2 的字段
    pub fn mark_transport_closed(&mut self, reason: TransportCloseReason);
    pub fn next_outbound(&mut self) -> Option<Vec<u8>>;
    pub fn receive_inbound(&mut self, bytes: &[u8]) -> Result<(), ClientError>;
    pub fn cancel(&mut self, target: RequestId) -> Result<RequestId, ClientError>;
    pub fn dispatch_workspace_current(&mut self) -> Result<RequestId, ClientError>;
    pub fn dispatch_workspace_list(&mut self, params: WorkspaceListRequest) -> Result<RequestId, ClientError>;
    pub fn dispatch_preview_request(&mut self, params: PreviewRequest) -> Result<RequestId, ClientError>;
    pub fn dispatch_file_read(&mut self, params: FileReadRequest) -> Result<RequestId, ClientError>;
    pub fn dispatch_file_write_text(&mut self, params: FileWriteTextRequest) -> Result<RequestId, ClientError>;
    pub fn dispatch_config_load(&mut self) -> Result<RequestId, ClientError>;
    pub fn dispatch_config_save(&mut self, params: ConfigSaveRequest) -> Result<RequestId, ClientError>;
    pub fn dispatch_slicer_list(&mut self, params: SlicerListRequest) -> Result<RequestId, ClientError>;
    pub fn dispatch_export_run(&mut self, params: ExportRunRequest) -> Result<RequestId, ClientError>;
    pub fn subscribe_directory_watch(&mut self, params: WatchParams) -> Result<RequestId, ClientError>;
}
```

`ManagedClient` 仍然同步 + pull 驱动，不引入 `async fn`、`impl Future`、`wasm-bindgen-futures`。Phase 2 在 `studio-common` 内实现 `ManagedClient`，`crates/studio-web-wasm` 仅以薄 `#[wasm_bindgen]` wrapper 暴露上述方法。

## 10. 硬约束（违反即视为 Phase 2 回归）

- wasm 内部**不持有**任何 JS Promise；不通过 `wasm_bindgen_futures` 等待 JS 异步结果。
- JS 负责 WebSocket 生命周期 / envelope 搬运 / watch 推送投递；wasm 负责 protocol client 状态机 / mesh decode / renderer。
- cancel / reconnect / watch push / 请求完成 / 超时**全部**通过上述固定 API 表达；禁止新增隐式状态出口（例如额外 `#[wasm_bindgen]` getter 读 pending registry）。
- TS 侧不得实现 `AppServerClient` / `WorkspaceSession` / `PreviewState` / `DirectoryWatchLifecycle` / request id registry 的等价**业务状态机**。允许的唯一例外：TS 适配层为了把 `ClientEvent::RequestSucceeded { request_id }` / `RequestFailed` / `RequestTimedOut` 派发回发起方调用点，可持有一张 `Map<RequestId, resolver>` 形式的 Promise/callback 表；该表不保存任何业务状态（命令载荷、workspace 数据、preview 状态等），仅保存 resolver 闭包，且必须在收到对应事件后立即 `delete`。
- 所有新 export 函数必须在本文件出现；未在本文件列出的 wasm 导出视为桥接违规。
