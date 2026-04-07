# Tab 工作区重设计执行上下文

## 原始任务

用户要求：

1. 系统性评估当前 tab 系统的代码
2. 重新设计 Tab 系统
3. 直接按照存档 plan 执行

## 已完成的前置评估

已阅读并分析以下核心实现：

- `crates/scad-ui/src/tab_system.rs`
- `crates/scad-ui/tests/tab_manager_tests.rs`
- `src/app.rs`
- `src/work_area.rs`
- `src/layout.rs`
- `src/main.rs`
- `src/viewer_tab.rs`
- `src/markdown_tab.rs`
- `src/welcome.rs`
- `tests/studio_app_tests.rs`
- `prompt-archives/2026040600-studio-workspace-ui/*`
- `prompt-archives/2026040601-studio-ui-overhaul/*`

已确认当前主要问题不是视觉样式，而是抽象边界失真：

- `TabManager` 只表达顺序与激活状态，无法表达文档身份、空状态、溢出策略与事件分发
- `ViewerTab` 已经不能被 `WorkTab::show()` 这种通用契约完整承载
- `main.rs`、`work_area.rs`、`app.rs` 对具体 tab 类型存在大量特判
- `WelcomeTab` 混入文档标签系统，污染关闭与空状态逻辑
- 左侧 `Chat / Files` 与右侧文档标签在视觉语言上断裂

## 用户确认的设计决策

### 产品范围

- 采用“简单工作区型”，不做 split view / docking / pane tree
- 本轮同时重设计右侧文档标签与左侧 `Chat / Files` 切换器

### 右侧文档工作区

- Tab 只承载文档，不承载欢迎页
- 欢迎页从 tab 系统移除，已打开 workspace 且无文档时显示轻量空白工作区
- 同一路径文件在同一窗口只允许单实例
- 标签默认只显示文件名，遇到同名冲突时给冲突项补最短可辨识路径后缀
- 标签视觉采用极简层级：文件类型图标 + 文件名 + 悬停关闭按钮 + 小状态点
- 溢出策略为单行横向滚动，不允许自动换行

### 视觉方向

- 使用 `design-taste-frontend`
- 使用 `high-end-visual-design`
- 用户确认视觉方向为 `A Precision Rail`
- 关键词：安静、精密、克制、弱装饰、稳定轨道

## 执行注意事项

- 当前工作树存在未提交改动，且部分改动已触达 Studio UI / Viewer 相关区域；实现时必须基于现状代码演进，不可重置工作树
- 必须遵守根 `AGENTS.md` 中的 Plan Mode 约束：
  - 先存档再干活
  - plan 按 Phase 拆分
  - 每个 Phase 需要写入 `plan-00-result.md`
  - 每个 Phase 完成后执行 review 与回归
- 由于本任务涉及行为重构，必须坚持 TDD：先写失败测试，再写最小实现

## 本轮目标

将当前“泛化 tab 容器”重构为“文档工作区 + 共享精密轨道组件”：

- Root crate 引入 `DocumentWorkspace`
- 移除 `WorkTab + Any downcast` 作为右侧工作区的核心模型
- 使用枚举式文档会话承载 `Viewer` / `Markdown`
- `scad-ui` 提供共享轨道组件，而不再持有右侧文档状态
- 统一左右两侧的轨道视觉语言
