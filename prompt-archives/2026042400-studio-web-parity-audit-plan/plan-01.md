# Studio Web Layout 与 OpenSCAD Fallback 修复计划

## 背景

本计划是 `plan-00.md` 后的补充计划，只处理用户在 2026-04-24 反馈的问题。产品名已确定为 `budn'`，代码和配置标识符使用 `budn`。`plan-00.md` 已完成参数、预设、viewer 控件、配置链路和 watch 刷新，但当前 Web 仍存在以下问题：

1. OpenSCAD CLI 路径检测对“配置路径存在但不可执行或不存在”的情况处理错误。当前 `resolve_openscad_path` 会直接选中 `configured_path`，即使该路径不存在，随后 `Command::new` 才返回 `No such file or directory (os error 2)`。这会绕过已有 `PATH` / 平台默认路径候选。
2. Workbench 布局仍偏离 Buddin 设计：右栏承担了文件列表，设置是独立路由，`.scad` 参数和 preset 仍在中间预览区内部，预览错误提示会和顶部工具栏互相遮挡。
3. 中央预览区的 status bar 不是固定页面框架元素；它应该固定在中间预览区域最底部，占满可用宽度，固定高度，并且不遮挡预览内容。
4. Markdown 预览应直接使用 `@uiw/react-markdown-preview`，并开启 Mermaid 支持；必须引入 sanitize / URL 安全策略，不能把不可信 Markdown 当成可信 HTML。
5. Web 图标库应统一为 `@phosphor-icons/react`，侧边栏 icon 使用 `weight="bold"`。
6. 浏览器标题需要由 app 状态维护，展示当前文件名与产品名 `budn'`。
7. Log 应成为左侧 rail tab，入口位于底部，顺序为 Log 在 Settings 正上方。
8. 左侧 panel tab 与浏览器 URL 需要双向联动，例如使用 `#left-panel=chat|files|settings|log` 或等价机制。

已按用户要求在大尺寸屏幕渲染 `/Users/krhougs/LocalCodes/buddin/ui_kits/app/index.html`。渲染命令：

```bash
bunx playwright screenshot --viewport-size=1920,1080 --full-page file:///Users/krhougs/LocalCodes/buddin/ui_kits/app/index.html /tmp/scad-studio-audit/buddin-1920x1080.png
```

截图已复制到：

```text
prompt-archives/2026042400-studio-web-parity-audit-plan/buddin-1920x1080.png
```

## Buddin 设计分析

- 主体是固定桌面工作台：`52px rail + 360px 左侧工作区 + 1fr 中央 3D canvas + 320px 右侧 inspector`，顶部 `44px` 贯穿全宽。
- 中央 3D canvas 是视觉主角，默认占满中间整列；顶部工具栏和右上信息卡可以是 canvas chrome，status bar 必须是中间列底部固定框架元素，不以浮层覆盖模型。
- 左侧工作区随 rail tab 切换内容。截图中默认是 Agent，但 Library / Files / Settings / Log 都应复用同一侧栏位置，而不是进入右侧 inspector 或独立页面。
- 右侧 inspector 是当前模型的属性面板，按 section 组织：features、parameters、material、build。映射到当前 Studio Web，应放 preview、config、parameters、presets、export、slicers 等当前模型相关 section；Log 不属于右侧 inspector，进入左侧 Log tab。
- section 之间用 1px 规则线分隔，标题是 mono uppercase；section 应支持展开和收起，避免右栏在小高度窗口内不可用。
- 文件列表属于左侧 Library / Files tab。右侧不应显示 workspace tree；右侧只显示当前文档或模型相关信息。

## 目标

修复 OpenSCAD CLI 路径检测 fallback，并把 Web workbench 调整为 Buddin 设计语义：中央预览区默认占满中间空间，status bar 作为中间列底部固定框架元素，左侧 rail tab 承载 Agent / Files / Settings / Log，URL 与左侧 panel tab 双向联动，右侧 inspector 承载可展开 section，`.scad` 参数和 preset 移入右栏。

## 非目标

- 不回退 `plan-00.md` 已完成的参数、预设、watch、导出、切片器和 viewer 控件能力。
- 不引入 shadcn 组件；当前仓库没有 `components.json`，本计划使用 repo-native React 组件和现有 CSS token。
- 不实现完整移动端布局；本计划覆盖桌面尺寸与较窄桌面尺寸，重点保证 `1920×1080`、`1440×900`、`1280×800` 下不遮挡、不误放区域。
- 不新增独立的业务状态模型；参数与预设继续复用 `plan-00.md` 引入的共享 wasm 语义。
- 不允许为了 Mermaid 支持关闭 Markdown 安全策略；不可信 Markdown 必须经过 sanitize 与 URL 限制。

---

## Phase 1：补充回归用例，先复现当前问题

### 目标

先让测试准确描述本轮反馈问题，避免继续通过弱断言掩盖真实缺陷。

### 前序目标保护

- 保护 `plan-00.md` 已完成的配置透传、参数预设、导出、切片器和 viewer 控件用例。
- 不修改生产代码来适配测试。
- 不把“元素存在”当成布局正确；布局测试必须检查区域归属或元素矩形关系。

### 输入

- 用户 2026-04-24 反馈的全部问题。
- `crates/app-server-core/src/preview.rs`
- `crates/app-server-core/tests/openscad_command_tests.rs`
- `packages/studio-web/src/workbench/{workbench-layout,canvas-zone,inspector,rail,scad-workbench}.tsx`
- `packages/studio-web/tests/playwright/{config-settings,canvas-interaction,browser-smoke}.spec.ts`
- Buddin 设计截图 `prompt-archives/2026042400-studio-web-parity-audit-plan/buddin-1920x1080.png`

### 操作步骤

1. 在 `crates/app-server-core/tests/openscad_command_tests.rs` 增加失败用例：
   - 配置路径不存在时，应继续尝试环境变量路径；
   - 配置路径和环境变量路径都不存在时，应继续尝试自动检测路径；
   - macOS `.app` bundle 路径应解析到 `Contents/MacOS/OpenSCAD`；
   - 所有候选都不可用时，错误信息应说明没有可用 OpenSCAD CLI，而不是暴露 `os error 2` 作为主要判断依据。
2. 新增或扩展 `packages/studio-web/tests/unit/tab-kind.test.ts` / 新建 `file-kind.test.ts`：
   - `.scad` 显示 `SCAD`；
   - `.stl` 显示 `STL`；
   - `.3mf` 显示 `3MF`；
   - `.md` 显示 `MD`；
   - 图片显示具体扩展名；
   - 目录显示 `DIR`。
3. 新增 `packages/studio-web/tests/playwright/workbench-layout.spec.ts`：
   - `1920×1080` 打开默认 workbench，中央 canvas 宽度应大于左栏和右栏之和中的任一栏，且占据 grid 第三列；
   - 点击 Files rail 后，文件列表应出现在左侧工作区，右侧 inspector 内不应存在 `inspector-entries`；
   - 点击 Settings rail 后，设置表单应出现在左侧工作区，URL 不应跳到独立 `/settings` 页面；
   - 点击 Log rail 后，日志列表应出现在左侧工作区，Log 入口位于 rail 底部且在 Settings 正上方；
   - 切换 Chat / Files / Settings / Log 时，URL 中的 left-panel 状态同步变化；带 left-panel 状态打开页面时，初始 active rail 与左栏内容匹配；
   - 打开 `.scad` 后，Parameters 和 Presets 应在右侧 inspector section 内，`scad-workbench` 内不应再存在参数侧栏；
   - 右侧 section 的展开和收起按钮应能隐藏与恢复 section 内容。
4. 扩展 `packages/studio-web/tests/playwright/canvas-interaction.spec.ts`：
   - 在 `1920×1080`、`1440×900`、`1280×800` 三个 viewport 下检查 viewer toolbar、canvas info、错误提示和 status bar 的 `boundingBox` 不相交；
   - 检查 status bar 固定在中间预览区域底部、固定高度、占满可用宽度，并且 `mesh-canvas` 的可绘制区域不被 status bar 覆盖；
   - 使用缺失 OpenSCAD 配置或模拟错误返回复现当前“preview error 与工具栏重合”问题。
5. 扩展 `packages/studio-web/tests/playwright/config-settings.spec.ts`：
   - 不再通过 `/settings` 独立页面完成主要设置流程；
   - 通过 rail Settings tab 编辑并保存 `openscad_path`、`floating_panel_opacity` 和 slicer；
   - 保留 `/settings` 兼容入口时，只允许它跳回 workbench 并激活 Settings tab。
6. 新增或扩展 Markdown 预览测试：
   - `packages/studio-web/tests/unit/markdown-preview-security.test.ts` 验证链接 URL、图片 URL 和 HTML sanitize 策略；
   - `packages/studio-web/tests/playwright/markdown-preview.spec.ts` 验证 Mermaid fenced code block 渲染为 diagram 容器，而不是普通代码文本；
   - 测试必须覆盖脚本 URL、内联事件属性和 iframe 被拒绝；
   - 测试必须覆盖 Markdown 链接保留 `target="_blank"` 与 `rel="noopener noreferrer"`；
   - 测试必须覆盖 Mermaid 渲染后的 SVG 不允许脚本、事件属性或危险链接进入 DOM。
7. 新增 app shell 测试：
   - 当前无文件时标题为 `budn'`；
   - 打开 `cube.scad` 后标题为 `cube.scad · budn'`；
   - 切换文件时标题同步更新。
8. 新增设计系统与图标库检查：
   - `docs/design-system/studio-datasheet-workbench.md` 记录 `@phosphor-icons/react`；
   - rail 图标渲染包含 Phosphor bold weight，不再使用 `lucide-react`；
   - 新增检查或测试防止 `packages/studio-web/src` 继续引入 `lucide-react`。
9. 运行最小失败验证：
   - `cargo test -p app-server-core openscad_command_tests`
   - `bun run --cwd packages/studio-web test:unit -- file-kind`
   - `bun run --cwd packages/studio-web test:unit -- markdown-preview-security`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/config-settings.spec.ts`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/markdown-preview.spec.ts`

### 验收标准

- Rust OpenSCAD fallback 用例在当前代码上失败，失败原因指向“配置路径不存在仍被选中”。
- Web 布局用例在当前代码上失败，失败原因分别指向：
  - 文件列表仍在右栏；
  - Settings 仍是独立页面；
  - `.scad` 参数和 Presets 仍在中间区域；
  - 右栏 section 不能展开收起；
  - Log 仍在右栏 section；
  - URL 不能恢复左栏 tab；
  - status bar 覆盖预览区域或不是固定框架元素；
  - 错误提示和工具栏存在遮挡风险。
- Markdown 用例在当前代码上失败，失败原因指向仍使用项目内极简 parser，且没有 Mermaid / sanitize 覆盖。
- 标题栏用例在当前代码上失败，失败原因指向没有根据 active file 更新 `document.title`。
- 新增用例不删除 `plan-00.md` 已有验收覆盖。

---

## Phase 2：修复 OpenSCAD CLI 路径检测与 fallback

### 目标

让 App Server 对配置路径、环境变量、`PATH`、平台默认路径使用同一套候选检测规则。配置路径无效时继续尝试后续候选，避免 `Command::new` 才报 `os error 2`。

### 前序目标保护

- 保护桌面端和 Web 端都通过 app server 核心代码调用 OpenSCAD 的架构边界。
- 保护 `ExportRun` 和 `PreviewRequest` 使用同一个 OpenSCAD detection 入口。
- 不把 OpenSCAD detection 移到 Web 或桌面壳层。

### 输入

- `crates/app-server-core/src/preview.rs`
- `crates/app-server-core/src/export.rs`
- `crates/app-server-core/tests/openscad_command_tests.rs`
- `crates/app-server-core/src/lib.rs`

### 操作步骤

1. 在 `preview.rs` 中把当前 `resolve_openscad_path(configured_path, env_path, auto_path)` 改成验证候选是否可用的实现：
   - 候选优先级：有效配置路径 → 有效 `OPENSCAD_PATH` → `PATH` 中的 `openscad` / `openscad.exe` → 平台默认路径；
   - 无效配置路径不能阻止后续候选；
   - 候选验证至少要求 `is_file()`；Unix 下可进一步检查执行权限，但不能因为权限检测不可用导致有效文件被拒绝。
2. 增加配置路径展开规则：
   - macOS 路径以 `.app` 结尾或指向 `.app` 目录时，追加 `Contents/MacOS/OpenSCAD` 作为候选；
   - 配置值是裸命令名 `openscad` / `openscad.exe` 时，允许走 `PATH` 搜索；
   - 配置值是直接可执行文件时，优先使用该文件。
3. 保持 `detect_openscad_path` 为 `PreviewRequest` 和 `ExportRun` 唯一入口，避免 export 与 preview 出现不同检测行为。
4. 调整错误信息：
   - 所有候选都不可用时返回“未找到 OpenSCAD CLI，可设置环境变量 OPENSCAD_PATH 或在 Settings 中配置 OpenSCAD 路径”；
   - 如果有无效配置路径，错误信息可以附带“已忽略不可用配置路径: ...”，但主要错误不能是 `No such file or directory (os error 2)`。
5. 跑通 Phase 1 的 Rust 用例，再跑：
   - `cargo test -p app-server-core openscad_command_tests`
   - `cargo check --workspace`

### 验收标准

- 配置路径不存在时，存在有效 env/PATH/平台候选就能继续预览或导出。
- env/PATH/平台候选都不存在时，错误是明确的 OpenSCAD 未找到提示。
- `ExportRun` 和 `.scad` `PreviewRequest` 使用相同 detection 行为。
- 不需要 Web 端写任何 OpenSCAD 路径猜测逻辑。

---

## Phase 3：重组 Workbench 壳层，左栏承载 Files、Settings 与 Log

### 目标

把 Workbench 调整为 Buddin 设计语义：rail 负责切换左侧工作区内容，文件列表、设置和 Log 都在左侧工作区，右侧 inspector 不再承担文件树或日志。

### 前序目标保护

- 保护 `plan-00.md` 的 workbench transport、watch、打开文件、tab、导出和切片器链路。
- 保护左侧 Agent 面板现有输入草稿，不因 Files / Settings / Log tab 引入协议业务状态。
- 不破坏现有 `pathKey`、`pathLabel`、`resolveTabKind` 文件打开逻辑。
- 不破坏 `LogPanel` 当前日志来源；只改变呈现位置。

### 输入

- `packages/studio-web/src/workbench/workbench-layout.tsx`
- `packages/studio-web/src/workbench/rail.tsx`
- `packages/studio-web/src/workbench/chat-zone.tsx`
- `packages/studio-web/src/workbench/inspector.tsx`
- `packages/studio-web/src/workbench/log-panel.tsx`
- `packages/studio-web/src/routes/settings.tsx`
- `packages/studio-web/src/App.tsx`
- `packages/studio-web/src/state/ui-store.ts`
- `packages/studio-web/src/styles/{workbench,workbench-zones}.css`

### 操作步骤

1. 新建 `packages/studio-web/src/workbench/file-kind.ts`：
   - 导出 `fileKindLabel(entry)`，目录返回 `DIR`；
   - 文件按扩展名返回 `SCAD`、`STL`、`3MF`、`MD`、`PNG`、`JPG`、`JSON` 等；
   - 未识别文件返回扩展名大写；无扩展名返回 `FILE`。
2. 新建 `packages/studio-web/src/workbench/workspace-tree.tsx`：
   - 从 `inspector.tsx` 移出 `EntryRow` 递归渲染；
   - 保留目录展开、收起、打开文件、active file 高亮；
   - 使用 `fileKindLabel` 替代写死的 `file`。
3. 新建 `packages/studio-web/src/workbench/files-panel.tsx`：
   - 使用 Buddin 左侧工作区 header 样式；
   - 渲染 workspace root、加载状态、空目录状态和 `WorkspaceTree`。
4. 新建 `packages/studio-web/src/workbench/settings-panel.tsx`：
   - 从 `routes/settings.tsx` 提取设置表单和保存逻辑；
   - 表单消费同一个 `WasmClient` 和 `app-config-store`；
   - 支持编辑 `openscad_path`、`floating_panel_opacity`、slicers；
   - 保存后不离开 workbench。
5. 新建 `packages/studio-web/src/workbench/left-panel.tsx`：
   - 根据 `activeRail` 渲染 `ChatZone`、`FilesPanel`、`SettingsPanel`、`LogPanel` 或简短占位；
   - 统一把 Files 的内部 rail id、URL 状态和测试断言命名为 `files`，不再使用 `workspace` 作为左栏 tab id；
   - rail 的 `files` 对应 Files panel；
   - rail 的 `settings` 对应 Settings panel；
   - rail 的 `log` 对应 Log panel。
6. 修改 `rail.tsx`：
   - 将现有 `workspace` rail item id 改为 `files`；
   - 点击 Settings 不再 `navigate("/settings")`；
   - 点击 Log 不再依赖 inspector section；
   - rail 底部入口顺序为 Log 在 Settings 正上方；
   - 所有 rail 点击只更新 `activeRail` 与 URL 中的 left-panel 状态；
   - 若当前 URL 不是 `/`，先回到 `/`，但 Settings UI 仍由左栏 tab 呈现。
7. 新建或修改 `packages/studio-web/src/workbench/left-panel-routing.ts`：
   - 解析 `location.hash` 或等价 URL 状态，例如 `#left-panel=chat|files|settings|log`；
   - URL 状态非法时回退到 `chat`；
   - `activeRail` 改变时写回 URL；
   - 浏览器后退 / 前进时同步更新 `activeRail`；
   - 若采用 hash，必须保留当前 `location.search`，尤其是 `?ws=`；
   - 解析和序列化函数必须有 unit test。
8. 修改 `App.tsx`：
   - 主路由仍渲染 `WorkbenchLayout`；
   - `/settings` 只作为兼容入口，进入后设置 left-panel 为 `settings` 并跳回 `/`；
   - `/settings?ws=...` 跳回 `/` 时必须保留原始 query，例如 `/settings?ws=...` → `/?ws=...#left-panel=settings`；
   - 不再渲染独立 Settings 页面。
9. 修改 `workbench-layout.tsx`：
   - 用 `LeftPanel` 替代硬编码 `ChatZone`；
   - 不再把 `entries`、`expandedDirectories` 等文件树 props 传给 `Inspector`；
   - 文件打开、展开、watch 刷新状态仍留在 `WorkbenchLayout`，通过 props 交给 `FilesPanel`。
10. 修改 CSS：
   - 保持 Buddin `52px 360px 1fr 320px` 主结构；
   - 左侧 `.chat` 泛化为 `.side-panel`，Agent / Files / Settings / Log 复用同一列；
   - 文件树样式从 inspector 样式中抽出，避免右栏依赖。
11. 跑通：
    - `bun run --cwd packages/studio-web typecheck`
    - `bun run --cwd packages/studio-web test:unit`
    - `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts`
    - `bun run --cwd packages/studio-web test:e2e tests/playwright/config-settings.spec.ts`

### 验收标准

- 文件列表只在左侧 Files tab 中出现，右侧 inspector 不再包含 `inspector-entries`。
- 文件类型标签不再统一显示 `FILE`；常见类型显示具体类型。
- Settings 是左侧 rail tab，不是独立设置页面。
- Log 是左侧 rail tab，不再是右栏 section；rail 底部顺序为 Log、Settings。
- URL 与左侧 panel tab 双向联动，刷新页面、后退、前进都能恢复对应左栏内容；`?ws=` 等 query 不因 panel 切换或 `/settings` 兼容入口丢失。
- `/settings` 兼容入口不会展示独立页面，而是进入 workbench 并激活 Settings tab。
- 打开文件、目录展开、watch 刷新仍按 `plan-00.md` 行为工作。

---

## Phase 4：把右栏整理为可展开 section，并迁移参数与 Presets

### 目标

右侧 inspector 只承载当前文档或模型相关 section，所有 section 支持展开和收起。`.scad` 参数与 Presets 从中间预览区移到右栏。

### 前序目标保护

- 保护 Phase 3 的文件列表左栏化和 Settings 左栏化。
- 保护 `plan-00.md` 的参数解析、默认 defines、预设兼容路径、导出和切片器动作。
- 不引入第二套参数或预设模型。

### 输入

- `packages/studio-web/src/workbench/inspector.tsx`
- `packages/studio-web/src/workbench/scad-workbench.tsx`
- `packages/studio-web/src/workbench/parameters-panel.tsx`
- `packages/studio-web/src/workbench/presets-panel.tsx`
- `packages/studio-web/src/workbench/export-panel.tsx`
- `packages/studio-web/src/workbench/slicer-panel.tsx`
- `packages/studio-web/src/styles/workbench-zones.css`

### 操作步骤

1. 新建 `packages/studio-web/src/workbench/inspector-section.tsx`：
   - props 包含 `id`、`title`、`defaultOpen`、`children`、可选 `actions`；
   - header 使用 Buddin `.insp-sec h5` 视觉语言；
   - 展开状态可本地维护，也可由父组件传入；
   - 内容收起时完全隐藏，但 header 保持可访问。
2. 为 `InspectorSection` 增加 unit 或 Playwright 覆盖：
   - 默认展开时内容可见；
   - 点击 header button 后内容隐藏；
   - 再次点击后内容恢复。
3. 修改 `Inspector`：
   - 移除文件树 section；
   - 使用 `InspectorSection` 包装 preview、config、parameters、presets、export、slicers；
   - 不再在 inspector 中渲染 Log section，Log 只由左侧 Log tab 承载；
   - `showMeshPanels` 为 false 时隐藏 export / slicers；
   - 无 `.scad` 参数时 parameters / presets section 不显示或显示空状态，但不占用中间预览区。
4. 拆分 `ScadWorkbench`：
   - 提取 `useScadWorkbenchState` 或等价 hook，保留源码读取、参数解析、预设读取、defines 更新逻辑；
   - 中央 Canvas 只渲染 `.scad` 预览 viewer；
   - `ParametersPanel` 与 `PresetsPanel` 由右侧 inspector 渲染；
   - `activeDefines` 仍传给 export / slicer。
5. 修改 `CanvasZone`：
   - `.scad` 激活时只显示真实 preview viewer，不再渲染 `scad-workbench__panels`；
   - 保持 `onAppliedDefinesChange`、`onMeshStats`、`onPreviewStatus` 行为。
6. 修改 CSS：
   - 删除或停用 `.scad-workbench__panels` 占用中间区域的布局；
   - 右栏 section 内容使用统一 spacing；
   - 参数输入、preset 按钮、export、slicer 继续使用现有控件样式。
7. 跑通：
   - `bun run --cwd packages/studio-web typecheck`
   - `bun run --cwd packages/studio-web test:unit`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/parameters-presets.spec.ts`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/export-slicer.spec.ts`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts`

### 验收标准

- 右栏所有 section 都能展开和收起。
- 右栏 section 仅承载当前文档或模型信息，不包含文件树和 Log。
- `.scad` 参数和 Presets 位于右侧 inspector section。
- 中央 3D preview 默认占满中间空间，不再被参数或 preset 面板挤压。
- 参数修改、恢复默认值、加载 preset 后，预览、导出和切片器仍使用最新 defines。
- export / slicer 行为保持 `plan-00.md` 验收结果。

---

## Phase 5：固定 Canvas Status Bar 并修复提示遮挡

### 目标

让 preview pending、preview ready、preview error 等提示在不同桌面尺寸下都不遮挡顶部 viewer toolbar、右上 canvas info 和底部 status bar。status bar 必须是中间预览区域的固定底部框架，占满可用宽度、固定高度，并且不覆盖 canvas 可绘制区域。

### 前序目标保护

- 保护 Phase 4 后的中央预览区全空间使用。
- 保护 viewer toolbar 对 render mode、projection、grid、axis、build plate、shadow 的真实控制。
- 不让 status bar 覆盖 Three.js canvas 的可交互区域；canvas stage 填满 status bar 以上区域。
- 不把 status bar 做成浮在 canvas 上的文档流元素或 overlay。

### 输入

- `packages/studio-web/src/workbench/canvas-zone.tsx`
- `packages/studio-web/src/viewers/mesh-viewer.tsx`
- `packages/studio-web/src/styles/{workbench-zones,viewers}.css`
- `packages/studio-web/tests/playwright/canvas-interaction.spec.ts`
- `packages/studio-web/tests/playwright/workbench-layout.spec.ts`

### 操作步骤

1. 修改 `MeshViewer` 状态提示结构：
   - pending / ready 使用轻量状态 chip；
   - error 使用居中的错误 card，包含完整错误文本，允许换行；
   - error card 不使用顶部工具栏所在的 `top: 14px` 位置。
2. 修改 `CanvasZone` 结构：
   - `.canvas-well` 分成 `.canvas-frame` 与 `.canvas-statusbar`；
   - `.canvas-statusbar` 固定高度，位于中间预览区域最底部，占满 `CanvasZone` 可用宽度；
   - `.canvas-stage` 高度扣除 status bar，不让 status bar 覆盖 canvas；
   - `part-meta` 的内容迁入 `.canvas-statusbar`，不再作为悬浮底部 chrome。
3. 为 canvas chrome 定义安全区域 CSS 变量：
   - 顶部安全区域覆盖 toolbar 和 canvas info；
   - 底部安全区域覆盖固定 status bar；
   - 状态 chip 和 error card 只能出现在 canvas stage 中部，不进入顶部和底部安全区域。
4. 调整 `.viewer-toolbar`：
   - 保持 `flex-wrap`；
   - 在 `1440px` 和 `1280px` 下缩短按钮 padding；
   - 必要时把低优先级 visibility toggles 放到第二行，但不能覆盖错误提示。
5. 调整 `.canvas-info`：
   - 在较窄桌面尺寸下保持右上角，不与 toolbar 第二行相交；
   - 如宽度不足，优先缩短文字而不是覆盖 toolbar。
6. 更新 Playwright 布局检查：
   - `1920×1080`、`1440×900`、`1280×800` 三个 viewport；
   - 检查 toolbar 与 error card 不相交；
   - 检查 toolbar 与 canvas info 不相交；
   - 检查 error card 与 status bar 不相交；
   - 检查 status bar 底边与中间预览区域底边一致；
   - 检查 status bar 宽度等于中间预览区域宽度；
   - 检查 `mesh-canvas` 仍覆盖整个 `.canvas-stage`。
7. 跑通：
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts`

### 验收标准

- 复现 `preview error: 启动 OpenSCAD CLI 失败...` 类错误时，错误信息可读且不遮挡工具栏。
- 三个桌面 viewport 下 toolbar、canvas info、error card、status bar 没有矩形相交。
- status bar 是中间预览区域底部固定框架，占满可用宽度，固定高度，不覆盖预览内容。
- 中央 canvas stage 填满 status bar 以上的中间区域。
- viewer 控件功能不退化。

---

## Phase 6：Markdown、图标库、标题栏与产品命名

### 目标

把 Markdown 预览切换到 `@uiw/react-markdown-preview` 并开启 Mermaid 支持；把 Web 图标统一到 `@phosphor-icons/react`；用 app 状态维护浏览器标题；把产品名 `budn'` / 代码名 `budn` 固化到根文档和本地设计系统。

### 前序目标保护

- 保护 Phase 1 到 Phase 5 的验收结果。
- 保护 Markdown 文件 watch 刷新能力。
- 保护左侧 rail tab 与 URL 联动。
- 不允许 Mermaid 支持绕过 Markdown sanitize 与 URL 安全策略。

### 输入

- `packages/studio-web/src/viewers/markdown-viewer.tsx`
- `packages/studio-web/src/viewers/markdown-parser.ts`
- `packages/studio-web/src/workbench/rail.tsx`
- `packages/studio-web/src/workbench/workbench-layout.tsx`
- `packages/studio-web/src/state/ui-store.ts`
- `packages/studio-web/package.json`
- `docs/design-system/studio-datasheet-workbench.md`
- `AGENTS.md`
- `README.md`

### 操作步骤

1. 查阅官方文档和安装后的 package 内容后更新依赖：
   - 必选：`@uiw/react-markdown-preview`、`rehype-sanitize`、`@phosphor-icons/react`；
   - Mermaid 依赖策略：先确认 `@uiw/react-markdown-preview` 当前版本是否已经提供 Mermaid 渲染所需运行时代码；如果已自带，则不新增 `mermaid` 依赖；如果官方示例或 package 内容仍要求外部 `mermaid` import，才新增 `mermaid`；
   - 如果实现需要提取 code block 文本，不允许从 `rehype-rewrite` 这类传递依赖直接 import；要么使用组件 `children` 提取文本，要么把所需包声明为直接依赖；
   - 依赖安装使用 `bun add --cwd packages/studio-web ...`，更新 `bun.lock`；
   - 结果必须记录在 `plan-01-result.md`：说明是否新增 `mermaid`，以及判断依据。
2. 修改 `MarkdownViewer`：
   - 继续通过 `FileRead` 获取 Markdown 文本；
   - 使用 `@uiw/react-markdown-preview` 渲染；
   - Mermaid fenced code block 渲染为 diagram；
   - 传入 `rehype-sanitize`，并限制链接和图片 URL，只允许安全协议与相对路径；
   - 所有 Markdown 链接都必须在新的浏览器 tab 中打开，anchor 统一设置 `target="_blank"` 与 `rel="noopener noreferrer"`；
   - 自定义 sanitize schema 必须允许安全的 `a[target][rel]`，避免 `target` / `rel` 被 sanitize 移除；
   - 禁止执行 HTML 中的脚本、内联事件和 iframe；
   - Mermaid 渲染属于 Markdown sanitize 之后的 DOM 写入路径，必须单独处理：若使用 `mermaid`，初始化时设置 `securityLevel: "strict"` 或等价安全配置；渲染失败信息必须用文本节点显示，不能作为 HTML 写入；Mermaid SVG 注入路径必须有安全测试覆盖。
3. 让 Markdown 预览接入当前设计系统：
   - `MarkdownViewer` 外层使用现有 viewer 容器，不引入 GitHub 默认浅色背景；
   - `MarkdownPreview` 使用 `data-color-mode="dark"` 或等价配置；
   - 在 `packages/studio-web/src/styles/viewers.css` 增加 `.markdown-preview` 样式，使用当前 token：`--font-body`、`--font-mono`、`--fg-body`、`--fg-muted`、`--bg-canvas-well`、`--border-hairline`；
   - 标题、表格、引用、代码块、Mermaid 容器、链接 hover/focus 必须保持 zero-radius、1px hairline、dark-only；
   - 不允许直接套用库默认 GitHub light theme 视觉。
4. 删除或停止使用项目内极简 Markdown parser：
   - 若 `markdown-parser.ts` 只剩未使用代码，则删除；
   - 若测试仍引用旧 parser，则迁移到新 renderer 安全测试。
5. 修改图标库：
   - `packages/studio-web/src/workbench/rail.tsx` 从 `@phosphor-icons/react` 导入图标；
   - rail icon 统一传 `weight="bold"`；
   - `packages/studio-web/src` 内所有图标导入同步迁移到 `@phosphor-icons/react`；
   - `rg "lucide-react" packages/studio-web/src` 必须零命中；
   - 从 `packages/studio-web/package.json` 移除 `lucide-react`。
6. 更新本地设计系统：
   - `docs/design-system/studio-datasheet-workbench.md` 明确 Web 图标库为 `@phosphor-icons/react`；
   - 明确 rail 图标使用 `weight="bold"`；
   - 明确 Markdown 预览必须接入当前 dark-only datasheet 视觉系统，不使用第三方默认浅色主题；
   - 明确用户可见产品名为 `budn'`，代码和配置标识符使用 `budn`。
7. 更新根文档：
   - `AGENTS.md` 增加产品命名规则；
   - `README.md` 首屏说明 `budn'` / `budn` 的命名关系；
   - 保留 `scad-studio` 作为仓库和历史工程名说明。
8. 新建 `packages/studio-web/src/workbench/document-title.ts`：
   - 导出纯函数 `titleForActiveDocument(activeLabel: string | null): string`；
   - 无活动文件返回 `budn'`；
   - 有活动文件返回 `<filename> · budn'`；
   - 函数必须有 unit test。
9. 在 `WorkbenchLayout` 中维护 `document.title`：
   - active tab 变化时更新；
   - tab 关闭后更新；
   - 组件卸载时无需恢复旧标题，因为 Web app 单页即产品边界。
10. 跑通：
   - `bun run --cwd packages/studio-web typecheck`
   - `bun run --cwd packages/studio-web test:unit`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/markdown-preview.spec.ts`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts`

### 验收标准

- Markdown 预览使用 `@uiw/react-markdown-preview`，GFM 能力由该库提供。
- 只有在该库当前版本没有提供 Mermaid 运行时代码时，才新增独立 `mermaid` 依赖；判断依据写入结果存档。
- Mermaid code block 能渲染为 diagram，并使用 `securityLevel: "strict"` 或等价安全配置；渲染失败信息不能作为 HTML 写入。
- Markdown 预览采用当前 dark-only datasheet 设计系统，不能出现默认浅色 GitHub 主题。
- Markdown 内所有链接都在新的浏览器 tab 中打开，并带 `rel="noopener noreferrer"`；sanitize schema 不会移除这些安全属性。
- 不可信 Markdown 中的脚本 URL、内联事件、iframe 被拒绝或清理。
- `packages/studio-web/src` 不再引用 `lucide-react`；rail 图标使用 `@phosphor-icons/react` 且 `weight="bold"`。
- 浏览器标题随当前文件变化，无文件时显示 `budn'`。
- `AGENTS.md`、`README.md`、本地设计系统都记录 `budn'` / `budn` 命名规则。

---

## Phase 7：文档、结果记录、独立 review 与最终验证

### 目标

把本计划的实现结果写入 `plan-01-result.md`，并通过独立 review 和完整验证确认没有破坏 `plan-00.md` 已完成能力。

### 前序目标保护

- 保护 Phase 1 到 Phase 6 的验收结果。
- 不把已知仍未解决的问题写成已完成。
- 不删除 `plan-00-result.md` 中已有真实验证记录。

### 输入

- `prompt-archives/2026042400-studio-web-parity-audit-plan/plan-01.md`
- 本计划所有变更 diff
- `docs/web-platform-limits.md`
- `docs/known_issues.md`

### 操作步骤

1. 每个 Phase 编码后调用独立 subagent review，review 输入必须包含：
   - 当前 Phase 目标与验收标准；
   - 完整 `plan-01.md`；
   - 本次 diff 或涉及文件清单。
2. 对 review blocker 立即修复并重新回归。
3. 每个 Phase 完成后实时更新：
   - `prompt-archives/2026042400-studio-web-parity-audit-plan/plan-01-result.md`
4. 若发现无法在本计划内解决但会影响后续判断的问题，更新：
   - `docs/known_issues.md`
5. 如 Web 平台边界发生变化，更新：
   - `docs/web-platform-limits.md`
6. 最终验证命令：
   - `git diff --check`
   - `cargo test -p app-server-core openscad_command_tests`
   - `cargo check --workspace`
   - `bun run --cwd packages/studio-web typecheck`
   - `bun run --cwd packages/studio-web test:unit`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/workbench-layout.spec.ts`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/config-settings.spec.ts`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/markdown-preview.spec.ts`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/parameters-presets.spec.ts`
   - `bun run --cwd packages/studio-web test:e2e tests/playwright/export-slicer.spec.ts`
   - `! rg "lucide-react" packages/studio-web/src`
   - `bun run web:build`
7. 最终执行 Markdown 用词自检，避免项目黑名单词汇进入用户可见文档。

### 验收标准

- `plan-01-result.md` 记录每个 Phase 的完成情况、review 结论、验证证据和遗留问题。
- 所有最终验证命令通过，或明确记录非本计划引入的既有 warning。
- 独立 review 没有未处理 blocker。
- Buddin 设计映射在文档和代码中一致：左栏是 tab 工作区，中央是完整 3D preview，右栏是可展开 inspector section。
