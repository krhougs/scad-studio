# CadQuery Web 端到端补缺验收执行结果

## 执行上下文

- 执行日期：2026-04-30
- 执行范围：基于 `prompt-archives/2026042700-cadquery-mvp-design/plan-00.md` 与本轮用户补充要求，对 Web Chat、CadQuery Agent、文件列表预览、Viewer Ref 选择和后续修改链路查漏补缺。

## Phase 0 — 执行前基线检查

### 完成情况

已完成。计划存档无阻塞性 TBD；Web dev server 使用 `CADQUERY_RUNNER_PYTHON=python3.11 BUDN_LLM_CONFIG=llm.toml RUST_LOG=info bun run dev --workspace workspace/cadquery-web-e2e-gapfill --web-port 5173` 启动，页面地址为 `http://localhost:5173/`。

### 验证记录

- 已确认 app server、WebSocket host、Vite、LLM 配置和 CadQuery runner 可用。
- 已确认工作树中存在上一轮 CadQuery runner drain、Agent error 持久化和 Chat history tool 记录相关改动，并纳入本轮验证。

### 遗留问题

无阻塞遗留问题。

## Phase 1 — Web Chat 到 CadQuery 建模链路补缺

### 完成情况

已完成。真实 Web Chat `chat-5` 从“我想做一个放在车里的无线充电板上的给AirPods用的垫子”进入 Agent 流程，最终通过 `cadquery_check_source` 与 `cadquery_execute` 生成 `parts/airpods_charging_pad.py`，导出 `outputs/airpods_charging_pad.step`。

本 Phase 修复和收敛了以下问题：

- OpenAI-compatible SSE `reasoning_content` 未进入前端：新增 `agent.reasoning` 协议事件、LLM reasoning callback、前端 `Thinking`/latest reasoning data part 展示。
- LLM 只返回 reasoning 且没有正文或 tool call 时会中断：tool loop 增加 reasoning-only retry。
- CadQuery contract 错误对 `REFS.features` 形状提示不足：补充 tool schema 与 check/execute 错误示例。
- Web handshake 仍使用旧协议版本：前端改为消费 `CURRENT_PROTOCOL_VERSION`。

### 验证记录

- Web 中 `chat-5` 最终得到 `cq_8e10aaf5f1dbc224`，后续修改得到 `cq_d35f4abfb2d02adb`。
- 运行中 `Thinking` 文案已在页面中按原大小写显示。

### 遗留问题

无阻塞遗留问题。

## Phase 2 — 文件列表打开与预览补缺

### 完成情况

已完成。文件列表原先不能打开 `.py` CadQuery 源文件；本轮将 `.py` 路由为 CadQuery 模型文件，打开时通过现有 `cadquery.preview` 协议重新生成预览 result，再复用现有 CadQuery Viewer。

### 验证记录

- 浏览器回归从 Files 面板打开 `parts/airpods_charging_pad.py`，文件类型显示 `PY`。
- 预览状态为 `cadquery ready`。
- Inspector 显示 `804 VERTS · 804 IDX`，尺寸 `92.00 × 72.00 × 5.00 MM`。

### 遗留问题

无阻塞遗留问题。`outputs/*.step` 当前作为导出文件保留，Web 端可视化入口是对应 CadQuery `.py` 源模型。

## Phase 3 — Viewer Ref 选择与后续修改入口补缺

### 完成情况

已完成。CadQuery Viewer 中可交互选择 Ref，selection 通过 protocol 更新到共享状态，并进入 Chat 输入区的 context pill。随后基于当前选中 Ref 发起后续修改请求，Agent 读取当前模型并重新执行 CadQuery。

### 验证记录

- 浏览器选择结果：`FACE @face[airpods_charging_pad:f_22]`。
- Chat context pill：`@face[airpods_charging_pad:f_22]`。
- 后续修改请求把中央凹槽加深到 `2.6mm`，`parts/airpods_charging_pad.py` 更新为 `recess.extrude(3.0).edges("|Z").fillet(4).translate((0, 0, 2.4))`。
- 新结果：`cq_d35f4abfb2d02adb`，导出 `outputs/airpods_charging_pad.step`，拓扑仍为 `23` 面、`60` 边、`40` 顶点。

### 遗留问题

无阻塞遗留问题。

## Phase 4 — 回归验证与结果归档

### 完成情况

已完成。相关 Rust、TypeScript、Web 单元测试、协议构建和真实浏览器回归均已执行。

### 验证记录

- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run --cwd packages/studio-web test:unit tests/unit/workbench-wiring.test.ts tests/unit/protocol-package-import.test.ts tests/unit/chat-runtime.test.ts tests/unit/chat-zone.test.tsx tests/unit/tab-kind.test.ts tests/unit/file-kind.test.ts tests/unit/cadquery-source-preview.test.tsx tests/unit/cadquery-viewer.test.tsx`：77 个测试通过；存在既有 React `act(...)` warning。
- `cargo fmt --check`：通过。
- `cargo test -p app-server-protocol --test borsh_payload_roundtrip_tests agent_push_events_and_busy_error_roundtrip -- --nocapture`：通过。
- `cargo test -p app-server-core --test llm_tests read_sse_stream_forwards_reasoning_deltas_to_callback -- --nocapture`：通过。
- `cargo test -p app-server-core --test agent_tool_tests run_tool_loop_forwards_reasoning_callback -- --nocapture`：通过。
- `cargo test -p app-server-core --test agent_tool_tests run_tool_loop_retries_reasoning_only_response -- --nocapture`：通过。
- `cargo test -p app-server-core --test agent_tool_tests workspace_tool_executor_cadquery_check_source_explains_refs_shape -- --nocapture`：通过。
- `cargo test -p app-server-core --test agent_tool_tests workspace_tool_executor_cadquery_execute_explains_missing_refs_shape -- --nocapture`：通过。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests shared_dispatcher_roundtrips_handshake_workspace_file_and_preview -- --nocapture`：通过。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests dispatcher_persists_agent_error_message_when_llm_is_unavailable -- --nocapture`：通过。
- `bun run protocol:build`：通过。
- `bun run check:wasm-bindgen`：通过。
- `git diff --check`：通过。
- 浏览器截图证据：
  - `/tmp/budn-file-list-cadquery-preview.png`
  - `/tmp/budn-ref-selection-context.png`
  - `/tmp/budn-ref-followup-agent.png`
  - `/tmp/budn-final-web-regression.png`
  - `/tmp/budn-thinking-label.png`

### 遗留问题

无阻塞遗留问题。Web dev server 保持运行，便于继续手工检查。
