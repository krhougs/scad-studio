# Plan-02 执行结果：Studio Web 预览与侧栏继续补齐

执行时间：2026-04-24 15:54:39 CST

## 总体结果

- Phase 0 至 Phase 8 的实现项已完成，代码以当前 `studio-web`、`studio-common`、dev 脚本和文档为准。
- 本轮新增了 dev server 同源 WebSocket proxy、左右侧栏拖动宽度、侧栏外观修正、Tab 横向滚动、mesh bounds 数据流、动态底板和网格、相机控制、渲染模式、同一文件刷新保留上一帧、参数自动预览、数字 slider、显示单位设置和 Preview 尺寸展示。
- 发现并修复一次完整端到端测试失败：日志列表测试标识在 UI 改造中丢失，导致 `.scad` 自动重新渲染测试无法读取日志内容；已恢复 `log-list` 标识并保留 `log-panel` 区域标识。
- dev smoke 过程中发现 `launchWebsocketHost` 在目标端口已被旧进程占用时会误判启动成功；已增加端口占用前置检测，避免后续命令等待子进程失败。

## Phase 0：Dev Server 外部访问基线

完成情况：

- Vite dev server 默认监听 `0.0.0.0`。
- Vite 配置 `/app-server/ws` WebSocket proxy，默认指向 `VITE_WS_PROXY_TARGET`、`SCAD_STUDIO_WS_URL` 或 `ws://127.0.0.1:38421`。
- Web 客户端新增 `resolveWorkbenchWsUrl`：优先 `?ws=`，其次环境变量，默认使用同源 `/app-server/ws`。
- `run_studio_web_dev.ts` 默认使用 proxy；仅在 `--ws-url` 或 `SCAD_STUDIO_WS_URL` 明确存在时注入直接 WebSocket 地址。
- README 与 getting-started 已补充局域网访问、proxy 路径和覆盖参数说明。
- 补充 `ws-url` 单元测试。

验证：

- `bun --filter '@scad-studio/studio-web' test:unit`
- `bun scripts/run_studio_web_dev.ts --web-port 5199`：在本机默认 `38421` 已被现有 `bun run web` 进程占用时，按预期输出 `websocket host port is already in use: 127.0.0.1:38421`。
- `bun scripts/run_studio_web_dev.ts --web-port 5201 --ws-url ws://127.0.0.1:38429`：正常输出 `starting vite dev on http://0.0.0.0:5201` 和 `frontend websocket override: ws://127.0.0.1:38429`，退出干净。

## Phase 1：侧栏与 Inspector 外观修正

完成情况：

- Inspector section 使用首行 `+` / `-` 文本按钮表示展开状态。
- 新增左侧标题组件，Files、Log、Settings 共享标题区域样式。
- Log 改为纯列表内容组件，条数移动到左侧标题区域。
- Files、Log、Settings 改为满宽内容布局，减少嵌套边框。
- Log rail 图标改为 `TerminalWindow`，与 History 区分。
- 左右侧栏宽度改为 CSS 变量驱动，拖动条会更新当前宽度并保存到配置。
- Tab 栏支持横向滚动和滚轮横向滚动。
- 配置模型新增 `left_panel_width`、`right_panel_width`，旧配置缺字段时使用默认值。

验证：

- `bun --filter '@scad-studio/studio-web' test:e2e`
- `bun --filter '@scad-studio/studio-web' test:unit`
- `cargo test -p studio-common`

## Phase 2：文件打开去重回归

完成情况：

- 保留 `ui-store` 按 `tab.id` 去重的现有行为。
- 增加浏览器回归断言，重复打开同一文件只保留一个 Tab，并激活已有 Tab。

验证：

- `bun --filter '@scad-studio/studio-web' test:e2e`
- 现有 `ui-store` 单元测试继续通过。

## Phase 3：模型 Bounds、尺寸与动态底板数据流

完成情况：

- 新增 `mesh-info.ts`，根据 positions 计算 bounds、center、dimensions、radius、vertices、indices。
- Three.js viewer 使用 bounds 计算动态 build plate、网格、坐标轴和相机初始 framing。
- `MeshViewer`、`ScadPreviewViewer`、`CanvasZone`、`Inspector` 传递 `MeshInfo`。
- mesh 与 image viewer 支持同一文件刷新时保留上一帧内容，并显示非阻断加载提示；切换文件时重置为新加载态。
- 空 mesh 与错误状态保持安全 fallback。

验证：

- `bun --filter '@scad-studio/studio-web' test:unit`
- `bun --filter '@scad-studio/studio-web' test:e2e`

## Phase 4：相机控制与预设视角补齐

完成情况：

- Web 端相机纯函数支持按 bounds 和 aspect ratio 计算默认距离。
- 预设视角基于当前模型 bounds 计算，不再依赖固定距离。
- 增加 Camera 控制面板，支持 target、distance、azimuth、elevation、reset 和预设按钮。
- mesh 更新时区分切换文件与同一文件刷新；同一文件刷新不重置相机。
- Pointer 事件使用 capture，支持完整水平环绕，纵向角度限制在稳定范围内。
- 左下角三线 handle 用作相机面板入口。

验证：

- `bun --filter '@scad-studio/studio-web' test:unit`
- `bun --filter '@scad-studio/studio-web' test:e2e`

## Phase 5：渲染模式、剖切与可见性修正

完成情况：

- `MeshViewerOptions` 增加 `colorMode`、`fogEnabled`、`clipPlaneEnabled`。
- Canvas toolbar 增加 mono/color、fog、clip 控制。
- Three.js viewer 支持 mono 材质、vertex color、fog、local clipping、增强光照和深色背景可读性。
- Playwright 覆盖渲染按钮状态切换与 canvas 可交互。

验证：

- `bun --filter '@scad-studio/studio-web' test:e2e`
- `bun --filter '@scad-studio/studio-web' build`

## Phase 6：参数自动预览、Preset 与数字 Slider

完成情况：

- 删除 `apply` 按钮，参数变化立即更新表单状态，并通过 250ms 定时更新 preview defines。
- 保存 preset 表单移动到 Parameters section；Presets section 只保留加载、删除和刷新。
- 数字参数支持 number input 与 slider；有范围时使用解析范围，无范围时按当前值和默认值推导范围。
- 单元测试覆盖 slider 范围策略；浏览器测试覆盖 slider 自动预览。

验证：

- `bun --filter '@scad-studio/studio-web' test:unit`
- `bun --filter '@scad-studio/studio-web' test:e2e`

## Phase 7：尺寸单位设置与 Preview 信息展示

完成情况：

- `studio-common::AppConfig` 增加 `display_unit`，默认 `millimeter`。
- Web config normalize 与 Settings panel 支持 `mm`、`cm`、`in`。
- Preview section 展示模型 `width / depth / height`，并按设置单位格式化。
- Rust 与 Web 配置测试覆盖旧配置缺字段兼容。
- 浏览器测试覆盖单位保存后 Preview 尺寸显示更新。

验证：

- `cargo test -p studio-common`
- `bun --filter '@scad-studio/studio-web' test:unit`
- `bun --filter '@scad-studio/studio-web' test:e2e`

## Phase 8：回归验证与独立 Review

完整验证：

- `bun --filter '@scad-studio/studio-web' typecheck`：通过。
- `bun --filter '@scad-studio/studio-web' test:unit`：12 个文件、54 个测试通过。
- `bun --filter '@scad-studio/studio-web' test:e2e`：42 个测试通过。
- `bun --filter '@scad-studio/studio-web' build`：通过；Vite 仍报告部分 chunk 超过 500 kB，这是现有构建体积 warning，本轮未改变处理策略。
- `cargo test -p studio-common`：通过。
- `cargo check --workspace`：通过；`app-server-core::watch` 仍有既有 dead_code warning，本轮未改该模块行为。
- `bun scripts/run_studio_web_dev.ts --web-port 5199`：本机默认端口被已有进程占用时，输出明确错误。
- `bun scripts/run_studio_web_dev.ts --web-port 5201 --ws-url ws://127.0.0.1:38429`：通过，确认 dev server 监听 `0.0.0.0`。

独立 review：

- 状态：待独立 subagent 完整 review 后补充。

## 遗留问题

- 无功能遗留问题。
- 本机已有 `bun run web` 进程占用默认 WebSocket 端口 `38421`；本轮未结束该进程，只使用独立端口完成 dev smoke，并增加了端口占用前置检测。
- 构建 chunk 体积 warning 与 `app-server-core::watch` dead_code warning 仍存在，均非本轮新增行为。
