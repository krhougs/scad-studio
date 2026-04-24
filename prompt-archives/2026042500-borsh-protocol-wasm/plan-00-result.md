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
