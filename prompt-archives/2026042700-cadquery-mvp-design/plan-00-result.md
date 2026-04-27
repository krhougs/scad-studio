# CAD Agent Harness MVP — plan-00 执行结果

## 执行上下文

- 执行分支：`cadquery-mvp-design-execution`
- 执行 worktree：`~/.config/superpowers/worktrees/scad-studio/cadquery-mvp-design-execution`
- 基线验证：`cargo test --workspace` 通过。

## Phase 0a — 规则与文档一致性前置

### 完成情况

- 在 `AGENTS.md` 增加 CadQuery Python 子进程豁免边界，明确仅允许 `budn_cad_runner` 作为 app server 外部 CAD 工具，不允许扩展为项目内任意 Python 辅助脚本。
- 在 `AGENTS.md` 增加 CAD Agent / CadQuery 架构约束，记录 CadQuery 方向、app server 归属、tool call 写入、staging 原子执行、protocol 数据边界和 MVP 5 层 Ref。
- 更新 `docs/cadquery-mvp/ref_components_parts_assemblies.md`，删除 selector / subshape 用户可见 Ref 描述，移除 `candidate_selector_ref` 示例，统一 Selection 示例为 `ref_text`、`owner_ref_text`、`owner_object_kind`。
- 更新 Ref PRD 的 Assembly metadata 要求，明确 child metadata 使用 `ref_text` / `object_kind`；若 CadQuery API 只能稳定保存短字段，它只能作为 Python metadata 输入别名，runner stdout、protocol payload、SelectionRef 一律归一为 `ref_text`。
- 更新 `docs/architecture.md`，把 WebSocket 线格式从旧 UTF-8 JSON 改为当前 Borsh binary frame，并保留 `app-server-protocol` 是唯一线格式来源的约束。
- 更新 `docs/cadquery-mvp/decisions.md`，把 Rig 评估改为 Phase 1 按 crates.io / docs.rs 当前版本验证，不固定旧版本号。
- 更新 `plan-prompt.md`，追加本轮“连续执行完整计划”的用户 prompt 存档。

### 验证记录

- `rg "### 7\\.5 Selector Ref|### 7\\.6 Subshape Ref|@selector\\[|@subshape\\[|candidate_selector_ref|Agent 能把 @selector|feature / selector / subshape|UTF-8 JSON|rig-core v0\\.31|metadata=\\{\"ref\"" docs/cadquery-mvp docs/architecture.md`：无命中。
- `git diff --check`：通过。
- 独立 review subagent：无阻断项。review 观察到一处“nearest selector”表述，我已改为“内部 selector candidate”。

### 遗留问题

- Phase 0a 未发现需要写入 `docs/known_issues.md` 的新问题。
