# DocumentWorkspace 迁移代码审查上下文

## 原始任务

用户要求：

1. 作为 Tab 工作区重构的代码质量 reviewer
2. 只审查，不修改代码
3. 审查范围限定为以下文件：
   - `src/document_session.rs`
   - `src/document_workspace.rs`
   - `src/studio_document.rs`
   - `src/app.rs`
   - `src/welcome.rs`
   - `src/work_area.rs`
   - `src/main.rs`
   - `src/viewer_tab.rs`
   - `src/markdown_tab.rs`
   - `tests/document_workspace_tests.rs`
   - `tests/studio_app_tests.rs`
4. 忽略仓库里与当前范围无关的预存脏改动

## 已知验证结果

- `cargo test --test document_workspace_tests -- --nocapture` 通过
- `cargo test --test studio_app_tests -- --nocapture` 通过
- `cargo build` 通过

## 本轮重点

1. `DocumentWorkspace` 的标题冲突算法和激活/关闭逻辑是否存在边界问题
2. `main / work_area / app` 这一轮接线是否有行为回归风险
3. `welcome` 视图从 tab 迁出后是否还存在逻辑漏洞
4. 当前代码里是否留下明显会拖累后续 Phase 3 轨道组件重构的结构坏味道

## 审查约束

- 不修改业务代码
- findings 优先，按严重程度排序
- 若没有问题，必须明确写“无 findings”
- 结论必须附带文件与行号证据
