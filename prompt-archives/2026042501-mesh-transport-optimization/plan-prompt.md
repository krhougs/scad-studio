# 原始 Prompt

## 2026-04-25

用户在完成 Borsh 协议迁移后，对 WebSocket 传输的 3D mesh 和图片数据进行了分析诊断，确认了以下三个优化方向：

### 2.1 发送原始 artifact 字节而非预解码数组

当前服务端总是先解码 3MF/STL → MeshData → PreviewMeshPayload（拆散为 positions/normals/vertex_colors/indices 四个数组），再 Borsh 编码后发送。导致：
- STL 从 ~5 MB 膨胀到 ~13.2 MB（2.6×），因为不做顶点去重 + 无条件发送 vertex_colors
- 3MF 从 2-5 MB（已压缩）膨胀到 ~6.4 MB（未压缩）
- `PreviewArtifact::ThreeMf` variant 已存在于 protocol 中但未使用

### 2.2 消除 WASM→JS 桥接层的嵌套数组开销

`client_drain_events` 使用 `serde_wasm_bindgen::to_value` 将整个 `ClientEvent::RequestSucceeded { payload: CommandSuccess::PreviewReady(...) }` 传给 JS。`serde_wasm_bindgen::to_value` 将 `Vec<[f32; 3]>` 序列化为 `Array<Array<number>>`，而非 typed array。然后 TS 端 `flattenVec3/flattenVec4/flattenU32` 再逐元素复制到 `Float32Array`。对于 300K 顶点的 mesh，产生约 90 万个中间 JS 数组对象。

项目中已有高效路径：`mesh_decode` → `MeshHandle` 直接返回 typed array（wasm_bridge/mesh.rs），但当前预览流程不走此路径。

### 2.3 启用 WebSocket permessage-deflate

当前 `tokio-tungstenite` 的 `accept_async` 未启用任何压缩。tungstenite 0.28 的 `deflate` feature 未启用（Cargo.lock 无 `flate2` 依赖）。mesh 数据中大量相近浮点值具有很高的压缩比。

## 用户要求

按照项目规范出 plan，不动手。

---

## 2026-04-25 (后续)

### Codex 独立 review 结论

对 plan-00.md 的独立 review 发现以下关键问题：

1. **Phase 1 (permessage-deflate) 不可行**：tungstenite 0.28 和最新 0.29 均不支持 deflate feature（自 2017 年开放 issue 至今未合并）。
2. **Phase 2 与 Phase 3 不可分离**：`serde_wasm_bindgen 0.6.5` 对 `Vec<u8>` 默认走 `serialize_seq` 产生 `Array<number>`，5MB 字节会产生 500 万个 JS number 对象。不做 side buffer 就无法安全发送原始字节。
3. **`studio-common` 受影响**：`preview_state.rs` 的 `complete()` 只认 `Mesh` 变体为 `Ready`，其余走 `Unsupported`；`ready_summary()` 直接访问 `PreviewMeshPayload` 的字段计算统计量。
4. **桌面端只处理 Mesh**：`protocol_client.rs` 的 match 只有 `Mesh` 分支，其余返回错误。
5. **`finalize_job` 先解码后删除临时文件**：需在删除前保留原始字节。

### 用户指令

1. 继续研究 WebSocket 压缩替代方案
2. 前端兼容直接支持 STL（新增 `PreviewArtifact::Stl` 变体）
3. 重新仔细评估，修订 plan

### 压缩方案调研结论

| 方案 | 可行性 | 迁移代价 | 客户端改动 | 是否透明 |
|------|--------|----------|-----------|---------|
| tungstenite deflate feature | 不存在 | — | — | — |
| sockudo-ws / yawc | 可行但需完整替换 WS 库 | 高（1-2 天） | 无 | 透明 |
| 应用层压缩（flate2 压缩 wire payload） | 可行，flate2 已在依赖树中 | 低 | WASM 侧解压 | 对 JS 透明 |
| 反向代理 (nginx) | 不可行（需后端协商扩展） | — | — | — |

结论：采用应用层 flate2 压缩方案。理由：(1) 无需替换 WS 库，改动集中在 wire codec 层；(2) flate2 已通过 `scad-scene → zip` 进入依赖树；(3) 压缩/解压均在 Rust 层完成（服务端 + WASM），对 TS/JS 完全透明；(4) 可按 payload 类型选择性压缩（跳过已 ZIP 压缩的 3MF，仅压缩 STL 和其他大帧）。

---

## 2026-04-25 (第三轮修订)

### Codex 独立 review (第二轮) 关键发现

对合并后的 plan-00.md 进行独立 codebase review（session: 019dc3ac-e047-7991-80e2-5781959c7b36），发现以下问题：

**Blocker**：
1. Phase 2 wire version 方案不可执行：codec 是无状态函数，固定 WIRE_VERSION=1，无连接级压缩协商能力
2. "保留 Mesh fallback" 与 "删除 meshFromPayload / flattenVec / mesh_from_preview_payload" 互相矛盾
3. PreviewArtifact3mf 字段变更影响 roundtrip 测试和生成产物

**High Risk**：
4. 原始字节传输改变错误语义（坏文件从请求阶段失败变为渲染阶段失败）
5. .scad 路径失去 finalize_job 的 3MF 可解析校验
6. "只压缩 STL、跳过 3MF" 与 codec 层无法区分 artifact 类型冲突
7. Side buffer 生命周期对过期请求不完整

### 用户指令

放弃应用层压缩方案。不自己做压缩，用现有的支持 permessage-deflate 的 WebSocket 库替换 tokio-tungstenite。

### WS 库调研结论

| 库 | deflate 支持 | 最近更新 | 成熟度 |
|---|---|---|---|
| tungstenite 0.28/0.29 | 无（2017 年 issue 至今未合并） | — | — |
| yawc 0.3.3 | 默认启用，miniz_oxide 纯 Rust | 2026-04-21 | 声称用于交易系统 |
| ratchet_rs 1.2.1 | feature flag | 2025-01-27 | Autobahn 全通过，README 标注 "not production tested" |
| soketto 0.8.1 | feature flag | 2026-02-10 | 25M 下载，有浏览器 deflate 互操作 bug 历史 |

项目 WS 层隔离度高：仅 `app-server-host` 一个 crate 依赖 tokio-tungstenite，使用集中在 `websocket.rs`（192 行）和测试文件。dispatcher、protocol、transport 层完全不依赖 tungstenite。

结论：选用 yawc。理由：(1) 最活跃（4 天前更新）；(2) deflate 默认启用，纯 Rust 后端；(3) 浏览器原生支持 permessage-deflate，客户端零改动；(4) 替换范围仅 websocket.rs + 测试。

---

## 2026-04-25 (第四轮修订)

### Codex 独立 review (第三轮) 关键发现

session: 019dc4ac-e7c6-7880-a742-ca2ec5a596ac

**Blocker**：
1. yawc 不是 `accept_async` 等价替换——需要 hyper HTTP upgrade server，不是裸 TCP accept。Phase 1 范围被低估。
2. yawc 默认 `max_payload_read` 1 MiB / `max_read_buffer` 2 MiB，大 mesh 会被截断。
3. "deflate 默认启用"不可靠，需显式 `with_compression_level(...)`。
4. `PreviewArtifact::Stl = 3` 是协议破坏性变更，旧客户端解码失败。（用户判定：预期功能补差，不处理）
5. Web 端 `ManagedClient` 收到 PreviewReady 立即标记 Ready；takePreviewMesh 解码失败后 snapshot 仍显示 Ready。
6. Side buffer 缺少 client_destroy 清理、容量上限、非 latest preview 清理。
7. 计划错误假设"被取消请求仍推送 RequestSucceeded"——源码在 cancelled 时直接返回。
8. .scad 路径不 load_3mf 验证，坏 3MF 也会从服务端失败变客户端失败。
9. Sentinel 修复不够——混合有色/无色顶点时 [0,0,0,-1] 在 Three.js 渲染为黑色。

**Sequencing 建议**：Phase 2 拆成两步——先做客户端消费能力，再让服务端停止产出 Mesh。

### 用户决策

- Blocker 4（协议破坏性变更）：预期的功能补差，不需要版本协商。
- Blocker 2：需要配置允许超大 payload。
- 其余 blocker 和 sequencing 建议：全部采纳。

---

## 2026-04-25 (第五轮修订)

### Codex 独立 review (第四轮) 关键发现

session: 019dc502-db7b-7df2-a741-2864e5ca283f (model: gpt-5.5)

**Blocker**：
1. Web 端解码失败无法按 plan 回退共享 preview 状态——`ManagedClient` 收到 `PreviewReady` 后直接置 `Ready`，`studio-web-wasm` 没有公开 API 可以把某个 preview request 从 Ready 改为 Error。需新增 `ManagedClient::fail_preview_decode(request_id, message)` 或等效方法。
2. side buffer 的"按 target 替换旧条目"缺少数据来源——`client_drain_events` 事件里没有 target，`ManagedClient.preview_tasks` 字段是 `pub(super)`，WASM bridge 不可读。

**High Risk**：
3. 新增 Borsh enum variant 但不升级协议版本，混合版本会在解码层失败。建议至少升级协议版本到 1,2。
4. side buffer 只按条数限制（8条）不足以控制 WASM 内存——8 条 50 MiB 级 artifact 会长期占用 WASM 线性内存。建议同时设置 byte cap。
5. yawc 迁移需要引入 hyper/hyper-util/http-body-util/bytes，并重写 run_websocket_host_once 的单连接语义。

**Missing**：
6. 缺少 `client_take_preview_mesh` 与 TS promise 语义的具体契约——当前 promise resolve 值没有标准化携带 request id。
7. 测试覆盖缺少 bridge 层关键断言（WASM bridge 层的 ThreeMf/Stl 成功、坏 STL 失败、重复 take 等）。
8. `serde_bytes` 需要覆盖 tuple variant 写法（`FileReadContents::Binary(Vec<u8>)`）和 TS 兼容测试。
9. 桌面端解码错误路径需要明确 UI 行为。

**Sequencing**：
10. 协议版本和 capability 应在 Phase 2 最前面完成。
11. Web 状态回退必须早于服务端错误语义变更——Phase 3 前必须有自动化测试覆盖坏 STL/坏 3MF 的 Web error UI。

### 用户决策

- Blocker 1（Web 端解码失败回退 preview 状态）：采纳，需要新增公开方法。
- Blocker 2（side buffer target 数据来源缺失）：采纳，需要解决 target 追踪。
- High Risk 3（协议版本升级）：不管，预期功能补差，不升版本。
- High Risk 4（side buffer byte cap）：不管，条数限制即可。
- High Risk 5（yawc 需要 hyper 栈）：采纳，plan 中已有 spike 但需更明确。
- Missing 6（TS promise request_id 契约）：不管。
- Missing 7（bridge 层测试覆盖）：采纳。
- Missing 8（serde_bytes tuple variant 覆盖）：采纳。
- Missing 9（桌面端错误 UI 行为）：不管。
- Sequencing 10（协议版本先行）：因 3 不处理，版本升级部分不适用。
- Sequencing 11（Web 状态回退先于服务端切换）：采纳。
