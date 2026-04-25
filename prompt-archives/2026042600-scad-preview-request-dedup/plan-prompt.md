# Prompt Archive: scad preview request dedup

## 原始问题

用户在前端 debugger 中发现：打开新的 3D 模型预览时，WebSocket 中收到两次 mesh。

用户要求：

- 启动独立 subagent 排查原因。
- 找出问题在后端还是前端。
- 给出石锤证据和修复方案。
- 开一个新的 plan。

## 已完成诊断

主 agent 与独立 subagent 均判断：问题更可能来自前端重复请求，而不是后端重复发送同一个 response，也不是 wasm bridge 重复处理同一个 response。

随后主 agent 开启独立 dev server 实测：

- Web: `http://127.0.0.1:5187`
- WebSocket: `ws://127.0.0.1:39487`
- Workspace: `tests/studio-web-smoke-workspace`
- Recorder: Playwright 注入 WebSocket send/message hook，并用 `@budn/app-server-protocol` 解码 client/server frame。

实测结果：

```text
打开 model.stl：
outgoing preview.request: request_id=5
incoming preview_ready:   request_id=5, artifact=stl

结论：STL 路径没有双发 mesh。
```

```text
打开 examples/cube.scad：
outgoing preview.request: request_id=10, source=examples/cube.scad, defines=[]
incoming preview_ready:   request_id=10, artifact=three_mf

outgoing preview.request: request_id=12, source=examples/cube.scad, defines=[]
incoming preview_ready:   request_id=12, artifact=three_mf

两次 preview.request 间隔约 271ms。
```

271ms 与前端 `.scad` 参数更新链路中的 250ms debounce 对齐：

- `packages/studio-web/src/workbench/scad-workbench.tsx` 中源码读取完成后会设置 `appliedDefines`。
- 同文件中 250ms debounce 后会再次设置内容相同的 `appliedDefines`。
- `packages/studio-web/src/workbench/parameter-model.ts` 的 `formatCurrentDefines` 每次返回新数组。
- `packages/studio-web/src/viewers/mesh-viewer.tsx` 的预览 effect 依赖 `defines` 数组引用，因此等价内容的新数组会触发第二次 `dispatchPreviewRequest`。

后端路径已排除：

- `crates/app-server-host/src/websocket.rs` 对每个 `ClientEnvelope::Request` 只调用一次 `dispatcher.dispatch_envelope` 并发送一次 response。
- `crates/app-server-host/src/dispatcher.rs` 对一个 `ClientRequestEnvelope` 只构造一个 `ServerResponseEnvelope`，沿用原 `request_id`。
- 实测中的两次 mesh response 对应不同 request id，因此不是同一个请求被后端重复发送。

wasm bridge 重复处理已排除：

- `crates/studio-common/src/managed_client/inbound.rs` 用 `pending.remove(&response.request_id)` 处理 response。
- `packages/studio-web/src/wasm-bridge/request-resolvers.ts` 中 resolver resolve 后立即 delete。

## 强制约束

- 不通过移除 React `StrictMode` 解决问题。
- 本轮目标是修复 `.scad` 预览重复 `PreviewRequest`，不要改后端 protocol、transport 或 app-server-host 语义。
- 使用 repo 现有工具链，命令优先使用 `bun`。
- 按项目 Plan Mode 要求，每个 Phase 完成后必须使用独立 subagent review，并把结果写入 `plan-00-result.md`。
- 每个 Phase 执行时要保护前序 Phase 已达成目标，不得为了通过局部验证重新引入重复请求。

## 预期产出

- 一个可回归验证的测试用例，证明打开同一个 `.scad` 预览时不会发出两个等价 `preview.request`。
- 前端状态更新链路中的等价 `defines` 更新不再触发重复预览请求。
- 必要时在预览请求层增加幂等保护，避免未来其他等价状态更新再次造成重复请求。
