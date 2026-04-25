# plan-00：Mesh 传输优化

## 背景

Borsh 二进制协议迁移已完成，WebSocket wire 层不再有 JSON/base64。但 mesh 数据从服务端到浏览器渲染的端到端路径上仍存在两处系统性浪费：

- **Wire 膨胀**：服务端解码 3MF/STL → `PreviewMeshPayload`（positions / normals / vertex_colors / indices 四个独立数组）→ Borsh 编码。100K 三角面 STL 从 ~5 MB 膨胀到 ~13.2 MB（`from_triangles` 不做顶点去重、无条件填充 `vertex_colors` sentinel `[0.0, 0.0, 0.0, -1.0]`、顺序索引无信息量）。3MF 自带 ZIP 压缩（2-5 MB），服务端先解压再重编码为未压缩数组，也产生膨胀。
- **WASM→JS 桥接开销**：`client_drain_events` 使用 `serde_wasm_bindgen::to_value` 将含完整 mesh 数组的 `ClientEvent` 传给 JS。`Vec<[f32; 3]>` 被序列化为 `Array<Array<number>>`（嵌套 JS 数组），300K 顶点产生约 90 万个中间 JS 对象。进一步地，`serde_wasm_bindgen 0.6.5` 对 `Vec<u8>`（无 `serde_bytes` 标注）走 `serialize_seq` 产生 `Array<number>`，5 MB 原始字节会产生 500 万个 JS 对象——因此原始字节传输必须搭配 side buffer 绕过 serde_wasm_bindgen，否则性能反而恶化。

此外，当前 `tokio-tungstenite` 0.28 不支持 permessage-deflate（RFC 7692），该 feature 自 2017 年起 issue 至今未合并。mesh 数据（密集浮点值）和 STL 二进制格式具有很高的压缩比，但全部以未压缩帧传输。

项目中已存在但当前主预览流未使用的高效路径：
- `PreviewArtifact::ThreeMf(PreviewArtifact3mf)` protocol 变体——可直接携带原始 3MF 字节，当前服务端未产出。
- `mesh_decode` → `MeshHandle`（`wasm_bridge/mesh.rs`）——从原始 3MF/STL 字节解码，通过 wasm_bindgen 直接返回 typed array（`Vec<f32>` / `Vec<u32>` 映射为 JS `Float32Array` / `Uint32Array`），不经过 `serde_wasm_bindgen`。

## 用户强制约束

1. 覆盖 2.1（原始 artifact 字节）、2.2（消除 WASM→JS 嵌套数组开销）、2.3（传输压缩）。
2. 前端原生支持 STL 格式：新增 `PreviewArtifact::Stl` protocol 变体，客户端直接解码 STL 字节，服务端不做格式转换。
3. 不改变桌面端 `tokio::mpsc` in-memory 通信方式。
4. 桌面与网页走同一份 protocol，protocol 不绑定平台。
5. 不引入新的序列化格式；优化路径在 Borsh + 已有格式（3MF/STL 原始字节）范围内完成。
6. 传输压缩通过替换 WebSocket 库实现 permessage-deflate，不做应用层压缩。选用 yawc 替换 tokio-tungstenite。

## 架构约束摘要（摘自 AGENTS.md）

- `studio-common`：管共享状态与行为，允许依赖 `app-server-protocol`，禁止依赖 transport。允许谨慎依赖 `scad-scene` 的纯渲染数据结构。
- `studio-web-wasm`：WASM bridge 层，负责参数反序列化、错误转换和结果序列化，禁止引入协议状态机。
- `app-server-protocol`：只描述命令、事件、错误、能力与数据模型。
- `app-server-host`：WebSocket host，调用 transport 编解码函数。
- `studio-app`：桌面专属外壳，不直接触碰 I/O。

## 数据流现状

```
Server (.scad): OpenSCAD CLI → .3mf temp file → load_3mf → MeshData
               → mesh_to_preview_payload → PreviewArtifact::Mesh(PreviewMeshPayload)
               → Borsh encode → WS binary frame (无压缩)

Server (.stl/.3mf 直接预览): 读文件 → load_stl/load_3mf → MeshData
               → mesh_to_preview_payload → PreviewArtifact::Mesh(PreviewMeshPayload)
               → Borsh encode → WS binary frame

Client WASM: WS binary → Borsh decode → ManagedClient.handle_success
            → events.push_back(ClientEvent::RequestSucceeded { payload: PreviewReady })
            → client_drain_events → serde_wasm_bindgen::to_value(&events)
            → JS: Array<Array<number>> 嵌套数组

Client TS: event-stream.ts → resolvers.resolve(requestId, payload)
          → mesh-viewer.tsx → payloadFromPreview → meshFromPayload
          → flattenVec3/flattenVec4/flattenU32 逐元素复制到 typed array
          → Three.js BufferGeometry

Desktop: Borsh/mpsc → PreviewArtifact::Mesh(mesh) → mesh_from_preview_payload
        → MeshData → wgpu renderer
```

## 目标数据流

```
Server (.scad): OpenSCAD CLI → .3mf temp file → load_3mf 验证 → 读取原始字节 → 删除临时文件
               → PreviewArtifact::ThreeMf(bytes) → Borsh encode
               → WS binary frame (permessage-deflate 透明压缩)

Server (.3mf): 读取文件原始字节 → PreviewArtifact::ThreeMf(bytes) → 同上

Server (.stl): 读取文件原始字节 → PreviewArtifact::Stl(bytes) → 同上

Client WASM: WS binary (浏览器自动 deflate 解压) → Borsh decode
            → ManagedClient.handle_success
            → WASM bridge 拦截 PreviewReady：重载荷（Stl/ThreeMf 原始字节）存入 side buffer
            → client_drain_events → serde_wasm_bindgen::to_value (只含轻量事件标记)
            → client_take_preview_mesh(request_id) → 按 artifact 类型调用对应解码 → MeshHandle

Client TS: event-stream.ts → resolvers.resolve(requestId, 轻量标记)
          → mesh-viewer.tsx → wasmClient.takePreviewMesh(requestId)
          → MeshHandle.positions() / .normals() / .colors() / .indices()
          → Float32Array/Uint32Array → Three.js BufferGeometry
          （解码失败 → 报告错误 → preview 状态转为 Error）

Desktop: Borsh/mpsc → PreviewArtifact::ThreeMf(bytes) 或 Stl(bytes)
        → scad-scene decode → MeshData → wgpu renderer
```

---

## Phase 1：WebSocket permessage-deflate（yawc 替换 tokio-tungstenite）

**目标**：用支持 permessage-deflate 的 WebSocket 库替换 tokio-tungstenite，为所有 WebSocket 帧启用透明压缩。

**要保护的前序目标**：无（首个 Phase）。

**背景与动机**：

tokio-tungstenite 0.28/0.29 不支持 permessage-deflate。浏览器 WebSocket 原生支持 permessage-deflate——若服务端在握手阶段协商成功，浏览器自动处理解压，客户端代码零改动。

项目的 WS 层隔离度很高：仅 `app-server-host` 一个 crate 直接依赖 tokio-tungstenite。dispatcher、protocol、transport 层不碰 tungstenite 类型。

选用 yawc 0.3.x：deflate 支持，纯 Rust 后端（miniz_oxide），tokio 兼容。

**关键技术决策**：

1. **yawc 的服务端 API 需要 hyper HTTP upgrade**。当前 `websocket.rs` 是裸 `TcpListener.accept()` → `accept_async(stream)` 模式。yawc 的服务端路径基于 `hyper::Request` / `WebSocket::upgrade`，因此 Phase 1 需要在 `websocket.rs` 中引入最小化的 hyper HTTP 服务来处理 WebSocket upgrade。这将 `websocket.rs` 从"裸 TCP + accept_async"改为"hyper service + WS upgrade"，范围超出纯粹的类型替换，但仍限制在 `app-server-host` crate 内部。
2. **显式配置压缩和 payload 限制**。yawc 的 `Options::compression` 需要显式设置 `with_compression_level(CompressionLevel::fast())`，不能依赖默认行为。同时必须调大 `max_payload_read` 和 `max_read_buffer`（至少 64 MiB），否则大 mesh 帧会被截断（yawc 默认 `max_payload_read` 1 MiB、`max_read_buffer` 2 MiB）。
3. **集成测试的客户端也需迁移**。当前测试用 `tokio-tungstenite::connect_async`，需改为 yawc 的客户端 API 或其他兼容 deflate 协商的 WS 客户端。测试仍需覆盖 binary frame、text rejection、wire version rejection 等场景。
4. **先做 spike 验证**。在正式替换前，先用一个最小化 spike 证明 yawc 的 hyper upgrade server 能在现有架构下工作。spike 验收标准：(a) 最小 hyper HTTP/1 server 接受 TCP 连接并完成 WebSocket upgrade；(b) `WebSocket::upgrade_with_options` 配置 compression + payload 限制成功；(c) binary/text/close frame 行为与当前 tungstenite 一致；(d) `run_websocket_host_once` 单连接语义可在 hyper service 中实现。spike 需要引入 `hyper`/`hyper-util`/`http-body-util`/`bytes` 依赖。若 spike 失败或 API 不兼容，需重新评估 WS 库选型（备选：ratchet_rs 可直接接受 `AsyncRead + AsyncWrite` 流，无需 hyper）。

**验收标准**：

1. 所有现有 Rust 集成测试通过（`websocket_smoke_roundtrip`、`websocket_rejects_text_handshake_frame`、`websocket_rejects_unsupported_wire_version_before_dispatch`）。
2. 所有现有 Playwright 端到端测试通过。
3. WS 握手响应头包含 `Sec-WebSocket-Extensions: permessage-deflate`（通过集成测试验证）。
4. 大帧（> 1 MiB）能正常收发，不被 payload 限制截断。
5. 桌面端不受影响（不经过 WebSocket）。

---

## Phase 2：Protocol 扩展与客户端消费能力

**目标**：扩展 protocol 支持 STL 原始字节变体，更新所有消费端（桌面、Web WASM bridge、Web TS）使其能处理 `ThreeMf` 和 `Stl` artifact，引入 side buffer + typed array 直传路径。本 Phase 结束时服务端仍产出 `Mesh`，但所有客户端已具备处理新格式的能力。

**要保护的前序目标**：
- Phase 1：permessage-deflate 正常工作。

**背景与动机**：

将协议扩展和客户端适配与服务端切换分开：先让所有消费端具备处理新格式的能力，再由 Phase 3 切换服务端输出。这样把兼容性风险和性能优化解耦——若 Phase 3 出问题，可以确认是服务端切换导致，而非客户端解析问题。

**关键技术决策**：

1. **新增 `PreviewArtifact::Stl` protocol 变体**（用户强制约束）：携带原始 STL 字节。discriminant = 3。同步新增 `PreviewResponseFormat::Stl = 3`。新增 `PreviewArtifactStl` 结构体。这是协议破坏性变更（旧客户端无法解码新 discriminant），属于预期的功能补差，不需要版本协商。
2. **不修改 `PreviewArtifact3mf` 的字段结构**：`ready_summary()` 对 ThreeMf/Stl 显示字节大小，不需要 triangle_count，避免 Borsh 布局变更。
3. **所有 `Vec<u8>` 字段标注 `serde_bytes`**：覆盖 `PreviewArtifact3mf.bytes`、`PreviewArtifactStl.bytes`、`PreviewRenderedImagePayload.bytes`，并排查 protocol 中其他 `Vec<u8>` 字段（如 `FileReadContents::Binary`）��并标注。`FileReadContents::Binary(Vec<u8>)` 是 tuple variant，需要字段级 serde 标注（`#[serde(with = "serde_bytes")]` 或 `serde_bytes::ByteBuf` wrapper）。需引入 `serde_bytes` crate 作为 `app-server-protocol` 的直接依赖。此标注不影响 Borsh 序列化。标注后需验证现有 TS 端（`decodeFileRead` 等）在接收 `Uint8Array`（而非 `Array<number>`）时仍正常工作。
4. **Side buffer 生命周期**：按 request_id 索引。清理时机：(1) TS 调用 `take_preview_mesh` 后消费并移除；(2) `client_destroy` 时清空全部 buffer；(3) WebSocket 断连时清空全部 buffer；(4) 设置容量上限（如 8 条），超出时 LRU 淘汰最旧条目；(5) 新 preview 到达时，若同一 target 已有旧条目，替换旧条目——target 信息从 `ClientEvent::RequestSucceeded` 事件中获取：WASM bridge 拦截 `PreviewReady` 时，通过 `ManagedClient::snapshot().preview_tasks` 查找 request_id 对应的 target（`PreviewTaskState.target`），或在 `ClientHandle` 中维护 `request_id → target` 辅助映射（dispatch preview 时记录）。注意：被取消的请求不会推送 `RequestSucceeded`（`inbound.rs` 在 `info.cancelled` 时直接返回），因此不会进入 side buffer。
5. **Web 端解码失败的错误闭合**：`ManagedClient` 在收到 `PreviewReady` 时立即标记 Ready（`inbound.rs` 第 88-96 行）并清空 `preview_error`。当 `take_preview_mesh` 解码失败时，必须将 preview 状态从 Ready 回退到 Error。当前 `ManagedClient` 没有公开 API 可以从外部将某个 preview request 从 Ready 改为 Error（`preview_tasks` 和 `preview_error` 都是 `pub(super)`）。因此需要新增公开方法，例如 `ManagedClient::fail_preview_decode(request_id: RequestId, message: String)`——该方法将对应 `preview_tasks` 条目的 phase 设为 `Error`，设置 `preview_error`，并确保 snapshot dirty（后续 `snapshot()` 调用能反映最新状态）。`client_take_preview_mesh` 在解码失败时调用此方法。TS 层在 `takePreviewMesh` 返回错误时展示解码错误信息。
6. **`MeshHandle.colors()` sentinel 修复**：`MeshData::from_triangles` 写入 `[0.0, 0.0, 0.0, -1.0]` 作为"无颜色"sentinel，但 `MeshHandle::colors()` 检查 `[1.0, 1.0, 1.0, 1.0]`（纯白）。修复方案：sentinel 检测条件改为 `alpha < 0`（`v.color[3] < 0.0`）。对于混合有色/无色顶点的情况，无色顶点的 sentinel 值替换为 `[1.0, 1.0, 1.0, 1.0]`（白色，Three.js vertex color 模式下的中性基色），避免 `[0,0,0,-1]` 渲染为黑色。若所有顶点均为 sentinel，返回空 `Vec<f32>` 让 TS 走无色路径。
7. **`decode_mesh_bytes` 的协议格式提示**：WASM bridge 的 `take_preview_mesh` 根据 artifact 变体类型调用对应的 `load_stl_from_reader` 或 `load_3mf_from_reader`，而非依赖 `decode_mesh_bytes` 的魔数嗅探。`decode_mesh_bytes` 作为通用工具函数保留不动。
8. **生成产物同步更新**：`packages/app-server-protocol/src/index.ts` 的 TS 类型定义需新增 `"stl"` 格式。`app-server-protocol-wasm` 和 `studio-web-wasm` 的生成产物需重新生成。

**影响面**：

- `app-server-protocol`：新增 `PreviewArtifactStl` 结构体和 `Stl` 枚举变体。`PreviewResponseFormat` 新增 `Stl = 3`。`Vec<u8>` 字段添加 `serde_bytes` 标注。
- `studio-common`：`preview_state.rs` 的 `complete()` 接受 `ThreeMf` 和 `Stl` 为 `Ready`。`ready_summary()` 对新变体显示字节大小。
- `studio-app`：`protocol_client.rs` 新增 `ThreeMf` 和 `Stl` 处理分支（调用 `scad-scene` 解码）。现有 `Mesh` 处理保留。
- `studio-web-wasm`：bridge 拦截 `PreviewReady` 事件中的 `ThreeMf`/`Stl` artifact 存入 side buffer。新增 `client_take_preview_mesh` wasm_bindgen 导出（返回 Result，失败时回退 preview 状态）。统一 `MeshHandle.colors()` sentinel。`client_destroy` 时清空 side buffer。
- `studio-web` (TS)：`mesh-viewer.tsx` 新增通过 `takePreviewMesh → MeshHandle` typed array 获取 mesh 的路径。`event-stream.ts` 的 resolve 值对 preview ready 事件变为轻量标记。解码失败时展示错误。
- `Mesh` fallback：消费端保留 `Mesh` 处理作为 fallback（桌面端继续调用 `mesh_from_preview_payload`，TS 端继续走 `payloadFromPreview → flattenVec*`）。这些函数不删除。
- 测试：Borsh roundtrip 测试补充 `Stl` 用例。`preview_state_tests` 补充 ThreeMf/Stl 变体用例。`studio-web-wasm` 补充 bridge 层测试：ThreeMf/Stl 成功解码、坏 STL 解码失败且 preview 状态回退到 Error、重复 take 返回空、`client_destroy` 后 side buffer 清空、transport close 后 side buffer 清空、LRU 淘汰。`serde_bytes` 标注后验证 `serde_wasm_bindgen` 输出为 `Uint8Array`（覆盖 `PreviewArtifact3mf.bytes`、`PreviewArtifactStl.bytes`、`PreviewRenderedImagePayload.bytes`、`FileReadContents::Binary`）。
- 生成产物：TS 类型定义和 WASM 生成产物重新生成。

**验收标准**：

1. 手动构造 `PreviewArtifact::Stl` 和 `PreviewArtifact::ThreeMf` 的 Borsh roundtrip 测试通过。
2. `preview_state.complete()` 对 ThreeMf 和 Stl 变体转为 Ready 状态。
3. 桌面端能处理 ThreeMf 和 Stl artifact（从原始字节解码 mesh）。
4. Web 端 `takePreviewMesh` 能从 side buffer 取出字节、解码为 MeshHandle、返回 typed array。
5. Web 端 `takePreviewMesh` 解码失败时，preview 状态正确转为 Error，TS 展示错误信息。
6. `MeshHandle.colors()` 对 STL 文件（全部无色 sentinel）返回空数组。
7. `MeshHandle.colors()` 对混合有色/无色顶点的 3MF 文件，无色顶点输出白色而非黑色。
8. Side buffer 在 `client_destroy` 时清空。
9. Side buffer 在 transport close 时清空。
10. 坏 STL 字节传入 `takePreviewMesh` 时解码失败，preview 状态回退到 Error（snapshot 中 `preview_error` 非空），TS 展示错误信息。
11. `serde_bytes` 标注后，`serde_wasm_bindgen::to_value` 对 `Vec<u8>` 字段输出 `Uint8Array` 而非 `Array<number>`。
12. 服务端仍产出 `Mesh`，所有现有测试不受影响。

---

## Phase 3：服务端切换为原始 artifact 字节

**目标**：服务端三条预览路径全部切换为产出 `ThreeMf` 或 `Stl` 原始字节，不再经过解码-重编码。

**要保护的前序目标**：
- Phase 1：permessage-deflate 正常工作。
- Phase 2：所有消费端已具备处理 ThreeMf/Stl 的能力，side buffer 和 typed array 直传路径正常工作。
- Phase 2 的 Web 端解码失败 → preview 状态 Error 路径必须已有自动化测试覆盖（坏 STL/坏 3MF 的 WASM bridge 测试和 Web error UI 测试）。Phase 3 将 .stl/.3mf 坏文件的错误从服务端延后到客户端，若 Phase 2 的错误路径未验证，Phase 3 会引入"服务端成功但前端空白或 snapshot 仍 Ready"的回归。

**背景与动机**：

Phase 2 已让所有消费端具备处理新格式的能力。本 Phase 切换服务端输出，消除 wire 膨胀（STL 13.2 MB → 5 MB，3MF 解码重编码 → 直传原始字节）。

**关键技术决策**：

1. **.stl 路径**：读取文件原始字节 → `PreviewArtifact::Stl(bytes)`。不做服务端解码验证——坏文件的错误从服务端失败变为客户端失败。客户端解码失败通过 Phase 2 建立的错误路径处理（Web 端 `takePreviewMesh` 失败 → preview 状态转 Error；桌面端 `scad-scene` 解码失败 → 预览错误状态）。
2. **.3mf 路径**：读取文件原始字节 → `PreviewArtifact::ThreeMf(bytes)`。与 .stl 同理，错误延后到客户端。
3. **.scad 路径**：OpenSCAD CLI 输出 3MF 临时文件 → 读取原始字节 → **保留 `load_3mf_from_reader` 服务端验证**（在读取字节后、发送前验证 3MF 可解析）→ 验证通过后发送 `PreviewArtifact::ThreeMf(bytes)`，验证失败返回错误。保留验证的原因：OpenSCAD 可能因内部错误生成损坏的 3MF 文件，此类错误应在服务端暴露而非让客户端静默失败。OpenSCAD CLI 退出码和输出文件存在性检查也保留。
4. **`finalize_job` 和 `RenderedArtifact` 改动**：`RenderedArtifact` 从携带 `mesh: MeshData` 改为携带原始字节。`finalize_job` 改为先 `std::fs::read` 读取原始字节，再 `load_3mf_from_reader` 验证，再删除临时文件。
5. **死代码清理**：服务端 `mesh_to_preview_payload` 不再被调用，应清理该函数及其 `lib.rs` 公开导出。

**影响面**：

- `app-server-core`：三条预览路径改为产出 `ThreeMf` 或 `Stl`。`finalize_job` 和 `RenderedArtifact` 携带原始字节。`mesh_to_preview_payload` 成为死代码，清理。
- 测试：`websocket_smoke_roundtrip` 的预览断言从 `PreviewArtifact::Mesh(mesh)` + `mesh.positions.is_empty()` 改为 `PreviewArtifact::Stl(stl)` + `stl.bytes.is_empty()`。其他集成测试同步适配。Playwright 端到端测试覆盖 STL 和 3MF 预览路径。

**验收标准**：

1. `.stl` 预览路径：WebSocket 帧中 artifact 为 `Stl` 变体，帧大小接近原始 STL 文件大小（而非膨胀后的 13 MB）。
2. `.scad` 预览路径：WebSocket 帧中 artifact 为 `ThreeMf` 变体，帧大小接近 OpenSCAD 输出的 3MF 文件大小。
3. `.3mf` 预览路径：WebSocket 帧中 artifact 为 `ThreeMf` 变体，直接读取文件字节。
4. `client_drain_events` 返回的 JS 事件中，preview ready 事件不包含 mesh 数据或原始字节（仅轻量标记）。
5. TS 端通过 `MeshHandle` typed array 获取 mesh 数据，Three.js 渲染正常，视觉结果与改动前一致。
6. 桌面端预览功能正常（从原始字节解码 mesh）。
7. 已有 Playwright 测试和 Rust 集成测试全部通过。
8. 服务端不再调用 `mesh_to_preview_payload`，该函数及其公开导出已从代码中移除。

---

## 风险评估

1. **yawc 需要 hyper HTTP upgrade（高）**：yawc 的服务端 API 基于 `hyper::Request` / `WebSocket::upgrade`，不是裸 TCP accept_async 的等价替换。websocket.rs 需要从"裸 TCP 监听 + WS 握手"改为"hyper HTTP 服务 + WS upgrade"，需要引入 `hyper`/`hyper-util`/`http-body-util`/`bytes` 依赖，并重写 `run_websocket_host_once` 的单连接语义、错误响应和任务生命周期。缓解：Phase 1 先做 spike 验证可行性（详见 Phase 1 关键技术决策 4）；若 yawc 的 hyper 依赖不可接受，备选 ratchet_rs 直接接受 `AsyncRead + AsyncWrite` 流，无需引入 HTTP 框架。
2. **yawc 库成熟度（中）**：yawc 0.3.x 是 pre-1.0 库。缓解：WS 层隔离度高（仅 websocket.rs），可快速替换或回退。
3. **桌面端回归风险（中）**：Phase 3 改变 protocol 层 artifact 格式。缓解：Phase 2 已让桌面端具备处理新格式的能力，Mesh fallback 保留。
4. **错误语义变更（中）**：.stl/.3mf 直接预览的坏文件从服务端失败变为客户端失败。.scad 路径保留服务端 `load_3mf` 验证。缓解：Phase 2 建立了完整的客户端解码失败 → preview 状态 Error 的路径。
5. **`MeshHandle.colors()` sentinel 不一致（中）**：`from_triangles` 写入 `[0,0,0,-1]`，`MeshHandle::colors()` 检查 `[1,1,1,1]`。混合有色/无色顶点时，`[0,0,0,-1]` 会让无色顶点渲染为黑色。已纳入 Phase 2 强制修复，sentinel 检测改为 `alpha < 0`，无色顶点替换为白色。
6. **Side buffer 内存（低）**：容量上限 + client_destroy 清理 + 断连清理。被取消的请求不会进入 side buffer（`inbound.rs` 在 `cancelled` 时直接返回，不推 `RequestSucceeded`）。
7. **Web 端 preview 状态回退复杂度（中）**：`ManagedClient` 的 `preview_tasks` 和 `preview_error` 是 `pub(super)`，WASM bridge 无法直接修改。需新增公开方法 `fail_preview_decode`，确保 snapshot 一致性。缓解：Phase 2 的 bridge 层测试覆盖 decode 失败 → snapshot error 路径。
