# 执行结果存档：`2026042300-studio-web-feature-parity`

本文件在对应 Phase 执行过程中**实时追加**，与 `plan-00.md` 同步维护。

## 记录要求

- 每个 Phase 完成后，必须补充：完成情况、变更文件范围、验证结果、遗留问题。
- 若某 Phase 发现前序目标被破坏，必须在该 Phase 内修复并记录。

| Phase | 状态 | 摘要 | 遗留问题 |
|-------|------|------|----------|
| 0 | 已完成 | 产出 5 份契约：`plan-00-naming.md` / `plan-00-bridge.md` / `plan-00-toolchain.md` / `plan-00-ownership.md` / `plan-00-smoke.md`。覆盖：命名矩阵、wasm 桥接 API（含 `client_begin_handshake` / `client_destroy` / `ManagedClient` 方法清单）、params 类型与 `app-server-protocol` 映射、错误模型（含 `InvalidHandle` / `NotReady`）、超时 / watch 节流 / reconnect / trait 适配审查、bun-only 工具链、状态归属、S1a–S4 smoke 矩阵。Buddin 设计参考可获取性兜底写入 toolchain §5。 | 无 block；Phase 4 启动前需再次检查 `/Users/krhougs/LocalCodes/buddin/*` 可读性。 |
| 1 | 未开始 |  |  |
| 2 | 未开始 |  |  |
| 3 | 未开始 |  |  |
| 4 | 未开始 |  |  |
| 5 | 未开始 |  |  |
| 6 | 未开始 |  |  |
| 7 | 未开始 |  |  |
| 8 | 未开始 |  |  |

## Phase 0 Review 归档

- 独立 subagent review 发现 4 条 P0 + 5 条 P1 + 3 条 P2。
- P0#1 `client_dispatch_*` 签名冲突：已改为 `Result<RequestId, ClientError>`（bridge §1 / §5）。
- P0#2 首次握手触发时机未定义：已新增 `client_begin_handshake(handle, params)`（bridge §2）。
- P0#3 params / payload 字段缺席：已新增 §1.1 params → `app-server-protocol` 映射表；`WatchEventPayload` schema 写入 §7。
- P0#4 ownership §2 禁止项误伤 TS resolver table：已在 ownership §3 显式允许单一文件 `packages/studio-web/src/wasm-bridge/request-resolvers.ts`；bridge §10 同步放行。
- P1#5 `client_destroy` 语义补齐、`InvalidHandle` 纳入错误枚举。
- P1#6 `WatchEventPayload` 字段固定。
- P1#7 watch registry 归属显式写明在 `ManagedClient`。
- P1#8 `SCAD_STUDIO_WS_URL` URL vs 端口措辞统一。
- P1#9 S1c diff 文件清单明示。
- P2#10 `ManagedClient` 定名，删除备选。
- P2#11 `RequestId` 明确为 `app_server_protocol::RequestId` newtype，跨 JS 用 `bigint`。
- P2#12 本 Phase 0 结果存档已写入本文件（本块即是）。
