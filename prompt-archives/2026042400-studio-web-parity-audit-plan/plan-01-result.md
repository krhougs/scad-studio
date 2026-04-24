# Studio Web Layout 与 OpenSCAD Fallback 修复执行记录

## Phase 1：补充回归用例，先复现当前问题

### 完成情况

- 已补充 OpenSCAD fallback、文件类型、左栏路由、标题栏、Markdown 安全、Workbench 布局、Canvas status bar、Settings 左栏、Log 左栏相关回归用例。
- 已按独立审查反馈补齐三种桌面 viewport、Log tab 切换、Markdown iframe / 内联事件 / 危险图片 URL / Mermaid SVG 危险链接、大尺寸列布局、`document.title`、多个 inspector section 展开收起、React Router search params 初始恢复与 history 覆盖。
- 独立复核结论：Blocker 无，Important 无。

### 验证记录

- `cargo test -p app-server-core --test openscad_command_tests`：预期失败，失败点指向 OpenSCAD missing configured/env fallback、auto fallback、macOS `.app` bundle 解析、通用错误文案。
- `bun run --cwd packages/studio-web test:unit -- file-kind left-panel-routing document-title markdown-preview-security`：预期失败，失败点为目标生产模块尚未实现。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts --list`：列出 8 个测试。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts --list`：列出 9 个测试。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/markdown-preview.spec.ts --list`：列出 1 个测试。

### 变更摘要

- 修改 `crates/app-server-core/tests/openscad_command_tests.rs`。
- 新增 `packages/studio-web/tests/unit/file-kind.test.ts`。
- 新增 `packages/studio-web/tests/unit/left-panel-routing.test.ts`。
- 新增 `packages/studio-web/tests/unit/document-title.test.ts`。
- 新增 `packages/studio-web/tests/unit/markdown-preview-security.test.ts`。
- 新增 `packages/studio-web/tests/playwright/workbench-layout.spec.ts`。
- 新增 `packages/studio-web/tests/playwright/markdown-preview.spec.ts`。
- 扩展 `packages/studio-web/tests/playwright/canvas-interaction.spec.ts`。
- 调整 `packages/studio-web/tests/playwright/config-settings.spec.ts`、`export-slicer.spec.ts`、`browser-smoke.spec.ts`、`browser-watch-smoke.spec.ts`、`parameters-presets.spec.ts` 的入口假设。

### 遗留问题

- 无阶段内遗留阻塞项。后续 Phase 需要实现生产代码使红灯用例通过。

## Phase 2：修复 OpenSCAD CLI 路径检测与 fallback

### 完成情况

- 已把 OpenSCAD detection 改为配置路径、`OPENSCAD_PATH`、`PATH`、平台默认路径统一验证后再选中。
- 无效配置路径不会阻止后续候选；裸命令名 `openscad` / `openscad.exe` 只通过 `PATH` 展开为实际文件路径，不再保留原始裸命令候选。
- macOS `.app` bundle 路径会展开到 `Contents/MacOS/OpenSCAD`。
- `OPENSCAD_PATH` 改用 `env::var_os` 读取，避免非 UTF-8 路径被忽略。
- Preview 与 Export 继续共用 `detect_openscad_path`，没有在 Web 或桌面壳层新增检测逻辑。
- 独立审查指出环境变量测试存在并发风险，已用进程内 `Mutex` 保护相关测试。

### 验证记录

- `rustfmt --edition 2024 --check crates/app-server-core/src/preview.rs crates/app-server-core/tests/openscad_command_tests.rs`：通过。
- `cargo test -p app-server-core --test openscad_command_tests`：11 passed。
- `cargo check --workspace`：通过，仅有既有 dead_code warning。

### 变更摘要

- 修改 `crates/app-server-core/src/preview.rs`。
- 修改 `crates/app-server-core/tests/openscad_command_tests.rs`。

### 遗留问题

- 无阶段内遗留阻塞项。

## Phase 3：重组 Workbench 壳层，左栏承载 Files、Settings 与 Log

### 完成情况

- 已新增 `LeftPanel`、`FilesPanel`、`SettingsPanel`、`WorkspaceTree` 和文件类型标签工具，Files / Settings / Log 均由左侧工作区承载。
- 右侧 inspector 不再显示文件树和 Log。
- Log 入口位于 rail 底部，顺序为 Log 在 Settings 正上方。
- `/settings?ws=...` 作为兼容入口跳回 `/` 并激活 Settings 左栏 tab。
- 已根据用户补充意见核对 React Router 7.14.2 本地实现：当前应用使用 `BrowserRouter`，left panel 状态改用 `useSearchParams` / `setSearchParams` 函数式更新维护 `?left-panel=`，没有自行解析 URL hash。

### 验证记录

- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run --cwd packages/studio-web test:unit`：53 passed。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts tests/playwright/parameters-presets.spec.ts tests/playwright/export-slicer.spec.ts --grep "scad|parameters|export|right inspector|settings and log|restores left panel|settings route"`：12 passed。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts tests/playwright/config-settings.spec.ts tests/playwright/canvas-interaction.spec.ts tests/playwright/markdown-preview.spec.ts tests/playwright/parameters-presets.spec.ts tests/playwright/export-slicer.spec.ts tests/playwright/browser-smoke.spec.ts`：32 passed。

### 变更摘要

- 修改 `packages/studio-web/src/App.tsx`、`packages/studio-web/src/workbench/workbench-layout.tsx`、`packages/studio-web/src/workbench/rail.tsx`、`packages/studio-web/src/state/ui-store.ts`。
- 删除独立设置页面 `packages/studio-web/src/routes/settings.tsx`。
- 新增 `packages/studio-web/src/workbench/left-panel.tsx`、`files-panel.tsx`、`settings-panel.tsx`、`workspace-tree.tsx`、`file-kind.ts`、`left-panel-routing.ts`。
- 更新 `packages/studio-web/tests/playwright/workbench-layout.spec.ts`、`config-settings.spec.ts`、`browser-watch-smoke.spec.ts`、`browser-smoke.spec.ts`。

### 遗留问题

- 无阶段内遗留阻塞项。

## Phase 4：把右栏整理为可展开 section，并迁移参数与 Presets

### 完成情况

- 已新增共享 `InspectorSection`，右侧 preview、config、parameters、presets、export、slicers section 均支持展开和收起。
- `.scad` 中间区域已移除 source / preview 分屏，只渲染 3D preview。
- `ScadWorkbenchState` 已由 `WorkbenchLayout` 持有；参数和 preset section 由右侧 inspector 直接消费该状态。
- `CanvasZone` 不再通过回调向父层注入参数或 preset React 节点。

### 验证记录

- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts tests/playwright/parameters-presets.spec.ts tests/playwright/export-slicer.spec.ts --grep "scad|parameters|export|right inspector"`：通过，包含参数 / presets 位于 `workbench-inspector` 且 `workbench-canvas` 内不存在对应 panel 的断言。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts tests/playwright/config-settings.spec.ts tests/playwright/canvas-interaction.spec.ts tests/playwright/markdown-preview.spec.ts tests/playwright/parameters-presets.spec.ts tests/playwright/export-slicer.spec.ts tests/playwright/browser-smoke.spec.ts`：32 passed。

### 变更摘要

- 修改 `packages/studio-web/src/workbench/inspector.tsx`、`scad-workbench.tsx`、`canvas-zone.tsx`、`parameters-panel.tsx`、`presets-panel.tsx`、`export-panel.tsx`、`slicer-panel.tsx`。
- 新增 `packages/studio-web/src/workbench/inspector-section.tsx`、`parameter-model.ts`。
- 新增 `packages/studio-web/src/viewers/scad-preview-viewer.tsx`，删除 `packages/studio-web/src/viewers/scad-split-viewer.tsx`。
- 更新参数、预设、导出和布局相关测试。

### 遗留问题

- 无阶段内遗留阻塞项。

## Phase 5：固定 Canvas Status Bar 并修复提示遮挡

### 完成情况

- Canvas 中间列已拆为 `canvas-frame` 与固定高度 `canvas-statusbar`，status bar 在中间预览区域底部，占满中间列可用宽度，不作为预览浮层。
- 顶部 viewer toolbar、canvas info、错误卡片和 status bar 在 `1920×1080`、`1440×900`、`1280×800` 下均有矩形不相交测试。
- Mesh / SCAD 预览状态节点改为隐藏文本节点；错误文本使用独立 `preview-error-message`，错误卡片可滚动且 `pointer-events: auto`。

### 验证记录

- `bun run --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts tests/playwright/parameters-presets.spec.ts --grep "status bar|preview error|typed controls"`：7 passed。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts tests/playwright/config-settings.spec.ts tests/playwright/canvas-interaction.spec.ts tests/playwright/markdown-preview.spec.ts tests/playwright/parameters-presets.spec.ts tests/playwright/export-slicer.spec.ts tests/playwright/browser-smoke.spec.ts`：32 passed。

### 变更摘要

- 修改 `packages/studio-web/src/workbench/canvas-zone.tsx`。
- 修改 `packages/studio-web/src/viewers/mesh-viewer.tsx`、`mesh-three.ts`、`image-viewer.tsx`。
- 修改 `packages/studio-web/src/styles/workbench-zones.css`、`viewers.css`、`phase7.css`。
- 扩展 `packages/studio-web/tests/playwright/canvas-interaction.spec.ts`。

### 遗留问题

- 无阶段内遗留阻塞项。

## Phase 6：Markdown、图标库、标题栏与产品命名

### 完成情况

- Markdown 预览已改用 `@uiw/react-markdown-preview`，并通过动态 `import("mermaid")` 渲染 Mermaid fenced code block。
- Markdown 链接默认在新浏览器 tab 打开，带 `rel="noopener noreferrer"`。
- Markdown 和 Mermaid SVG 均经过安全处理，覆盖危险链接、图片 URL、iframe、内联事件和 SVG 危险属性。
- 旧的项目内 Markdown 简化解析器及对应单元测试已删除，避免与当前 `@uiw/react-markdown-preview` 实现并存。
- Web 图标库统一为 `@phosphor-icons/react`，rail 图标使用 `weight="bold"`；`lucide-react` 已从 `studio-web` 依赖移除。
- 浏览器标题由 app 状态维护，无文件为 `budn'`，打开文件为 `${file} · budn'`。
- `AGENTS.md` 和 `README.md` 已记录产品名 `budn'` 与代码标识 `budn`。

### 验证记录

- `bun run --cwd packages/studio-web test:unit`：53 passed。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/markdown-preview.spec.ts`：已纳入完整 Playwright 集合并通过。
- `bun run --cwd packages/studio-web build`：通过；Vite 仍提示部分 chunk 超过 500 kB，主要来自 Mermaid / WASM / 主应用包体。

### 变更摘要

- 修改 `packages/studio-web/src/viewers/markdown-viewer.tsx`。
- 新增 `packages/studio-web/src/viewers/markdown-security.ts`。
- 删除 `packages/studio-web/src/viewers/markdown-parser.ts` 与 `packages/studio-web/tests/unit/markdown-parser.test.ts`。
- 修改 `packages/studio-web/package.json`、`bun.lock`。
- 修改 `packages/studio-web/src/workbench/rail.tsx`、`topbar.tsx`、`chat-zone.tsx`、`inspector.tsx`、`workspace-tree.tsx`。
- 新增 `packages/studio-web/src/workbench/document-title.ts`。
- 修改 `packages/studio-web/vite.config.ts`、`AGENTS.md`、`README.md`、`docs/design-system/studio-datasheet-workbench.md`。

### 遗留问题

- Vite build 存在大 chunk warning，当前不阻断构建。若后续要求控制首屏包体，需要单独规划 Mermaid / viewer / WASM 的代码分割策略。

## Phase 7：文档、结果记录、独立 review 与最终验证

### 完成情况

- 已更新执行记录到本文件。
- 已按要求完成最终独立 review，限定只读检查当前 diff、plan 和用户最新路由要求。
- 最终 review 曾指出 SCAD 参数数据仍由中间预览组件读取源码后回传；已修复为 `useScadWorkbenchState` 直接读取 SCAD 源码并解析参数，`ScadPreviewViewer` 只渲染 `MeshViewer`。
- 最终 review 曾指出旧 Markdown 简化解析器和文档描述残留；已删除旧代码与测试，并更新相关文档。
- 最终追加复核曾指出 smoke case 仍保留旧 `scad_split_view` 名称；已改为 `scad_viewer` 并验证 dispatcher 能运行 `@scad-viewer` 用例。
- 最终复查结论：未发现 Blocker，未发现 Important。

### 验证记录

- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run --cwd packages/studio-web test:unit`：45 passed。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts tests/playwright/parameters-presets.spec.ts --grep "status bar|preview error|typed controls"`：7 passed。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts tests/playwright/parameters-presets.spec.ts tests/playwright/export-slicer.spec.ts --grep "scad|parameters|export|right inspector|settings and log|restores left panel|settings route"`：12 passed。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts tests/playwright/parameters-presets.spec.ts tests/playwright/export-slicer.spec.ts tests/playwright/browser-smoke.spec.ts --grep "scad|parameters|export|right inspector|opens a real viewer"`：10 passed。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts tests/playwright/config-settings.spec.ts tests/playwright/canvas-interaction.spec.ts tests/playwright/markdown-preview.spec.ts tests/playwright/parameters-presets.spec.ts tests/playwright/export-slicer.spec.ts tests/playwright/browser-smoke.spec.ts`：32 passed。
- `bun run --cwd packages/studio-web build`：通过，保留大 chunk warning。
- `cargo test -p app-server-core --test openscad_command_tests`：11 passed。
- `cargo check --workspace`：通过，仅有既有 `watch.rs` dead_code warning。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/browser-smoke.spec.ts --grep '@scad-viewer' --list`：列出 1 个测试。
- `bun scripts/run_smoke.ts --case scad_viewer`：1 passed。
- `git diff --check`：通过。

### 变更摘要

- 更新 `docs/design-system/studio-datasheet-workbench.md`、`docs/web-platform-limits.md`、`docs/known_issues.md`、`docs/architecture.md`、`docs/getting-started.md`。
- 更新 `prompt-archives/2026042400-studio-web-parity-audit-plan/plan-01-result.md`。

### 遗留问题

- 无阶段内遗留阻塞项。Vite 大 chunk warning 已登记到 `docs/known_issues.md`，不阻断本轮验收。
