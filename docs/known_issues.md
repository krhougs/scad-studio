# 已知问题记录

## 2026-04-02 16:47:56: 本地环境缺少可验证 3MF 彩色预览的 OpenSCAD CLI / Nightly

- 来源：为 3MF 彩色预览计划检查本机 OpenSCAD 环境时，执行 `command -v openscad` 与读取 `OPENSCAD_PATH`，结果均为空。
- 原因：当前工作机未安装可直接调用的 OpenSCAD CLI，因此无法确认是否具备支持彩色 3MF 预览的 Nightly 能力。
- 影响范围：
  - 无法在本机完成 `scad -> OpenSCAD 3MF -> 彩色预览` 的端到端闭环验证。
  - 后续实现阶段只能先依赖 3MF fixture、单元测试和用户环境联调来验证颜色解析与渲染。
- 可能的解法：
  - 在执行阶段安装 OpenSCAD Nightly，并通过 `OPENSCAD_PATH` 或设置窗口显式指向该版本。
  - 在仓库中加入最小彩色 3MF fixture，用于脱离 OpenSCAD 环境验证解析与渲染链路。
  - 将“Nightly 环境下的人工联调”列为独立验收项，而不是与纯单元测试混在一起。
- 当前处理方式：已补 `tests/three_mf_tests.rs`、`tests/mesh_tests.rs`、`tests/pipeline_tests.rs` 等回归测试，自动化验证覆盖 3MF 解析与颜色渲染协议；在具备 Nightly 的环境前，不宣称完成 `scad -> OpenSCAD 3MF -> 彩色预览` 的端到端人工验收。

## 2026-04-01 13:20: feature-roadmap 与现行 plan 在 3MF 解析范围上不一致

- 来源：对照 [docs/feature-roadmap.md](/Users/krhougs/.config/superpowers/worktrees/scad-studio/codex-full-features/docs/feature-roadmap.md) 与 [plan-00.md](/Users/krhougs/.config/superpowers/worktrees/scad-studio/codex-full-features/prompt-archives/2026033101-full-features/plan-00.md)。
- 原因：roadmap 仍包含“3MF 文件解析（支持颜色信息）”，但当前 plan 仅覆盖 3MF 导出，不包含 3MF 导入解析。
- 影响范围：即使按现行 plan 完成所有 Phase，也无法直接把 roadmap 全部未完成项勾选为已完成。
- 可能的解法：
  - 单独补一轮 3MF 解析计划，明确是否需要颜色贴图、零件层级和 ZIP 容器读取。
  - 或者回写 roadmap/plan，明确当前版本仅支持 3MF 导出，不支持导入解析。
- 当前处理方式：本轮已实现 3MF 预览解析并同步更新 `docs/feature-roadmap.md`，该问题不再阻塞后续开发判断。
