# 已知问题记录

## 2026-04-07 21:39:25: DocumentWorkspace 迁移后仍保留 `DocumentKey` 与 `TabId` 双身份体系

- 来源：对 `src/app.rs`、`src/main.rs`、`src/studio_document.rs`、`src/viewer_tab.rs`、`src/markdown_tab.rs` 的迁移代码审查。
- 原因：文档工作区已经以 `DocumentKey` 作为主身份，但运行时消息分发仍依赖 `legacy_tab_id()`，`ViewerTab`/`MarkdownTab` 继续实现 `WorkTab`，`main.rs` 仍通过 `document_by_legacy_tab_id_mut()` 查找会话。
- 影响范围：
  - Phase 3 若要彻底移除旧 `tab_system`，仍需先清理这条遗留依赖链。
  - 文档身份与运行时消息身份分裂，后续改动容易在 `DocumentKey` 与 `TabId` 之间引入不一致。
  - 现有 `studio_app_tests` 在 `cfg(test)` 下把会话类型替换成 `()`，无法覆盖这条真实运行时代码路径。
- 可能的解法：
  - 让运行时事件直接携带 `DocumentKey` 或由 `DocumentWorkspace` 维护稳定的会话句柄，去掉 `legacy_tab_id()`。
  - 将 `ViewerTab`/`MarkdownTab` 从 `WorkTab` 抽象中彻底剥离，避免继续保留“可被旧 tab 系统驱动”的假接口。
  - 为真实会话分发路径补测试，避免 `cfg(test)` 绕开生产分支。
- 当前处理方式：仅记录为 review finding，作为 Phase 3 前的结构整理输入。

## 2026-04-07 21:39:25: DocumentWorkspace 真实运行时分支缺少自动化测试

- 来源：对 `src/app.rs`、`src/main.rs`、`src/work_area.rs` 的 DocumentWorkspace 迁移代码审查。
- 原因：当前 `studio_app_tests` 只验证通用状态与欢迎态，未覆盖真实文档会话下的打开文件、watch 回调、Viewer/Markdown 分发与工作区轨道交互；生产代码中的真实会话分支仍主要依赖 `cargo build` 做编译级回归。
- 影响范围：
  - 后续调整 `DocumentWorkspace` 接线、文件监听或 Viewer/Markdown 路由时，较难通过自动化测试及时发现行为退化。
  - 真实运行时路径的回归保障弱于纯状态层测试。
- 可能的解法：
  - 为 `main/work_area/app` 增加更贴近运行时的集成测试或最小会话桩，覆盖打开文件、激活切换、watch 消息分发和空状态切换。
  - 在完成 `DocumentKey` / `TabId` 收敛后，补一组面向真实会话分支的回归测试，避免继续依赖 `cfg(test)` 下的轻量替身。
- 当前处理方式：本轮先保留为已知问题；当前仅通过 `cargo build` 与状态层测试保证迁移不破坏编译和核心纯逻辑。

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
