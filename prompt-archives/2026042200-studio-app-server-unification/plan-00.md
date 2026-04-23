# Studio 统一 App Server 架构与多端协议重构 — 执行计划

## Context

- 当前基线锁定在提交 `7b232bdbdb751da84adbe6ec7d4fa28175b8cf97`（短哈希 `7b232bd`）。本计划中的所有重构都必须保护该提交中**所有已完成功能、构建结果、测试结果和桌面 GUI 可用性**。
- 用户已明确否定“只做一个 Web MVP”的方向。本轮目标是面向长期演进的统一架构，而不是一次性浏览器适配。
- 用户已拍板：`app server` 是唯一能力层，统一承接文件系统 I/O、目录列举、文件读取、文件监听、OpenSCAD 与预览相关外部调用，以及未来云 Agent / 沙盒扩展入口。
- 用户已拍板：桌面 GUI 与网页必须走**同一份 app server 核心代码**与**同一份 protocol**，不允许桌面保留绕过协议的本地直连能力路径。
- 用户已拍板：GUI 场景下，app server 在同进程内以 Tokio task 启动，客户端与 server 通过 `tokio::mpsc` transport 通信；网页与未来其他 client 通过 WebSocket 或其他 transport 通信。
- 用户已拍板：transport 和 protocol 必须彻底分离。protocol 只描述命令、事件、错误、能力与数据模型，不绑定 HTTP、WebSocket 或 `mpsc`。
- 用户已拍板：根 crate `scad-studio` 不应包含任何业务代码，只作为 workspace 根使用。
- 用户已拍板：以当前目标看，现有 Studio 代码至少拆分为 `studio-app`、`studio-web` 和 `studio-common` 三个包，再基于这三个包继续细化边界。
- 用户已拍板：`studio-common` 允许少量 `egui` 基础类型和无平台共享 UI 状态，但不承载页面级布局、widget 组装、`egui::Context` 驱动逻辑或平台事件接线。
- 用户已拍板：`studio-common` 管共享状态与行为，`scad-ui` 管可复用组件与呈现；若某段代码主要负责“画出来”，优先归入 `scad-ui`。
- 用户已拍板：本轮删除独立 `Viewer` 的产品边界与可执行边界，但**不直接删除 `scad-viewer` crate**——Studio 仍以库方式消费 `scad_viewer::app/ui` 的部分内容。Phase 1 等价覆盖核实后，删除 `scad-viewer` 的 `[[bin]]` 与独立 Viewer 应用专属 UI/依赖，crate 形态变为纯共享 lib；Phase 4 把剩余 lib 内容继续按 `studio-common` / `scad-ui` 边界规则归位；Phase 7 在内容归零后物理删除目录，否则保留为纯共享 lib。本轮目标不是把预览”迁回” Studio。
- 用户已拍板：预览状态机放入 `studio-common`；桌面与网页只保留各自的预览 UI 呈现层。
- 用户已拍板：浏览器中需要完整实现 fake chatbox（纯前端假 UI）以及“目录树 + 当前目录文件列表”。
- 用户已拍板：fake chatbox 当前阶段只保留在 `studio-web`，不进入 `studio-common`。
- 当前仓库现实结构仍然是根 crate `scad-studio` 作为桌面入口，workspace 当前只有 `scad-data`、`scad-scene`、`scad-ui`、`scad-viewer` 四个成员；下文出现的 `studio-app`、`studio-web`、`studio-common`、`app-server-*` crate 名称均是目标落点，不代表这些 crate 已经存在。

## 目标

1. 建立一套**统一的 App Server Core + Protocol + Transport Adapter** 架构，作为桌面 GUI、网页端和未来其他 client 的共同能力底座。
2. 保证桌面 GUI 与网页端消费**同一套 protocol**，且桌面端不保留任何绕过 protocol 的 I/O 或外部调用捷径。
3. 将根 crate `scad-studio` 收敛为纯 workspace 根，不再承载任何业务代码。
4. 将现有 Studio 代码至少拆分为 `studio-app`、`studio-web`、`studio-common`，并在拆分过程中明确每个包的职责边界。
5. 本轮删除独立 `Viewer` 的产品与可执行边界（Phase 1 删 `[[bin]]` + 独立 Viewer 应用专属 UI/依赖），瘦身 `scad-viewer` 为纯共享 lib；Phase 4 把剩余 lib 内容继续按目标 crate 边界归位，Phase 7 视情况物理删除 crate 目录。消除 Studio 与独立 Viewer 之间的重复职责和重复接线。
6. 在浏览器端提供完整的 Studio 主界面骨架，包括：目录树、当前目录文件列表、预览区域、fake chatbox。
7. 在整个重构过程中，**不得破坏锁定提交 `7b232bd` 中所有已完成的功能**。任何阶段若出现功能回退，必须回到对应阶段修正并重新回归。

## 非目标（本轮不做）

- 不在本轮实现真实 Agent 对话能力；fake chatbox 只实现前端假 UI 与交互壳层。
- 不在本轮实现离线纯浏览器模式。
- 不在本轮绑定具体云厂商部署方案。
- 不在本轮设计“桌面专用私有协议”或“Web 专用私有协议”；协议必须统一。

## 硬约束与设计原则

### 已知问题持续登记（每个 Phase 都受约束）

- 任意 Phase 执行过程中若确认存在当前无法直接解决、又会影响后续开发判断的问题（例如某个原计划方案在实际编码时撞死、某个外部依赖发现 bug、某项验收暂时无法通过等），必须**在该 Phase 退出前**同步更新 `docs/known_issues.md`，按 AGENTS.md 要求记录：发现时间、来源、原因、影响范围、可能的解法、当前处理方式。
- 禁止把这类问题留到 Phase 8（终态文档交付）才统一登记；Phase 8 只做"风险评估收敛 → 最终结论"的归档工作，不能用作"漏登记的兜底"。
- 该约束同时适用于第二轮以后才发现的问题（如 codex review 找出的硬冲突在执行中再次确认无法按现 plan 修），必须立即登记并触发对应 Phase 的 plan 修订流程。

### 锁定基线保护

- `7b232bd` 是本轮唯一功能基线。
- 所有 Phase 的计划与验收都必须显式保护该基线中的既有功能，包括但不限于：桌面 GUI 可启动、现有已完成功能可用、现有测试可通过、现有构建流程可通过。
- 禁止以“重构中暂时坏掉”为理由破坏基线能力。若某阶段导致回退，必须在该阶段内修正并恢复。
- 对于计划明确要移除的独立 `scad-viewer` 产品边界，保护对象是**锁定基线中的用户可见预览能力**，而不是旧二进制壳本身。等效能力的核实在 Phase 1 完成；核实通过后，Phase 1 内删除 `scad-viewer` 的 `[[bin]]` 与独立 Viewer 应用专属 UI/依赖，不再要求 `scad-viewer` 二进制可启动；但 `scad-viewer` 作为共享 lib 仍保留在 workspace 与根 crate 依赖中（`viewer_tab.rs` 仍需要它的 `app::UiCommand` / `ui::*`）。Phase 4 在 crate 物理拆分时把瘦身后的 `scad-viewer` lib 内容继续按 `studio-common` / `scad-ui` 边界归位；Phase 7 在内容归零后物理删除 crate 目录，否则保留为纯共享 lib。

### 协议与传输分离

- protocol 层必须平台无关、传输无关。
- transport 层必须以 trait 抽象，允许 `tokio::mpsc`、WebSocket、未来其他 transport 复用同一份 protocol。
- protocol 需要显式考虑平台和 I/O 差异，例如：同步/异步完成、文件系统可见路径差异、能力协商、错误模型、取消语义、watch 事件节流与重连。

### 产品结构约束

- Studio 主界面采用**单窗口单 workspace session**。
- 主界面至少包含：目录树、当前目录文件列表、预览区域、fake chatbox。
- 删除独立 `Viewer` 产品边界，但预览能力始终以 Studio 既有能力的形式保留，不把它表述为“迁回” Studio。

### 计划中的 crate 与模块边界

- `crates/app-server-protocol`：只放 protocol 类型、命令、事件、错误、能力协商和路径模型。
- `crates/app-server-transport`：只放 transport trait、envelope、请求/推送抽象与 transport 级错误，不放业务逻辑。
- `crates/app-server-core`：只放 workspace、file、watch、preview 等服务与业务编排。
- `crates/app-server-host`：承载 server host 运行时与 transport bridge，负责把 `app-server-core` 暴露给 GUI 的 `tokio::mpsc` transport 与网页/未来 client 的 WebSocket transport。
- 根 `scad-studio`：只保留 workspace、统一工具链配置与顶层文档，不承载任何业务代码。
- `crates/studio-common`：承接桌面与网页共享的 Studio 领域模型、客户端状态机、文档/会话模型、工作区树与当前目录文件列表所需的共享表示、预览请求编排接口、与 fake chatbox UI 无关的共享逻辑。
- `crates/studio-app`：桌面专属外壳，承接桌面 GUI 入口、窗口生命周期、平台菜单、同进程 `app-server-host` 启动与 `tokio::mpsc` 客户端接线，消费 `studio-common`，但不直接碰 I/O 或外部调用。
- `crates/studio-web`：网页专属外壳，承接网页客户端入口、浏览器主界面骨架、目录树 + 当前目录文件列表 + fake chatbox UI、网页 transport 接线，消费 `studio-common`，但不直接碰 I/O 或外部调用。
- `scad-scene`：继续承接渲染与几何相关公共能力，且**必须能在 `wasm32-unknown-unknown` 目标下编译通过**，以便浏览器端用同一份渲染代码完成预览；当前带平台 I/O 的模块（如 `system_fonts.rs`）必须按 `cfg(target_arch = "wasm32")` 做替代实现或显式禁用入口。wgpu 在浏览器上锁定 WebGPU 后端。
- `scad-ui`：收敛为**纯跨端共享 UI 基础层**，主要依赖 `egui` / `egui_commonmark` 这类与平台无关的 crate；**允许谨慎依赖 `scad-scene` 的纯渲染数据结构**（如 `CameraMatrices` / `OrbitalCamera` 等纯数据类型），但禁止依赖 `scad-scene` 的 renderer / window / GPU 生命周期能力（与 AGENTS.md 中 `studio-common` 对 `scad-scene` 的依赖宽容度一致）。现有 `muda`、`winit`、`platform_support`、`font_setup` 中的桌面专属逻辑必须剥离并迁入 `studio-app`，不新建 `scad-ui-desktop` 中间 crate。`scad-ui` 必须能在 `wasm32-unknown-unknown` 目标下 `cargo check` 通过。若 `scad-viewer` 中存在纯 UI 基础组件，应优先迁入这里；scene-aware overlay（依赖 camera/scene 纯数据）也可迁入 `scad-ui`，但 GPU/window 接线必须留在 `scad-scene` 或端壳层。
- `studio-common` 与 `scad-ui` 的职责划分：`studio-common` 只管理共享状态、共享行为和少量无平台 UI 状态；`scad-ui` 承接真正可复用的组件、视觉规范、壳层组件与 widget 组合。
- `crates/scad-data` 当前承担的能力（OpenSCAD 调用、文档模型、参数/预设、文件 watcher、导出、配置）按以下原则归位（详细模块清单见"风险评估"第 5 项）：
  - 文件 I/O、文件 watcher、`notify`、`stl_io`、`dirs`、子进程调用以及任何依赖本地系统资源的逻辑，统一迁入 `app-server-core` 的对应 service。
  - 与协议交互的纯数据类型（文档模型、参数模型、预设模型、错误模型等可序列化结构）迁入 `app-server-protocol`，由 server 与 client 共同消费。
  - 配置与用户偏好统一由 server 维护（进 `app-server-core`），client 通过协议读写，不在 client 端持有副本。
  - `rfd` 文件对话框不进 `app-server-core`：桌面文件选择器留在 `studio-app`（继续用 `rfd` 并升级到当前最新可用版本），网页文件选择器由 `studio-web` 调用浏览器原生能力实现。
  - **`crates/scad-data` 在 Phase 3 内一刀切迁移并物理删除**，不允许保留 re-export / facade 壳。Phase 1 已将 `scad-viewer` 瘦身为纯共享 lib（删除 `[[bin]]` 与独立 Viewer 应用专属代码），因此 Phase 3 一刀切需要同步切根 crate 与瘦身后的 `scad-viewer` 这两个调用方到新接口。
  - **过渡期 in-process 调用面**：Phase 3 一刀切后，`app-server-core` 必须显式暴露稳定的 public Rust 服务 API；根 crate 与瘦身 `scad-viewer` 在 Phase 3 → Phase 5 之间直接调用该 API，不走协议；Phase 5 GUI 接入 mpsc transport 后，`studio-app` 改为走 protocol 路径，根 crate 对 Rust API 的直接消费窗口由 Phase 5 验收强制关闭。详细约定见"风险评估"第 5 项。

### 三个 Studio 包的最低能力边界

- `studio-common` 只放**跨端共享且不依赖平台入口**的内容：
  - 文档与 workspace session 领域模型；
  - 目录树、当前目录文件列表、预览面板所需的共享状态机；
  - 预览请求编排、预览任务状态、当前激活预览目标与预览错误状态；
  - 统一 protocol client facade 所需的端无关接口；
  - 纯函数、共享事件、共享错误模型。
  - 明确不包含 fake chatbox 当前阶段的本地 UI 状态。
  - 允许少量 `egui` 基础类型和无平台共享 UI 状态，例如面板开关、少量颜色/标识符、预览面板共享状态。
  - 不允许页面级布局、widget 组装、`egui::Context` 驱动逻辑、浏览器 API、桌面平台 API 或事件循环接线。
  - 允许依赖 `app-server-protocol`。
  - 禁止依赖 `app-server-transport`。
  - 允许谨慎依赖 `scad-scene` 的纯渲染数据结构，但禁止依赖 renderer / window / GPU 生命周期能力。
  - 若某段逻辑已经开始承担“组件如何呈现、如何布局、如何被绘制”的职责，应迁回 `scad-ui`。
- `studio-app` 只放**桌面专属外壳**：
  - 桌面窗口、多窗口生命周期、平台菜单、快捷键；
  - 同进程 server host 启动；
  - `tokio::mpsc` transport 客户端接线；
  - 桌面平台特有 UI 编排与预览区域呈现。
- `studio-web` 只放**网页专属外壳**：
  - Web 入口、浏览器环境接线；
  - Web transport 客户端接线；
  - 浏览器主界面、fake chatbox、目录树与当前目录文件列表的网页端呈现；
  - 网页端预览区域呈现；
  - 不可避免的网页平台差异处理。
- fake chatbox 当前阶段视为网页专属假 UI，不形成跨端共享领域模型；若未来引入真实 Agent 或统一聊天协议，再单独规划是否抽入共享层。
- 预览属于跨端共享正式能力，因此状态机必须统一放在 `studio-common`，不得在 `studio-app` 与 `studio-web` 中各自维护一套独立预览状态。

### `studio-common` 与 `scad-ui` 的判定规则

- 如果一段代码主要回答“当前处于什么状态、收到什么命令、转移到什么状态”，优先归入 `studio-common`。
- 如果一段代码主要回答“这个东西长什么样、怎么排版、怎么被画出来、如何组合多个 widget”，优先归入 `scad-ui`。
- 若代码同时混合了状态机和呈现逻辑，应优先拆分，而不是把整块内容塞进 `studio-common`。
- 若某段逻辑既不属于 `studio-app` 也不属于 `studio-web`，优先评估是否应放入 `studio-common`，而不是继续堆在任一端壳层中。

## 风险评估与未决项

以下条目在动手前已知存在不确定性，必须在对应 Phase 落地前先给出明确结论；若结论与本计划冲突，先回头修订计划再继续推进，禁止在执行过程中默默作出技术拍板。

1. **egui 在 wasm 目标上的可复用度**
   - 影响范围：`scad-ui` 是否能被 `studio-app` 与 `studio-web` 同时复用，以及 `studio-common` 中允许保留的少量 `egui` 基础类型在 wasm 目标下是否有 ABI/特性差异。
   - 触发 Phase：Phase 2 协议数据结构、Phase 4 crate 拆分、Phase 6 浏览器接入。
   - 处理要求：Phase 2 内只产出 `scad-ui` / `scad-scene` 的剥离与 wasm 化方案文档（不强制 wasm check 通过）；正式实施与 wasm `cargo check` 验收都放在 Phase 4。Phase 2 仅强制 `app-server-protocol` 与 `app-server-transport` 自身可在 `wasm32-unknown-unknown` 目标下 `cargo check` 通过。
   - **已拍板结论**：
     - `scad-ui` 收敛为**纯跨端共享 UI 基础层**，主要依赖 `egui` / `egui_commonmark`；允许谨慎依赖 `scad-scene` 的纯数据类型（如 `CameraMatrices` / `OrbitalCamera`），禁止依赖 `scad-scene` 的 renderer / window / GPU 能力。现有 `muda`、`winit`、`platform_support`、`font_setup` 中的桌面专属逻辑必须从 `scad-ui` 中剥离。
     - **不**新建 `scad-ui-desktop` 中间 crate；剥离出来的桌面 UI 辅助（平台菜单接线、`muda`、桌面字体回退、平台窗口辅助）直接迁入 `studio-app`。
     - `scad-ui` 必须能在 `wasm32-unknown-unknown` 目标下 `cargo check` 通过；该校验是 **Phase 4** 的硬验收项（Phase 2 只产出剥离方案文档，不强制 wasm check 通过）。
     - 桌面端 crate 名称沿用 `studio-app`，本轮不改名。

2. **协议中的路径模型如何统一桌面与浏览器**
   - 影响范围：`workspace.current` / `workspace.list` / `file.read` / 预览请求的路径表达；浏览器侧没有 POSIX 文件系统、桌面端存在 Windows / macOS / Linux 路径差异。注意：协议中**不引入** `workspace.open(path)`，workspace 绑定/切换是 host-local Rust API 职责，不进协议。
   - 触发 Phase：Phase 2 协议核心。
   - 处理要求：Phase 2 必须在协议中显式定义路径句柄（例如 workspace-relative 不透明 ID + 显示名），并明确禁止在协议层裸传 `PathBuf` / 操作系统原生路径字符串。
   - **已拍板结论**：
     - 协议层路径句柄使用**结构化形式** `{ workspace_id, path_segments: ["src", "main.scad"] }`，`path_segments` 为规范化后的 UTF-8 段数组；段内禁止出现 `..` / `.` / 空串 / 操作系统原生分隔符；client 不允许自行构造或拼接 `path_segments`，所有句柄必须由 server 通过 `workspace.list` 等命令下发。
     - 句柄本身即显示名：UI 层把 `path_segments` 用 `/` 拼接即可得到统一的跨端显示路径，不再为"显示路径"开独立字段；若 server 端真实路径中含非 UTF-8 字节，server 在生成句柄时用显示替换字符并对应拒绝 `file.read`，不污染协议形态。
     - 协议显示分隔符**固定为 `/`**，与操作系统无关；桌面 UI 如需展示 `\`，由桌面壳层自行翻译，不进协议。
     - server 端在生成句柄前**严格规范化**：NFC Unicode、解析符号链接到真实物理路径、消解 `..` / `.` 段；保证一个物理文件只对应一个句柄，避免 watch 与状态错配。
     - 句柄绑定在当前 workspace session 生命周期内；workspace 关闭或切换后旧句柄一律失效，server 返回明确的协议错误。本轮"单窗口单 workspace session"下天然满足。

3. **`app-server-host` 在同进程模式与远端模式下的生命周期与取消语义统一**
   - 影响范围：GUI 关窗、热重启、网页 client 断线、watch 任务取消、子进程取消、未来云 Agent / 沙盒的会话回收。
   - 触发 Phase：Phase 2 transport trait、Phase 3 host runtime、Phase 5 GUI 接入、Phase 6 浏览器接入。
   - 处理要求：Phase 2 transport trait 必须显式包含取消、关闭、错误传递语义；Phase 3 host runtime 必须保证 GUI 进程退出时所有后台任务可被同步等待结束，禁止留下孤儿任务。
   - **已拍板结论**：
     - **Session 与 transport 连接解耦**：session 独立于具体 transport 连接，连接断开后 server 保留 session 上下文一段短窗口（默认 30 秒，可由 host 启动参数覆盖），允许 client 在窗口内通过 session token 重新连接并续上同一 session；超过窗口未重连则 session 销毁，关联句柄、订阅、上下文一并失效。
     - **Session token 发放与重领**：初次连接完成能力协商时，server 在响应中下发 session token；client 重连时在握手阶段或专门的 `session.reclaim` 命令中携带该 token；token 在 session 销毁后立即失效。
     - **断开时立即取消 in-flight 任务**：transport 连接断开瞬间，server 立刻取消该 session 当前所有未完成请求（包括正在跑的 OpenSCAD 子进程，统一通过 `ChildTerminator` 跨平台抽象终止子进程——Unix 走 SIGTERM、Windows 走 `TerminateProcess` / `Child::kill`，不等子进程自然结束）；任务结果一律丢弃，**不**向已断开的连接尝试返回任何协议错误；重连后由 client 决定是否重发。本轮不实现内容寻址缓存。
     - **重连后保留与不保留的内容**：保留 workspace 上下文、已下发的路径句柄、能力协商结果；**不保留** 任何 in-flight 请求、任何 server push 订阅。client 重连后必须自行重新订阅 watch / 预览进度等推送通道；该约定与"风险评估"第 4 项中"断线重连后能力重新协商"一致。
     - **同进程模式（GUI in-process host）**：session 概念仍然存在，但 GUI 进程退出即 session 终结，"重连窗口"对该模式无意义；GUI 关停遵循下条。
     - **GUI 关窗的 server 关停语义（3-c-3）**：GUI 主线程退出前向 server 发送取消信号，并 join 所有后台任务，超时阈值 **5 秒**；超时则 log 警告 + 强制 abort 退出，不 panic、不无限等待。该 5 秒值固化为协议/host 常量，不向外暴露为 UI 可调项。
     - **远端模式（WebSocket host）单 workspace 单进程**：host 进程在启动参数中绑定一个 workspace 路径；不通过协议切换 workspace；所有连进来的 client 共享同一 workspace 状态。多 workspace 部署形态留给未来云端版本。
     - **取消语义协议形状（3-e-1）**：每个请求自带 `request_id`；client 可发 `cancel(request_id)`；server 收到后中断对应任务（含通过 `ChildTerminator` 跨平台终止 OpenSCAD 子进程），任务以协议级"已取消"错误返回；订阅退订也走显式命令，不复用 `cancel`。注意：协议级"已取消"错误**仅在连接仍在线、client 显式发送 cancel 时返回**；连接断开导致的取消属于上一条（任务被丢弃，不向已断开连接发任何错误）。

4. **watch / 文件变更事件的统一推送与节流**
   - 影响范围：协议事件模型、server 端 watcher 实现、所有 client 的刷新心智。
   - 触发 Phase：Phase 2 协议事件模型、Phase 3 server core watch service、Phase 5 / Phase 6 client 接入。
   - **已拍板结论**：
     - 文件 watch 是 **server 端能力**，由 `app-server-core` 的 watch service 唯一持有；client 一律不做任何文件 I/O，包括 watch。
     - 协议中的 watch 事件流**所有 client 一视同仁**：桌面 client、网页 client、未来云端 / 沙盒 client 都走同一份订阅通道与同一份事件类型，不引入"按 client 平台关闭 watch 订阅"的 capability。
     - 节流责任在 **server**：watch service 必须对底层 `notify` 抖动（保存触发的连续多次写、临时文件、目录批量操作）做合并与节流，对外只暴露语义稳定的事件；具体节流策略（合并窗口、coalesce 规则）在 Phase 2 协议设计与 Phase 3 实现时确定并固化为常量，不向 UI 暴露为可调项。
     - client 收到 watch 事件后的行为（重新拉取目录、重新发预览请求等）属于 `studio-common` 状态机职责，桌面与网页共用同一份处理逻辑；UI 呈现层（如"正在刷新"提示）由各自端壳层自行决定。
     - 重连后订阅不自动恢复（与第 3 项 session 重连约定一致），client 重连后必须自行重新订阅 watch 通道。

5. **`scad-data` / `scad-viewer` 拆分顺序与基线保护的兼容性**
   - 影响范围：`scad-data` 当前直接被根 crate `scad-studio` 与 `crates/scad-viewer` 同时依赖；`scad-viewer` 既包含独立 `[[bin]]` 桌面 Viewer 应用，又导出 `app::UiCommand` / `ui::*` 等被根 crate `src/viewer_tab.rs` 实际作为库依赖使用的 lib 内容。两者同时迁移过程中不得破坏锁定基线。
   - 触发 Phase：Phase 1 `scad-viewer` 瘦身、Phase 3 server core 提取一刀切、Phase 4 共享 lib 内容并入 crate 物理拆分、Phase 7 残留清理。
   - **已拍板结论**：
     - **Phase 1 内瘦身 `scad-viewer`，不删 crate**：删除 `crates/scad-viewer/src/main.rs` 与 `Cargo.toml` 中的 `[[bin]]` 定义；同步删除 crate 内**只为独立 Viewer 桌面应用服务**的 UI 组件、独立窗口/菜单接线、独立事件循环等代码，以及随之可移除的依赖（独立桌面应用才需要的 `muda`、`winit`、`egui-winit`、`rfd`、`pollster`、`env_logger` 等）；保留被根 crate 实际引用的 lib 内容（`scad_viewer::app::UiCommand`、`scad_viewer::ui::*` 中 Studio 引到的部分）。瘦身后 `scad-viewer` 仍是 workspace member、仍是根 crate 库依赖，形态从"独立桌面应用 + 共享 lib"变为"纯共享 lib"。
     - **Phase 4 进一步迁移 `scad-viewer` 剩余 lib 内容**：crate 物理拆分阶段把瘦身后的 `scad-viewer` lib 内容按 `studio-common` / `scad-ui` 边界规则继续归位；如 Phase 4 完成后 `scad-viewer` 已不再被任何调用方引用，则该 crate 在 Phase 7 物理删除；如仍存在不便迁出的纯 lib 内容，可保留 crate 但禁止重新承载独立应用职责。
     - **Phase 3 内一刀切迁移并删除 `scad-data`**：Phase 3 一次性把 `scad-data` 内容按目标边界全部移走，迁移当 commit 内 server core 的对应 service 必须已能承接所有调用方，根 crate 与瘦身后的 `scad-viewer` 同步切到新接口；移完立即从 workspace 中删除 `crates/scad-data`，不允许保留任何 re-export 或 facade 壳。
     - **Phase 3 → Phase 5 过渡期的 in-process 调用面**：`app-server-core` 的服务必须显式暴露**稳定的 public Rust API**，作为同进程调用方（in-process host、过渡期根 crate 调用方）的长期消费面。Phase 3 一刀切后，根 crate 与瘦身 `scad-viewer` 直接调用这套 Rust API（不走协议）；Phase 5 GUI 接入 mpsc transport 后，`studio-app` 改为走 protocol 路径，根 crate 对 Rust API 的直接消费窗口关闭。该窗口属于"重构期受控协议旁路"，Phase 3 必须在 `plan-00-result.md` 中明确登记调用点清单和 Phase 5 的旁路关闭验收（依赖图校验 + 源码 grep 校验）；AGENTS.md 禁的是"长期保留绕过协议的本地直连"，本窗口由 Phase 5 验收强制收敛，不构成长期旁路。
     - **逐模块归位清单**（在 Phase 1 事实核查中固化为最终清单，Phase 3 严格按此执行）：
       - `openscad.rs`（OpenSCAD 子进程调用）→ `app-server-core` 的 OpenSCAD/preview 服务。
       - `watcher.rs`（`notify`）→ `app-server-core` 的 watch service，节流责任在该 service 内。
       - `export.rs`（`stl_io` 写文件）→ `app-server-core` 的 file/export 服务。
       - `config.rs` 必须先**三分再归位**（事实核查显示当前 `AppConfig` 同时含 `openscad_path` / slicers / `recent_workspaces` 等 server 配置，与 `floating_panel_opacity` / `param_panel_pos` / `log_panel_pos` 等纯 UI 状态，混在一起进 server 会强迫 UI 浮动面板位置走协议同步，无意义）：
         - **server 配置**（`openscad_path`、slicers、`recent_workspaces` 等需要 server 持有/持久化、影响 server 行为的部分）→ Phase 3 进 `app-server-core`；client 通过协议读写。
         - **桌面壳层配置**（仅影响桌面壳层行为、与 server 完全无关的部分，如桌面 OS 集成参数）→ Phase 4 进 `studio-app`，本地存储不上协议。
         - **共享 UI 状态**（`floating_panel_opacity`、`param_panel_pos`、`log_panel_pos` 等所有 client 通用的面板布局偏好）→ Phase 4 进 `studio-common`（如果所有 client 端共用），或各端壳层各自维护（如果端壳层差异大）；不进 server。
         - 三分清单在 Phase 1 事实核查中固化为最终清单（每条字段标目标边界），Phase 3 / Phase 4 严格按此执行；`dirs` 等系统目录解析仅用于 server 配置部分，进 `app-server-core`，不进 client。
       - `document.rs` / `params.rs` / `presets.rs` 必须**先按纯数据 / stateful 二分**再归位（事实核查显示 `DocumentState` 含 `PathBuf` / `Instant` / debounce 与 UI input state，整体进 protocol 会破坏 protocol 平台无关性）：
         - **纯可序列化数据**（`ExportFormat`、参数解析结果、预设文件结构、source 文本快照等无 `PathBuf` / 无 `Instant` / 无 UI 输入态的部分）→ `app-server-protocol`，含 serde round-trip 测试。
         - **含 `PathBuf` / `Instant` / 防抖状态 / UI 输入态的 stateful 部分**（如 `DocumentState::pending_render_at` / `preset_name_input` / `selected_preset` / `warnings` / 当前打开 source 的本地路径句柄等）→ Phase 3 内**暂存于根 crate**（直接 inline 到现有 src 模块或新建 `src/document_state.rs` 之类），不进 protocol。**Phase 3 退出时必须把临时模块清单（路径 + 类型名 + 后续目标 crate）写入 `plan-00-result.md`**；Phase 4 必须在执行步骤里点名迁出该清单中的全部模块到 `studio-common`，验收时增加 `rg` 检查：清单中类型的 `struct`/`enum` 定义点必须从根 crate 完全消失，仅出现在 `studio-common`。
         - 含 I/O 的部分（解析参数从源码字符串中读、写预设文件等）一律不进 protocol；I/O 行为进 `app-server-core`，纯解析逻辑（输入字符串 → 输出结构）保留为 `app-server-protocol` 的纯函数。
       - `rfd` 文件对话框 → 不进 server core、不进 `scad-data` 的目标边界；按"前端壳层各管自己的文件选择器"原则，桌面端在 `studio-app` 内继续使用 `rfd`（升级到当前最新可用版本），网页端在 `studio-web` 内使用浏览器原生文件/目录选择能力（如 `<input type="file">` 或 File System Access API）。
     - **`scad-data/tests/*` 必须按对应关系迁移**：Phase 3 一刀切前先把 `crates/scad-data/tests/` 现有测试逐个对应到目标 crate（`app-server-core` / `app-server-protocol`），迁移前后测试名单一一对应、覆盖关系写入 `plan-00-result.md`；缺一不可，不允许"删除时丢测试"。
     - 边界拿不准的模块（例如 `config.rs` 中混合 client 偏好与 server 配置的部分）必须在 Phase 1 事实核查中标注，并在 Phase 3 实现前完成最终归位拍板，禁止 Phase 3 执行中临时拍板。

6. **浏览器端预览渲染路径与源文件可见性边界**
   - 影响范围：浏览器侧预览能力实现路径、`scad-scene` 是否需要在 wasm 目标下编译、协议中预览结果数据形态、`file.read` 能否对 web client 暴露 `.scad` / `.stl` / `.3mf` 等几何源文件、wgpu 在浏览器上的 backend 选择。
   - 触发 Phase：Phase 2 协议核心、Phase 3 server core 提取、Phase 6 浏览器接入。
   - **已拍板结论**：
     - 浏览器端必须具备**完整预览能力**（不是占位预览）；server 端只负责把 OpenSCAD 编译结果（mesh / 3MF 等几何产物）通过协议推到浏览器，浏览器使用 `scad-scene` 在本地完成渲染。Server 不需要 headless GPU 渲染。
     - `scad-scene` 必须能在 `wasm32-unknown-unknown` 目标下编译通过；当前 `system_fonts.rs` 等带平台 I/O 的模块必须按 `cfg(target_arch = "wasm32")` 做替代实现或显式禁用入口。
     - 协议中的 `file.read` 必须按**扩展名 allowlist** 做 client 能力门禁：web client 仅允许读取 `.md`、纯文本预览、图片等"非几何源"文件；`.scad`、`.stl`、`.3mf` 等几何源文件对 web client **一律不暴露字节流**，浏览器仅能通过 `preview.request` 拿到 server 端编译后的几何产物。桌面 client 不受该 allowlist 限制。
     - 浏览器端 wgpu 锁定 **WebGPU only** 后端，不实现 WebGL2 fallback；这是面向 2026 年浏览器栈的明确取舍，未来如有兼容投诉再单独评估。
     - 协议中的预览命令必须显式区分"请求几何产物"与"请求渲染图像"两种语义；本轮浏览器端走前者，未来若引入纯瘦客户端再走后者，但本轮不实现。

---

## Phase 1：锁定基线能力与兼容性清单

### 目标

- 在动架构前，明确 `7b232bd` 这条基线到底保护哪些功能、测试和人工回归路径。
- 建立后续所有 Phase 都必须满足的“不可回退清单”。

### 前序目标保护

- 本 Phase 不改 **Studio（根 crate `scad-studio`）的产品行为**：补充文档、清单与验证脚本、以及"瘦身 `scad-viewer`"（删除 `[[bin]]` 入口与独立 Viewer 应用专属 UI/依赖）以外的代码改动一律禁止。
- "瘦身 `scad-viewer`"是本 Phase 内**唯一允许的运行时行为变更**，且仅影响 `crates/scad-viewer` 二进制壳的可启动性——该二进制壳已被基线保护清单显式排除（操作步骤 2）。Studio 桌面端用户可见行为必须保持锁定基线。
- 禁止为后续重构预埋其它会改变运行时行为的代码（包括协议层骨架、新 crate 占位等，这些都是 Phase 2 / Phase 4 的工作）。

### 输入

- 锁定提交 `7b232bd`
- 当前仓库中的桌面 GUI、预览、工作区、构建与测试路径

### 操作步骤

1. 梳理锁定提交中的既有功能清单，至少覆盖：桌面 GUI 启动、工作区相关行为、预览相关行为、现有菜单/交互、构建与测试命令。
2. 把下列能力明确写入”不可回退清单”：
   - `src/main.rs` 的多窗口桌面运行时与事件循环；
   - `src/app.rs`、`src/workspace.rs` 的工作区打开、最近工作区、窗口标题更新；
   - `src/platform_menu.rs` 的平台菜单、最近工作区菜单和快捷键；
   - `src/document_workspace.rs`、`src/studio_document.rs` 的文档标签与会话分发；
   - `src/viewer_tab.rs`、`src/markdown_tab.rs`、`src/image_tab.rs` 的文件打开与刷新路径；
   - `src/main.rs` 中由 watcher 驱动的缓存失效与重新加载路径；
   - 已包含在 Studio（即根 crate `scad-studio`）中的预览能力，包括独立 `scad-viewer` 当前覆盖的全部用户可见预览功能。注意：本轮锁定基线保护的是 Studio 内的预览能力本身，**不**包括 `crates/scad-viewer` 二进制壳（`[[bin]]`）的可启动性；该二进制壳在本 Phase 内将被删除。
3. 完成关键 crate 的事实核查并写入 `plan-00-result.md`，至少包含：
   - `crates/scad-viewer` 的现状审计：列出 `[[bin]]` 入口（`src/main.rs`）与所有”只为独立 Viewer 桌面应用服务”的源码模块（独立窗口/菜单接线、独立事件循环、`muda` / `winit` / `egui-winit` / `rfd` / `pollster` / `env_logger` 直接消费点等），以及被根 crate 实际作为库引用的 lib 内容（至少包括 `scad_viewer::app::UiCommand`、`scad_viewer::ui::*` 中被 `src/viewer_tab.rs` 引到的 item）。瘦身名单写入 `plan-00-result.md`，作为本 Phase 删除动作的输入。
   - `crates/scad-data` 当前对外暴露的模块清单（`config`、`document`、`export`、`openscad`、`params`、`presets`、`watcher`）、全部依赖项、在根 crate 与瘦身后 `scad-viewer` 中的调用点、`crates/scad-data/tests/*` 现有测试逐项清单（用例名 + 覆盖目标），作为 Phase 3 一刀切迁移与测试迁移的输入。
   - 当前 workspace 现实成员清单（`scad-data`、`scad-scene`、`scad-ui`、`scad-viewer`），与本计划目标 crate（`app-server-protocol` / `app-server-transport` / `app-server-core` / `app-server-host` / `studio-common` / `studio-app` / `studio-web`）的差集，作为新建 crate 的总账。
   - “风险评估”第 5 项中的逐模块归位清单，按 `scad-data` 现有模块逐一标注目标边界（`app-server-core` / `app-server-protocol` / `studio-app` / `studio-web`），把所有边界拿不准的模块在本 Phase 内拍板，禁止 Phase 3 执行中临时拍板。
4. 完成 Studio 对独立 `scad-viewer` 用户可见能力的等价覆盖核实：逐项对比独立 `scad-viewer` 二进制当前提供的预览、相机/视图操作、文件打开等用户可见能力，确认 Studio 内已有等价实现；若发现缺失，先在本 Phase 内补齐，再继续后续步骤。等价覆盖矩阵写入 `plan-00-result.md`。
5. **瘦身 `scad-viewer`**：按步骤 3 的瘦身名单删除：
   - `crates/scad-viewer/src/main.rs`（独立 Viewer 应用主入口）；
   - `Cargo.toml` 的 `[[bin]]` 定义；
   - **`crates/scad-viewer/src/bin/font_probe.rs` 与整个 `src/bin/` 目录**——Cargo 会自动把 `src/bin/*.rs` 识别为额外的 bin target，仅删 `[[bin]]` 段不够；该工具属于独立 Viewer 应用调试用途，与 Studio 无关；
   - 独立 Viewer 应用专属 UI/接线代码（含 `src/platform_menu.rs`、`src/ui/` 中只为独立 Viewer 服务的部分等，按步骤 3 审计名单确定）；
   - 随之可移除的依赖（`muda` / `winit` / `egui-winit` / `rfd` / `pollster` / `env_logger` 等独立桌面应用才需要的 crate）。
   保留被根 crate 实际作为库引用的 lib 内容。瘦身后 `scad-viewer` 仍是 workspace member、仍是根 crate 库依赖；`cargo check -p scad-studio` 与 `cargo test --workspace` 必须通过。Phase 4 在 crate 物理拆分时把瘦身后剩余 lib 内容继续归位；Phase 7 视情况物理删除目录。
6. 明确自动化验证命令和人工回归路径，作为后续所有 Phase 的通用验收前置条件。
7. 在 `plan-00-result.md` 中预留基线回归记录格式，后续每个 Phase 都必须填写。

### 验收标准

- "不可回退清单"（操作步骤 2）已落档为可逐项核对的 markdown 列表，每项绑定文件路径或具体功能名；后续 Phase 的 review 可通过对照该清单完成基线核对。
- 后续每个 Phase 都能直接复用本 Phase 的兼容性清单做回归。
- **`scad-viewer` 瘦身机械化断言（必须全部通过）**：
  - `crates/scad-viewer/src/main.rs` 物理不存在（`test ! -f crates/scad-viewer/src/main.rs`）。
  - `crates/scad-viewer/src/bin/` 目录物理不存在或为空（`test ! -d crates/scad-viewer/src/bin || [ -z "$(ls -A crates/scad-viewer/src/bin)" ]`）。
  - `crates/scad-viewer/Cargo.toml` 不含 `[[bin]]` 段（`grep -c '\[\[bin\]\]' crates/scad-viewer/Cargo.toml` 输出 0）。
  - `cargo metadata --format-version 1` 输出中 `scad-viewer` package 不含任何 `targets[*].kind` 包含 `bin` 的项（覆盖 `[[bin]]` 显式声明 + `src/bin/*.rs` 自动识别两条路径）。
  - `cargo metadata` 输出中 `scad-viewer` 的 dependencies 列表不包含瘦身名单上的桌面应用专属依赖（`muda` / `winit` / `egui-winit` / `rfd` / `pollster` / `env_logger`）。
  - `cargo check --workspace` 与 `cargo test --workspace` 通过。
  上述命令与输出全部写入 `plan-00-result.md`。

### 最小 QA 场景

- 自动化命令：`cargo check --workspace`、`cargo test --workspace`。瘦身后 `scad-viewer` 仍参与 workspace 构建，但 `cargo build -p scad-viewer --bin scad-viewer` 必须报"unknown binary"或等价错误（验证 `[[bin]]` 已删除）。
- 自动化命令（依赖快照对比）：`cargo metadata --format-version 1 -p scad-viewer | jq '.packages[] | select(.name=="scad-viewer") | .dependencies[].name'`，结果应当不再包含 `muda` / `winit` / `egui-winit` / `rfd` / `pollster` / `env_logger`；命令与输出写入 `plan-00-result.md`。
- 人工步骤：启动 `scad-studio`（即根 crate 的桌面入口），打开工作区，确认最近工作区可见、窗口标题更新、文件树可浏览、`.scad` / `.md` / 图片文件可打开；预览能力按"等价覆盖矩阵"逐项确认。
- 人工步骤：核对 `plan-00-result.md` 中"等价覆盖矩阵"已完整列出独立 `scad-viewer` 二进制原本提供的全部用户可见能力以及 Studio 内的对应实现位置。
- 预期结果：Studio 内的预览能力与 `7b232bd` 一致，无新增回退；`scad-viewer` 二进制壳已删除，crate 形态收敛为纯共享 lib。

---

## Phase 2：协议核心（Protocol Core）与传输抽象（Transport Trait）

### 目标

- 先固定 protocol 与 transport 的边界，避免后续 server core 和 client 一边实现一边漂移。
- 明确 protocol 如何表达平台差异、I/O 差异、能力协商、异步任务和错误模型。

### 前序目标保护

- 只允许新增协议层与 transport trait，不得直接改坏现有桌面 GUI 行为。
- 禁止在 protocol 里泄漏 WebSocket、HTTP、`mpsc` 或平台私有类型。

### 输入

- 用户已拍板的统一 server / protocol / transport 约束
- 旧 plan 中对工作区、watch、OpenSCAD / 3MF 回传的需求

### 操作步骤

1. 新建 `crates/app-server-protocol`，定义命令、事件、错误、能力协商、会话和路径模型；路径在协议层必须使用结构化 workspace-relative 句柄 `{ workspace_id, path_segments: [..] }`，`path_segments` 为规范化后的 UTF-8 段数组（段内禁止 `..` / `.` / 空串 / 操作系统原生分隔符），不再开"显示名"独立字段，跨端显示路径统一用 `/` 拼接 `path_segments` 得到。client 不允许自行构造或拼接句柄，所有句柄必须由 server 通过 `workspace.list` 等命令下发；server 在生成句柄前必须做 NFC 规范化与符号链接解析，保证一个物理文件只对应一个句柄。禁止裸传 `PathBuf` 或操作系统原生路径字符串。
2. 新建 `crates/app-server-transport`，定义 transport trait，必须显式覆盖：request/response、server push 订阅与退订、关闭、错误传递、取消语义、断线重连后的能力重新协商。**watch 事件节流不属于 transport trait 责任**，归 `app-server-core` 的 watch service 在事件源处合并/节流（详见 Phase 3 步骤 1）；transport 只负责把节流后的事件原样推送到 client。同时在协议/transport 边界明确以下 session 与取消语义：
   - 每个请求必须携带 `request_id`；`cancel(request_id)` 是独立协议命令，server 收到后中断对应任务（含通过 `ChildTerminator` 跨平台抽象终止 OpenSCAD 子进程，平台实现见 Phase 3 步骤 13），任务以协议级"已取消"错误返回；订阅退订走独立命令，不复用 `cancel`。
   - Session 与 transport 连接解耦：transport 连接断开时，server 立刻取消该 session 全部 in-flight 请求与全部 server push 订阅；session 上下文（workspace 上下文、已下发路径句柄、能力协商结果）保留默认 30 秒重连窗口（host 启动参数可覆盖）；超过窗口 session 销毁。
   - Session token 在初次连接的能力协商响应中由 server 下发；client 重连通过握手阶段或 `session.reclaim` 命令携带 token 续上同一 session；token 在 session 销毁后立即失效。
   - 重连成功后，client 必须自行重新订阅之前的 server push 通道；server 不保留旧订阅。
3. 明确单窗口单 workspace session 语义，以及目录树、当前目录文件列表、预览请求、watch 事件流的协议模型；watch 事件由 server 端统一推送，所有 client 走同一通道、同一事件类型，不引入"按 client 平台开关 watch 订阅"的 capability，节流责任在 server 端。
   - 预览命令必须**显式区分"请求几何产物"与"请求渲染图像"两种语义**；本轮所有 client 仅使用"请求几何产物"，渲染图像语义保留协议位但不实现 server 端逻辑，避免后续被反向加入。
   - **预览几何产物的 payload DTO 必须由 `app-server-protocol` 自身定义**（如 `PreviewMeshPayload` / `PreviewArtifact3mf` 等可序列化类型），**禁止协议直接复用 `scad_scene::MeshData` 等带 `wgpu` / `winit` 依赖的渲染层类型**；协议层 DTO 只描述顶点 / 索引 / 法线 / 材质等纯数据 + 必要元信息（坐标系、单位等），与渲染后端解耦。配套加 serde round-trip 测试与最小 / 最大 payload 边界测试。
   - `file.read` 命令必须支持 server 端按**扩展名 allowlist** 拒绝读取请求，并在协议层定义对应的"该文件类型对当前 client 不可读"错误码与 client capability 字段。默认 web client 的 allowlist 拒绝 `.scad` / `.stl` / `.3mf` 字节流，桌面 client 不受该 allowlist 限制；具体清单与默认值在协议常量中明确。
4. 为协议数据模型补全 serde round-trip 测试（命令、事件、错误、能力协商、路径句柄、预览几何产物 DTO），以及版本/能力协商在最小集与最大集两种情况下的兼容性测试。
   - **路径句柄行为测试分层**（覆盖"风险评估"第 2 项已拍板的全部行为；按 crate 职责分层放置，避免 protocol 误依赖文件系统/session runtime）：
     - 在 `app-server-protocol` 内：`path_handle_rejects_dot_dot_segment`、`path_handle_rejects_single_dot_segment`、`path_handle_rejects_empty_segment`、`path_handle_rejects_native_separator`（segment 内含 `\` / `/` 字面值）、`path_handle_nfc_canonical_equivalent`（NFC 不同写法在规范化后等价）、`path_handle_serde_roundtrip`。这些都是纯函数测试，不依赖任何运行时。
     - 在 `app-server-core` 内（Phase 3 步骤 6 同期落地，因为需要文件系统）：`path_handle_symlink_resolved_to_canonical`——server 端生成句柄时符号链接已解析为真实物理路径。
     - 在 `app-server-host` 内（Phase 3 步骤 13 内一并实现，需要 session runtime）：`path_handle_stale_after_session_close`——workspace 关闭/切换后旧句柄查询返回明确"句柄已失效"协议错误。
   - **源码守门**：在 `app-server-protocol/src/` 上 `rg "PathBuf|std::path::Path[^B]"`，预期无匹配（白名单仅允许 host-local Rust API 的内部辅助类型，不允许出现在协议公开命令/事件/错误/数据模型签名中）。命令与输出写入 `plan-00-result.md`。
5. 为 transport trait 补充至少一个内存内的参考实现并基于该实现完成 envelope / roundtrip / 取消 / 关闭 / push 订阅退订的单元测试，作为后续 mpsc 与 WebSocket adapter 的回归基准。
6. 在仓库内验证 `app-server-protocol` 与 `app-server-transport` 自身能在 `wasm32-unknown-unknown` 目标下 `cargo check` 通过（这两个 crate 是网页端 client 的协议 / 传输底座，必须 wasm-clean）；命令与结果写入 `plan-00-result.md`。
7. 设计 `scad-ui` 与 `scad-scene` 的剥离 / wasm 化方案并写入 `plan-00-result.md`：`scad-ui` 待剥离的桌面依赖清单（`muda` / `winit` / 桌面 `font_setup` / `platform_support` 等）、`scad-scene` 待 cfg 门禁或替代实现的模块清单（`system_fonts.rs` 等）、wgpu 锁定 WebGPU 后端不引入 WebGL2 fallback 的具体落点。这两份方案的**实施与 wasm `cargo check` 验收都放在 Phase 4**，不在本 Phase 内强制通过；本 Phase 只产出方案文档。

### 验收标准

- 协议层不依赖具体 transport，且不出现 WebSocket、HTTP、`tokio::mpsc` 或平台私有类型。
- transport trait 在内存内参考实现下可完整跑通 request/response、server push 订阅退订、取消、关闭、错误传播。
- 协议能表达工作区、文件列表、预览（区分几何产物与渲染图像两种语义）、watch 事件、能力协商、`file.read` 扩展名 allowlist 与错误语义；路径模型不向上泄漏操作系统差异。
- `app-server-protocol` 与 `app-server-transport` 在 `wasm32-unknown-unknown` 目标下 `cargo check` 通过。
- `scad-ui` / `scad-scene` 的剥离 / wasm 化方案文档已落档（实施与 wasm check 验收在 Phase 4）。

### 最小 QA 场景

- 自动化命令：`cargo test -p app-server-protocol`、`cargo test -p app-server-transport`、`cargo check -p app-server-protocol --target wasm32-unknown-unknown`、`cargo check -p app-server-transport --target wasm32-unknown-unknown`。
- 人工步骤：抽查 protocol 类型，确认不出现 WebSocket、HTTP、`tokio::mpsc`、`PathBuf` 或平台私有类型泄漏到 protocol crate。
- 人工步骤：抽查 transport trait 与内存内参考实现，确认取消、关闭、能力协商相关方法存在且有对应测试。
- 人工步骤：核对 `plan-00-result.md` 中已落档 `scad-ui` / `scad-scene` 的剥离 / wasm 化方案文档。
- 预期结果：协议和传输抽象可以独立编译与测试，边界清晰，wasm 目标可编译；`scad-ui` / `scad-scene` 的 wasm 化方案已就位，等 Phase 4 实施。

---

## Phase 3：App Server Core 与 Host Runtime 提取

### 目标

- 将文件系统 I/O、目录列举、文件读取、watch、OpenSCAD / 预览外部调用，统一迁入 app server core。
- 建立同一份 server host 运行时，用来承接 GUI 的 `tokio::mpsc` transport 和网页/未来 client 的 WebSocket transport。
- 为未来云 Agent / 沙盒保留清晰的能力扩展点。

### 前序目标保护

- 本 Phase 完成前，桌面 GUI 仍可暂时通过兼容适配层维持基线行为；**唯一允许的"非协议路径"是步骤 2 显式登记的 `app-server-core` public Rust API 直连**（在 `plan-00-result.md` 内有调用点清单），Phase 5 必须强制关闭这些直连点；除此之外禁止引入新的协议旁路。
- 所有抽取动作都必须保证锁定基线功能不退化。

### 输入

- 现有桌面代码中直接访问文件系统、watch、OpenSCAD、预览的实现
- Phase 2 固定后的 protocol 与 transport trait
- 当前 workspace 布局与未来新增 crate 落点

### 操作步骤

1. 新建 `crates/app-server-core`，建立 app server core 的服务边界，至少拆出：workspace service、file service、watch service、preview service。watch service 必须对底层 `notify` 抖动（连续保存、临时文件、批量目录操作）做合并与节流，对外只暴露语义稳定的事件流；节流策略（合并窗口、coalesce 规则）固化为常量，不对 client 暴露调节项。
2. **`app-server-core` 必须显式暴露稳定的 public Rust 服务 API**，作为 in-process host 与过渡期同进程调用方的长期消费面；这套 API 是 protocol/transport 层的下层底座（protocol 层在 `app-server-host` 中通过它构造服务实例并对外提供命令）。Phase 3 → Phase 5 之间根 crate 与瘦身后的 `scad-viewer` 直接调用该 API（不走协议）；Phase 5 GUI 接入 mpsc transport 后，根 crate 对该 API 的直接消费窗口由依赖图守门 + 源码 grep 守门强制关闭（详见 Phase 5）。Phase 3 必须在 `plan-00-result.md` 中登记当前所有直接调用点清单（按文件 + 函数级别）。
3. 按"风险评估"第 5 项中固化的逐模块归位清单，把 `crates/scad-data` 全部内容**一刀切**迁移到目标边界（`openscad.rs` / `watcher.rs` / `export.rs` / `config.rs` 进 `app-server-core`，`document.rs` / `params.rs` / `presets.rs` 中的纯数据进 `app-server-protocol`），同步切根 crate 与瘦身后 `scad-viewer` 的所有调用点到步骤 2 的 Rust API；迁移当 commit 内 server core 的对应 service 必须已能承接全部调用方。迁移完成后立即从 workspace 中删除 `crates/scad-data`，禁止保留 re-export / facade 壳。配置与用户偏好统一由 server 维护，client 通过协议读写，不在 client 端持有副本。**`rfd` 在本 Phase 内只是"排除出 server core 边界"**——既不进 `app-server-core`，也不进 `app-server-protocol`；它的归位目标是 `studio-app`（桌面壳层），但 `studio-app` 要到 Phase 4 才物理建立，因此 Phase 3 内 `rfd` 暂时保留在根 crate（与桌面 GUI 壳层共存），Phase 4 随桌面壳层一并迁入 `studio-app` 并升级到最新可用版本。网页端在 `studio-web` 中改用浏览器原生文件/目录选择能力（不引入 `rfd`），`studio-web` 同样在 Phase 4 建立，本 Phase 不动。
4. **`crates/scad-data/tests/*` 必须按对应关系迁移**：迁移前先按 Phase 1 步骤 3 的测试清单逐项映射到目标 crate（`app-server-core` / `app-server-protocol`）；迁移后在 `plan-00-result.md` 中给出"原测试名 → 新位置 + 新测试名"的对应表；迁移前后测试用例数差为 0（不允许净减少）。
5. 把现有其它 I/O 与外部调用逻辑（散落在根 crate 中、不在 `scad-data` 内的部分）迁入对应 service，不再允许 UI 直接触碰本地文件系统或子进程。
   - **预览 DTO ↔ 渲染数据的转换层**：在 `app-server-core` 的 preview service 内（不在 protocol 内）实现 `PreviewMeshPayload` 等协议 DTO 与底层几何/网格表示之间的转换；在 `studio-common` 或客户端侧（Phase 4 落地后）实现 protocol DTO → `scad_scene` 内部渲染数据的反向转换（client 侧渲染消费）。两端的转换都必须有单元测试覆盖空 payload、最大 payload、退化几何等边界。
6. **watch service 机械化测试**：在 `app-server-core` 的 watch service 上挂一个 fake notify source（不依赖真实文件系统），覆盖以下场景的单元/集成测试——连续多次写合并为一个事件、临时文件（编辑器原子保存的 `.~tmp` / `~` 后缀）被过滤、退订后无事件、断开模拟下订阅失效、重连模拟下订阅不自动恢复（client 必须重新订阅）；测试用例与命令写入 `plan-00-result.md`。
7. 新建 `crates/app-server-host`，实现统一的 server host 运行时，负责承载 transport bridge，而不是把 transport acceptor 散落在客户端里。
8. 在 `app-server-host` 中实现 GUI 的 in-process host 启动入口，明确它如何驱动 `tokio::mpsc` transport。GUI 关窗时 host 必须按以下流程关停：向所有后台任务广播取消信号 → join 全部 task，超时阈值 **5 秒** → 超时则 log 警告 + abort 退出，禁止 panic、禁止无限等待。该 5 秒值固化为常量，不向 UI 暴露为可调项。
9. **明确产出 `tokio::mpsc` transport adapter**：作为 `app-server-transport` trait 的具体实现落地在 `app-server-host` 中（与 in-process host 同 crate），覆盖 request/response、server push 订阅退订、取消、关闭、错误传播；为该 adapter 编写 roundtrip / cancel / close / push 订阅退订单元测试，作为 Phase 5 GUI 接入的回归基准。
10. 在 `app-server-host` 中实现网页/未来 client 使用的 WebSocket host 暴露面（落点固定在本 crate 内，不再独立成 crate），明确启动方式、绑定参数、与 core 的接线，并保证它与 in-process host 共享同一份 server core 实例化路径。WebSocket host 在本轮固定为**单 workspace 单进程**形态：workspace 路径由进程启动参数指定，不通过协议切换；所有连进来的 client 共享同一份 workspace 状态。Session 重连窗口默认 30 秒，可由启动参数覆盖。
11. **协议中不引入 `workspace.open(path)`**：协议层只暴露 `workspace.current`（读取当前会话已绑定的 workspace 句柄）；workspace 的"绑定 / 切换"是 host 端职责，不进入跨端协议（避免协议层裸传 `PathBuf` 与"远端模式拒绝 open"的冲突）。in-process 桌面模式下，`studio-app` 通过 `app-server-core` 的 Rust API（如 `host::rebind_workspace(PathBuf)`）直接告诉同进程 host 重新绑定 workspace；该 API 属于 host-local Rust API，不属于协议；远端 WebSocket host workspace 由进程启动参数固定，根本不暴露 rebind 入口。Phase 3 smoke 走 `workspace.current` → `workspace.list` → `file.read` → `preview.request`，全部通过协议完成，in-process 与远端模式都能完整跑通同一条 smoke 路径。
12. 为 server core 与 host runtime 的核心纯逻辑、bridge 边界和错误路径补测试。
13. **session lifecycle 与取消语义机械化测试**（覆盖"风险评估"第 3 项已拍板的全部行为）：在 `app-server-host` 内针对内存内 transport（步骤 9 的 mpsc adapter）与 fake child 进程，新增以下具名测试，命令与覆盖范围写入 `plan-00-result.md`：
    - `session_token_reclaim_within_window`：断开后 30s 窗口内 reclaim 成功，session 上下文（workspace 句柄、能力协商结果）保留。
    - `session_token_invalid_after_window`：断开超过窗口后 reclaim 失败，token 失效。
    - `explicit_cancel_returns_cancelled_error`：连接仍在线时显式 `cancel(request_id)`，被取消任务以协议级"已取消"错误返回到 client（这是唯一会返回"已取消"协议错误的路径）。
    - `disconnect_abandons_in_flight_tasks`：模拟正在跑的长任务，连接断开瞬间该任务被取消、子进程被终止、不留下孤儿任务、**不**向断开的连接尝试返回任何协议错误（连接已断，无路可送）；reclaim 成功后 client 重新发请求才能拿到新结果。
    - `disconnect_cancels_subscriptions_no_auto_resume`：断开时所有 push 订阅退订，重连后不自动恢复，必须 client 主动重新订阅。
    - `child_terminate_on_cancel`：用 fake child 验证取消请求时 server 调用 `ChildTerminator` trait 终止子进程。`ChildTerminator` 是 server 端跨平台抽象——Unix 平台实现走 SIGTERM，Windows 平台走 `TerminateProcess`/`Child::kill()`；该 trait 与平台实现都在 `app-server-core`，便于测试用 mock 实现替换。协议层、风险结论与本测试均不再用"SIGTERM"作为跨平台描述，统一用"terminate child process"。
    - `gui_shutdown_5s_join_then_abort_strategy`：本测试**不真正调用 `process::abort()`**（会终止测试进程）。把关停决策抽象为 `AbortStrategy` trait（`JoinThenAbort { timeout: Duration }` 是默认实现），单元测试验证：5 秒内全部 task join 完毕走正常退出路径；超时后 strategy 被调用且记录 log 警告。**真实 abort 行为**由独立 subprocess smoke 验证（启动一个故意挂起后台任务的 mini binary，断言 5 秒后进程退出码非 0、stderr 含警告日志），smoke 命令名与启动方式写入 `plan-00-result.md`。
    - `workspace_open_variant_does_not_exist`：在 `app-server-protocol` 内，断言 protocol command/event 枚举中**不存在** `WorkspaceOpen` variant（`rg "WorkspaceOpen|workspace_open|workspace\\.open" crates/app-server-protocol/src/` 零匹配，白名单仅允许风险评估第 5 项中"协议中不引入 workspace.open"的注释/文档引用）。
    - `workspace_open_serde_unknown_variant`：构造一段含 `"workspace.open"` 命令名的 JSON，反序列化到 protocol command 类型应返回明确的"未知命令"反序列化错误，server 收到后转发为协议级错误事件。

### 验收标准

- `app-server-core` 能通过统一 protocol 驱动文件 / watch / OpenSCAD / 预览能力。
- GUI 的 in-process host 与网页侧的 WebSocket host 都有明确代码落点和可执行启动路径。
- 桌面 UI 端的本地 I/O / 外部调用直连点已切换到 `app-server-core` 的 Rust API（步骤 2 登记的 facade 调用面）；通过本 Phase 定义的源码级 `rg` 守门 + 临时白名单（见下）机械化验证。
- session lifecycle 与取消语义按步骤 13 的具名测试全部通过。
- `scad-data/tests/*` 迁移按步骤 4 的对应表完成，迁移前后用例数差为 0。
- 协议层预览几何产物 DTO 已落地，`app-server-protocol` 不依赖 `scad-scene` / `wgpu` / `winit`。

### 最小 QA 场景

- 自动化命令：`cargo test -p app-server-protocol`、`cargo test -p app-server-core`、`cargo test -p app-server-host`、`cargo test -p app-server-host websocket_smoke_roundtrip -- --nocapture`。
- 自动化命令（**Phase 3 即开始的源码守门**，不等到 Phase 5）：在根 crate `src/` 与瘦身后 `crates/scad-viewer/src/` 上跑 `rg` 扫描，禁止 `std::fs::`、`std::process::Command`、`File::open`、`read_to_string`、`notify::`、`stl_io::` 等模式；本 Phase 由于桌面壳层尚未迁出（`studio-app` 在 Phase 4 才建），允许临时白名单——白名单条目必须指向**步骤 2 登记的 `app-server-core` Rust API 直连点**或本 Phase 内已确认无法立刻关闭的特定调用，每条注明 Phase 5 的关闭计划。命令、白名单、Phase 5 关闭对应关系全部写入 `plan-00-result.md`。
- 自动化命令：`cargo metadata --format-version 1` 验证 `app-server-protocol` 不依赖 `scad-scene` / `wgpu` / `winit` / `egui-wgpu`。
- 人工步骤：本 Phase 必须产出一个唯一的 WebSocket smoke 入口，优先收敛为名为 `websocket_smoke_roundtrip` 的测试或等价 repo-local harness，并把实际启动命令、绑定地址、所需环境变量写入 `plan-00-result.md`。
- 人工步骤：运行该 smoke 入口，按固定顺序完成一次 `workspace.current` → `workspace.list` → `file.read` → `preview.request` 的请求往返；若最终协议命名有所调整，必须在结果文档中记录最终命令与协议名的对应关系。
- 人工步骤：在同进程模式下启动 GUI 时，确认 server host 能随 GUI 生命周期启动和关闭（覆盖步骤 13 的 `gui_shutdown_5s_join_then_abort` 场景）。
- 预期结果：同一个 server core 能同时被 in-process host 与 WebSocket host 驱动；源码守门通过（白名单受控）；session lifecycle 测试全绿；scad-data 测试无丢失。

---

## Phase 4：Crate 物理拆分与共享状态机/领域模型迁移

### 目标

- 把根 crate `scad-studio` 收敛为不含业务代码、不含 `[[bin]]` 的纯 workspace 根。
- 物理建立 `crates/studio-common`、`crates/studio-app`、`crates/studio-web` 三个目标 crate，并把现有 `src/` 中的业务模块按"共享状态/桌面壳层/网页壳层"三分迁入对应 crate。
- 把跨端共享的领域模型与状态机（文档/会话模型、目录树与当前目录文件列表共享状态、预览请求编排与预览状态机、共享错误模型）正式迁入 `studio-common`，为 Phase 5 / Phase 6 的客户端接入预留稳定接口。
- 本 Phase **不引入任何用户可见行为变化**，所有改动都是物理移动 + 接线调整。

### 前序目标保护

- 锁定提交中的桌面 GUI 功能、构建与测试结果必须全部保留；若迁移过程中出现行为回退，必须立即在本 Phase 内修复，不得带病推进。
- 禁止借本 Phase 引入新功能、新依赖、新协议旁路或新对外 API；本 Phase 不允许改 Phase 2 已固定的协议形状。
- `app-server-protocol` / `app-server-transport` / `app-server-core` / `app-server-host` 在 Phase 2 / Phase 3 形成的边界禁止被回退或重新混入业务代码。

### 输入

- Phase 1 形成的"不可回退清单"与 `scad-data` / `scad-viewer` 事实核查结果。
- Phase 2 已落地的 `app-server-protocol` 与 `app-server-transport`。
- Phase 3 已落地的 `app-server-core` 与 `app-server-host`。
- 当前根 crate `scad-studio` 的全部业务模块。

### 操作步骤

1. 新建空 crate 并注册到 workspace：`crates/studio-common`、`crates/studio-app`、`crates/studio-web`；为每个 crate 写好最小 `lib.rs`（`studio-app` 暂含旧入口的 `[[bin]]` 占位，`studio-web` 暂为占位 lib），保证 workspace 整体可编译。
2. 把根 crate `src/` 中**主要承担状态/行为**的模块（包括但不限于 `document_session.rs`、`document_workspace.rs`、`studio_document.rs`、`workspace.rs` 中纯状态部分、`viewer_tab.rs` / `markdown_tab.rs` / `image_tab.rs` 中与呈现解耦的状态部分）迁入 `studio-common`，并在迁移过程中按"长期约束"中 `studio-common` 与 `scad-ui` 的判定规则拆分混合代码。
3. 把根 crate `src/` 中**主要承担呈现/组件**的代码（如布局壳层、左栏、工具栏片段、可复用 widget）按需迁入 `scad-ui`；本 Phase 不做风格重构，只做归位。
4. 把根 crate `src/` 中**桌面平台壳层**（`main.rs`、`app.rs`、`platform_menu.rs`、`macos_fused_titlebar.rs`、窗口生命周期、多窗口、入口接线、桌面端事件循环）整体迁入 `studio-app`，并把根 crate `Cargo.toml` 中的 `[[bin]]` 与桌面相关依赖一并迁过去。
   - 同步按 Phase 2 已落档的剥离方案，把 `scad-ui` 中现存的桌面专属代码（`muda` 接线、桌面 `font_setup`、`platform_support`、`winit` 相关辅助等）迁入 `studio-app`，使 `scad-ui` 收敛为主要依赖 `egui` / `egui_commonmark`、并允许谨慎依赖 `scad-scene` 纯数据类型（不依赖 renderer / window / GPU）的跨端共享 UI 基础层；不新建 `scad-ui-desktop` 中间 crate。剥离完成后 `scad-ui` 必须在 `wasm32-unknown-unknown` 目标下 `cargo check` 通过。
   - 同步按 Phase 2 已落档的 wasm 化方案完成 `scad-scene` 改造：`system_fonts.rs` 等带平台 I/O 的模块按 `cfg(target_arch = "wasm32")` 做替代实现或显式禁用入口；wgpu 在浏览器端锁定 WebGPU 后端，不引入 WebGL2 fallback。改造完成后 `scad-scene` 必须在 `wasm32-unknown-unknown` 目标下 `cargo check` 通过。
   - **`scad-scene::Renderer` 与 `CameraInteraction` 都必须与 winit 解耦**：当前 `Renderer::new(Arc<winit::Window>)` 与 `CameraInteraction` 直接消费 `winit::event::WindowEvent`，本步骤新增 `RendererTarget` 抽象（或显式 `Renderer::new_desktop(Arc<winit::Window>)` / `Renderer::new_web(canvas)` 双入口），并把 `CameraInteraction` 的输入事件类型从 `winit::event::*` 替换为 `scad-scene` 自定义的与平台无关的输入事件枚举（如 `PointerEvent` / `KeyEvent`），由桌面壳层（`studio-app`）从 `winit::event` 转换、网页壳层（`studio-web`）从浏览器事件转换。桌面端继续走 winit 路径，浏览器端接收 HTML canvas / WebGPU surface 并锁定 WebGPU adapter。两种路径必须在各自目标下 `cargo check` 通过；`scad-scene` 在 wasm 目标下不允许任何 `winit` 引用（`rg "winit::" crates/scad-scene/src/` 在 wasm 路径上零匹配，桌面 cfg-gated 模块除外）。web 路径在 Phase 6 真正接通运行时。
   - **`scad-ui::file_tree` 纯化为零 I/O 渲染组件**：当前 `file_tree` 直接持有 `PathBuf` 并执行 `std::fs::read_dir`，违反"client 不做 I/O"基线；本步骤改为接收 server 下发的结构化目录 entry（来自 `app-server-protocol` 的句柄类型），不在 `scad-ui` 内做任何 I/O；同时禁止 `scad-ui` 引入 `std::fs` / `std::process` / `notify` 等 I/O 依赖。
   - **处理 `scad-ui::chat_panel` 残留**：`scad-ui` 当前导出 `chat_panel`，但拍板结论是"fake chatbox 当前阶段只保留在 `studio-web`，不进入 `studio-common`/`scad-ui`"。本步骤逐文件审计 `chat_panel.rs`：若包含与 fake chatbox 产品行为强耦合（消息存储、发送动作、本地状态机等）则**整体迁入 `studio-web`**；若只含通用无状态组件壳（如纯渲染的消息气泡 widget），可重命名为通用 widget（如 `message_bubble`、`message_list`）保留在 `scad-ui`，但必须证明其无任何 fake chat 产品语义。审计结论与处理动作写入 `plan-00-result.md`。
   - **`scad-viewer` 剩余 lib 内容继续归位**：按 Phase 1 瘦身后的 `scad-viewer` lib 内容（`app::UiCommand`、`ui::*` 等被根 crate 引用的部分），按 `studio-common` / `scad-ui` 边界规则继续归位。归位完成后若 `scad-viewer` 已无对外暴露内容，可在本 Phase 末从 workspace 摘除（物理删除留 Phase 7）；若仍有不便迁出的纯共享 lib 内容，保留 crate，但形态必须是纯共享 lib，禁止承载任何独立应用职责。
5. **根目录 `Cargo.toml` 转为 virtual workspace**：删除根 `Cargo.toml` 的 `[package]` 段、`[[bin]]`、`[dependencies]`、`[target.*.dependencies]`、`[package.metadata.bundle]` 等所有业务相关条目（macOS bundle 等桌面元数据迁入 `studio-app`），只保留 `[workspace]`、`[workspace.dependencies]`（如有）、`[profile.*]` 这类纯 workspace 元数据；删除根 crate `src/` 目录；根 `scad-studio` 不再是 Rust package，仅作为 virtual workspace 根存在。`rfd` 随桌面壳层一并归入 `studio-app` 并升级到当前最新可用版本；网页端在 `studio-web` 中改用浏览器原生文件/目录选择能力，不引入 `rfd`。
6. 在 `studio-common` 中正式定义统一的预览请求编排接口与预览状态机，作为 Phase 5（桌面）与 Phase 6（浏览器）共同消费的接口；桌面侧本 Phase 临时保留旧调用通道，下个 Phase 才接入 `tokio::mpsc` transport。
7. `studio-web` 在本 Phase 仅落地最小 lib 与 `wasm32-unknown-unknown` 目标可编译占位，业务实现留给 Phase 6。
8. **冻结 `studio-common` 共享状态机的最终类型名清单**（供 Phase 7 重复职责扫描复用）：本 Phase 内 `studio-common` 落地后，把所有跨端共享状态机/领域模型的具体 Rust 类型名（如实际使用的 `PreviewState` / `WorkspaceSession` / `DocumentTab` 等，按本 Phase 实际命名为准）写入 `plan-00-result.md`，作为 Phase 7 重复职责扫描的精确 `rg` pattern 输入；不允许 Phase 7 时再用占位符或重新猜类型名。
9. 全程不引入新的用户可见行为；过渡期出现的临时 facade 必须在本 Phase 末尾清理或在 `plan-00-result.md` 中显式登记并指明清理 Phase。

### 验收标准

- 根 `scad-studio` 已转为 virtual workspace：根 `Cargo.toml` 不含 `[package]` 段（`grep -c '^\[package\]' Cargo.toml` 输出 0）、不含 `[[bin]]`、不含业务依赖；`src/` 目录已删除（`test ! -d src`）；`cargo metadata --format-version 1` 输出中不存在名为 `scad-studio` 的 package。
- `studio-common` / `studio-app` / `studio-web` 物理存在并被 workspace 注册；`studio-app` 可单独以人工启动方式打开桌面 GUI，按 Phase 1 不可回退清单逐项通过；`studio-app` 同时有自动退出 smoke（具体命令同 Phase 5 QA 中的 `studio-app` 启动 smoke），用于无人参与时的回归。
- `studio-common` 不依赖 `app-server-transport`、不依赖 `winit` / `muda` / 桌面平台 crate、不依赖浏览器 API（`cargo metadata` + `rg` 守门）。
- `scad-ui` 已剥离桌面专属代码，依赖图只剩 `egui` / `egui_commonmark` / `scad-scene`（且对 `scad-scene` 仅消费纯数据类型，无 renderer / window / GPU 引用），`file_tree` 已纯化为零 I/O 渲染组件（`rg "std::fs|read_dir|PathBuf"` 在 `scad-ui/src/` 无匹配，白名单除外）；`scad-ui` 在 `wasm32-unknown-unknown` 目标下 `cargo check` 通过。
- `scad-scene` 已完成 wasm 化改造，且引入 `RendererTarget`（或等价桌面/web 双入口）抽象；在 `wasm32-unknown-unknown` 目标下 `cargo check` 通过。
- `scad-viewer` 剩余 lib 内容已按规则归位；若 Phase 4 末已无对外暴露内容则可摘除（物理删除留 Phase 7），否则保留为纯共享 lib。
- `studio-app` 与 `studio-web` 都没有重复实现预览状态机或文档/会话状态机，预览状态机的唯一定义点位于 `studio-common`（机械化扫描方法见 Phase 7 步骤 2）。
- **Phase 3 临时模块清单中的 stateful 类型已全部迁出根 crate**：使用 Phase 3 退出时落档的"临时模块清单"中每个类型名跑 `rg "(struct|enum)\\s+<TypeName>"`，必须只在 `studio-common/src/` 出现，根 crate `src/`（即将清空）与其它 crate 无定义点。

### 最小 QA 场景

- 自动化命令：`cargo check --workspace`、`cargo test --workspace`、`cargo check -p scad-ui --target wasm32-unknown-unknown`、`cargo check -p scad-scene --target wasm32-unknown-unknown`、`cargo check -p studio-web --target wasm32-unknown-unknown`。
- 自动化命令：`rg "std::fs::|read_dir|std::process::Command|File::open|read_to_string|notify::|stl_io::" crates/scad-ui/src/`，预期无匹配（白名单除外）；命令与输出写入 `plan-00-result.md`。
- 自动化命令：`cargo metadata --format-version 1` + 脚本验证 `scad-studio` 根 crate 无业务依赖、`studio-common` 无 transport / 平台 crate 依赖；命令与输出写入 `plan-00-result.md`。
- 人工步骤：人工启动 `studio-app` 桌面 GUI（按 Phase 5 QA 中已记入 `plan-00-result.md` 的启动方式），按 Phase 1 不可回退清单逐项回归。
- 预期结果：crate 边界守门通过、wasm 编译通过、桌面 GUI 行为与锁定基线一致；目录结构已收敛到长期架构形态，可直接承接 Phase 5 / Phase 6。

---

## Phase 5：GUI 客户端迁移到同进程 App Server + `tokio::mpsc` Transport

### 目标

- 桌面 GUI 不改变产品功能边界，但内部接线完全改为通过 protocol 与 app server 通信。
- 在此 Phase 结束时，桌面端必须已不依赖任何 protocol 旁路。

### 前序目标保护

- 锁定提交中的桌面 GUI 功能、测试和可用性必须全部保留。
- 若迁移过程中出现行为回退，必须先恢复桌面基线，再继续向后推进。
- Phase 4 形成的 crate 边界（根 crate 不含业务、`studio-common` 不依赖 transport 与平台 crate、预览状态机唯一存在于 `studio-common`）不得被回退。

### 输入

- Phase 3 的 app server core、host runtime、`tokio::mpsc` transport adapter（Phase 3 步骤 9 已落地）
- Phase 3 步骤 2 登记的"`app-server-core` Rust API 直连调用点清单"（本 Phase 必须把这些直连点全部关闭）
- Phase 4 的 `studio-app` / `studio-common` crate 与共享预览状态机接口

### 操作步骤

1. 在 `studio-app` 中启动同进程 `app-server-host` 的 Tokio task，并把 `tokio::mpsc` transport 客户端接入 `studio-common` 暴露的协议消费接口。
2. 用 `tokio::mpsc` transport adapter 替换 Phase 3 → Phase 4 期间根 crate / `studio-app` 对 `app-server-core` Rust API 的直连调用，桌面端所有 I/O / 外部调用从此都经由 protocol 完成；**唯一允许长期保留的非协议路径是 `app-server-host::rebind_workspace(PathBuf)`**——它仅承担"传递启动/重绑路径"，禁止用它返回文件内容、执行 I/O 结果或承担其它能力（这条边界写入 `plan-00-result.md`，作为长期约束）。Phase 3 步骤 2 登记的其它直连调用点必须全部关闭，关闭情况逐项写入 `plan-00-result.md`。
3. 回归桌面工作区、预览、配置、菜单等既有能力，确保全部符合锁定基线。
4. 完成"协议旁路"**Cargo 依赖**机械化校验：禁止 `studio-common`、`studio-web`、`scad-ui`、`app-server-protocol`、`app-server-transport` 这些 crate 的 `Cargo.toml` 直接依赖 `notify`、`stl_io`、`rfd`、`dirs` 等本地文件系统 / 外部子进程 / 系统对话框相关 crate；`studio-app` 作为桌面壳层**允许** `rfd`（桌面文件对话框，按"前端壳层各管自己的文件选择器"原则）、`muda`、`winit`、`egui-winit` 这类桌面 UI 必需的 crate；`app-server-core` 与 `app-server-host` 允许 `notify` / `stl_io` / `dirs` 等 server 端 I/O crate（这是它们本来的职责），但禁止 `rfd`（系统对话框是壳层职责）。校验结果与依赖快照写入 `plan-00-result.md`，每条允许项必须注明所在 crate 与用途。
5. 完成"协议旁路"**源码级**机械化校验：在 `studio-app/src/`、`studio-common/src/`、`scad-ui/src/` 三处用 `rg` 扫描，禁止出现 `std::fs::`、`std::process::Command`、`File::open`、`read_to_string`、`write!.*to_file`、`notify::`、`stl_io::`、`tokio::fs::`、对 `app_server_core::` 的直接消费等模式（依赖图校验只能拦住 Cargo 依赖，源码可能直接调用标准库或共享 crate 中的禁用 API）。**允许的例外清单**（必须以白名单形式列出并写入 `plan-00-result.md`，每条例外注明用途与关闭计划）：
   - `studio-app` 调用 `app_server_host::rebind_workspace(PathBuf)`（桌面"打开 workspace"的合法路径，因为协议本身不暴露 `workspace.open(path)`；这是 host-local Rust API，长期允许）。
   - `studio-app` 内 `rfd` 文件对话框相关代码（弹出 OS 对话框是壳层职责）。
   - 其它白名单项必须有用途说明 + 是否长期允许的判断 + 若临时允许则关闭计划。
   校验脚本同样写入 `plan-00-result.md`，作为后续 Phase 8 终态验收的复用入口。

### 验收标准

- 桌面 GUI 用户可见行为不劣于 `7b232bd`。
- 桌面 GUI 内部除 `app-server-host::rebind_workspace(PathBuf)` 这一条已登记的 host-local API 外不存在 protocol 旁路：Cargo 依赖图守门 + 源码级 `rg` 守门同时通过；Phase 3 步骤 2 登记的其它 Rust API 直连调用点 100% 关闭；`rebind_workspace` 的限定职责（仅传路径、不传内容、不传 I/O 结果）有显式守门验证（如 API 签名 review + 调用点 grep）。
- `studio-app` 与 `studio-common` 的依赖图通过本 Phase 定义的机械化校验，源码白名单（如有）每条都有用途与关闭计划。

### 最小 QA 场景

- 自动化命令：`cargo check --workspace`、`cargo test --workspace`。
- 自动化命令：**`studio-app` 启动 smoke**——使用带 timeout 的启动命令或专门的 `--smoke-exit` 隐藏参数（启动 → 完成 in-process host 接线 + 单次协议握手 → 自动退出 0），不能用 `cargo run -p studio-app`（会阻塞等待人工关闭窗口）。具体命令名、退出码契约写入 `plan-00-result.md`。
- 人工步骤：启动 `studio-app`，验证新窗口、打开工作区、最近工作区、平台菜单、快捷键、目录树、Viewer / Markdown / Image 文档打开与刷新。
- 人工步骤：修改被打开文件，确认 watcher 驱动的 UI 刷新路径仍生效。
- 自动化命令：`cargo metadata --format-version 1` + 自定义脚本，确认 `studio-app` / `studio-common` 不直接依赖被禁用的 I/O / 子进程 crate；命令与输出写入 `plan-00-result.md`。
- 自动化命令：`rg` 守门脚本（具体 pattern 见步骤 5）扫描 `studio-app/src/` / `studio-common/src/` / `scad-ui/src/`，无匹配（白名单除外）；命令与输出写入 `plan-00-result.md`。
- 预期结果：桌面用户看不到协议迁移痕迹，行为与 `7b232bd` 等效或更稳定，且依赖图守门 + 源码守门 + Rust API 直连关闭三项检查全部通过。

---

## Phase 6：Web 客户端接入同一协议，并实现浏览器主界面骨架

### 目标

- 网页端通过 `app-server-host` 提供的 WebSocket transport 接入同一套 app server / protocol。
- 浏览器中完整实现：目录树、当前目录文件列表、预览区域、fake chatbox。

### 前序目标保护

- 本 Phase 不得为了浏览器适配而破坏桌面 GUI 已完成能力。
- fake chatbox 仅实现纯前端假 UI，不得反向污染 protocol 的长期设计。
- Phase 4 / Phase 5 形成的 crate 边界与依赖守门检查不得被回退；`studio-common` 不得在本 Phase 引入浏览器 API 或 fake chatbox 状态。

### 输入

- Phase 2 的 protocol 与 transport 抽象
- Phase 3 的 app server core 与 host runtime
- Phase 4 已迁移完成的 `studio-common` 共享状态机与 `studio-web` 占位 crate
- Phase 5 已固定的桌面端协议接入路径

### 操作步骤

1. 在本 Phase 开始时先固定唯一的 Web 客户端构建命令与启动命令，并写入 `plan-00-result.md`；默认验收命令收敛为 `cargo check -p studio-web --target wasm32-unknown-unknown` 与 `cargo build -p studio-web --target wasm32-unknown-unknown`，若还需要额外静态资源服务或开发服务器，则其 repo-local 启动命令也必须一并记录。
2. **WebSocket client transport 落位在 `app-server-transport`**（用 `cfg(target_arch = "wasm32")` 区分 server 端 / client 端实现，或拆出 `app-server-transport-ws` 子模块作为 server / client 双端共用底层），`studio-web` 只负责 wire（实例化 + 连接 server + 暴露给 `studio-common` 的协议消费接口）；不允许把 WebSocket client transport 实现私藏在 `studio-web` 内，避免未来其它 Web client 重复实现。本步骤完成后必须**重新跑 `app-server-transport` 的双 target 编译验收**：`cargo check -p app-server-transport`（native）+ `cargo check -p app-server-transport --target wasm32-unknown-unknown`（wasm），确保 Phase 2 的 wasm-clean 性质没被本次新增 client transport 实现破坏；命令与输出写入 `plan-00-result.md`。浏览器 client 与桌面 client 共用同一份 watch 事件订阅与处理逻辑（来自 `studio-common`），不在 capability 上做平台差异。
3. 在浏览器中建立单窗口单 workspace session 的 Studio 主界面，主界面布局至少包含：左侧目录树、当前目录文件列表、预览区域、fake chatbox 区域。
4. 实现目录树与当前目录文件列表的联动浏览，至少包含：展开/收起目录、点击目录切换当前目录、当前目录文件列表随选中目录刷新、空目录显示空状态。
5. 实现 fake chatbox 的完整前端交互外壳，明确为前端假 UI（不接入任何真实协议命令），至少包含：消息输入框、发送动作（按钮 + 回车快捷键）、消息列表（区分本地/对方）、滚动到底部行为、空状态提示、清空会话交互。fake chatbox 状态仅存在于 `studio-web`，不进入 `studio-common`。
6. 接通预览区域：浏览器端通过 `preview.request`（"请求几何产物"语义）向 server 申请已编译的几何产物（mesh / 3MF 等），由浏览器本地的 `scad-scene` 在 WebGPU 后端完成渲染；预览状态机仍由 `studio-common` 唯一持有。Server 不需要 headless GPU 渲染，不实现"请求渲染图像"语义。
7. 在 web transport 客户端的 capability 协商中显式声明：`file.read` 的扩展名 allowlist 拒绝 `.scad` / `.stl` / `.3mf` 字节流。浏览器端 UI 在用户尝试"读取源码"类操作时直接走协议错误反馈，不绕开 server 的拒绝。
8. 验证浏览器端没有私有协议或绕过 server host 的旁路调用，并把验证方法（依赖图、代码搜索范围）写入 `plan-00-result.md`。

### 验收标准

- 浏览器端主界面结构完整，目录树、当前目录文件列表、预览区域、fake chatbox 四个区域都能正常出现并按本 Phase 操作步骤具备对应交互。
- 浏览器与桌面端都在消费同一 protocol；capability 协商上**只有源文件可见性差异**（`file.read` 扩展名 allowlist 拒绝 web client 读取 `.scad` / `.stl` / `.3mf` 字节流），watch 订阅、事件流、节流策略对所有 client 完全一致。
- 浏览器端预览走"请求几何产物 → 本地 `scad-scene` + WebGPU 渲染"路径，不出现浏览器端"请求渲染图像"或 server 端 headless GPU 渲染。
- fake chatbox UI 完整可用，且未侵入 server 协议边界、未污染 `studio-common`。

### 最小 QA 场景

- 自动化命令：`cargo check -p studio-web --target wasm32-unknown-unknown`、`cargo build -p studio-web --target wasm32-unknown-unknown`；若浏览器入口还依赖额外静态资源服务或开发服务器，则对应 repo-local 启动命令也必须在本 Phase 结束前写入 `plan-00-result.md`。
- 人工步骤：先启动 `app-server-host` 的 WebSocket 模式，再按本 Phase 固定的唯一浏览器客户端启动命令启动网页端，验证单窗口单 workspace session、目录树、当前目录文件列表、预览区域、fake chatbox UI 全部出现。
- 人工步骤：在浏览器中展开目录树、点击目录项、查看当前目录文件列表、打开可预览文件并看到预览。
- 人工步骤：在 fake chatbox 中分别完成发送（按钮与回车）、消息列表渲染、滚动到底部、空状态、清空会话五项交互验证。
- **机械化运行时验证（硬验收）**：必须落地一个机械化浏览器 smoke，覆盖：(1) `studio-web` wasm 模块加载完成；(2) WebGPU adapter 可成功获取（`navigator.gpu.requestAdapter()` 返回非 null）；(3) `app-server-host` WebSocket 连接成功（连接到本 Phase 启动的本地 host）；(4) 发出一次 `preview.request` 并收到非空 `PreviewMeshPayload`；(5) WebGPU canvas 上能采样到至少一个非背景色像素（证明渲染真的发生）。可选实现路径：`wasm-pack test --headless --chrome` + `wasm-bindgen-test`、Playwright + 本地 web 资源服务器、`chromedriver` 直驱等；本 Phase 开始时按当前工具链生态拍板一种实现，把入口名、命令、CI 集成方式写入 `plan-00-result.md`。**机械化 smoke 是 Phase 6 完成的硬条件**：若实施期间确认所选方案不可行（如某 driver 在 2026-04 仍不支持 WebGPU），必须在 `docs/known_issues.md` 登记并选其它路径继续，**不允许以"降级到人工"为由标记 Phase 6 完成**——人工 smoke 仅作为机械化方案落地前的临时验证手段，不是终态验收。
- 人工 WebGPU 验证（仅作为机械化方案落地过程中的辅助）：在受支持的浏览器（Chrome / Safari / Firefox 稳定版，启用 WebGPU）中打开 `studio-web`，对一份预置 `.scad` 测试样本触发 `preview.request`，观察预览区域 WebGPU canvas 挂载、帧循环、几何非空。该步骤可在机械化 smoke 实现期间用作快速回归，但不替代上面的硬验收。
- 预期结果：浏览器端主界面完整，且所有能力都经由统一 protocol 与 host 暴露面；fake chatbox 行为符合本 Phase 列出的具体清单；浏览器端的 build/run 入口在仓库内有唯一口径，不依赖口头说明或未提交的外部工具配置。

---

## Phase 7：清理 `scad-viewer` 残留与 Studio 内重复职责

### 目标

- 在桌面与网页接入完成后，扫描并清理 Studio 内可能残留的重复状态、重复 UI、重复协议接线、重复产品边界。
- 根据 Phase 4 末 `scad-viewer` 剩余 lib 内容情况决定：若 Phase 4 已把所有 lib 内容归位完毕，物理删除 `crates/scad-viewer` 目录；否则保留 crate 为纯共享 lib（禁止重新承载独立应用职责）。
- 确认终态目录结构与 AGENTS.md 长期约束一致，没有”等待迁移完成”的过渡态。

### 前序目标保护

- 物理删除目录或保留共享 lib 都不得导致锁定基线中的预览能力回退；删除/保留决定前必须再次按 Phase 1 的”等价覆盖矩阵”逐项回归。
- Phase 4 / Phase 5 / Phase 6 形成的 crate 边界与依赖守门检查不得回退。

### 输入

- Phase 1 的”等价覆盖矩阵”与瘦身记录
- Phase 3 一刀切迁移后的 server core / protocol 终态
- Phase 4 末 `scad-viewer` 的剩余 lib 内容审计结果
- Phase 5 / Phase 6 已完成的桌面与网页端预览接线

### 操作步骤

1. 按 Phase 1 的”等价覆盖矩阵”再次逐项回归，确认 Studio 桌面端与网页端覆盖原独立 `scad-viewer` 的全部用户可见能力，无新增回退；回归命令与结果写入 `plan-00-result.md`。
2. 用机械化方式扫描重复职责并写入 `plan-00-result.md`：
   - 重复状态机检测：使用 **Phase 4 步骤 8 已冻结的状态机类型名清单**作为精确 `rg` pattern（不允许在本 Phase 现场猜类型名），覆盖 `studio-app/src/` / `studio-web/src/` / `studio-common/src/`，确认每个状态机类型的 `struct` / `enum` 定义点只出现在 `studio-common`，端壳层只能 `use` 不能重新 `struct`/`enum` 定义。
   - 重复 UI 检测：对 `scad-ui` 中的可复用组件名做 `rg`，确认 `studio-app` / `studio-web` 没有重复 widget 实现。
   - 重复协议接线检测：`rg “app_server_protocol|app_server_transport”` 覆盖 `studio-app/src/` / `studio-web/src/`，与预期接入点逐项比对。
   每条扫描命令的预期匹配集合与实际匹配集合都必须落档。
3. 评估 `scad-viewer` 的最终去留：
   - 若 Phase 4 末 `scad-viewer` 已无对外暴露的 item（`cargo metadata` 显示无反向依赖），物理删除 `crates/scad-viewer` 目录；删除前确认根 `Cargo.toml`、`studio-app/Cargo.toml`、`studio-common/Cargo.toml`、`scad-ui/Cargo.toml` 中均无引用。
   - 若仍有不便迁出的纯共享 lib 内容，保留 crate；保留时必须满足：无 `[[bin]]`、无独立桌面应用专属依赖、未承载任何独立应用职责，状态写入 `plan-00-result.md`。
4. 确认 workspace 终态成员清单：必须包含 `app-server-protocol` / `app-server-transport` / `app-server-core` / `app-server-host` / `studio-common` / `studio-app` / `studio-web` / `scad-ui` / `scad-scene`；不再存在 `scad-data`；`scad-viewer` 视步骤 3 决定。`Cargo.toml` 实际成员列表与该预期清单的 diff 写入 `plan-00-result.md`。

### 验收标准

- 重复职责扫描脚本输出符合预期，无超预期匹配。
- `scad-viewer` 视步骤 3 决定：要么物理删除（仓库内无残留引用），要么保留为纯共享 lib（无 `[[bin]]`、无桌面应用专属依赖）。
- workspace 终态成员清单的实际 diff 与 AGENTS.md 长期约束一致。
- 用户可见能力与 Phase 1 等价覆盖矩阵一致或更稳定。

### 最小 QA 场景

- 自动化命令：`cargo check --workspace`、`cargo test --workspace`。
- 自动化命令：步骤 2 中所有 `rg` 扫描命令、步骤 4 中 workspace member diff 命令；命令与输出写入 `plan-00-result.md`。
- 自动化命令（若 `scad-viewer` 保留）：`cargo metadata` 验证 `scad-viewer` 无 `[[bin]]`、无桌面应用专属依赖。
- 人工步骤：按 Phase 1 等价覆盖矩阵逐项回归桌面与网页端的预览路径。
- 预期结果：Studio 内无重复职责、`scad-viewer` 状态明确，预览能力与 Phase 1 基线一致或更稳定。

---

## Phase 8：回归、稳态验证与文档交付

### 目标

- 在统一架构完成后，给出完整回归、限制说明和后续扩展边界。
- 保证执行者和后续维护者可以基于存档继续推进，而不是依赖口头背景。

### 前序目标保护

- 必须重新验证锁定基线中的所有已完成功能。
- 禁止为了”看起来完成”而删减测试或弱化验收标准。
- Phase 4 / Phase 5 的 crate 边界与依赖守门检查必须再跑一次，作为终态确认。

### 输入

- 前面所有 Phase 的实现结果
- 锁定基线兼容性清单
- 风险评估章节的全部未决项当前结论

### 操作步骤

1. 运行自动化测试与构建验证（含工作区编译、协议/传输/核心/host 各 crate 测试、wasm 目标编译、Phase 3 固定的 WebSocket smoke 入口、Phase 6 固定的浏览器构建命令）。
2. 做桌面 GUI 与网页端的人工回归，按 Phase 1 不可回退清单逐项核对。
3. 重新执行 Phase 5 定义的依赖图守门校验，确认终态依赖关系仍符合长期约束。
4. 更新 `plan-00-result.md`、相关开发文档和 `docs/known_issues.md`，把风险评估中已经收敛的条目落档为最终结论，未收敛条目转为正式已知问题。
5. 明确后续扩展到云 Agent、沙盒和其他 transport 时的延展点，写入对应文档。

### 验收标准

- 锁定基线能力无回退。
- 桌面与网页的统一协议架构成立，依赖守门检查通过。
- 风险评估章节的所有条目都有最终结论或已落档为已知问题。
- 文档足以支持后续多会话和多执行者继续推进。

### 最小 QA 场景

- 自动化命令（终态全集，按顺序）：
  - `cargo check --workspace`
  - `cargo test --workspace`
  - `cargo check -p app-server-protocol --target wasm32-unknown-unknown`
  - `cargo check -p app-server-transport --target wasm32-unknown-unknown`
  - `cargo check -p scad-ui --target wasm32-unknown-unknown`
  - `cargo check -p scad-scene --target wasm32-unknown-unknown`
  - `cargo check -p studio-web --target wasm32-unknown-unknown`
  - `cargo build -p studio-web --target wasm32-unknown-unknown`
  - `cargo test -p app-server-host websocket_smoke_roundtrip -- --nocapture`
  - `cargo test -p app-server-host`（覆盖 Phase 3 步骤 9 的 mpsc adapter roundtrip / cancel / close 测试）
  - `cargo test -p app-server-core`（覆盖 Phase 3 步骤 6 的 watch service fake notify 测试与步骤 4 迁移过来的 `scad-data` 原 tests）
  - Phase 5 步骤 5 固定下来的源码级 `rg` 守门脚本
  - Phase 5 步骤 4 + Phase 4 验收 + Phase 7 步骤 4 固定的 `cargo metadata` / workspace member diff 守门
  - 锁定基线清单中定义的全部附加验证命令
- 人工步骤：按 Phase 1 形成的不可回退清单重新走一遍桌面 GUI 回归，再走一遍网页端主界面回归（含目录树、当前目录文件列表、预览、fake chatbox 全部交互）。
- 人工步骤：核对 `plan-00-result.md` 是否逐 Phase 记录了完成情况、验证结果和遗留问题；核对 `docs/known_issues.md` 是否覆盖未收敛风险。
- 预期结果：锁定基线无回退，新增统一架构能力可被后续执行者直接接手。

---

## 备注（无上下文重启任务时阅读）

- 本计划是对 `2026040800-studio-web-wasm-backend` 方向的升级，不再把目标限定为“Web 后端协同”，而是提升为“统一 App Server + 多端统一协议”。
- 若后续代码与本计划冲突，以**锁定提交 `7b232bd` 的真实行为**与用户后续拍板结果为准修订计划。
