# DocumentWorkspace 迁移代码审查计划

## Context

本轮不是实现任务，而是对 DocumentWorkspace 迁移后的当前实现做定向代码审查。已知构建与两组测试通过，因此审查重点不在“是否能编译”，而在“是否存在边界 bug、行为回归、结构退化与测试盲区”。

---

## Phase 1：状态模型与文档身份审查

### 目标

- 核对 `DocumentSession / DocumentWorkspace / StudioApp` 的文档身份建模是否稳定
- 审查标题冲突、单实例、激活切换、关闭邻接逻辑
- 明确当前测试是否覆盖关键边界

### 前序目标保护

- 不把审查扩散到用户未指定范围之外
- 不因为现有测试通过就默认逻辑正确
- 保持“只审查不改代码”的边界

### 输入

- `src/document_session.rs`
- `src/document_workspace.rs`
- `src/studio_document.rs`
- `src/app.rs`
- `tests/document_workspace_tests.rs`
- `tests/studio_app_tests.rs`

### 操作步骤

1. 阅读数据结构与关键方法
2. 逐个推演打开、重复打开、切换激活、关闭激活项、关闭非激活项、路径冲突与标题冲突
3. 对照测试，标出未覆盖边界

### 验收标准

- 能明确给出状态模型层的 bug 风险、回归风险或测试缺口
- 若无问题，也能说明为何现有测试足以支撑结论

---

## Phase 2：工作区接线与事件流审查

### 目标

- 审查 `main / work_area / welcome` 的接线是否保留原有行为
- 检查 viewer / markdown / 空状态 / welcome 状态切换是否存在漏洞

### 前序目标保护

- Phase 1 已确认的状态模型边界不能在 UI 接线层被绕开
- 不把 renderer、watcher、快捷键等副作用路径遗漏

### 输入

- `src/main.rs`
- `src/work_area.rs`
- `src/welcome.rs`
- `src/viewer_tab.rs`
- `src/markdown_tab.rs`

### 操作步骤

1. 核对文件打开、消息分发、watch 回调、重绘同步路径
2. 推演“无 workspace / 有 workspace 无文档 / 有激活 viewer / 有激活 markdown / 文档关闭后回退”几条主路径
3. 标记仍然耦合旧 tab 抽象的残留点

### 验收标准

- 能明确指出接线层的行为回归风险或结构坏味道
- 能判断 welcome 从 tab 迁出后是否还有状态漏洞

---

## Phase 3：结论整理

### 目标

- 产出按严重程度排序的 findings
- 明确 open questions、测试缺口与后续 Phase 3 风险点

### 前序目标保护

- 结论必须基于代码证据，不基于猜测
- findings 的严重程度要与真实影响一致，避免夸大

### 输入

- 前两阶段审查记录

### 操作步骤

1. 汇总可复现或高概率的风险点
2. 绑定文件与行号
3. 写入结果存档并准备最终答复

### 验收标准

- 最终输出可直接给实现者行动
- 若存在测试盲区，需明确指出缺少哪类测试
