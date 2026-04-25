# plan-00 执行结果

## Phase 1：WebSocket permessage-deflate

完成情况：

- 已用 `yawc` 替换 `app-server-host` 的服务端 `tokio-tungstenite` 运行时依赖。
- WebSocket 服务端改为最小 hyper HTTP/1 upgrade 服务，保留 `run_websocket_host` 与 `run_websocket_host_once` 对外入口。
- 已显式启用 `CompressionLevel::fast()`，并将 `max_payload_read` 与 `max_read_buffer` 配置为 64 MiB。
- `tokio-tungstenite` 仅保留为 `app-server-host` 测试依赖，用于覆盖非压缩客户端兼容路径。
- 未改动 protocol、transport 抽象或桌面 `tokio::mpsc` 通信路径。

验证结果：

- `cargo test -p app-server-host --test websocket_smoke_roundtrip`
- 结果：6 个测试全部通过。
- 覆盖项：原有 roundtrip、文本帧拒绝、wire version 拒绝、`permessage-deflate` 握手响应头、压缩客户端协议 roundtrip、>1 MiB 出站帧、>1 MiB 入站帧。

独立 review：

- 初次 review 发现两个验证缺口：压缩协商后的协议 roundtrip、大帧收发覆盖不足。
- 复审发现剩余验证缺口：只覆盖出站大帧，未覆盖服务端入站大帧。
- 最终复审结论：Blocker/High/Medium 均无，Phase 1 可以进入 Phase 2。

遗留问题：

- Playwright 端到端测试将在后续阶段和最终全量验证中统一运行。

## Phase 2：Protocol 扩展与客户端消费能力

完成情况：

- `app-server-protocol` 已新增 `PreviewArtifact::Stl`、`PreviewArtifactStl` 与 `PreviewResponseFormat::Stl = 3`。
- `PreviewArtifact3mf.bytes`、`PreviewArtifactStl.bytes`、`PreviewRenderedImagePayload.bytes` 与 `FileReadContents::Binary` 已添加 `serde_bytes` 标注，并同步更新 Rust 与 TypeScript protocol 类型。
- `studio-common` 已支持 `ThreeMf` / `Stl` preview ready 状态与字节摘要；新增 `ManagedClient::fail_preview_decode`，用于 Web bridge 解码失败后把最新 preview 回退为 Error。
- 桌面端已支持从 `ThreeMf` / `Stl` 原始字节解码为 `MeshData`，保留既有 `Mesh` fallback。
- Web WASM bridge 已实现 request_id side buffer：`client_drain_events` 拦截 `ThreeMf` / `Stl` 重载荷并清空事件内字节，`client_take_preview_mesh` 解码并返回 `MeshHandle` typed array 入口。
- Side buffer 已覆盖 destroy、transport close、容量上限、同 target 新旧替换、旧响应晚到保护、重复 take、坏 STL 解码失败回退 Error。
- `MeshHandle.colors()` 已修复 sentinel 语义：全部无色时返回空数组；混合有色/无色时无色顶点输出白色。
- Web TS 端已在 preview resolver 中调用 `client_take_preview_mesh`，成功时返回 typed array payload，失败时触发 snapshot 刷新并 reject；既有 Mesh fallback 保留。
- `packages/app-server-protocol/generated` 与 `packages/studio-web-wasm/generated` 已重新生成。

验证结果：

- `cargo test -p app-server-protocol --test borsh_payload_roundtrip_tests`：7 个测试通过。
- `cargo test -p studio-common --test managed_client_tests --test preview_state_tests`：17 + 7 个测试通过。
- `cargo test -p studio-web-wasm --target wasm32-unknown-unknown --no-run`：通过。
- `bun run web:build`：通过，并重新生成 `studio-web-wasm` 绑定与 Vite 产物。
- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run --cwd packages/studio-web test:unit`：19 个测试文件、84 个测试通过。
- `cargo check -p studio-app`：通过，仅保留既有 `app-server-core::watch` dead_code warning。
- `wasm-pack test --headless --chrome crates/studio-web-wasm`：未执行到断言，仍因本机已知 ChromeDriver `http status: 404` / `SIGKILL` 问题失败；该问题已记录在 `docs/known_issues.md`。

独立 review：

- 初次 review 发现两个需要修复的问题：stale preview 解码失败可能污染当前 `preview_error`，`preview_targets` 在 Mesh 成功、失败、超时路径没有清理。
- 复审发现同 target 乱序返回问题：旧 preview 晚到时可能删除较新的 side buffer。
- 最终复审结论：Blocker/High/Medium 均无；新增补充测试覆盖混合有色/无色 sentinel 后，Phase 2 可以进入 Phase 3。

遗留问题：

- 浏览器 wasm 测试仍受本机 ChromeDriver 环境问题影响，暂以 wasm32 编译检查、Rust 状态测试、Web 构建、TypeScript 类型检查和单元测试作为本 Phase 验证证据。
- Playwright 端到端测试将在 Phase 3 服务端切换和最终全量验证中统一运行。
