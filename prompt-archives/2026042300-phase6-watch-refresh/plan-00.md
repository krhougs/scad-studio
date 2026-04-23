# Phase 6 最小 watch 刷新闭环 — 执行计划

## Context

- 归档计划 `prompt-archives/2026042200-studio-app-server-unification/plan-00.md` 的 Phase 6 已要求：watch 事件后的客户端刷新逻辑应收敛到 `studio-common`，网页端与桌面端共享同一份处理语义。
- 当前 `studio-web` 已具备 WebSocket transport 的 `watch.subscribe` / `watch.unsubscribe` 能力，但 UI 还没有真正消费 watch 推送。
- 本轮用户要求仅补最小闭环：当前目录/root 的 watch 生命周期 + `WatchChanged` 到 `workspace.list` 刷新，不改协议、不改桌面、不扩展状态机。

## Goal

- 在 `studio-common` 增加一个纯 Rust 的共享 watch 生命周期 helper，负责当前目录 watch 的目标、订阅、退订与匹配刷新判断；`studio-web` 接入它，在浏览器 smoke 中证明文件系统变化能出现在文件列表里。

## 非目标

- 不改 protocol 类型。
- 不改 `studio-app` 现有 watch 逻辑。
- 不做 debounce 框架或更广泛的 workspace 状态重构。
- 不更新 `docs/*`。

## Phase 1：锁定最小接线点与测试入口

### 前序目标保护

- 保护 archived Phase 6 已完成的目录树、当前目录列表与预览 smoke；不为了 watch 接线破坏现有 `workspace.current -> workspace.list -> preview.request` 浏览器路径。

### 输入

- `crates/studio-web/src/app.rs`
- `crates/studio-common/src/app_server_client.rs`
- `crates/studio-app/src/protocol_client.rs`
- `crates/studio-web/tests/browser_smoke.rs`
- `tests/studio_web_smoke.sh`

### 操作步骤

1. 确认 `studio-web` 当前目录切换与 `workspace.list` 的实际落点。
2. 确认 watch 协议类型、ack / push 形态与 `AppServerClient` 已有接口。
3. 确认现有 browser smoke 与 shell smoke 的运行方式，收敛一个 repo-local 的 watch 刷新证明路径。

### 验收标准

- 明确 helper 只需要处理：目标目录变更、订阅发送、退订发送、ack 推进、匹配 push 刷新。
- 明确 browser smoke 可以用 repo-local 临时文件变更证明刷新，无需引入额外外部服务。

## Phase 2：TDD 补失败测试

### 前序目标保护

- 保护 Phase 1 确认的最小边界；测试只覆盖本轮新增 watch 行为，不顺带改写现有 preview/tree smoke 断言。

### 输入

- `crates/studio-common/tests/`
- `crates/studio-web/tests/`
- `tests/studio_web_smoke.sh`

### 操作步骤

1. 为 `studio-common` 新 helper 增加单元测试，先覆盖：
   - 初次进入 root 时发起 subscribe；
   - 已有活动订阅时切换目录，会先退订旧订阅、再订阅新目录；
   - 匹配 `WatchChanged` 时给出当前目录刷新目标，非匹配订阅不刷新。
2. 为浏览器 smoke 增加 watch 驱动刷新路径：测试在初始列表加载后等待一个临时文件因为 watch 刷新而出现。
3. 先运行上述测试，确认失败原因是“helper / web 接线尚未实现”，而不是测试书写错误。

### 验收标准

- `cargo test -p studio-common --test <watch helper test>` 先红。
- `tests/studio_web_smoke.sh` 中新增的 watch smoke 路径先红，且失败点是列表未刷新到新文件。

## Phase 3：最小实现与验证

### 前序目标保护

- 保护 Phase 2 已写测试与原有 browser smoke；不为让 smoke 通过而把 watch 逻辑写进浏览器专属代码中。

### 输入

- `crates/studio-common/src/`
- `crates/studio-web/src/app.rs`
- `crates/studio-web/tests/`
- `tests/studio_web_smoke.sh`

### 操作步骤

1. 在 `studio-common` 实现纯 Rust watch 生命周期 helper，并导出给客户端壳层使用。
2. 在 `studio-web` 中接入 helper：
   - `workspace.list` 成功后驱动 watch 目标；
   - `WatchSubscribed` / `WatchUnsubscribed` response 推进生命周期；
   - `WatchChanged` push 命中当前订阅时，重新请求当前目录列表。
3. 更新 shell smoke，安全创建 / 清理 repo-local 临时文件，单独执行 watch smoke。
4. 运行变更文件诊断、helper 单测与浏览器 smoke，修到全部通过。

### 验收标准

- `studio-common` helper 纯 Rust、无浏览器 API。
- `studio-web` 真正发送 subscribe / unsubscribe，而不是仅更新状态文本。
- 机械化 smoke 能证明 root 或当前目录中文件变化会反映到浏览器文件列表。
