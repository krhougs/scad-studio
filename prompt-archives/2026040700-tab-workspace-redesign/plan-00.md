# Tab 工作区重设计执行计划

## Context

当前 Studio 的右侧工作区基于 `TabManager + WorkTab` 运行，但实际业务已经突破了这套抽象：

1. `TabManager` 只管理顺序与激活状态，不管理文档身份、标题冲突、空状态、滚动策略
2. `ViewerTab` 的渲染、输入与事件处理依赖主窗口视口和主循环特判，无法被统一 `show()` 契约自然承载
3. `main.rs` 对 `ViewerTab` / `MarkdownTab` 执行大量类型下钻，说明工作区状态没有收拢
4. `WelcomeTab` 与文档标签混用，干扰关闭、恢复、空状态逻辑
5. 左侧 `Chat / Files` 切换和右侧标签没有共享语言，整体 UI 不像一套系统

本轮目标不是继续修补 `TabManager`，而是把 Studio 收敛到“文档工作区 + 精密轨道组件”的结构。

---

## Phase 1：文档工作区核心模型

### 目标

- 引入 `DocumentKey / DocumentKind / DocumentSession / DocumentWorkspace`
- 让右侧工作区状态从 `TabManager` 中脱离，转移到 root crate 的文档工作区
- 用测试先行覆盖单实例、激活切换、关闭邻接、空状态和标题冲突规则

### 前序目标保护

- 保持现有 `ViewerTab` / `MarkdownTab` 的业务逻辑与文件监听能力不被破坏
- 不提前改动 `main.rs` 的事件流，避免在核心模型未稳定前把输入与渲染一起改乱
- 不把左侧 `Chat / Files` 状态硬塞进文档模型

### 输入

- `src/app.rs`
- `src/viewer_tab.rs`
- `src/markdown_tab.rs`
- `src/welcome.rs`
- `tests/studio_app_tests.rs`

### 操作步骤

1. 新增纯状态层模块，例如：
   - `src/document_workspace.rs`
   - `src/document_session.rs`
2. 写失败测试，覆盖：
   - 打开新文档后成为激活项
   - 同一路径重复打开不新增，只激活
   - 关闭激活文档后优先切右侧，再退左侧
   - 打开 workspace 且无文档时不再生成欢迎 tab
   - 同名文件路径冲突时生成短后缀标题
3. 以最小实现让测试通过
4. 将 `StudioApp` 从持有 `TabManager` 改为持有 `DocumentWorkspace`
5. 保留 `ViewerTab` / `MarkdownTab` 现有文件名，优先把它们当作会话实现，避免本 Phase 大规模改名

### 验收标准

- 出现新的文档工作区状态层，且关键行为有测试
- `StudioApp` 不再依赖 `TabManager` 管理右侧文档
- `WelcomeTab` 不再作为文档集合成员存在

---

## Phase 2：工作区接线与事件分发收拢

### 目标

- `main.rs`、`work_area.rs`、`layout.rs` 只与 `DocumentWorkspace` 对话
- 文件打开、文件变更、OpenSCAD 消息、watch error 通过工作区分发
- 当前激活文档渲染与空状态渲染收敛到统一入口

### 前序目标保护

- Phase 1 的文档身份、关闭逻辑和标题规则不能回退
- `Viewer` 的视口渲染、输入路由与 `Markdown` 的热重载行为必须保持
- 左侧文件树、日志面板、工作区打开流程不能被破坏

### 输入

- `src/main.rs`
- `src/work_area.rs`
- `src/layout.rs`
- `src/app.rs`
- `src/viewer_tab.rs`
- `src/markdown_tab.rs`

### 操作步骤

1. 写失败测试或扩展现有测试，覆盖：
   - 打开文件入口调用工作区统一打开逻辑
   - 无文档时渲染空白工作区而非欢迎页
   - 针对 `Viewer` / `Markdown` 的事件由工作区分发
2. 将 `main.rs` 中的“按具体类型下钻处理”收敛为工作区方法调用
3. 将 `work_area.rs` 改为：
   - 顶部文档轨道
   - 中部当前活动文档内容或空状态
4. 保留主循环中的视口判断，但把具体 `Viewer` 获取逻辑收敛到工作区方法
5. 清理已失效的欢迎页与旧 tab API 接口

### 验收标准

- `main.rs` 不再直接依赖 `TabManager`
- `main.rs` 中对 `ViewerTab` / `MarkdownTab` 的分支显著减少，只保留必要的视口与渲染特化入口
- 无文档时显示工作区空状态

---

## Phase 3：Precision Rail 共享组件与左右一致的轨道语言

### 目标

- 用 `scad-ui` 提供新的共享组件，替代旧 `tab_system.rs`
- 右侧实现单行横向滚动的极简文档轨道
- 左侧实现与右侧一致语言的 `Chat / Files` 切换器

### 前序目标保护

- Phase 1 的状态模型和 Phase 2 的事件分发不能再次散落回 UI 层
- 文档标题冲突、关闭逻辑、空状态规则不能在重做 UI 时被绕开
- Viewer 内容区、Markdown 内容区和空状态内容区的统一壳层要保持

### 输入

- `crates/scad-ui/src/lib.rs`
- `crates/scad-ui/src/tab_system.rs`
- `src/work_area.rs`
- `src/left_panel.rs`
- 可能新增 `crates/scad-ui/src/document_tabs.rs`
- 可能新增 `crates/scad-ui/src/panel_switcher.rs`

### 操作步骤

1. 将 `tab_system.rs` 从“状态 + UI 混合管理器”重构为纯表现层组件
2. 设计 `DocumentTabPresentation` 一类的轻量输入模型，由 root crate 传给 UI 组件
3. 实现右侧轨道规则：
   - 单行
   - 横向滚动
   - 极简图标 + 文件名 + 悬停关闭按钮
   - 激活态、悬停态、状态点遵循 `Precision Rail`
4. 实现左侧 `PanelSwitcher`，与右侧共享边框、圆角、留白和状态层级
5. 保证 Studio 的深色主题、内容区边界与文档轨道节奏一致

### 验收标准

- `scad-ui` 中存在共享轨道组件，root crate 不再依赖旧 `TabManager`
- 右侧轨道不换行，溢出时可横向滚动
- 左右两侧具有统一的视觉语言

---

## Phase 4：清理、回归、独立复审

### 目标

- 删除失效接口与无用代码
- 完成必要测试与构建回归
- 记录每个 Phase 的结果，形成可续作的存档

### 前序目标保护

- 不为了清理而改动无关模块
- 不引入新的抽象层级或投机性能力
- 保持当前 Viewer 功能、Markdown 热重载与文件树打开流程

### 输入

- 前 3 个 Phase 所有涉及文件
- `prompt-archives/2026040700-tab-workspace-redesign/plan-00-result.md`

### 操作步骤

1. 逐 Phase 自检：
   - 状态模型
   - 工作区接线
   - 共享轨道组件
2. 运行相关测试与构建
3. 调用独立 subagent 做 review，优先检查：
   - 设计一致性
   - 事件分发回归
   - Viewer / Markdown 行为退化
4. 修复 review findings
5. 更新结果存档

### 验收标准

- 相关测试与构建通过
- 结果存档完整记录每个 Phase 的完成情况与遗留问题
- 没有残留旧 `TabManager` 核心依赖链
