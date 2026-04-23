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

## 2. 源码暴露边界

- `web_file_read_capability()` 默认把 `.scad`、`.stl`、`.3mf` 列入
  `denied_extensions`。这是**访问/暴露边界**（保护源码），不是平台能力
  差异——即便浏览器理论上可以读二进制，server 也拒绝返回 `.scad`
  字节流。
- `viewers/scad-split-viewer.tsx` 因此只能通过 `PreviewRequest` 拿到
  mesh 结果；源码面板显示 `source unavailable: 当前 client 不允许读取
  .scad 文件`。
- 如果将来放开某类扩展（例如公司内部工具希望显示源码），在
  `app-server-protocol::web_file_read_capability` 修改，不是在 client
  侧加绕过路径。

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

- `viewers/markdown-parser.ts` 是项目内极简 parser，覆盖：`#` / `##` /
  `###` 标题、`*` 与 `-` 无序列表、围栏代码块（```lang）、段落、行内
  代码 `` ` `` 与链接 `[text](url)`。
- **不**支持 GFM 扩展（表格、任务列表、删除线、脚注等）、setext 标题、
  有序列表、块引用、HTML 嵌入、嵌套列表。若后续 Phase 需要这些能力，
  再评估引入外部库（见 plan-00 §Phase 6 决策）。

## 7. 图片查看限制

- 图片通过 `FileRead` 以二进制形式拉取；超过 20 MiB 的图片直接拒绝
  （显示 `image too large: {bytes}`），避免浏览器因一次性 Blob 解码
  爆内存。
- 图片解码后自然尺寸超过 4096×4096 时记录 `console.warn`；不强制拒绝，
  但用户会通过 DevTools 看到告警。
- Object URL 在组件卸载或 `path` 变化时 `URL.revokeObjectURL`，避免
  内存泄漏。

## 8. 3D 交互与相机（Phase 7）

- `packages/studio-web/src/canvas/camera-*` 负责相机状态机与工具栏/
  状态栏输入处理。桩 renderer（`renderer_create` 返回 Err）不会真的
  把 camera 状态画出像素；本 Phase 只对齐交互与协议接线，不涉及真实
  GPU 渲染。
- 具体 renderer 接入要等到未来 Phase（见 plan-00 Phase 8 之后）。

## 9. 参数编辑与预设（Phase 7）

- Web 端不能通过 `FileRead` 读 `.scad` 源码（server `denied_extensions`
  覆盖了 `.scad`；见 §2）。因此参数编辑**不做源码解析**：用户在参数
  面板里手动输入 `name=value` 形式的 overrides，这些字符串作为
  `PreviewRequest.defines` 直接送回 server，由 OpenSCAD CLI 处理。
- 预设以同级 `<source>.presets.json` 文件持久化，读写走 `FileRead` /
  `FileWriteText`。格式：`{ "version": 1, "presets": [{ name, defines:
  string[] }] }`。
- 协议 `ParsedParameters` / `PresetFile` 类型虽然已经在
  `app-server-protocol` 中导出，但 `PreviewRequest` 服务端链路目前不会
  返回解析后的参数定义；这条路径尚未打通，web 端无法接。已记录为
  `docs/known_issues.md` 中的独立条目，不视为平台限制。

## 10. 导出与切片器（Phase 7）

- `SlicerList` 面板列出的是 **server 机器**上的安装，不是浏览器机器。
  面板空列表显示 "no slicer configured"，web 端绝不尝试调起本地切片器
  进程（参见 §1）。
- `ExportRun` 的 `output_path` 在协议上是 server 侧 `PathBuf`；浏览器
  无法知道 server 的绝对路径。web 端目前发相对文件名（如
  `params-cube.stl`），由 OpenSCAD CLI 相对 server 进程的 cwd 解析。
  涉及真实多 workspace 的输出路径语义待 Phase 8+ 评估。

## 11. 配置与设置（Phase 7）

- `/settings` 路由加载完整 `AppConfig` JSON 后只显示少数常用字段
  （OpenSCAD path、recent workspaces 数量、slicers 数量）。`AppConfig`
  是协议层数据，只放在该路由的本地 `useState`，**不**进入 Zustand
  store；保证 UI store 的协议纯净边界。

## 12. 日志面板与 `.scad` 自动重渲染（Phase 7）

- 日志面板挂在 Inspector 底部，使用 React 内 ring buffer（默认 50
  条），不写入 store、不做持久化。监听的事件：transport open/close、
  handshake accepted、watch resubscribed、watch push、自动重渲染触发。
- 当激活 tab 是 `.scad` 且 watch 推送的 `changed_paths` 命中当前文件
  时，WorkbenchLayout 递增 `scadRefreshSignal`，ScadWorkbench 将该
  signal 和 defines 一起作为 key 传给 `ScadSplitViewer`，触发其内部
  effect 重新发 `PreviewRequest`。
