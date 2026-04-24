# 执行结果存档：`2026042400-studio-web-parity-audit-plan`

## 当前状态

- Phase 1：已完成。回归断言从占位文本升级为真实 canvas、请求参数和打开中文档刷新。
- Phase 2：已完成。设置页配置已进入 workbench 预览、导出、切片器请求链。
- Phase 3：已完成。`.scad` tab 已进入真实 mesh viewer 工作流，并保留源码为附加信息。
- Phase 4：已完成。参数与预设收敛到共享语义，不再依赖用户手写 `name=value`。
- Phase 5：已完成。打开中的 Markdown、图片、mesh、`.scad` 和预设文件会因 watch 事件刷新。
- Phase 6：已完成。viewer 控件补齐 render mode、projection、grid、axis、build plate、shadow，并保留切片器真实动作。
- Phase 7：已完成。图片类型差距已补，Markdown 能力边界和旧 result 误导项已明确记录。

## Phase 1 执行结果

- 新增并跑通真实回归：`.scad` tab 必须存在 `mesh-canvas`、export / slicer 面板，设置配置必须进入 `PreviewRequest` / `SlicerList` / `ExportRun`，切片器动作必须发出 `ExportRun.slicer_name`。
- 预设兼容覆盖 `*.scad.json`、旧 `<stem>.presets.json`，并在本轮后续补齐旧 Web `<source>.presets.json`。
- watch 回归覆盖 Markdown、图片、mesh、`.scad` 自动刷新与预设文件刷新。

## Phase 2 执行结果

- Web 侧共享配置快照来源为首次 workbench `ConfigLoad` 与 `/settings` 保存后的更新。
- `.scad` `PreviewRequest.configured_openscad_path`、`SlicerList.configured`、`ExportRun.configured_openscad_path` / `configured_slicers` 都消费同一份配置快照。
- 设置页可编辑 `openscad_path`、`slicers`、`floating_panel_opacity`。
- workbench Inspector 显示 `config ready`、`config incomplete`、`config error`。

## Phase 3 / 5 执行结果

- `.scad` tab 不再只有“源码 + 状态文本”：`ScadSplitViewer` 右侧为真实 `MeshViewer`，源码为附加信息。
- Inspector 对 `.scad` 也显示 export / slicer 面板，view pills 已接入 `.scad` viewer 相机 preset。
- watch 刷新从“目录树刷新”扩展到“活动文档刷新”：Markdown 文本、图片 blob、mesh 预览、`.scad` 源码与真实 viewer、预设列表都会重新读取或重新请求。

## Phase 4 执行结果

- `studio-web-wasm` 新增参数与预设桥接：`parameters_parse_source`、`parameters_format_defines`、`presets_parse_shared_file`、`presets_stringify_shared_file`。
- Web 参数面板改为共享 `ParameterEntry` / `ParameterValue` 语义，按数值、布尔、枚举渲染控件，并支持单项恢复默认值。
- `.scad` 源码解析完成前禁止 preview 抢跑；解析完成后首次预览请求携带默认 `current_defines`。
- 参数修改、单项恢复默认值、加载预设都会同步更新后续预览、导出和切片器请求使用的 defines。
- 预设默认写入 `*.scad.json`，磁盘结构为共享 `PresetFile`；兼容读取旧 `<stem>.presets.json` 与旧 Web `<source>.presets.json`。

## Phase 6 执行结果

- 新增 `MeshViewerOptions`，通过 Canvas 顶部 viewer toolbar 驱动真实 Three.js viewer 状态。
- 已接入控件：render mode（solid / wireframe / xray）、projection（perspective / orthographic）、grid、axis、build plate、shadow。
- `mesh-three.ts` 实际消费上述状态：切换材质 wireframe / transparent、透视 / 正交相机、网格 / 坐标轴 / 底板可见性与阴影配置。
- 保留基础 orbit / pan / zoom；切片器面板继续通过 `ExportRun.slicer_name` 形成真实动作。

## Phase 7 执行结果

- 图片路由补齐 `gif` / `bmp` / `tif` / `tiff` / `ico`，并补充 unit 覆盖。
- Markdown 继续使用项目内极简 parser；不声称 CommonMark / GFM parity。
- `docs/web-platform-limits.md` 更新为当前真实边界，移除旧 `.scad` deny、手工参数和 `<source>.presets.json` 默认写入描述。
- `docs/known_issues.md` 新增 Markdown CommonMark / GFM 能力差距，并说明旧 Web parity 已知问题条目已有多项被本计划修复。
- `prompt-archives/2026042300-studio-web-feature-parity/plan-00-result.md` 顶部标记旧 Phase 7 中不存在文件与“已完成”描述不再作为当前验收依据。

## Review 与修正

- Phase 4 第一轮独立 review 发现 blocker：默认 defines 未按桌面端 `current_defines` 语义进入初始预览与参数修改请求链。已修正并新增 Playwright 覆盖。
- Phase 4 第二轮独立 review 发现 blocker：旧 Web 默认预设路径 `<source>.presets.json` 未兼容。已补候选路径顺序 `*.scad.json` → `<source>.presets.json` → `<stem>.presets.json`，并新增 Playwright 与 unit 覆盖。
- Phase 6 独立 review 未发现 blocker；指出 shadow 肉眼不明显风险。已增强 build plate 接收阴影材质与 directional light shadow camera 范围。
- Phase 7 独立 review 未发现功能 blocker；指出结果存档未更新。已更新本文件，并修正 `docs/web-platform-limits.md` 中陈旧 Phase 引用。

## 验证结果

- `git diff --check`：通过。
- `cargo check --workspace`：通过；现有 `app-server-core` dead code warning 保留，非本轮引入。
- `cargo test -p studio-common --tests`：通过，43 个测试通过。
- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run --cwd packages/studio-web test:unit`：通过，7 个测试文件，40 个测试通过。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/parameters-presets.spec.ts`：通过，6 个用例通过。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts`：通过，3 个用例通过。
- `bun run web:build`：通过，PWA precache 6 个条目，包含 wasm。
- 变更内 Markdown 规范用词检查：通过，未命中项目黑名单词汇。

## 本轮涉及的关键改动

- `crates/studio-common/src/params.rs`、`crates/studio-web-wasm/src/wasm_bridge/params.rs`：共享参数解析、define 格式化与预设序列化桥接。
- `packages/studio-web/src/workbench/{parameter-model,parameters-panel,preset-io,presets-panel,scad-workbench}.tsx`：类型化参数、共享预设和兼容旧路径。
- `packages/studio-web/src/viewers/{mesh-three,mesh-viewer,scad-split-viewer,viewer-options}.ts*`：真实 viewer 控件状态与 Three.js 消费。
- `packages/studio-web/src/workbench/canvas-zone.tsx`、`packages/studio-web/src/styles/workbench-zones.css`：viewer toolbar UI 与状态传递。
- `packages/studio-web/src/workbench/tab-kind.ts`：补齐低优先级图片扩展名。
- `docs/web-platform-limits.md`、`docs/known_issues.md`、旧 `2026042300` result：同步当前真实能力边界和遗留项。
- `packages/studio-web/tests/{unit,playwright}/`：补齐参数、预设、viewer 控件、图片类型回归。

## 遗留问题

- Web Markdown 仍未达到桌面端 `egui_commonmark` 的 CommonMark / GFM 能力；已正式记录在 `docs/known_issues.md`，不能作为已完成 parity 宣称。
- `WatchChangedEvent` 文件级粒度、`ExportRun.output_path` server 侧路径语义等协议层问题仍按既有已知问题跟踪。
