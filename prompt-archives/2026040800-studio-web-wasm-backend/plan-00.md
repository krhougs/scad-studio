# Studio 浏览器端（WASM）与 Web 后端协同 — 执行计划

## Context

- 当前 `scad-studio` 为 **winit + egui-winit + wgpu** 单进程桌面应用，`scad-scene::Renderer` 自建 **Surface + 场景管线 + egui_wgpu**。
- `scad-data` 使用 **`notify` 文件监控**、**本机路径**、**OpenSCAD 子进程**、部分 **rfd** 能力；这些在 `wasm32-unknown-unknown` 上不能原样保留。
- 已确认产品方向：**浏览器内只跑 UI 与渲染**；**文件系统与 `notify` 由独立 Web 后端**在其运行环境中完成；前端通过 **HTTP / WebSocket（或 SSE）** 使用这些能力。用户在前端**填写的工作区路径**解释为**后端可见路径**（或后端映射后的逻辑根），不是浏览器直接读用户本机磁盘。

## 目标

1. 在 **WebGPU 可用**的浏览器中运行 **单窗口** Studio 界面（egui + wgpu 三维视图路径与桌面版一致或刻意等价）。
2. 桌面版 **默认行为与构建保持可用**：`cargo build`、`cargo test`、现有工作流不因 wasm 工作而损坏。
3. 提供（或接入）**后端服务**：工作区挂载、目录列举、文件读写（按产品需要）、**基于 `notify` 的变更推送**。
4. 在首版可交付范围内，将 **多窗口、muda 系统菜单、rfd 文件夹选择** 在 wasm 目标中移除或替换为 **文本输入工作区路径**（与后端约定一致）。

## 非目标（可在后续迭代再议）

- 离线纯浏览器、无后端情况下的完整工作区能力。
- Electron / Tauri 等混合壳（本计划聚焦 **WASM + 独立 HTTP 后端**）。
- 与具体云厂商绑定的部署方案（计划中仅要求后端可容器化或进程部署）。

## 架构假设

- 后端与前端 **同源部署** 或 **已正确配置 CORS**；**HTTPS 或 localhost** 以满足 WebGPU 策略。
- **OpenSCAD（已拍板）**：与桌面版 **同一策略**——在 **后端运行环境** 内解析可执行文件路径（复用 `scad-data` 中 `detect_openscad_path` / `OPENSCAD_PATH` 等与桌面一致的规则），启动 CLI 完成渲染，将 **生成的 3MF** 通过 HTTP API **传回前端**；前端用与桌面相同的 **3MF 解析与网格加载** 路径更新 Viewer。日志与错误体在协议中与桌面体验对齐（可简化为单次响应内文本字段，流式为可选增强）。

## 风险与待决事项（执行中若无法拍板须写入 `docs/known_issues.md`）

- **WebGPU 与 wgpu 功能差异**：线框模式、深度模板格式、adapter 限制可能与桌面 Vulkan/Metal 不一致，需在 `scad-scene` 层做能力检测与降级（与现有 `wireframe_supported` 等逻辑衔接）。
- **后端路径安全**：必须防止工作区根路径之外的任意读取；所有路径经后端规范化与边界检查。
- **`scad-data` 依赖面**：`dirs`、`rfd`、`notify`、线程与进程在 wasm 侧需 **条件编译或拆 crate**，避免强行链接不可用依赖。

---

## Phase 1：协议与仓库布局

### 目标

- 冻结 **前后端协作契约**（资源路径、认证占位、错误码、WebSocket 事件形状），使后续 trait 实现与后端可无歧义并行开发。
- 确定 **crate / 目录划分**（例如独立 `crates/studio-backend`、共享 `crates/studio-remote-protocol` 或 `scad-data` 内子模块），避免循环依赖。

### 前序目标保护

- 本 Phase **不改** 现有桌面运行时行为与公共 API 语义；仅新增文档与空壳 crate 时不得改变默认 feature 解析结果。

### 输入

- `crates/scad-data/src/watcher.rs`
- `crates/scad-data/src/config.rs`
- `src/main.rs` 中与 `FileWatcher`、`UserEvent::SourceChanged` 相关的数据流

### 操作步骤

1. 在 `docs/` 下按项目约定新建模块文档目录（例如 `docs/2026040800-studio-web-api/`），编写：
   - 工作区会话：挂载路径、会话标识、释放语义；
   - REST：列举目录、读文件、写文件（若首版只读则明确只读）；
   - **OpenSCAD 预览**：请求字段（会话内源文件相对路径、可选参数覆盖）、响应体（**3MF 二进制** 或协商编码）、`Content-Type`、错误体、超时与 **响应体大小上限**；
   - 实时通道：WebSocket 消息类型（例如 `changed`、`deleted`、`error`）与节流策略说明。
2. 定义 **最小 JSON schema 或 Rust `serde` 类型**（放在新建共享 crate 或 `scad-data` 的 `remote` 模块），供后端与客户端共用。
3. 在根 `Cargo.toml` workspace `members` 中注册新成员（若新增 crate），保证 **默认 `cargo check -p scad-studio` 仍成功**。

### 验收标准

- 文档与类型定义可被另一名工程师直接实现后端与 WASM 客户端，无需再口头解释「路径指哪一台机器」；其中 **OpenSCAD 与 3MF 回传** 的契约已写明，并与「架构假设」中的桌面一致策略一致。
- `cargo check`（桌面默认目标）通过，无行为变化。

---

## Phase 2：工作区访问抽象（本地实现先行）

### 目标

- 将「工作区文件访问 + 变更通知」从 `StudioApp` / `main.rs` 的直接 `FileWatcher` 调用中 **抽象为 trait（或小型门面）**。
- 提供 **本地实现**：内部仍使用 **`notify` + `std::fs`**，与当前桌面行为一致。

### 前序目标保护

- Phase 1 的协议类型与文档中的命名保持一致；若发现冲突，**先修订 Phase 1 文档与类型**再改实现。
- 桌面版 **文件监听、OpenSCAD 触发、最近工作区列表** 等现有功能不得静默失效；回归以现有测试与手动场景为准。

### 输入

- `crates/scad-data/src/watcher.rs`
- `src/main.rs`、`src/workspace.rs`（若存在）、`src/app.rs` 中与 watcher 相关的接线

### 操作步骤

1. 设计 trait 方法集合（示例方向，名称以代码为准）：
   - 挂载工作区根路径；
   - 列出子项、读取文件字节或文本；
   - 订阅变更：以 `async`/`Stream` 或现有 `UserEvent` 风格回调统一输出 **规范化相对路径或 URI**。
2. 实现 `LocalWorkspaceAdapter`（名称可调整），封装现有 `FileWatcher` 与 `std::fs` 调用。
3. 将 `main.rs`（及少数调用点）改为依赖 trait 对象或泛型参数，**桌面路径默认构造本地实现**。
4. 为 **纯函数**（例如路径拼接安全、相对路径规范化）补充 `tests/` 下单元测试，覆盖非法 `..`、空段、尾随分隔符等边界。

### 验收标准

- 桌面 `cargo test` 通过；Studio 手动打开工作区、编辑文件触发重载的行为与改前一致。
- 新抽象在代码审查中可见：**WASM 客户端实现该 trait 时不需要链接 `notify`**。

---

## Phase 3：后端服务实现（notify + REST + WebSocket）

### 目标

- 实现可运行的 **HTTP 服务**：会话式挂载工作区根目录；提供 Phase 1 文档中的 REST 与 WebSocket。
- 在后端进程内使用 **`notify`**（或等价库）监控已挂载根下的变更，并 **推送** 到订阅连接。

### 前序目标保护

- Phase 2 的 trait **语义不变**；后端是 trait 的一种 **进程外实现**，不要为迁就后端而削弱本地实现的类型安全（例如不要把一切皆改成无类型字符串而丢失不变量）。
- 路径安全策略必须可测试：单元测试覆盖 **根目录逃逸** 尝试。

### 输入

- Phase 1 文档与共享类型 crate
- Rust 异步运行时选型（建议与生态一致的 `tokio` + `axum` 或团队既定栈，**以查阅官方文档后的结论为准**）

### 操作步骤

1. 新建后端 binary crate，依赖共享协议类型；实现启动参数或环境变量：**绑定地址、默认允许的工作区父目录（可选）**。
2. 实现 REST 处理器：挂载、列举、读取（及写，若协议包含）；统一错误体。
3. 实现 WebSocket：客户端订阅某会话后，接收文件系统事件；定义 **退避与批量** 策略，避免高频写入压垮前端。
4. 编写 **集成测试**（在 `tests/`）：临时目录 + 写入文件 + 断言 WS 收到事件（可用测试用端口）。

### 验收标准

- 后端可在本地启动，用最小脚本或 `curl`/`websocat` 验证列举与推送。
- 集成测试在 CI 可运行（不依赖真实浏览器）。

---

## Phase 4：远程客户端实现（供 WASM 使用）

### 目标

- 实现 Phase 2 trait 的 **`RemoteWorkspaceAdapter`**（名称可调整）：通过 **reqwest（或 wasm 兼容 HTTP 栈）+ WebSocket** 与 Phase 3 后端通信。
- 该实现编译目标包含 **`wasm32-unknown-unknown`**，**不依赖 `notify`、不依赖 `rfd`**。

### 前序目标保护

- Phase 3 的协议保持稳定；若必须变更，**同时更新** Phase 1 文档、后端与客户端，并记录于 `plan-00-result.md`。
- 桌面版不得被迫依赖仅 wasm 可用的 crate；使用 **条件依赖与 feature** 明确分离。

### 输入

- Phase 2 trait 与 Phase 3 运行中的后端
- `wasm-bindgen`、`web-sys` 等与所选 HTTP 客户端的兼容性说明（查阅文档后固定版本）

### 操作步骤

1. 为根包或 `scad-studio` 增加 feature，例如 `remote-workspace`，仅在该 feature 下引入远程客户端依赖。
2. 实现连接建立、会话恢复、断线重连（至少定义策略：首版可「重连后全量刷新树」）。
3. 将文件变更事件 **映射为与本地适配器相同** 的上层消息类型，使 `main` 或 `StudioApp` **不区分** 本地与远程来源（除构造处外）。
4. 在 `tests/` 中用 **mock HTTP 服务**（例如 `wiremock` 或轻量手写 TCP）对解析与状态机做单元或集成测试；**纯解析逻辑**必须有单测。

### 验收标准

- `cargo check --target wasm32-unknown-unknown` 在启用远程 feature 时通过（具体 feature 组合在结果文件中写明）。
- 无 `notify` / `rfd` 出现在 wasm 依赖图中（可用 `cargo tree` 抽查）。

---

## Phase 5：`scad-scene` 与 Web Surface

### 目标

- 在 **wasm 目标**下，`Renderer` 能从 **HTML canvas**（经 winit web 或 wgpu 文档推荐方式）创建 **Surface**，其余渲染与 egui 合成路径尽量复用。

### 前序目标保护

- 桌面 `Renderer::new` 与现有 **adapter 请求、`egui_wgpu::Renderer` 初始化** 行为不退化；新增代码以 `cfg` 分支隔离，避免 `#![cfg]` 大段复制。
- Phase 4 不得因渲染改动而被迫修改协议；本 Phase **专注 GPU 表面与尺寸变化**。

### 输入

- `crates/scad-scene/src/renderer.rs`
- wgpu 与 winit 针对 `wasm32` 的官方示例或文档（执行前必须查阅，禁止凭猜测写 `create_surface`）

### 操作步骤

1. 抽象 `Renderer::new` 的 **窗口句柄**来源：桌面保持 `Arc<Window>`；wasm 使用文档规定的 **Surface 目标**（可能与 `wgpu::SurfaceTarget` 相关）。
2. `resize` 与 `PhysicalSize` 在 wasm 下与 winit 行为对齐；处理 **设备像素比** 变化。
3. 针对 WebGPU 缺少的能力，复用或扩展 `wireframe_supported` 等分支，必要时在 UI 层隐藏不可用选项。
4. `scad-scene` 在 wasm 下运行 **不依赖** 仅桌面可用的 crate；调整 `Cargo.toml` 的 target-specific 依赖。

### 验收标准

- 桌面 `cargo test -p scad-scene` 通过。
- wasm 目标下 `scad-scene` 可独立 `cargo check --target wasm32-unknown-unknown`（若该 crate 单独检查；否则以根包为准）。

---

## Phase 6：`scad-studio` WASM 二进制与桌面条件编译

### 目标

- 提供 **WASM 入口**（`lib.rs` + `wasm-bindgen` 导出启动函数，或项目选定的 eframe/等价方案 —— **以 Phase 5 后实际渲染接线为准**）。
- **单窗口**：不实现 `MenuCommand::NewWindow` 的 wasm 分支；**系统菜单**：wasm 下不初始化 `muda`；**打开文件夹**：wasm 下使用 **文本输入 + 调用远程适配器挂载**。
- `rfd`、`macos_fused_titlebar` 等模块以 **`cfg(not(target_arch = "wasm32"))`**（或更精确条件）排除。

### 前序目标保护

- Phase 2～5 已建立的 **抽象与 Surface** 不得为本 Phase 临时拆掉；若入口结构必须调整，优先 **薄包装** 现有 `StudioDesktopApp` 逻辑而非复制第二套主循环。
- 桌面 `main.rs` 路径保持可读，避免 `#![cfg]` 整文件分裂成两份无人维护的副本；优先 **模块拆分 + 小 `main`**。

### 输入

- `src/main.rs` 全文结构
- `src/platform_menu.rs`、`src/macos_fused_titlebar.rs`
- Phase 4 远程适配器

### 操作步骤

1. 拆分 `main.rs`：将 **`ApplicationHandler` 实现体** 移至 `src/studio_desktop_loop.rs`（文件名可调整），`main` 仅保留 `env_logger`、配置加载与 `run_app`。
2. 新增 `src/lib.rs`（若尚无）与 wasm 启动 API：在 `wasm32` 下初始化 **远程工作区**、创建 **单窗口** winit 事件循环（参考 winit web 示例）。
3. 将 `UserEvent::Menu` 在 wasm 下的来源改为 **空操作或 UI 内按钮**（用户已确认菜单可不处理）。
4. 增加 **构建说明**：`trunk` 或 `wasm-pack`、静态资源服务方式、**必须与后端同域或 CORS** 的说明（可写在 `docs/` 或 crate `README` 片段）。

### 验收标准

- 桌面：`cargo run` 与改前功能对齐（抽样：打开工作区、切换文档、Viewer 交互）。
- 浏览器：在本地开发服务器下可加载 WASM，**至少** 显示 egui 框架与空白/占位工作区，并能通过输入路径 **调用后端挂载**（具体 UI 位置由实现决定，须在 `plan-00-result.md` 附截图或简短录屏说明）。

---

## Phase 7：OpenSCAD（后端 CLI + 3MF 回传）与 Web 配置

### 目标

- **OpenSCAD**：后端在自身环境中 **定位 OpenSCAD 二进制**（逻辑与桌面一致：`OPENSCAD_PATH` 优先，其次常见安装路径与 `PATH`，具体以 `crates/scad-data/src/openscad.rs` 为准），对 **已挂载工作区内的源文件** 执行与桌面等价的预览/渲染流程，产出 **3MF**，通过 API **把 3MF 字节（或经 Base64 等协商后的载荷）交给前端**；WASM 侧 **不** 启动子进程，仅发起请求并把返回的 3MF **喂给现有 mesh 加载路径**（与桌面解析 3MF 后送 Renderer 的方式一致）。
- **`AppConfig` 持久化**：桌面继续 `dirs` + 文件；WASM 使用 **`localStorage` 或后端用户配置 API**（择一并在文档说明）；若配置中含 OpenSCAD 路径，**仅在后端生效**（前端可把用户填写项转发给后端会话配置，由后端调用 `resolve_openscad_path` 系 API）。

### 前序目标保护

- 桌面 OpenSCAD 管线 **不得删除或弱化**；共享逻辑优先 **提取为 `scad-data`（或协议 crate）可调用的纯函数/模块**，避免桌面与后端各复制一份检测规则。
- Phase 1 的 API 文档须已包含 **渲染请求/响应**（含 3MF 主体、HTTP `Content-Type`、体积上限、超时与错误码）；若 Phase 1 未写，在本 Phase 开头 **补文档再写代码**。

### 输入

- `crates/scad-data/src/openscad.rs`（`detect_openscad_path`、`OpenScadRunner`、临时 3MF 文件处理）
- `src/main.rs` / `src/viewer_tab.rs`（或等价处）桌面侧 **OpenSCAD 消息与 mesh 更新** 数据流
- Phase 1 API 文档（渲染与 OpenSCAD 小节）

### 操作步骤

1. 在后端实现 **渲染端点**：入参为会话内 **相对路径**、导出选项（与桌面预览一致时可为固定「3MF 预览」）；内部复用或调用与 `OpenScadRunner` 相同的参数拼装方式，在服务器临时目录写 3MF 后 **读取为字节** 响应，或 **流式** 输出（首版允许整包响应，上限在文档中写明）。
2. 定义 **体积与安全限制**：单次响应最大字节数、渲染超时、并发限制，防止滥用占满磁盘与 CPU。
3. WASM 客户端：将原 `UserEvent::OpenScad` 或等价触发改为 **HTTP 请求**；收到 3MF 后走 **与桌面相同的解码入口**（必要时抽成与 UI 无关的函数以便两处调用）。
4. **`AppConfig`**：实现 WASM 侧持久化，并约定 **OpenSCAD 可执行路径** 是否允许用户通过 UI 提交给后端（若允许，须后端校验路径存在且为文件）。

### 验收标准

- `plan-00-result.md` 用一句话说明：**后端按桌面规则找 OpenSCAD，生成 3MF，前端用返回体更新 Viewer**。
- 集成测试或手工清单：**在已安装 OpenSCAD 的后端环境** 下，浏览器中打开 `.scad` 能完成一次完整预览（与桌面同源的 3MF 管线）。
- `cargo test`（桌面）全通过。

---

## Phase 8：回归、文档与交付清单

### 目标

- 全仓库 **桌面测试与关键 crate 测试** 通过；WASM 构建写入 CI 或 `justfile` / 脚本，避免回归无人发现。
- 更新 `docs/known_issues.md`（若存在未解决项）。

### 前序目标保护

- 前面各 Phase 的验收项 **全部重新执行一遍**（抽样 + CI）；禁止为「全绿」删除测试断言。

### 操作步骤

1. 在 CI 或本地脚本中增加 `cargo check --target wasm32-unknown-unknown`（含约定 feature）。
2. 撰写 **开发者文档**：如何启动后端、如何启动静态服务器、环境变量、常见 WebGPU 失败原因。
3. 使用 **独立 subagent** 按 `AGENTS.md` 要求，对本 Phase 涉及的全部 diff 做一次 review（review 结论不入库，仅用于修复问题）。

### 验收标准

- `plan-00-result.md` 中每个 Phase 有 **完成标记与变更摘要**。
- 新同事可按文档在 **30 分钟内** 跑通「后端 + 前端 WASM」开发链路；OpenSCAD 与 3MF 回传按 Phase 7 验收项在目标环境中验证。

---

## 附录：执行顺序说明

- Phase 1～3 可 **部分并行**（协议稳定前提下，后端与本地抽象并行），但 **Phase 4 依赖 Phase 2 trait 与 Phase 3 可联调的后端**。
- **Phase 5 与 Phase 2～4 可部分并行**，但 **Phase 6 必须在 Phase 5 的 Surface 可走通之后** 做集成交付。
- **Phase 7** 可与 Phase 6 后半重叠，但须在 OpenSCAD 数据流清晰后接，避免反复改 API。

---

## 备注（无上下文重启任务时阅读）

- 对话起点见同目录 `plan-prompt.md`。
- 若本计划与仓库当前结构不一致，以 **仓库代码** 为准修订计划，并同步更新本文件与 `plan-00-result.md`。
