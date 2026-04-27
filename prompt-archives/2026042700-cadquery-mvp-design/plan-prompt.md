# Plan prompt 存档

本目录对应任务：**CAD Agent Harness MVP 完整设计**——CadQuery 替代 OpenSCAD，构建 Agent 协作式 CAD 设计系统。

## 背景

产品方向转向 CadQuery Agent 协作式 CAD 设计。MVP 目标是跑通完整流程：

```
多 Chat 讨论 → Agent 生成 CAD Plan → 用户确认 → CadQuery 生成模型
→ Viewer 查看 → 精细选择 face/edge/vertex → Agent 基于选择继续修改
```

## 相关文档

- `docs/cadquery-mvp/init.md` — 产品 MVP 定义（PRD）
- `docs/cadquery-mvp/ref_components_parts_assemblies.md` — Ref 系统与对象关系
- `docs/cadquery-mvp/decisions.md` — 方向决策记录（14 项已确认决策）

## 用户原始请求（按时间顺序）

1. **基于 PRD 和 Ref 文档，生成完整技术设计方案**
   - 输入为 `docs/cadquery-mvp/init.md` 和 `docs/cadquery-mvp/ref_components_parts_assemblies.md`。
   - 需要覆盖架构、Python 执行框架、Ref 系统、Agent 工具调用、Rust 后端模块、协议扩展、Viewer 增强、实施分期。

2. **Ref 层级决策**
   - 用户确认：MVP 实现 5 层（component/part/assembly、instance、feature、face/edge/vertex），砍掉 selector 和 subshape 层，后续按需加回。

3. **Python 环境分发决策**
   - 用户确认：MVP 手动安装，分发策略留到产品化阶段。

4. **多项技术决策一次性确认**
   - mesh wire format：基于现有 Borsh 协议扩展，不另起炉灶。
   - 前端架构：基于现有框架增量改造，保持当前 UI，不大改架构。
   - 并发模型：限制同时只有一个 running agent session。
   - Chat 存储格式：JSONL。

5. **三轮 Codex 独立审查**
   - Round 1：20 findings，13 fixed，6 deferred，1 false positive。
   - Round 2：13 findings，8 fixed in plan，4 doc sync。
   - Round 3：8 findings，全部 fixed。
   - 审查覆盖：wire envelope 映射、协议版本策略、dispatcher 异步化、selection payload、EdgeGroup/VertexPoint 定义、renderer 路径修正、chat-zone placeholder、残留 candidate_selector_ref。

6. **文档同步**
   - 用户要求同步 decisions.md、init.md、ref PRD 与 plan 决策保持一致，但不写代码。
   - 当时记录为已完成：decisions.md 更新为 14 项决策、init.md 改 JSONL 格式、ref PRD 标注 MVP 范围并修正依赖方向。
   - 后续工程审查发现 Ref PRD、decisions.md 和 architecture 文档仍需 Phase 0a 复核修正。

7. **存档 plan**
   - 用户要求按 AGENTS.md 规范存档到 prompt-archives，不开始实施。

8. **工程审查后修订当前计划**
   - 用户要求按照 review 结果修正 `plan-00.md`。
   - 修订重点：Ref PRD 同步不实、CadQuery mesh 重载荷不能直接进入 JS `ClientEvent`、缺少 `studio-common` 状态归属、Phase 计划结构不足、CadQuery Python 豁免需前置、架构文档 wire format 过期、Rig 固定版本过期。

9. **连续执行完整计划**
   - 用户要求按照 `prompt-archives/2026042700-cadquery-mvp-design/plan-00.md` 和项目规范执行每个 Phase。
   - 每个 Phase 必须完成实现、独立 review、回归验证、修复、结果记录和 commit。
   - 执行过程中不得停下来等待用户意见，直到整个 plan 完成；只有真正阻塞且无法自行决策的问题才暂停。

10. **继续连续执行**
    - 用户再次要求继续执行，并重申按照项目规范对每个 Phase 完成实现、独立 review、收敛循环。
    - 中途不得停下来征求意见，直到整个 plan 完成执行。

## 注意事项

- 本 plan 只是设计文档，尚未开始任何代码实施。
- 实施时需要先读取本 plan 和 `docs/cadquery-mvp/` 下的三个文档获取完整上下文。
- Plan 经过三轮独立 Codex 审查和一次工程审查；工程审查 findings 已写入 `plan-00.md`，实施时必须先完成 Phase 0a。
- 已知风险列表见 plan §9 末尾，接受在实施时解决。
- CadQuery Python 子进程豁免项目 Python 约束（视为外部工具，同 OpenSCAD CLI），需在 Phase 0a 更新 AGENTS.md 后才能开始产品代码实施。
