# Plan 00 Review Report

## Review 对象

- Plan 目录：`prompt-archives/2026051801-agent-quality-and-selection-ux`
- Plan 文件：`plan-00.md`
- 执行结果：`plan-00-result.md`
- Review 时间：2026-05-19
- Review 目标：检查已完成实现是否满足 `plan-00.md` 的目标、强制约束、各 Phase 验收标准与结果归档声明。

## 结论

当前实现已有实质进展，但不能按 `plan-00-result.md` 标记为完整验收通过。主要缺口集中在三个方面：

1. 统一选择状态的关键竞态没有解决：删除 Chat pill 后立即发送消息，Agent 仍可能读取旧 selection snapshot。
2. Phase 3 E2E 和 Agent 功能性验收没有达到 plan 中定义的验收强度。
3. 基准评估记录不足：缺少真实 LLM rubric 评估结果，且 benchmark 结果只保存最终回复预览，不足以支撑第三方评估。

建议先修复阻塞项，再重新运行分层验证，并更新 `plan-00-result.md` 的完成状态。

## 审计清单

| Plan 要求 | 当前证据 | Review 结论 |
|---|---|---|
| Phase 0：Agent skill 注入机制可工作，Plan mode 不注入，system prompt 不变，6 个 CadQuery tool description 增强 | `agent_skill_tests` 10 个测试通过；`docs/cadquery-mvp/agent-system-prompt.md` 未出现在当前 diff 中；`registry.rs` 有 description 修改 | 基础机制有测试覆盖，未发现阻塞问题 |
| Phase 1-D：`bun run bench` 跑 5 个 benchmark，并记录用户输入、工具轨迹、工具结果、最终回复 | `benchmark_tests.rs` 有 5 个 scenario runner；当前普通测试中 benchmark ignored；`benchmarks/results` 不存在 | 框架存在，但没有真实执行结果；最终回复只保存 500 字预览 |
| Phase 2-A：删除 pill、Viewer、Ref Tree 统一写回 app server selection snapshot | `chat-zone.tsx` 删除 pill 调用 `dispatchSelectionUpdate`；host 使用 `selection_snapshot` 构造 `AgentTurnInput` | 状态源方向正确，但删除 pill 后发送消息存在未等待 selection.update 的竞态 |
| Phase 2-A：Agent turn preamble 只有一个 `Current selection` section | `llm_tests.rs` 覆盖统一 selection section；`context_refs` 已从主路径删除 | 该项有直接测试覆盖 |
| Phase 2-B：CadQuery 工具事件显示专用卡片，失败默认显示友好错误，traceback 放展开区 | `chat-messages.test.tsx` 覆盖 success、running、error、非 CadQuery fallback | 组件级覆盖基本满足，但 E2E 未验证真实事件流 |
| Phase 2-C：选择 Dock 分组、计数、清除按钮 | `cadquery-viewer.test.tsx` 覆盖分组、计数、清除按钮 dispatch | 组件级覆盖基本满足 |
| Phase 3：第三层 3 个端到端场景全部通过 | `selection-agent-cycle.spec.ts` 有 4 个 spec，但使用条件分支和手工 DOM 插入，没有覆盖 plan 中 3 个完整场景 | 不满足 |
| Agent 功能性验收：全部有第三方 LLM rubric 评估记录 | 只有 `scripts/run_bench_rubric.ts`；没有 `benchmarks/results/*-rubric.json` | 不满足 |

## 发现的问题

### 1. 删除 pill 后立即发送消息仍可能把旧选择传给 Agent

严重级别：高

证据：

- Plan 明确要求发送 Agent turn 前必须确保最近一次选择更新已被 app server 接受，避免删除 pill 后立刻发送时 Agent 读取旧快照：`plan-00.md:210`。
- `chat-zone.tsx:216-228` 中 `removePillRef.current` 调用 `client.dispatchSelectionUpdate(...)`，但没有 `await`，也没有把 pending selection update 纳入 send 前置条件。
- `chat-actions.ts:191-197` 中发送消息会直接 `dispatchAgentStartTurn(...)`。
- `dispatcher.rs:1003-1010` 启动 Agent worker 时复制当前 `selection_snapshot`；如果 selection.update 还没处理完成，Agent 会读取旧值。

影响：

- `plan-00.md` Phase 2-A 验收标准第 7 条不成立。
- 用户删除 pill 后立即发送“基于当前选择修改”时，Agent 可能基于已删除 ref 生成或修改模型。

建议修复：

- 在 Chat controller 中维护最近一次 `dispatchSelectionUpdate` Promise，发送消息前等待其完成。
- 或将 pill 删除动作做成 async，并在 Composer send 路径中统一 flush pending selection update。
- 增加单元测试：模拟 `dispatchSelectionUpdate` 延迟返回，点击删除 pill 后立即发送，断言 `dispatchAgentStartTurn` 在 selection update resolve 之后发生。

### 2. Phase 3 E2E 不是 plan 要求的端到端验证

严重级别：高

证据：

- Plan 要求第三层 3 个场景全部通过：选择到 Chat 到 Agent 循环、Agent CadQuery 执行循环、Plan 模式循环：`plan-00.md:320-344`、`plan-00.md:356`。
- `selection-agent-cycle.spec.ts:80-85` 中 context pill 测试只有当 pill bar 可见才断言，并且 `count >= 0` 恒成立；没有验证选择注入后 pill 必然出现。
- `selection-agent-cycle.spec.ts:94-108` 中 CadQuery 工具卡片测试直接向 DOM 插入 `.cadquery-tool-card`，不是通过真实 `agent.tool_start` / `agent.tool_result` 事件触发。
- `selection-agent-cycle.spec.ts:123-130` 中清除按钮测试只有按钮可见且 recorder 有 command 时才断言；按钮不可见或 command 为空也不会失败。

影响：

- 当前 Playwright 测试可能在关键 UI 或协议链路断开时仍通过。
- `plan-00-result.md:186` 将“4 specs”记录为 E2E 覆盖，证据强度不足。

建议修复：

- 移除 E2E 中的“可见才断言”分支，改为先通过真实可控入口建立前置状态，再强制断言目标 UI 和协议命令。
- 工具卡片 E2E 应通过真实或测试 harness 注入的 `agent.tool_start` / `agent.tool_result` 事件进入 React 状态，不应手工插入 DOM。
- 至少补齐 plan 中 3 个场景的可执行脚本；如真实 LLM 环境不可用，结果文档应标记为未完成或受阻，而不是完成。

### 3. Agent 功能性验收缺少第三方 LLM rubric 评估记录

严重级别：中

证据：

- Plan 要求功能性场景全部有第三方 LLM rubric 评估记录：`plan-00.md:350`、`plan-00.md:357`。
- 当前 `benchmarks/` 目录只有 5 个 scenario JSON，没有 `benchmarks/results` 目录，也没有 `*-rubric.json`。
- `plan-00-result.md:187` 只声明 `bun run bench:rubric` 脚本就绪，脚本就绪不能替代评估记录。

影响：

- Phase 1-A/B/C 的真实 LLM 行为未闭合验证。
- 无法判断倒角失败修复、默认值声明、brief 输出是否由真实 Agent 达成。

建议修复：

- 在具备本机 provider 配置和 LLM API key 的环境中运行 `bun run bench` 与 `bun run bench:rubric`。
- 将生成的 result/rubric 文件纳入归档或在结果文档中记录不可运行原因、已尝试命令和恢复条件。
- 不得读取、打印或归档 `agents.toml` 正文。

### 4. Benchmark 结果只保存最终回复预览，rubric 输入不完整

严重级别：中

证据：

- Plan 要求记录最终回复：`plan-00.md:350`。
- `benchmark_tests.rs:465-473` 输出 `agent_text_length` 和 `agent_text_preview`，其中 preview 通过 `truncate(&r.agent_text, 500)` 截断。
- `run_bench_rubric.ts:34-42` 的 `BenchResult` 类型只读取 `agent_text_preview`。
- `run_bench_rubric.ts:67-70` 只把 preview 传给第三方 LLM。

影响：

- 第三方 LLM 可能看不到完整假设、限制说明、失败说明或后半段幻觉内容。
- rubric 无法可靠判断“是否如实暴露限制、是否出现幻觉或无意义工作”。

建议修复：

- benchmark result JSON 增加完整 `agent_text` 字段。
- rubric 脚本优先读取完整 `agent_text`，只在展示摘要时使用 preview。
- 如担心文件过大，可以保留完整文本并额外生成摘要字段，不要只保存摘要。

### 5. `exports_generated` 被声明但未参与 pass/fail

严重级别：中

证据：

- 5 个 benchmark scenario 都设置 `"exports_generated": true`。
- `BenchmarkCriteria.exports_generated` 在 `benchmark_tests.rs:237-240` 标记为 `#[allow(dead_code)]`。
- `evaluate()` 只检查 `cadquery_execute_success`、`brief_expected`、`max_dry_run_attempts`：`benchmark_tests.rs:345-369`。

影响：

- 即使 Agent 没有生成导出文件，benchmark 也可能通过。
- Phase 1-D 中“导出文件生成”评分维度没有落实。

建议修复：

- 在 `evaluate()` 中检查 `cadquery_execute` 成功结果是否包含 exports。
- 如要验证文件确实存在，需要在 benchmark runtime commit 后记录导出路径，并检查 workspace 中对应文件。

## 已执行验证

本次 review 期间执行了以下验证命令：

```bash
bun run --cwd packages/studio-web test:unit -- chat-zone.test.tsx chat-messages.test.tsx cadquery-viewer.test.tsx
```

结果：

- 3 个 test file 通过
- 67 个测试通过
- 存在 React `act(...)` warning，未导致失败

```bash
cargo test -p app-server-core --test agent_skill_tests --test llm_tests --test benchmark_tests
```

结果：

- `agent_skill_tests`：10 passed
- `llm_tests`：68 passed，1 ignored
- `benchmark_tests`：0 passed，1 ignored
- live LLM / CadQuery benchmark 未执行

```bash
cargo test -p app-server-protocol --test borsh_payload_roundtrip_tests
```

结果：

- 18 passed

## 建议修复顺序

1. 修复 selection update 与 Agent start turn 的时序问题，并补充竞态单元测试。
2. 修正 Playwright E2E，让测试通过真实状态和事件流验证 plan 中 3 个场景。
3. 修正 benchmark 输出：保存完整最终回复，落实 `exports_generated` 判定。
4. 在完整环境中运行 `bun run bench` 和 `bun run bench:rubric`，归档结果或明确记录环境阻塞。
5. 更新 `plan-00-result.md`：把当前未满足的验收项从“完成”调整为“未完成 / 受阻 / 待验证”，避免结果文档误导后续执行者。
