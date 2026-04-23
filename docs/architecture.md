# Architecture

本文档说明仓库里各 crate / package 的能力边界以及它们之间的运行时关系。目标是让任何要改代码的人**先知道应该改哪一层**，再动手。

## 1. 顶层图

```
                   ┌──────────────────────────┐
                   │    app-server-protocol   │   类型 / 线格式 / 版本协商（无 I/O，无 transport）
                   └──────────────┬───────────┘
                                  │ （被所有层复用）
              ┌───────────────────┼───────────────────┐
              │                   │                   │
   ┌──────────▼─────┐   ┌─────────▼────────┐   ┌──────▼─────────┐
   │ app-server-core │   │ app-server-     │   │ studio-common  │
   │  （文件系统、    │   │  transport      │   │  （跨端 client │
   │   OpenSCAD、    │   │   trait + WS    │   │   状态机 +     │
   │   watch 聚合）  │   │   实现）        │   │   ManagedClient│
   └──────────┬─────┘   └─────────┬────────┘   └───────┬────────┘
              │                   │                    │
   ┌──────────▼──────────┐        │          ┌─────────▼──────────┐
   │  app-server-host    │        │          │ studio-web-wasm    │
   │  （websocket-host / │        │          │ （wasm-bindgen     │
   │   in-process host） │        │          │   wrapper）        │
   └──────────┬──────────┘        │          └─────────┬──────────┘
              │                   │                    │
              │   ┌───────────────▼──────────┐         │
              │   │ studio-app（桌面 egui）  │         │
              │   └──────────────────────────┘         │
              │                                        │
              └──────────────── ws ────────────────────┤
                                                       │
                                           ┌───────────▼───────────┐
                                           │ packages/studio-web   │
                                           │ （React PWA）         │
                                           │  - transport 层       │
                                           │  - wasm-bridge 适配   │
                                           │  - 五区工作台 UI      │
                                           └───────────────────────┘
```

## 2. Rust crate 能力边界

### 2.1 `app-server-protocol`

- 纯类型 crate：`ClientEnvelope` / `ServerEnvelope`（`#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]`）、`ClientCommand` / `CommandSuccess`、`ClientRequestEnvelope` / `ServerResponseEnvelope`、`ServerPushEnvelope` / `ServerPushEvent`、`CapabilityHandshakeRequest` / `CapabilityHandshakeResponse`、`WatchSubscribeRequest` / `WatchChangedEvent` / `WatchErrorEvent`、`PreviewRequest` / `PreviewReadyResponse`、`FileReadRequest` / `FileReadResponse` / `FileReadContents`（`utf8_text | binary`）、`ConfigLoad*` / `ConfigSave*`、`SlicerListRequest` / `SlicerListResponse`、`ExportRunRequest` / `ExportRunResponse`、`ParameterDefinition` / `ParsedParameters` / `PresetFile`、`ClientCapabilities` / `ServerCapabilities` / `ProtocolVersionRange` / `ClientPlatform`、`PathHandle` / `WorkspaceId` / `SubscriptionId` / `RequestId(u64)` / `SessionToken`。
- 无任何 I/O、无 transport、无 async。
- **改此 crate = 线格式变更**。需要同步更新 `studio-common::managed_client::envelopes`、`packages/studio-web/src/wasm-bridge/event-stream.ts`、`packages/studio-web/tests/unit/` 快照。

### 2.2 `app-server-core`

- 承接真正的 OS 能力：workspace 解析、目录树、文件读写、OpenSCAD CLI 启停、STL / 3MF 解析、watch notify 聚合、预览任务调度、配置（`dirs::config_dir()/scad-studio/config.json`）、切片器信息、导出。
- 暴露 `dispatch_client_command` 给 host 层；自身不绑定任何 transport。
- **改此 crate = server 端能力变更**。web 与桌面共享同一份实现；桌面端无独立 FS 代码。

### 2.3 `app-server-transport`

- `ClientTransport` trait（同步签名：`handshake / reconnect / request / subscribe / unsubscribe / cancel / poll_server_event / close`）。
- `WebSocketClientTransport`（wasm32 专属），`InProcessTransport`（桌面 `tokio::mpsc`）。
- `websocket_wire.rs`：`encode_client_envelope_text` / `decode_server_envelope_text` 等文本 JSON 编解码。
- `studio-common` **不**依赖 `app-server-transport`（架构硬约束）：前者只描述状态机，后者是平台适配。

### 2.4 `app-server-host`

- 可执行入口：
  - `websocket-host`（二进制 bin）：`--workspace` + `--bind` → tokio-tungstenite 起 WebSocket server，每个连接喂给 `app-server-core::dispatch_client_command`。
  - `in_process_host`：桌面端嵌入式入口（同进程启动 app-server-core + `tokio::mpsc` transport）。

### 2.5 `studio-common`

- 跨端共享 **client 侧** 状态机，**不能依赖** `app-server-transport`（平台特定）。
- 关键类型：
  - `AppServerClient<T>` / `AppServerTransportPort`（低层 client）
  - `ManagedClient<T>`（Phase 2a 新增的监督层）：pending request registry、watch 订阅 registry、超时 tick、watch 节流窗口、reconnect 重放、snapshot 聚合
  - `ClientEvent` / `ClientError` / `ClientSnapshot` / `TransportStatus` / `TransportCloseReason` / `WatchParams` / `WatchEventPayload` / `ClientTimeouts`
  - `WorkspaceSession` / `DirectoryWatchLifecycle` / `PreviewState`（跨端共享业务状态）
- serde 形态由测试快照冻结（`crates/studio-common/tests/managed_client_tests.rs`），web 与桌面 UI 依赖其稳定性。

### 2.6 `studio-web-wasm`

- `wasm_bridge/`：`#[wasm_bindgen]` 薄 wrapper，把 `ManagedClient<NullTransport>` 暴露给 JS（`client_*` 15 个导出 + `mesh_*` + `renderer_*` 桩）。`NullTransport` 是永远返回 `Ok` 的 trait 实现 —— envelope 通过内部字节队列（`next_outbound` / `receive_inbound`）流动，不经 trait。
- `mesh_decode` 顶层模块：`scad_scene::MeshData` 解码纯函数，host + wasm32 共享，测试文件 `tests/mesh_decode_tests.rs` 覆盖。
- `tests/wasm_bridge_smoke.rs`：`wasm-pack test --headless --chrome` 的 9 条 S1b 用例。
- **不含**任何业务 UI、任何 WebSocket、任何 `wasm-bindgen-futures`（契约禁止 wasm 侧等待 JS Promise）。

### 2.7 `studio-app`（桌面）

- egui 壳 + `app-server-host::in_process_host` + `InProcessTransport` 接上 `studio-common::ManagedClient`。
- 跟 `packages/studio-web` 的关系：**UI 壳层各自独立**，但协议 client 状态机和业务状态（workspace / preview）共用 `studio-common`。

### 2.8 `scad-ui` / `scad-scene` / `scad-data` / `scad-viewer`

- `scad-ui`：egui 共享主题与基础组件（目前桌面端消费；web 端不消费 CSS 系统）。
- `scad-scene`：wgpu 渲染管线、`MeshData` 中间表示、相机 / 光照 —— 目前桌面端使用；wasm32 的异步 wgpu 初始化仍在桩阶段。
- `scad-data`：STL / 3MF / 参数文件的字节级解析。
- `scad-viewer`：桌面端预览 widget。

## 3. JS / TS package 能力边界

### 3.1 `packages/studio-web-wasm`

- npm 产物包，**唯一职责** 是分发 `studio-web-wasm` crate 的 wasm-bindgen 产物。
- 内容只允许：
  - `package.json`
  - `README.md`
  - `generated/`（`studio_web_wasm.js` / `.d.ts` / `_bg.js` / `_bg.wasm` / `_bg.wasm.d.ts`，全部提交入库）
  - `src/index.ts`（只做 `export * from "../generated/studio_web_wasm.js"`）
- **不能**出现任何业务 TS 代码、React 组件、CSS。

### 3.2 `packages/studio-web`

React PWA 壳。目录结构：

```
src/
├── main.tsx                        # React root + BrowserRouter + CSS imports
├── App.tsx                         # 顶层路由（/ 工作台，/settings 设置）
├── routes/
│   ├── index.tsx
│   └── settings.tsx                # 独立 WasmClient 生命周期，ConfigLoad/Save
├── state/
│   └── ui-store.ts                 # Zustand：仅 UI 壳状态
├── wasm-bridge/
│   ├── client.ts                   # WasmClient：包 ManagedClient + pump 循环
│   ├── event-stream.ts             # client_drain_events → resolvers / 回调
│   ├── request-resolvers.ts        # 唯一允许的 Map<RequestId, {resolve,reject}>
│   └── index.ts
├── transport/
│   └── websocket-transport.ts      # 浏览器 WebSocket 生命周期 + 指数退避
├── canvas/
│   ├── renderer-controller.ts      # 与 wasm renderer 桩交互的 React hook
│   ├── camera-state.ts             # CameraState + 7 预设常量
│   ├── camera-controls.ts          # orbit / pan / zoom 纯函数
│   └── use-camera-controller.ts    # pointer / wheel 事件绑定
├── viewers/
│   ├── file-read-decoder.ts        # 唯一解包 FileReadContents 的位置
│   ├── markdown-viewer.tsx + markdown-parser.ts
│   ├── image-viewer.tsx
│   ├── scad-split-viewer.tsx
│   └── mesh-viewer.tsx
├── workbench/
│   ├── workbench-layout.tsx        # 五区 CSS Grid 外框 + 协议 pump 接线
│   ├── workbench-wiring.ts         # transport / client callbacks 构造
│   ├── topbar.tsx / rail.tsx / chat-zone.tsx / canvas-zone.tsx / inspector.tsx
│   ├── tabbar.tsx + tab-kind.ts    # 文档标签系统
│   ├── parameters-panel.tsx + presets-panel.tsx + preset-io.ts
│   ├── slicer-panel.tsx + export-panel.tsx
│   ├── log-panel.tsx + use-log-buffer.ts
│   ├── scad-workbench.tsx          # 整合 scad-split + params + presets
│   └── path-utils.ts
└── styles/
    ├── tokens.css                  # 色板 / 字体 / 间距变量
    ├── workbench.css               # 五区 grid + topbar + rail
    ├── workbench-zones.css         # chat + canvas + inspector 区样式
    ├── primitives.css              # 按钮 / chip / 滚动条
    ├── viewers.css                 # Tab bar + viewer
    └── phase7.css                  # camera toolbar / params / presets / slicer / export / log
```

### 3.3 归属硬约束（plan-00-ownership.md）

- **Zustand store 只允许 UI 壳状态**：route / openTabs / activeTabId / activeRail / sidePanelOpen / isSettingsModalOpen / inputDraft。
- **禁止出现** 在 `packages/studio-web/src/` 的同名协议状态机：`WorkspaceSession` / `PreviewState` / `DirectoryWatchLifecycle` / `AppServerClient` / 业务 `requestId` registry。
- **唯一例外**：`wasm-bridge/request-resolvers.ts` 的 `Map<RequestId, {resolve, reject}>`，仅用于派发事件回发起方，不保存业务载荷。
- **协议数据（AppConfig / SlicerConfig / ParsedParameters / PresetFile）只存于组件级 `useState`**，不入 store。

## 4. 运行时数据流

```
用户点击文件
   │
   ▼
Inspector.onRequestPreview(entry)
   │   tab-kind 按扩展名路由
   ▼
WorkbenchLayout.openTab(tab) —— 仅更新 Zustand UI 状态
   │
   ▼
CanvasZone 激活对应 viewer
   │
   ▼
viewer 组件挂载 → WasmClient.dispatchFileRead({path}) / dispatchPreviewRequest({source,…})
   │
   ▼
WasmClient 调 Wasm.client_dispatch_*(handle, params)
   │
   ▼
studio-common::ManagedClient.dispatch_*(...) 分配 RequestId + 构造 ClientEnvelope::Request → 序列化到 outbound 字节队列
   │
   ▼
JS pump 循环（5 Hz）：client_next_outbound → BrowserWebSocketTransport.send(bytes)
   │
   ▼
─────────── WebSocket ───────────
   │
   ▼
websocket-host 解码 → app-server-core::dispatch_client_command → 响应编码 → WebSocket
   │
   ▼
─────────── WebSocket ───────────
   │
   ▼
BrowserWebSocketTransport.onmessage → WasmClient.receiveInbound(bytes)
   │
   ▼
ManagedClient.receive_inbound：解码 ServerEnvelope::Response → 查 pending registry → 生成 ClientEvent::RequestSucceeded
   │
   ▼
JS pump 循环：client_drain_events → event-stream.dispatchClientEvents
   │   - RequestSucceeded → request-resolvers.resolve(requestId, payload)
   │   - onSnapshotDirty → React setSnapshot
   │
   ▼
viewer 组件 Promise 解析 / React 重渲染
```

## 5. 协议线格式

所有 WebSocket 帧都是 UTF-8 JSON，顶层外壳由 `ClientEnvelope` / `ServerEnvelope` 提供：

```json
// client → server
{ "kind": "handshake", "payload": { "capabilities": { ... } } }
{ "kind": "request", "payload": { "request_id": 42, "command": { "command": "workspace.list", "payload": { ... } } } }
{ "kind": "close" }

// server → client
{ "kind": "handshake_ack", "payload": { "session_token": "...", "server_capabilities": { ... }, "negotiated_version": 1 } }
{ "kind": "response", "payload": { "request_id": 42, "result": { "type": "workspace_list", "payload": { "entries": [...] } } } }
{ "kind": "push", "payload": { "event": { "event": "watch.changed", "payload": { "subscription_id": "...", "changed_paths": [...] } } } }
{ "kind": "closed" }
```

`CommandSuccess` / `ClientCommand` 都是 `#[serde(tag = "type"/"command", content = "payload")]` 的 tagged union。

## 6. 内部硬约束回顾

1. `app-server-protocol` 是唯一线格式来源；其它 crate 不允许私自再包一层 tag。Phase 2a codex review 就是因为 `studio-common` 造了 `{"frame":...}` 被打回。
2. `studio-common` **不能** 依赖 `app-server-transport`。
3. 桌面端**不能**绕过协议做本地 I/O。所有 FS / OpenSCAD / slicer 交互都必须走 `app-server-core`。
4. Web 端**不能**持有 WebSocket 之外的业务状态机；协议状态归 `ManagedClient`，UI 状态归 Zustand，两者不交叠。
5. wasm 内部**不**持有 JS Promise；不用 `wasm-bindgen-futures` 等待 JS 异步。
6. 新增 `#[wasm_bindgen]` 项或调整 `src/lib.rs` 模块顺序会让 wasm-bindgen 产物漂移；commit 前必须同步更新 `packages/studio-web-wasm/generated/`，否则 S1c smoke 会红。

## 7. 历史存档

所有 phase 的计划、评审、执行结果都在 `prompt-archives/`，按 `YYYYMMDDNN-description` 目录命名。存档不可变；对历史判断有疑问时读这里。
