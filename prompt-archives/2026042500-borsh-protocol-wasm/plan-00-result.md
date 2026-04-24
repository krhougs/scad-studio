# plan-00 执行结果

## 2026-04-25

### 初始研究状态

- 已完成原始 prompt 存档：`plan-prompt.md`。
- 已完成当前源码研究，覆盖 `app-server-protocol`、`app-server-transport`、`app-server-host`、`studio-common::ManagedClient`、`studio-web-wasm`、`packages/studio-web`、现有 smoke / Playwright harness。
- 已核对 `borsh` 与 `wasm-bindgen` 相关技术约束。
- 已生成架构迁移计划：`plan-00.md`。

### 初始关键发现

- 当前 WebSocket host 仍收发 text frame，`app-server-transport` wire helpers 仍使用 `serde_json`。
- `studio-common::ManagedClient` 已经以 `Vec<u8>` 作为队列边界，适合把内部编码从 JSON 替换为 Borsh。
- `packages/studio-web` 的 browser WebSocket transport 已经以 `Uint8Array` 为发送边界，但仍保留 string fallback。
- `PathBuf` 不适合作为跨端 wire contract，需要在 protocol 中改为自有路径类型。
- `PathHandle` 需要保留反序列化校验，不能只靠 Borsh derive 直接构造。

## 2026-04-25 路径策略补充

### 已完成

- 已查证 Windows、POSIX、Apple 官方路径 / 文件名资料。
- 已新增路径策略文档：[docs/2026042500-cross-platform-path-policy/README.md](/Users/krhougs/LocalCodes/scad-studio/docs/2026042500-cross-platform-path-policy/README.md)。
- 已把用户补充的路径策略要求写入 `plan-prompt.md`。
- 已重写 `plan-00.md`，补充：
  - npm scope 固定为 `@budn`。
  - 已有 `@scad-studio/*` package scope 迁移要求。
  - 配置文件和预设文件持久化格式不属于本轮迁移范围。
  - protocol wire payload 不得用 JSON 字符串承载结构化协议数据。
  - workspace path 使用 portable segment，禁止直接暴露 `PathBuf`。
  - 非法或大小写冲突的真实文件系统条目必须作为不可操作条目返回，不能静默隐藏。
  - TS 侧不得复制 Borsh schema 或路径校验规则，必须通过 protocol wasm。

### 路径策略结论

- 第一版 portable segment 不采用严格 ASCII；允许 Unicode 字母、组合标记、数字、ASCII 内部空格、JS 项目常见标点和 Unicode RGI emoji sequence，因此覆盖 CJK、常见 emoji 与常见 JS 项目文件名。
- JS 项目常见标点包含 `.`、`_`、`-`、`@`、`+`、`$`、`[`、`]`、`(`、`)`、`=`，用于兼容 `.gitignore`、`.env.local`、`@types`、`+page.svelte`、`[id].tsx`、`(group)`、`$route.tsx`、`[page=fruit]` 等命名。
- 额外拒绝 Windows 设备名、末尾句点、超长路径、过深路径、其他常见问题符号、非法零宽字符、非法全角符号、大小写不敏感冲突。
- `PathBuf` 可以被转换成 `&str[]` 风格的 segment 后序列化；禁止直接作为 wire contract 的原因是其平台语义不稳定，不是 Borsh 无法编码。
- 相对路径链接允许 `./`、`../` 和 `/` 分隔符，但只作为输入解析形式存在；解析后必须留在 workspace root 内，并转换成 canonical portable path。

### 本轮未改动

- 未修改业务代码。
- 未运行全量测试；本轮目标是研究、文档和计划更新。
- 未更新 `docs/known_issues.md`，因为本轮没有确认新的无法处理 blocker。

## 2026-04-25 plan 结构修正

### 已完成

- 已从 `plan-00.md` 删除方案 A / B / C 比较，避免在已确认方案后保留噪音。
- 已新增“已确认方案”与“新 wire protocol 契约”章节，明确 `app-server-protocol` 是唯一 schema 与 codec 来源。
- 已把 `PortablePath` 重新定位为新 wire protocol 的数据契约约束：Borsh 是 frame 编码方式，portable path 是 wire payload 数据模型限制条件。
- 已将“技术结论”拆成 “Borsh frame codec” 与 “Wire 数据契约限制”，避免把路径规则表述成和 Borsh 并列的协议方案。
- 已将 Phase 0 改为“新 wire protocol 契约固化”，Phase 1 改为“定义新 wire 数据模型并接入 Borsh codec”。

### 本轮未改动

- 未修改业务代码。
- 未运行代码测试；本轮只修正计划结构与表述。

## 2026-04-25 浮点数提示修正

### 已完成

- 已确认当前 plan 中浮点提示没有完全消失，但位置偏向 codec 实现细节，重要性不足。
- 已把 finite-only 浮点规则提升到“Wire 数据契约限制”和“新 wire protocol 契约”。
- 已在 Phase 0 契约固化中加入 finite-only 浮点规则。
- 已在 Phase 1 操作步骤和验收标准中明确 NaN / Inf 不能进入或通过 wire payload，失败路径必须返回可诊断 protocol / client error。

### 本轮未改动

- 未修改业务代码。
- 未运行代码测试；本轮仍是计划修正。

## 2026-04-25 独立 review 后修正

### Review 结论

- 独立 subagent 认为当前 plan 不能直接进入实现阶段。
- 主方向已正确：已确认方案后噪音基本删除，`PortablePath` 已放回 wire 数据契约层。
- 需要先修正的阻塞点：配置 wire JSON、导出目标语义、finite float 执行方式、写路径 symlink escape、当前必需的 `HostLocalPath`、Unicode validator 依赖、`WebSocketClientTransport` text JSON 残留、invalid workspace entry 下游适配、相对路径链接范围、wire frame marker/version。

### 已完成修正

- `ConfigLoad` / `ConfigSave`：计划已明确 wire payload 改为 typed config DTO；磁盘 `config.json` 格式不变，JSON 只能存在于 host 持久化边界。
- `ExportRun`：计划已明确输出目标改为 workspace 内 portable path，response 返回 portable output path，不再使用裸字符串或 `PathBuf`。
- `HostLocalPath`：计划已明确它是当前必须支持的 wire 类型，用于 OpenSCAD path、slicer path、recent workspace 等 server 机器路径。
- finite float：计划已要求统一 wire payload 校验入口，producer、typed config DTO、wasm JS 入参、Borsh decode 后都要覆盖 NaN / Inf。
- symlink escape：计划已加入 read 与 write/export 分离 resolver；写入和导出要 canonicalize 已存在父目录并确认仍在 workspace root 内。
- Unicode validator：计划已要求基于 docs.rs / 官方文档选择依赖并固定 Unicode 数据版本，禁止手写不完整 Unicode 表。
- WebSocket client：计划已明确 `WebSocketClientTransport` 也必须迁移或删除 JSON text 路径。
- invalid workspace entry：计划已补充 Web / `studio-common` 下游行为，不可操作条目可展示但不可打开、预览、watch 或导出。
- 相对路径链接：计划和路径文档已收窄范围，本轮不新增 Markdown 文件导航或 OpenSCAD include / use 解析能力。
- wire frame version：计划已要求 frame 外层包含 magic 和 wire version，旧 JSON frame / 错误二进制 frame / unsupported version 在 dispatch 前拒绝。

### 本轮未改动

- 未修改业务代码。
- 未运行代码测试；本轮为 plan 和路径策略文档修正。

## 2026-04-25 Phase 0 执行结果

### 已完成

- 已核对 `plan-00.md` 与路径策略文档，Phase 0 所需的新 wire protocol 契约已经固化。
- 已由独立 subagent 完成 Phase 0 review，结论为通过。
- Review 核对到的关键证据包括：用户强制约束、允许保留的 JSON 分类、`@budn` scope、wire frame magic / version、typed config DTO、finite-only 浮点契约、export target、`HostLocalPath`、portable path、相对链接、symlink 安全和 binary-only 策略。
- 路径策略文档已覆盖 portable path、host-local path、CJK / emoji 支持、JS 项目文件名兼容性、相对路径链接、非法条目处理和测试要求。

### 回归与边界

- Phase 0 只涉及计划和契约文档；本阶段未修改业务代码。
- 未运行代码测试；Phase 0 的验收对象是计划与路径策略文档。

### 遗留问题

- 无 Phase 0 blocker。后续进入 Phase 1 时需要保护本阶段固化的契约边界，不能恢复 JSON frame fallback、不能把完整 envelope 构造权交给 TypeScript，也不能把平台 `PathBuf` 直接作为 wire payload。

## 2026-04-25 Phase 1 执行结果

### 已完成

- `app-server-protocol` 已新增 Borsh wire frame codec，包含 `BDNP` magic、wire version、client / server frame encode / decode API 和可诊断错误分类。
- protocol 核心 envelope、命令、响应、推送、错误和导出格式已接入 Borsh；关键 enum 使用显式 discriminant，并增加 golden bytes 测试锁定 `ClientEnvelope`、`ClientCommand`、`ServerEnvelope` 与 `CommandSuccess` 的关键 wire 行为。
- `PathHandle` 已手写 Borsh decode，decode 后仍通过构造器执行路径校验。
- 已新增 `HostLocalPath`、typed config DTO、workspace portable export target；protocol wire payload 不再直接暴露 `PathBuf`，`ConfigLoad` / `ConfigSave` 不再使用 `json: String`。
- preview mesh 和 config DTO 的浮点 payload 已在 frame encode / decode 边界执行 finite 校验，拒绝 NaN / Inf。
- workspace path 校验已扩展到 CJK、常见 emoji、JS 项目常见文件名、Windows reserved name、全角问题符号、非法零宽字符、末尾句点、相对链接越界和大小写冲突 key。
- workspace list 已支持非法条目和大小写冲突条目作为不可操作 entry 返回，不再让整个目录列表失败；symlink 指向 workspace 外部和非 UTF-8 / replacement character 条目会返回 `path: None` 与 `path_error`。
- workspace 写路径和导出路径使用已存在父目录 canonicalize 校验，覆盖 symlink escape 回归测试。
- app server host、desktop protocol client 和 core 配置边界已适配 typed config DTO、`HostLocalPath` 和 portable export target。磁盘 `config.json` roundtrip 保持不变。
- `app-server-protocol` 已移除 `serde_json` crate 依赖；旧 JSON roundtrip 测试已替换为 Borsh / frame roundtrip 测试。

### Review 与收敛

- Phase 1 初次独立 review 结论为不通过，指出 invalid workspace entry、`serde_json` 依赖、golden bytes、host-local path 测试和导出 symlink 测试缺口。
- 第二次独立 review 仍指出非 UTF-8 / symlink escape entry 降级处理与 golden bytes 覆盖不足。
- 已按 review 反馈完成收敛；第三次独立 re-review 结论为通过。

### 验证

- `cargo check --workspace`：通过；仍有 `app-server-core` 既有 unused warning。
- `cargo test -p app-server-protocol --tests`：通过。
- `cargo test -p app-server-core --tests`：通过；仍有 `app-server-core` 既有 unused warning。
- `cargo test -p app-server-host --tests`：通过。
- `cargo test -p studio-common --tests`：通过。

### 遗留问题

- Phase 1 未切换 WebSocket binary-only；该项属于 Phase 3。
- Phase 1 未新增 protocol wasm / TypeScript package；该项进入 Phase 2。
- `studio-common::ManagedClient`、`studio-web-wasm`、`packages/studio-web` 仍存在 JSON envelope / string fallback 迁移任务；按计划属于 Phase 4。
- npm scope 迁移与最终 JSON wire 残留清理属于 Phase 5。

## 2026-04-25 Phase 2 执行结果

### 已完成

- 已新增 `app-server-protocol-wasm` crate，并加入 workspace。
- 已新增 `packages/app-server-protocol`，包名为 `@budn/app-server-protocol`，作为 protocol wasm 与 TypeScript 类型入口。
- 已新增 `protocol:build`、`protocol:smoke`、`protocol:check-generated` 脚本；wasm-bindgen 版本检查已覆盖 `studio-web-wasm` 与 `app-server-protocol-wasm`。
- protocol wasm 已暴露 decode、path handle、relative link、host-local path、finite numeric、config DTO 校验，以及按 command 拆分的 client frame 编码 helper。
- 已移除完整 envelope 的 generic encode 暴露入口，TS 无法通过 package API 传完整 `ClientEnvelope` / `ServerEnvelope` 后编码。
- TypeScript package 已导出 portable path、host-local path、typed config、config save request、export request / response、workspace、file、preview、watch、session、server response / event / error 等结构类型；这些类型不参与 Borsh 序列化实现。
- `studio-web` 已增加对 `@budn/app-server-protocol` 的 workspace dependency，并新增 import smoke，验证可导入类型和 codec API。
- protocol package smoke 已覆盖：
  - CJK / emoji path segment；
  - JS 项目常见文件名；
  - 合法相对链接；
  - Windows reserved name 和越界相对链接的结构化错误码；
  - Windows / macOS / Unix host-local path；
  - config DTO 校验、server frame decode、`config.save` typed request frame encode/decode；
  - finite numeric 错误；
  - export typed request frame encode/decode。
- generated 检查脚本已固定 expected wasm-bindgen 产物清单，要求生成物进入 index，并拒绝 generated 目录下额外未跟踪文件。
- `app-server-protocol::validate_f32` 已公开，protocol wasm 的 finite numeric helper 调用 protocol crate validator，不复制校验规则。

### Review 与收敛

- Phase 2 初次独立 review 结论为不通过，指出 generic complete envelope encode、TypeScript 类型覆盖不足、generated 检查缺失、decode smoke 缺口、export / finite smoke 缺口和自由文本错误。
- 第二次独立 review 结论为不通过，指出 generated 检查不能保证新快照存在、finite numeric 校验仍在 wasm 层复制。
- 第三次独立 review 结论为不通过，指出 config smoke 未构造 typed request params、`studio-web` import smoke 未导入 codec API。
- 已按 review 反馈完成收敛；第四次独立 review 结论为通过。

### 验证

- `bun run protocol:build`：通过。
- `bun run protocol:smoke`：通过。
- `bun run protocol:check-generated`：通过。
- `bun run check:wasm-bindgen`：通过。
- `cargo check -p app-server-protocol-wasm --target wasm32-unknown-unknown`：通过。
- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run --cwd packages/studio-web test:unit`：通过，16 个 test file、77 个 test 通过。
- `cargo check --workspace`：通过；仍有 `app-server-core` 既有 unused warning。

### 遗留问题

- generated `.d.ts` 的 wasm-bindgen 函数参数仍是 `any`；当前类型约束依赖 `packages/app-server-protocol/src/index.ts` 的手写结构类型和 smoke 覆盖。后续 Phase 4 实际接入 Web transport 时，应增加 typed wrapper 或更强类型测试，避免调用方绕过 request 类型。
- `protocol_validate_config` 目前通过合成 `ConfigLoaded` server frame 执行 DTO 校验与 decode smoke，不阻塞 Phase 2；后续可按实际使用情况改成更直观的 validate-only API。
- Phase 2 未切换 WebSocket binary-only；该项进入 Phase 3。

## 2026-04-25 Phase 3 执行结果

### 已完成

- `app-server-transport` WebSocket helper 已从 JSON text helper 切换为 Borsh binary helper，统一调用 `app-server-protocol` 的 frame codec。
- `app-server-transport` 不再导出可用的 text JSON WebSocket helper。
- wasm `WebSocketClientTransport` 已切换为：
  - 发送 `send_with_u8_array`；
  - 创建 socket 后设置 `binaryType = arraybuffer`；
  - 接收端只接受 `ArrayBuffer` / `Uint8Array`，不保留 string fallback。
- `app-server-host` WebSocket host 已切换为：
  - handshake 与后续消息只接受 `Message::Binary`；
  - text frame、错误 binary frame、unsupported wire version 在 dispatch 前返回 `TransportError`；
  - server response / push / error 均使用 `Message::Binary` 发送。
- `app-server-host` 与 `app-server-transport` 移除了当前不再需要的 `serde` / `serde_json` 依赖。
- `ClientTransport` trait、`InMemoryTransport`、host mpsc typed transport 保持 typed message 行为，未改成序列化路径。

### Review 与收敛

- Phase 3 初次独立 review 结论为不通过，指出 wasm `WebSocketClientTransport` 未设置 `binaryType = arraybuffer`，浏览器默认 Blob 会导致 binary server frame 无法进入 decode 路径。
- 已按 review 反馈补充 `BinaryType::Arraybuffer` 设置和 `web-sys` feature，并增加 wasm32-only test helper 锁定 socket binary type。
- 第二次独立 review 结论为通过。

### 验证

- `cargo test -p app-server-transport --tests`：通过，10 个 test 通过。
- `cargo test -p app-server-host --tests`：通过；仍有 `app-server-core` 既有 unused warning。
- `cargo check -p app-server-transport --target wasm32-unknown-unknown`：通过。
- `cargo check --workspace`：通过；仍有 `app-server-core` 既有 unused warning。

### 遗留问题

- host 端没有单独用 WebSocket end-to-end 测试覆盖“binary frame 但 magic 错误”的场景；当前由 shared binary decode helper 的旧 JSON bytes 测试覆盖同类错误分类，且 host 使用同一 decode path，不阻塞 Phase 3。
- `studio-common`、`studio-web-wasm`、`packages/studio-web` 仍有 JSON wire 相关残留；按计划进入 Phase 4 / Phase 5。

## 2026-04-25 Phase 4 执行结果

### 已完成

- `studio-common::ManagedClient` 的 outbound / inbound bytes 已切换为 `app-server-protocol` Borsh frame codec，出站使用 `encode_client_frame`，入站使用 `decode_server_frame`。
- `studio-web-wasm` smoke 已改为用 `encode_server_frame` 构造 server inbound frame，并用 `decode_client_frame` 断言 browser client outbound frame。
- `packages/studio-web` browser WebSocket transport 已移除 string frame fallback，只接受 `ArrayBuffer` / `Uint8Array`。
- Playwright harness 已改为捕获真实 `WebSocket.send` binary bytes，并通过 `@budn/app-server-protocol` 的 wasm decode helper 解析 outgoing client frame；旧的应用层 direct payload 记录路径已删除。
- 配置页已改用 typed config DTO：`ConfigLoad` 读取 `config` 字段，`ConfigSave` 发送 `ConfigSaveRequest { config }`；配置 JSON 只保留为用户可见 snapshot 和磁盘文件格式相关处理，不再作为 wire payload。
- 导出与 slicer 面板已通过 protocol wasm path helper 把默认输出文件名解析为当前源文件同目录的 portable `PathHandle`，outgoing frame 中 `output_path` 是 portable path，不是裸字符串或 host path。
- Web workspace tree 已接入 invalid workspace entry：保留 `name` / `path_error` 展示，设置不可操作状态，禁止把 `path: null` 当成 root 或合法 path 打开 / 展开。
- 已新增 invalid workspace entry Playwright smoke，覆盖“可展示但不可打开”行为。
- 已更新 `studio-web-wasm` generated wasm 快照，避免浏览器端继续加载旧 JSON frame 版本。

### Review 与收敛

- Phase 4 初次独立 review 结论为不通过，指出三项阻塞问题：
  - Playwright recorder 仍可能使用应用层 direct payload，无法证明真实 outgoing binary frame 可解码；
  - invalid workspace entry 的 `path: null` 可能被 Web 侧当成 root；
  - export helper 使用相对链接解析时会剥离 `#fragment`，导致非法文件名被静默改写。
- 已按 review 反馈完成收敛：harness 只 decode WebSocket binary frame，invalid entry 保留 `name` / `path_error` 且不可操作，export 输出路径改用 protocol path handle 构造。
- 第二次独立 review 结论为通过。
- 收敛后又删除了 `_smoke-harness.ts` 和 `wasm-bridge/client.ts` 中旧的 `__scadDispatchedCommands` direct payload 记录路径；轻量独立复核结论为通过。

### 验证

- `cargo test -p studio-common --tests`：通过。
- `cargo test -p studio-web-wasm --tests`：通过。
- `bun run --cwd packages/studio-web test:unit`：通过，19 个 test file、84 个 test 通过。
- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run protocol:smoke`：通过。
- `bun run check:wasm-bindgen`：通过。
- `cargo check --workspace`：通过；仍有 `app-server-core` 既有 dead code warning。
- `bun run --cwd packages/studio-web test:e2e -- config-settings.spec.ts export-slicer.spec.ts invalid-workspace-entry.spec.ts`：通过，4 个 Playwright 测试通过。
- `bun run web:smoke -- --case browser_smoke`：通过，5 个 Playwright 测试通过。
- `bun run web:smoke -- --case browser_watch_smoke`：通过，6 个 Playwright 测试通过。
- `bun run web:smoke -- --case export_slicer`：通过，1 个 Playwright 测试通过。
- `bun run web:smoke -- --case config_settings`：通过，2 个 Playwright 测试通过。
- `bun run web:smoke -- --case parameters_presets`：通过，7 个 Playwright 测试通过。
- `bun run web:build`：通过；仍有 Vite 既有 large chunk warning。
- `wasm-pack test --headless --chrome crates/studio-web-wasm --test wasm_bridge_smoke`：未通过，原因为本机 Chrome `147.0.7727.103` 与 wasm-pack 下载的 ChromeDriver `148.0.7778.56` 不匹配，`wasm-bindgen-test-runner` 报 `http status: 404` 且 driver 以 `signal: 9 (SIGKILL)` 退出；已记录到 [docs/known_issues.md](/Users/krhougs/LocalCodes/scad-studio/docs/known_issues.md:3)。

### 遗留问题

- Phase 4 仍未能在当前机器完成 `wasm-pack --headless --chrome` browser wasm smoke 的通过验收；该问题被确认是本机 ChromeDriver 版本匹配问题，不是本阶段代码路径失败。
- `packages/studio-web/tests/unit/protocol-paths.test.ts` 使用 mock 验证调用边界；真实 protocol wasm path helper 通过 export / config / invalid entry 相关 e2e 与 smoke 间接覆盖。若后续新增用户输入导出文件名校验 UI，应补真实 wasm path helper 的浏览器错误路径测试。
- npm scope 迁移、旧 JSON wire 残留全仓库 grep 白名单和最终全量回归进入 Phase 5。

## 2026-04-25 Phase 5 执行结果

### 已完成

- `packages/studio-web` 的 npm package name 已从 `@scad-studio/studio-web` 迁移为 `@budn/studio-web`。
- `packages/studio-web-wasm` 的 npm package name 已从 `@scad-studio/studio-web-wasm` 迁移为 `@budn/studio-web-wasm`。
- 已同步 workspace dependency、`bun.lock`、TypeScript path alias、Vitest alias、源码 import、wasm package smoke 校验、README 与 getting-started 文档中的 package scope。
- 已用 grep 验证 packages / docs / scripts / lockfile 中不再保留旧 `@scad-studio` package name 或 import。
- 已对白名单外 wire 关键路径做 grep 验收，确认：
  - WebSocket / transport / host / managed client / wasm bridge smoke 的 wire 路径没有 `serde_json::to_vec` / `serde_json::from_slice` / `serde_json::to_string` / `serde_json::from_str`。
  - browser transport 与 Playwright wire recorder 没有 `TextEncoder` / `TextDecoder` / `JSON.parse` / `JSON.stringify` / `__scadDispatchedCommands`。
  - `app-server-protocol` 的 wire payload 源码没有 `pub json: String`、`ConfigLoadResponse` / `ConfigSaveRequest` 的 JSON payload、`output_path: PathBuf` 或 protocol 层 `PathBuf` import。
- 已同步 `docs/web-platform-limits.md` 的导出说明：`ExportRun.output_path` 当前是 workspace 内 portable path，web 默认导出到当前源文件同目录。
- 已更新 `docs/known_issues.md` 中旧 `ExportRun.output_path: PathBuf` 问题的当前处理方式，明确该项已经由本计划 Phase 1 / Phase 4 解决，保留为历史记录。

### Review 与收敛

- Phase 5 独立 review 结论为通过。
- Review 核对到的关键证据包括：`@budn` scope 已迁移、旧 `@scad-studio` scope 在非 `prompt-archives` 范围无命中、WebSocket host 与 browser transport 仍保持 binary-only、Playwright recorder 只 decode 真实 binary frame、typed config DTO 与 portable export path 未回退。
- Review 识别的残余风险为本机 `wasm-pack --headless --chrome` 环境缺口，以及 `studio-web-wasm` smoke 中用于检查 wasm `JsValue` 的 `serde_json::Value`；后者不是 wire envelope encode / decode。

### 验证

- `rg -n '@scad-studio' packages`：无命中。
- `rg -n 'serde_json::(to_vec|from_slice|to_string|from_str)' crates/app-server-protocol crates/app-server-transport crates/app-server-host/src/websocket.rs crates/studio-common/src/managed_client crates/studio-web-wasm/tests/wasm_bridge_smoke.rs packages/studio-web/src/transport packages/studio-web/tests/playwright/_smoke-harness.ts`：无命中。
- `rg -n 'TextEncoder|TextDecoder|JSON\.parse|JSON\.stringify|__scadDispatchedCommands' packages/studio-web/src/transport packages/studio-web/tests/playwright/_smoke-harness.ts`：无命中。
- `rg -n 'pub json: String|ConfigLoadResponse[^\n]*json|ConfigSaveRequest[^\n]*json|output_path: PathBuf|use std::path::PathBuf' crates/app-server-protocol/src`：无命中。
- `cargo check --workspace`：通过；仍有 `app-server-core` 既有 dead code warning。
- `cargo test --workspace --tests`：通过；仍有 `app-server-core` 既有 dead code warning。
- `bun run protocol:smoke && bun run protocol:check-generated && bun run check:wasm-bindgen`：通过。
- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run --cwd packages/studio-web test:unit`：通过，19 个 test file、84 个 test 通过。
- `bun run web:smoke -- --case wasm_package_smoke`：通过，generated tree byte-identical。
- `bun run web:build`：通过；仍有 Vite 既有 large chunk warning。
- `bun run web:smoke`：S1a 通过，S1b 失败；失败原因与已记录 known issue 一致：本机 Chrome `147.0.7727.103` 与 wasm-pack 下载的 ChromeDriver `148.0.7778.56` 不匹配，runner 报 `http status: 404` 且 driver 以 `signal: 9 (SIGKILL)` 退出。
- `bun run web:smoke -- --case browser_smoke`：通过，5 个 Playwright 测试通过。
- `bun run web:smoke -- --case browser_watch_smoke`：通过，6 个 Playwright 测试通过。
- `bun run web:smoke -- --case export_slicer`：通过，1 个 Playwright 测试通过。
- `bun run web:smoke -- --case config_settings`：通过，2 个 Playwright 测试通过。
- `bun run web:smoke -- --case parameters_presets`：通过，7 个 Playwright 测试通过。
- `git diff --check HEAD`：通过。

### 遗留问题

- 当前机器仍无法完成默认 `bun run web:smoke` 的 S1b browser wasm smoke，通过条件是修复本机 Chrome / ChromeDriver 版本匹配；该问题已记录到 [docs/known_issues.md](/Users/krhougs/LocalCodes/scad-studio/docs/known_issues.md:3)。
- `wasm-bindgen` generated 产物中的 `TextEncoder` / `TextDecoder` / `JSON.stringify` 属于 wasm-bindgen glue 或调试显示代码；不属于本计划禁止的 WebSocket wire JSON fallback。
- 配置文件、预设文件、用户内容解析和错误展示中的 JSON 处理仍按计划允许保留。

## 2026-04-25 计划完成状态

- Phase 0 到 Phase 5 已连续执行完成，每个 Phase 均完成独立 subagent review 和收敛。
- WebSocket / protocol envelope wire 已迁移为 Borsh binary frame。
- `app-server-protocol` 仍作为唯一 schema 与 codec 来源；TypeScript 侧没有复制完整 Borsh schema 或完整 envelope 构造权。
- `studio-app` 的 in-memory typed transport 语义未被改为序列化路径。
- 剩余阻塞项仅为本机 `wasm-pack --headless --chrome` 环境问题，已作为 known issue 记录。
