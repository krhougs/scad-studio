# studio-web 平台限制说明

本文件列明 `packages/studio-web` 在浏览器环境下相对于桌面端 `studio-app`
的能力差异与处理方式。所有差异都由协议与服务端处理，不在 client 侧伪装
“本平台没 I/O”；详见 `/Users/krhougs/.claude/projects/.../memory/feedback_client_io_capability.md`
的基线约束。

## 1. 本地进程与外部工具

- 浏览器无法直接启动本地 OpenSCAD 或切片器进程。`PreviewRequest`、
  `ExportRun` 等命令依赖 server 端的 OpenSCAD CLI；如果 server 所在机器
  未安装或未配置 `OPENSCAD_PATH`，client 收到 `ProtocolError::Internal`
  并在状态栏显示 `preview error: ...`，不会尝试本地回退。
- 切片器管理（Phase 7）同理：`SlicerList` 列出的是 server 机器上的安装，
  不是浏览器机器上的。

## 2. 源码读取边界

- `web_file_read_capability()` 当前默认不拒绝扩展名。Web 端会读取 `.scad`
  源码，用同一份 `studio-common` 参数解析逻辑生成 Customizer 参数控件。
- `.stl` / `.3mf` 既可以通过 `PreviewRequest` 获取预览 artifact，也可以在
  已存在文件查看场景下通过 `FileRead` 消费二进制内容。
- 如果后续需要恢复源码读取限制，应先在协议与产品层明确用户可见影响：
  限制 `.scad` 读取会直接影响 Web 参数面板、源码附加视图和预设工作流。

## 3. 文件路径可见性

- `PathHandle` 的 `path_segments` 是相对 workspace root 的逻辑路径，不
  包含浏览器宿主机的绝对路径。任何看起来像“绝对路径”的展示仅在 server
  日志里出现；client 永远只通过 `PathHandle` 引用文件。
- 多 workspace 切换依赖 server 广播的 `WorkspaceCurrent` / `WorkspaceList`。
  浏览器不能通过本地 API 枚举目录。

## 4. Service Worker 与 wasm 更新

- dev 模式禁用 Service Worker（`vite-plugin-pwa` 的 `devOptions.enabled
  = false`）。生产构建发出 hashed wasm 文件名（`studio_web_wasm-<hash>.wasm`），
  Service Worker 只缓存稳定版本。
- 一旦 wasm 产物版本更新，hashed 文件名变化会让 Service Worker 在下次
  加载时拉到新资源；smoke 启动前仍然清空 `serviceWorker` 注册与
  `Cache Storage`，避免上次测试遗留缓存污染。

## 5. 文档标签系统（Phase 6）

- 文档标签（`DocumentTab`）存储在 Zustand UI store 中，只保留 `id` /
  `label` / `path` / `kind` 四个字段；不写 `localStorage`、不走
  `IndexedDB`。
- **刷新页面后所有文档标签清空**。这是 Phase 6 的默认行为，Phase 7/8
  若有需求可以评估会话级持久化，但必须走 wasm snapshot 或协议层
  session reclaim，不得在 React 侧自行实现。
- Viewer 组件负责按需请求文件内容（`FileRead` / `PreviewRequest`），
  组件卸载后释放 object URL 与 mesh handle，不在 store 缓存。

## 6. Markdown 渲染支持范围

- Web 端 Markdown 预览使用 `@uiw/react-markdown-preview`，并通过按需加载
  Mermaid 渲染 `mermaid` fenced code block。
- Markdown 链接默认在新浏览器 tab 打开，使用 `rel="noopener noreferrer"`。
- HTML、URL、图片 URL 和 Mermaid SVG 都经过安全处理。危险协议、iframe、
  内联事件属性和 SVG 危险链接会被拒绝或清理。
- 若后续需要与桌面端完全一致的 CommonMark / GFM 行为，需要继续补充
  跨端语法用例；不能恢复旧的简化解析方案。

## 7. 图片查看限制

- 图片通过 `FileRead` 以二进制形式拉取；超过 20 MiB 的图片直接拒绝
  （显示 `image too large: {bytes}`），避免浏览器因一次性 Blob 解码
  爆内存。
- 图片解码后自然尺寸超过 4096×4096 时记录 `console.warn`；不强制拒绝，
  但用户会通过 DevTools 看到告警。
- Object URL 在组件卸载或 `path` 变化时 `URL.revokeObjectURL`，避免
  内存泄漏。

## 8. 3D 交互与相机

- Web 端真实显示路径由 `viewers/mesh-three.ts` 的 Three.js WebGL renderer
  承担；wasm 侧只负责协议桥接与 mesh / 参数解析等纯逻辑。
- 工具条覆盖相机预设、render mode、projection、grid、axis、build plate
  与 shadow 开关。新增控件都写入 `MeshViewerOptions`，再由 Three.js 场景
  消费，不只改变 UI 文案。
- wasm `renderer_*` 仍是桥接占位 API，不参与当前 Web 端真实渲染路径。

## 9. 参数编辑与预设

- Web 参数面板通过 `studio-web-wasm` 调用 `studio_common::parse_parameters`，
  按共享 `ParameterEntry` / `ParameterValue` 语义渲染数值、布尔与枚举控件。
- 当前 `defines` 使用 `studio_common::parameter_entries_to_cli_defines` 格式化；
  初始预览、参数修改、单项恢复默认值、加载预设和导出 / 切片器请求都消费同一份
  当前参数值。
- 预设默认写入同级 `*.scad.json`，磁盘结构为共享 `PresetFile`：
  `{ "presets": { "<name>": { "<param>": <ParameterValue> } } }`。
- 历史 `<stem>.presets.json` 仍兼容读取，但不会作为默认写入目标。

## 10. 导出与切片器（Phase 7）

- `SlicerList` 面板列出的是 **server 机器**上的安装，不是浏览器机器。
  面板空列表显示 "no slicer configured"，web 端绝不尝试调起本地切片器
  进程（参见 §1）。
- `ExportRun` 的 `output_path` 在协议上是 server 侧 `PathBuf`；浏览器
  无法知道 server 的绝对路径。web 端目前发相对文件名（如
  `params-cube.stl`），由 OpenSCAD CLI 相对 server 进程的 cwd 解析。
  涉及真实多 workspace 的输出路径语义待 Phase 8+ 评估。

## 11. 配置与设置

- `/settings` 路由可编辑 OpenSCAD path、slicer name/path 与
  `floating_panel_opacity`，保存后更新同会话共享配置快照。
- workbench 的 `.scad` 预览、导出和切片器请求都消费同一份配置快照；
  不再存在设置页可保存但工作台不生效的分叉状态。
- `AppConfig` 不进入 Zustand UI store；设置页和 workbench 只通过配置模块
  维护快照与状态，避免把协议数据混入 UI 壳层状态。

## 12. 日志面板与文档刷新

- 日志面板挂在 Inspector 底部，使用 React 内 ring buffer（默认 50
  条），不写入 store、不做持久化。监听的事件：transport open/close、
  handshake accepted、watch resubscribed、watch push、文档刷新触发。
- watch 事件会刷新打开中的 Markdown、图片、mesh、`.scad` 与预设文件。
  当协议事件只给目录级路径时，Web 端采用保守刷新策略，并在日志里说明
  是目录变化触发。
