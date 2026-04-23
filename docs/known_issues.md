# 已知问题记录

## 2026-04-23 20:10:00: `WatchChangedEvent.changed_paths` 只给目录级路径，Web 端无法精确匹配文件

- 来源：执行 Phase 7 步骤 E（`.scad` 自动重渲染）时，Playwright smoke 观察 `client_drain_events` 产出的 `WatchEvent` payload：`changed_paths` 往往只包含目录级 `PathHandle`（`path_segments: []`），没有被修改的具体文件 handle。
- 原因：`app-server-host::watch` 聚合 notify 事件后目前只回传监听的目录 handle；文件级变更事件未投递到 `WatchChangedEvent`。
- 影响范围：
  - Web 端 WorkbenchLayout 不能仅凭 `changed_paths` 判断"当前激活的 scad 文件是否被修改"。现行退让方案：凡是 scad tab 激活且有任何 watch 事件，均触发 refreshSignal，smoke 写入 "auto rerender triggered by {path} (directory change)"。桌面端不受影响（桌面 client 可直接观察 notify 事件）。
  - 若多个文件同时变更，Web 端会做一次粗粒度重渲染而不是按文件去抖；对 preview 成本可控但理论上浪费。
- 可能的解法：
  - 服务端把 notify 事件里的文件 handle 透传到 `WatchChangedEvent.changed_paths` 而不是只回目录；需要在 `app-server-host::watch` 中按事件类型填充 `changed_paths`。
  - 或在协议层新增 `WatchChangedEvent.reason`（`DirectoryChanged` vs `FileChanged { paths }`）让 client 显式知道粒度。
- 当前处理方式：Phase 7 web 端先走"目录级触发重渲染"方案；日志 tail 明示 "directory change" 后缀，避免假装自己做了文件级匹配。

## 2026-04-23 20:05:00: `PreviewRequest` 链路未返回 `ParsedParameters`，Web 参数面板无法自动解析 .scad 参数

- 来源：执行 Phase 7（`prompt-archives/2026042300-studio-web-feature-parity/plan-00.md` §Phase 7 步骤 B）时，按计划应"优先走协议层 `ParsedParameters`，无法做到时回退到源码字符串解析"。审查 `crates/app-server-core/src/preview.rs` 与 `crates/app-server-host/src/dispatcher.rs` 后确认，`CommandSuccess::PreviewReady` 当前只带回 `PreviewReadyResponse { requested_kind, artifact }`，不包含 `ParsedParameters`；`studio-common` 的 re-export 也没有把解析能力接到 `ManagedClient` 的正常流程。
- 原因：协议层 `ParameterDefinition` / `ParsedParameters` 类型虽已存在，但没有命令能把它们投递给客户端；web 端想拿到参数定义就必须新增协议命令或给 `PreviewRequest` 加返回字段——都超出 Phase 7 范围（Phase 7 明确禁止改协议/server-core）。
- 影响范围：
  - Web 参数面板只能依赖"用户手工输入 `name=value` 后由 `PreviewRequest.defines` 透传"这一退化路径，没法像桌面端一样先解析 `.scad` 顶部的变量默认值再渲染成带类型控件。
  - `docs/web-platform-limits.md §9` 明确声明这条限制；但从协议层看，这属于"能力缺口"而非"平台差异"，放已知问题而不是平台限制页。
  - 未来若 Phase 8+ 想做参数 UI 的"恢复默认值"语义，必须先补协议层支持。
- 可能的解法：
  - 给 `PreviewRequest` / `PreviewReadyResponse` 增加可选 `ParsedParameters` 字段；或单独添加 `ParametersInspect` 命令返回 `ParsedParameters`。任一都要同步 `app-server-core` 的解析实现与 `studio-common::ManagedClient` 的回执处理。
  - 另起 plan，在其中定义协议扩展、兼容策略（`#[serde(default)]`）和两端同步改动。
- 当前处理方式：Phase 7 web 端参数面板走手工 `name=value` 回退路径；`docs/web-platform-limits.md §9` 记录用户可见约束；本条记录解释为什么不是"平台限制"。

## 2026-04-23 20:05:00: `ExportRun.output_path` 要求 server 侧绝对 `PathBuf`，web 端无法决定目标目录

- 来源：执行 Phase 7 步骤 C（Export 流）时，检查 `app-server-protocol::ExportRunRequest.output_path: PathBuf` 与 `crates/app-server-core/src/export.rs::export_model`。Web 端发的是浏览器侧 UTF-8 字符串，该字符串被 server 作为 CLI `-o` 参数传给 OpenSCAD，按 server 进程 cwd 或绝对路径解析。
- 原因：协议在定义 `ExportRunRequest` 时假定客户端知道 server 机器真实文件系统；这在桌面端成立，在 web 端不成立。没有 `PathHandle`-化的导出接口，也没有"写到 workspace 下某个相对路径"的语义。
- 影响范围：
  - Web 端 Phase 7 导出 UI 只接受用户输入文件名（默认 `<stem>.stl`），实际落地到 server 进程 cwd，不保证在 workspace 根目录下，也不保证对用户可见。
  - smoke（`@export-slicer`）只能断言 `export done|export error`，不能验证导出文件位置。
- 可能的解法：
  - 扩展协议：`ExportRunRequest.output` 改为 `PathHandleWritable`（新增路径类型），由 server 解析为 workspace 根下的相对路径；或复用现有 `PathHandle` 作为目录 + 文件名两字段。
  - 需求上若只要求"导出到 workspace 某目录"，可先约定 server 端默认写到 `workspace_root/exports/<filename>`。
- 当前处理方式：Phase 7 web 端记录在 `docs/web-platform-limits.md §10`；协议不改，以相对文件名透传为准。

- 来源：执行 `prompt-archives/2026042200-studio-app-server-unification/plan-00.md` 的验收过程中，已能通过 workspace 构建/测试和桌面二进制编译确认 `studio-app` 可进入运行路径，但当前会话没有桌面自动化能力，无法在同一条执行链中继续点击菜单、打开工作区、切换文档标签并观察真实窗口渲染。
- 原因：当前环境具备编译、测试和进程级启动能力，但不具备桌面 GUI 级别的交互自动化工具；已有自动化测试主要覆盖状态机和纯逻辑，不能等价替代完整的人机交互回归。
- 影响范围：
  - Phase 1、Phase 5、Phase 8 中要求的桌面 GUI 人工回归目前只能以“启动 smoke + 现有自动化测试 + 代码复用证据”部分替代，无法在本会话里做到逐点击验。
  - 后续如果出现只在真实桌面交互中暴露的问题（菜单焦点、窗口拖拽、平台快捷键、Open Folder 对话框等），当前自动化覆盖未必能提前发现。
- 可能的解法：
  - 为 `studio-app` 增加 repo-local 的桌面 smoke 模式或更细粒度的 UI harness，至少覆盖打开工作区、切换标签、触发 viewer 渲染与 watcher 刷新。
  - 引入可在本地桌面环境执行的 GUI 自动化工具链，并将关键回归场景沉淀为脚本。
  - 在有人值守的桌面环境中补一轮人工回归，并把结果补写回对应 `plan-00-result.md`。
- 当前处理方式：本轮先以 `cargo check --workspace`、`cargo test --workspace`、`cargo check -p studio-app --bin studio-app` 以及共享 UI / 共享状态代码证据作为主要验收依据；交互式桌面回归能力缺口单独记录为已知问题，供后续 Phase 5 / Phase 8 继续处理。

## 2026-04-07 21:39:25: DocumentWorkspace 迁移后仍保留 `DocumentKey` 与 `TabId` 双身份体系

- 来源：对 `crates/studio-app/src/app.rs`、`crates/studio-app/src/main.rs`、`crates/studio-app/src/studio_document.rs`、`crates/studio-app/src/viewer_tab/`、`crates/studio-app/src/markdown_tab.rs` 的迁移代码审查。
- 原因：文档工作区已经以 `DocumentKey` 作为主身份，但运行时消息分发仍依赖 `legacy_tab_id()`，`ViewerTab`/`MarkdownTab` 继续实现 `WorkTab`，`main.rs` 仍通过 `document_by_legacy_tab_id_mut()` 查找会话。
- 影响范围：
  - Phase 3 若要彻底移除旧 `tab_system`，仍需先清理这条遗留依赖链。
  - 文档身份与运行时消息身份分裂，后续改动容易在 `DocumentKey` 与 `TabId` 之间引入不一致。
  - 现有 `studio_app_tests` 在 `cfg(test)` 下把会话类型替换成 `()`，无法覆盖这条真实运行时代码路径。
- 可能的解法：
  - 让运行时事件直接携带 `DocumentKey` 或由 `DocumentWorkspace` 维护稳定的会话句柄，去掉 `legacy_tab_id()`。
  - 将 `ViewerTab`/`MarkdownTab` 从 `WorkTab` 抽象中彻底剥离，避免继续保留“可被旧 tab 系统驱动”的假接口。
  - 为真实会话分发路径补测试，避免 `cfg(test)` 绕开生产分支。
- 当前处理方式：仅记录为 review finding，作为 Phase 3 前的结构整理输入。

## 2026-04-07 21:39:25: DocumentWorkspace 真实运行时分支缺少自动化测试

- 来源：对 `crates/studio-app/src/app.rs`、`crates/studio-app/src/main.rs`、`crates/studio-app/src/work_area.rs` 的 DocumentWorkspace 迁移代码审查。
- 原因：当前 `studio_app_tests` 只验证通用状态与欢迎态，未覆盖真实文档会话下的打开文件、watch 回调、Viewer/Markdown 分发与工作区轨道交互；生产代码中的真实会话分支仍主要依赖 `cargo build` 做编译级回归。
- 影响范围：
  - 后续调整 `DocumentWorkspace` 接线、文件监听或 Viewer/Markdown 路由时，较难通过自动化测试及时发现行为退化。
  - 真实运行时路径的回归保障弱于纯状态层测试。
- 可能的解法：
  - 为 `main/work_area/app` 增加更贴近运行时的集成测试或最小会话桩，覆盖打开文件、激活切换、watch 消息分发和空状态切换。
  - 在完成 `DocumentKey` / `TabId` 收敛后，补一组面向真实会话分支的回归测试，避免继续依赖 `cfg(test)` 下的轻量替身。
- 当前处理方式：本轮先保留为已知问题；当前仅通过 `cargo build` 与状态层测试保证迁移不破坏编译和核心纯逻辑。

## 2026-04-02 16:47:56: 本地环境缺少可验证 3MF 彩色预览的 OpenSCAD CLI / Nightly

- 来源：为 3MF 彩色预览计划检查本机 OpenSCAD 环境时，执行 `command -v openscad` 与读取 `OPENSCAD_PATH`，结果均为空。
- 原因：当前工作机未安装可直接调用的 OpenSCAD CLI，因此无法确认是否具备支持彩色 3MF 预览的 Nightly 能力。
- 影响范围：
  - 无法在本机完成 `scad -> OpenSCAD 3MF -> 彩色预览` 的端到端闭环验证。
  - 后续实现阶段只能先依赖 3MF fixture、单元测试和用户环境联调来验证颜色解析与渲染。
- 可能的解法：
  - 在执行阶段安装 OpenSCAD Nightly，并通过 `OPENSCAD_PATH` 或设置窗口显式指向该版本。
  - 在仓库中加入最小彩色 3MF fixture，用于脱离 OpenSCAD 环境验证解析与渲染链路。
  - 将“Nightly 环境下的人工联调”列为独立验收项，而不是与纯单元测试混在一起。
- 当前处理方式：已补 `tests/three_mf_tests.rs`、`tests/mesh_tests.rs`、`tests/pipeline_tests.rs` 等回归测试，自动化验证覆盖 3MF 解析与颜色渲染协议；在具备 Nightly 的环境前，不宣称完成 `scad -> OpenSCAD 3MF -> 彩色预览` 的端到端人工验收。

## 2026-04-01 13:20: feature-roadmap 与现行 plan 在 3MF 解析范围上不一致

- 来源：对照 [docs/feature-roadmap.md](/Users/krhougs/.config/superpowers/worktrees/scad-studio/codex-full-features/docs/feature-roadmap.md) 与 [plan-00.md](/Users/krhougs/.config/superpowers/worktrees/scad-studio/codex-full-features/prompt-archives/2026033101-full-features/plan-00.md)。
- 原因：roadmap 仍包含“3MF 文件解析（支持颜色信息）”，但当前 plan 仅覆盖 3MF 导出，不包含 3MF 导入解析。
- 影响范围：即使按现行 plan 完成所有 Phase，也无法直接把 roadmap 全部未完成项勾选为已完成。
- 可能的解法：
  - 单独补一轮 3MF 解析计划，明确是否需要颜色贴图、零件层级和 ZIP 容器读取。
  - 或者回写 roadmap/plan，明确当前版本仅支持 3MF 导出，不支持导入解析。
- 当前处理方式：本轮已实现 3MF 预览解析并同步更新 `docs/feature-roadmap.md`，该问题不再阻塞后续开发判断。
