# plan-00：budn Borsh 二进制 wire protocol 迁移计划

## 背景

本计划针对 `studio-web` 与后端 app server 的 WebSocket wire protocol 做二进制迁移。当前仓库已经具备统一 app server / protocol / transport 的分层：

- `app-server-protocol` 定义命令、响应、推送、错误和数据模型。
- `app-server-transport` 当前提供 WebSocket JSON wire helpers，另有 in-memory transport。
- `app-server-host` 的 WebSocket host 当前读取 `Message::Text`，用 `serde_json` 解码 `ClientEnvelope`，再用 text frame 返回 `ServerEnvelope`。
- `studio-common::ManagedClient` 已经以 `Vec<u8>` 做出站 / 入站队列，但这些字节现在仍来自 `serde_json::to_vec` / `serde_json::from_slice`。
- `packages/studio-web` 的浏览器 WebSocket transport 已经把 `binaryType` 设置为 `arraybuffer`，并发送 `Uint8Array`；浏览器壳层本身不直接构造 protocol envelope，但仍保留 string fallback 和 JSON 测试 harness。
- `PathHandle` 当前是 `Vec<String>` segment 形态，已经比 `PathBuf` 更适合 wire contract，但校验只覆盖空 segment、`.`、`..` 和分隔符，不足以保证跨 Windows / macOS / Linux 一致。

本计划只覆盖 wire protocol、protocol wasm、TypeScript package 与 WebSocket transport 迁移。不改变 `studio-app` 内部基于 `tokio::mpsc` 的 typed in-memory 通信方式。不迁移配置文件和预设文件的持久化格式；如果这些文件内容作为用户文件内容传输，它们仍只是文件内容，不是 protocol envelope JSON。

## 用户强制约束

1. `studio-web` 与后端交互的序列化 / 反序列化方式改为二进制。
2. Rust 端序列化方案必须使用 `borsh`。
3. 新增 protocol wasm 包和对应 TypeScript 包，把协议数据结构暴露给 JS 侧。
4. 现有功能必须完整迁移；最终状态不得保留 WebSocket / protocol envelope 级 JSON。
5. 本次不涉及 `studio-app` 内部基于内存的通信。
6. 本次只涉及 wire protocol，不迁移配置文件和预设文件格式。
7. 项目 npm scope 必须使用 `@budn`；已有包的 scope 也要迁移。
8. TS 禁止手工构造完整 protocol envelope；序列化 / 反序列化完全交给 wasm。
9. 路径和文件名必须采用严格跨平台兼容策略，但不能限定 ASCII；至少支持 CJK 和常见 emoji，过滤常见问题符号，允许相对路径链接解析，并兼容常见 JS 项目文件名。

## 参考文档

- 路径策略文档：[docs/2026042500-cross-platform-path-policy/README.md](/Users/krhougs/LocalCodes/scad-studio/docs/2026042500-cross-platform-path-policy/README.md)
- Microsoft 文件命名规则：[Naming Files, Paths, and Namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)
- POSIX portable filename character set：[Open Group Definitions 3.264-3.265](https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap03.html)
- Apple 文件系统说明：[File System Basics](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html)
- Unicode emoji 与字素簇规范：[UTS #51 Unicode Emoji](https://www.unicode.org/reports/tr51/) 与 [UAX #29 Unicode Text Segmentation](https://www.unicode.org/reports/tr29/)

## 技术结论

### Borsh frame codec

- `borsh` 使用 `BorshSerialize` / `BorshDeserialize`，常规入口是 `borsh::to_vec(&value)` 与 `borsh::from_slice::<T>(&bytes)`；`from_slice` 要求输入被完整读取，适合作为 WebSocket frame decode 边界。
- `borsh` 的枚举默认按声明顺序写入 `u8` variant index，不读取 `serde(rename)`、`serde(tag)`、`serde(untagged)` 等属性。wire enum 必须显式分配稳定 discriminant，并用 `#[borsh(use_discriminant = true)]` 保护后续重排。
- `borsh` 对浮点数会拒绝 NaN；mesh、camera、layout 等浮点字段需要保持有限数值，错误路径要转为 protocol / client error。
- `wasm-bindgen` 可把 Rust `Vec<u8>` / `&[u8]` 暴露为 JS `Uint8Array`，适合作为 protocol wasm 包的 encode / decode API。
- `serde-wasm-bindgen` 可继续用于 wasm 与 JS 之间的结构化对象转换；它不是 WebSocket wire JSON。最终必须删除 JSON frame、`serde_json` wire helper、`TextDecoder + JSON.parse` 测试 recorder 与旧 scope import。

### Wire 数据契约限制

- frame 外层必须有可校验的 magic 和 wire version。旧 JSON text frame、错误二进制 frame、版本不支持 frame 必须在进入 command dispatch 前得到清晰拒绝。
- `borsh` 可以序列化 `String`、`Vec<T>` 和其他常见类型；`PathBuf` 也可以先转成字符串或 segment 再序列化。这里禁止直接暴露 `PathBuf` 的原因不是无法序列化，而是它的语义属于所在操作系统，不适合作为跨端 wire contract。
- 所有进入 wire payload 的浮点字段都必须是 finite value；NaN、`+Inf`、`-Inf` 均为非法协议值。encode 前要在 producer 边界校验，decode 后也要在 protocol 数据模型边界校验，失败时返回 protocol / client error，不能 panic。
- 配置文件仍以 JSON 持久化，但 `ConfigLoad` / `ConfigSave` 的 wire payload 必须是 typed config DTO，不能继续用 `json: String` 穿过 WebSocket frame 或 wasm client API。
- workspace 内路径必须使用 protocol 自有的 portable segment 模型。segment 第一版允许 Unicode 字母、组合标记、数字、ASCII 内部空格、JS 项目常见标点和 Unicode RGI emoji sequence；拒绝其他常见问题符号、不可见控制字符、Windows 设备名、末尾句点、深度过大、总长度过大和同目录大小写冲突。
- JS 项目常见标点包含 `.`、`_`、`-`、`@`、`+`、`$`、`[`、`]`、`(`、`)`、`=`，用于兼容 `.gitignore`、`.env.local`、`@types`、`+page.svelte`、`[id].tsx`、`(group)`、`$route.tsx`、`[page=fruit]` 等常见命名。
- 用户文件和文档中的相对路径链接可以包含 `./`、`../` 和 `/` 分隔符；解析后必须位于 workspace root 内，并转换成 canonical portable path。wire protocol 不保存原始相对路径字符串。
- `ExportRun` 的输出目标必须是 workspace 内 portable path，不能继续使用裸字符串或 `PathBuf`。默认导出文件名应解析为当前源文件所在目录下的 portable path，response 返回 portable output path。
- host-local executable path 是当前 wire protocol 必须支持的类型，用于 OpenSCAD path、slicer path、recent workspace 等 server 机器路径；它不能混入 portable workspace path，必须用单独 `HostLocalPath` 表示，并由 server 所在平台校验。
- `PathHandle` 的 Borsh decode 不能绕过校验。应通过 raw decode 后调用协议构造器，或手写 decode。

## 已确认方案

`app-server-protocol` 是唯一 schema 与 codec 来源；`app-server-protocol-wasm` 只把 Rust codec、类型转换和路径校验暴露给 JS；`packages/app-server-protocol` 以 `@budn/app-server-protocol` 作为 TypeScript import 入口。`studio-common::ManagedClient`、`app-server-host`、protocol wasm 和测试都调用同一组 Rust codec。TS 不实现 Borsh，不手工构造完整 protocol envelope，不复制路径校验规则。

## 新 wire protocol 契约

新 wire protocol 由两部分组成：

1. **Frame 编码**：WebSocket frame 使用 Borsh 二进制编码，不保留 JSON fallback。
2. **数据契约**：所有可跨端传输的数据必须是 `app-server-protocol` 自有类型，不能暴露平台私有语义。

`PortablePath` 是第二部分的数据契约约束，不是和 Borsh 并列的协议方案。具体要求：

- frame 外层必须包含 magic 和 wire version，旧 JSON text frame 与错误二进制 frame 必须有明确拒绝路径。
- `ConfigLoad` / `ConfigSave` wire payload 使用 typed config DTO；JSON 只允许存在于 server 读写磁盘文件的边界。
- workspace 文件引用只使用 protocol 自有的 portable path model。
- `ExportRun` 输出目标只允许 workspace 内 portable path，response 也返回 portable path。
- wire payload 不直接暴露 `PathBuf`、绝对路径、平台分隔符、Windows drive path 或 UNC path。
- OpenSCAD、slicer、recent workspace 等 server 机器路径使用 `HostLocalPath` wire 类型，不能套用 portable workspace path 规则。
- portable path 的每个 segment 必须通过跨平台文件名校验；该校验属于 protocol 数据模型的一部分，Borsh decode 后也必须执行。
- 浮点字段必须作为 protocol 数据模型的一部分定义 finite-only 约束；Borsh 本身会拒绝 NaN，但 plan 不能只依赖 codec 报错，必须在数据进入 wire frame 前形成可诊断错误。
- 相对路径链接只是一种输入解析形式；解析成功后必须转换为 canonical portable path，wire payload 不保存原始相对路径字符串。
- workspace 写入和导出必须使用写路径 resolver：canonicalize 已存在父目录并确认仍在 workspace root 内，拒绝 symlink escape。

## 目标架构

- `app-server-protocol`：
  - 继续作为协议数据模型唯一来源。
  - 新增 Borsh derives、稳定 enum discriminant、manual decode 校验、wire frame magic/version 和 wire frame codec。
  - 持有 wire 数据契约所需的 typed config DTO、`HostLocalPath`、export target、finite float 校验、portable path segment 校验、大小写冲突判定、相对路径链接解析和错误类型。
  - 提供 `encode_client_frame` / `decode_client_frame` / `encode_server_frame` / `decode_server_frame` 一类稳定 Rust API。
  - 定义独立 wire version。语义 protocol version 继续在 handshake 里协商，wire version 用于拒绝旧 JSON / 错误二进制帧。

- `app-server-protocol-wasm`：
  - 新增 wasm crate，只依赖 `app-server-protocol`、`wasm-bindgen` 和必要的 JS 值转换工具。
  - 暴露 encode / decode / validate / path constructor 辅助函数，输入输出以 `Uint8Array` 与结构化 `JsValue` 为边界。
  - 不持有 WebSocket、不持有 request registry、不持有 workspace / preview 状态。

- `packages/app-server-protocol`：
  - 新增 TypeScript package，package name 为 `@budn/app-server-protocol`。
  - re-export wasm-bindgen generated 产物，并导出协议 TypeScript 类型。
  - TypeScript 类型必须由 Rust schema 生成，或通过 generated smoke / golden tests 锁定；不得手写会影响序列化的 schema。
  - 包内不写业务状态机，不复制 Borsh schema，不复制路径校验规则。

- 现有 npm packages：
  - `@scad-studio/studio-web` 迁移为 `@budn/studio-web`。
  - `@scad-studio/studio-web-wasm` 迁移为 `@budn/studio-web-wasm`。
  - 所有 workspace dependency、tsconfig alias、Vitest alias、README 与注释中的 package scope 同步更新。

- `studio-common::ManagedClient`：
  - 状态机归属保持不变。
  - 只把 envelope bytes 的编码从 JSON helper 切换到 `app-server-protocol` 的 Borsh codec。

- `app-server-transport` / `app-server-host`：
  - WebSocket wire 从 text frame 切换为 binary frame。
  - in-memory transport 继续传 typed `ClientEnvelope` / `ServerEnvelope`，不做序列化改造。

- `packages/studio-web`：
  - WebSocket transport 只发送 / 接收 `Uint8Array`，移除 string fallback。
  - 继续通过 wasm client 派发命令和消费 snapshot / events。
  - 测试 recorder 若需要观察协议帧，必须通过 `@budn/app-server-protocol` decode binary frame，不再 `TextDecoder + JSON.parse`。

## Phase 0：新 wire protocol 契约固化

### 输入

- 用户本轮强制约束。
- 当前 `AGENTS.md` 架构约束。
- 当前源码中的 JSON wire 使用点、WebSocket text frame、测试 recorder、旧 npm scope。
- 路径策略文档：[docs/2026042500-cross-platform-path-policy/README.md](/Users/krhougs/LocalCodes/scad-studio/docs/2026042500-cross-platform-path-policy/README.md)

### 操作步骤

1. 明确“最终不得残留 JSON”的范围：WebSocket frame、protocol envelope、protocol command payload 内不得用 JSON 承载结构化协议数据。
2. 明确允许保留的非 wire JSON：配置文件、预设文件、工具链元数据、测试 fixture 中作为用户文件内容存在的 JSON。
3. 固定新增包命名：
   - Rust crate：`app-server-protocol-wasm`
   - npm package directory：`packages/app-server-protocol`
   - npm package name：`@budn/app-server-protocol`
4. 固定旧 npm scope 迁移：`packages/studio-web` 与 `packages/studio-web-wasm` 均改为 `@budn/*`。
5. 固定 TS 侧职责：TS 不手工构造完整 protocol envelope；TS 只把结构化参数传给 wasm，由 wasm/Rust codec 生成或解析 bytes。
6. 把 finite-only 浮点规则写入新 wire protocol 数据契约：mesh、camera、layout 等所有浮点 payload 不允许 NaN / Inf，错误要转成 protocol / client error。
7. 把 typed config DTO 写入新 wire protocol 数据契约：配置磁盘文件继续是 JSON，但 `ConfigLoad` / `ConfigSave` wire payload 不再出现 `json: String`。
8. 把 export target 写入新 wire protocol 数据契约：`ExportRun` 输出目标必须是 workspace 内 portable path，response 返回 portable output path。
9. 把 `HostLocalPath` 写入新 wire protocol 数据契约：OpenSCAD、slicer、recent workspace 等 server 机器路径使用独立 wire 类型，不套用 portable workspace path 规则。
10. 把 portable path 写入新 wire protocol 数据契约：workspace path 只能使用 portable segment；segment 支持 CJK、常见文字、Unicode RGI emoji 和 JS 项目常见标点，拒绝其他常见问题符号；非法或冲突的真实文件系统条目必须以不可操作条目暴露，不能静默隐藏。
11. 固定相对路径链接策略：相对链接允许 `./`、`../` 和 `/`，但解析结果必须转换为 canonical portable path，且不得越过 workspace root。本轮不新增 Markdown 文件导航或 OpenSCAD include / use 解析能力。
12. 固定 symlink 安全策略：read 与 write/export 使用不同路径解析；写入和导出必须校验已存在父目录的 canonical path 仍在 workspace root 内。
13. 固定 wire frame 策略：frame 外层包含 magic 和 wire version；旧 JSON client、错误二进制 frame 和不支持版本必须被清晰拒绝。
14. 固定不兼容策略：最终不保留 JSON v1 fallback；旧 JSON client 会被 binary wire 拒绝。

### 前序目标保护

本 Phase 是契约阶段，没有前序 Phase。执行时只允许写计划 / 契约文档，不改业务代码。

### 验收标准

- 计划列出全部强制约束、允许保留的 JSON 分类、`@budn` scope、wire frame magic/version、typed config DTO、finite-only 浮点数据契约、export target、`HostLocalPath`、portable path 数据契约、相对链接策略、symlink 安全策略和 binary-only 策略。
- 路径策略文档存在，并说明 portable path、host-local path、CJK / emoji 支持、JS 项目文件名兼容性、相对路径链接、非法条目处理和测试要求。

## Phase 1：定义新 wire 数据模型并接入 Borsh codec

### 输入

- `app-server-protocol` 现有命令、事件、错误、能力和数据模型。
- `PathHandle` 现有校验规则。
- 跨平台路径策略文档。
- 当前 wire payload 中的 `PathBuf`、`ConfigLoadResponse { json }`、`ConfigSaveRequest { json }`。
- 当前 `AppConfig` / `SlicerConfig` / `DisplayUnit` 配置模型；磁盘 JSON 格式只作为 host 持久化边界输入。
- 当前 `ExportRun`、`PreviewRequest`、`SlicerList` 中的 OpenSCAD / slicer host path 使用场景。
- 当前 workspace read / write / export path resolver 和 symlink 行为。

### 操作步骤

1. 基于 docs.rs / 官方文档确认并固定 Unicode 校验依赖与 Unicode 数据版本；禁止手写不完整 Unicode 表来实现 RGI emoji、extended grapheme cluster、Unicode category 或 case folding。
2. 为 protocol 类型引入 Borsh derive，并给所有 wire enum 分配稳定 discriminant。
3. 定义 wire frame 外层结构，包含 magic、wire version 和 envelope；错误 magic、错误版本、旧 JSON text frame 必须在 command dispatch 前被拒绝。
4. 在 protocol 数据模型中定义 typed config DTO。`ConfigLoadResponse` / `ConfigSaveRequest` 不再使用 `json: String`；host 只在读写磁盘 `config.json` 时做 typed config 与 JSON 文件格式互转。
5. 在 protocol 数据模型中定义 `HostLocalPath`，用于 OpenSCAD path、slicer path、recent workspace 等 server 机器路径；它是 UTF-8 host-local 字符串，不使用 portable workspace path 规则，并由 server 所在平台校验。
6. 在 protocol 数据模型中定义 export target。`ExportRunRequest.output_path` / `ExportRunResponse.output_path` 不再使用 `PathBuf` 或裸字符串，改为 workspace 内 portable path；默认导出文件名解析为当前源文件所在目录下的 portable output path。
7. 在 protocol 数据模型中把 workspace path 统一为 portable segment 模型，执行 Unicode normalization、CJK / emoji allowlist、JS project punctuation allowlist、problematic symbol denylist、reserved name、长度、深度和大小写冲突规则。
8. 调整 workspace entry 表达，确保非法文件名和大小写冲突能作为不可操作条目返回给 client。
9. 增加相对路径链接解析能力，把 `./`、`../` 和 `/` 分隔的输入解析为 canonical portable path，并拒绝越过 workspace root 的链接。本轮不新增 Markdown 文件导航或 OpenSCAD include / use 解析能力。
10. 为 read 与 write/export 定义不同 workspace path resolver；写入和导出必须 canonicalize 已存在父目录并确认仍在 workspace root 内，拒绝 symlink escape。
11. 增加统一 wire payload 校验入口，例如 `validate_wire_payload` 或有限浮点 wrapper。preview mesh producer、typed config DTO、wasm JS 入参转换、Borsh decode 后都必须调用；NaN / Inf 返回可诊断错误。
12. 新增可诊断错误分类，至少覆盖 invalid numeric value、invalid path、invalid host-local path、unsupported wire version。
13. 确保 Borsh decode 后仍调用 path / numeric / envelope 校验，非法值不能被构造出来。
14. 用 Borsh roundtrip 与 golden bytes 测试替换旧的 protocol JSON roundtrip 断言。

### 前序目标保护

保护 Phase 0 已确认的边界：不能因为让类型易于 derive 而把 `PathBuf`、JSON 字符串、JS Borsh schema 或 TS envelope 构造权重新放回 protocol / web 壳层。

### 验收标准

- `app-server-protocol` 的核心 envelope、命令、响应、推送和错误都能 Borsh roundtrip。
- NaN、`+Inf`、`-Inf` 不能进入或通过 wire payload；相关失败路径返回可诊断 protocol / client error。
- `ConfigLoad` / `ConfigSave` wire payload 不再出现 `json: String`；磁盘 `config.json` 格式保持不变。
- `ExportRun` 输出目标只能是 workspace 内 portable path；导出越界、非法文件名、symlink escape 都被拒绝，成功响应返回 portable output path。
- OpenSCAD path、slicer path、recent workspace 等 host-local 路径使用独立 `HostLocalPath` wire 类型，并覆盖 Windows path、macOS app bundle path 和 Unix path 用例。
- CJK、常见文字、Unicode RGI emoji、dotfile、scoped package、Next.js dynamic route、SvelteKit route、Remix route、非法 path segment、Windows reserved name、非法全角符号、非法零宽字符、末尾句点、超长路径、相对链接越界、大小写冲突在协议测试中被覆盖。
- Unicode validator 的依赖和 Unicode 数据版本被记录在 protocol crate 或计划结果中，相关 golden tests 锁定行为。
- workspace 写路径和导出路径覆盖 symlink escape 回归测试。
- `PathHandle` 的非法 segment 用例在 Borsh decode 路径仍被拒绝。
- protocol wire payload 中不再直接暴露 `PathBuf`。
- protocol envelope / command payload 不再使用 JSON 字符串承载结构化协议数据。
- `app-server-protocol` 的 wire codec 不再依赖 `serde_json`。
- `cargo test -p app-server-protocol` 通过。

## Phase 2：新增 protocol wasm crate 与 TypeScript package

### 输入

- Phase 1 后的 Borsh-ready `app-server-protocol`。
- 当前 `studio-web-wasm` 的 wasm-bindgen 构建约定和 generated 产物快照策略。
- bun-only 工具链约束。
- `@budn` npm scope 约束。
- Phase 1 的 typed config DTO、`HostLocalPath`、export target、portable path、relative link helper 和 finite float validation API。

### 操作步骤

1. 新增 `app-server-protocol-wasm` crate，作为 protocol codec 与 path validator 的 wasm-bindgen 暴露层。
2. 新增 `packages/app-server-protocol`，作为 `@budn/app-server-protocol` import 入口。
3. 增加 build / smoke 脚本，把 protocol wasm 产物生成到对应 package 的 `generated/` 目录。
4. TypeScript package 导出 request / response / event / error / typed config / host-local path / export target / portable path 等结构类型，以及二进制 encode / decode wrapper。
5. TypeScript 类型必须由 Rust schema 生成，或通过 generated smoke / golden tests 锁定；不得手写会影响序列化的 schema。
6. TypeScript package 暴露 path validation、relative link resolution、host-local path validation 和 finite numeric validation helper；helper 只调用 wasm，不复制 Rust 校验规则。
7. 增加 package import smoke，确认 `packages/studio-web` 可以从 protocol package import 类型与 codec。
8. 增加 generated 产物一致性检查，避免 wasm-bindgen 输出变化未提交。

### 前序目标保护

保护 Phase 1 的单一 schema：protocol wasm 包只能调用 `app-server-protocol`，不得复制一套 JS Borsh schema，也不得在 package 中写业务状态机或路径规则。

### 验收标准

- `cargo check -p app-server-protocol-wasm --target wasm32-unknown-unknown` 通过。
- protocol wasm package generated 产物可由 `bun` 脚本再生，且与仓库内快照一致。
- `bun` 侧 typecheck 能 import `@budn/app-server-protocol`。
- protocol package 的 decode smoke 可以把 Borsh server frame 转成结构化 JS 值。
- protocol package 的 path smoke 能接受 CJK / emoji segment、常见 JS 项目文件名、解析合法相对链接、拒绝非法 segment 和越界相对链接，并返回与 Rust 测试一致的错误分类。
- protocol package 的 config / export / host-local path smoke 能构造 typed request params，但不能构造完整 envelope 或复制序列化 schema。

## Phase 3：切换 WebSocket wire 到 Borsh binary frame

### 输入

- Phase 1 的 Rust codec。
- Phase 2 的 protocol wasm / TypeScript package。
- `app-server-transport` 当前 text helper。
- `app-server-host` 当前 WebSocket host。
- `app-server-transport::WebSocketClientTransport` 当前 JSON text client。

### 操作步骤

1. 在 Rust transport 层把 `websocket_wire.rs` text helper 替换为 Borsh binary helper。
2. 迁移或删除 `WebSocketClientTransport` 的 JSON text client 路径；不得保留可用的 JSON WebSocket client。
3. WebSocket host 只接受 binary frame；收到 text frame 时拒绝或关闭连接，并记录清晰错误。
4. WebSocket host 发送 `Message::Binary`，不再发送 text frame。
5. WebSocket smoke 测试改为发送 / 接收 binary frame，并覆盖错误 magic / unsupported wire version / text frame 拒绝。
6. 保持 `ClientTransport` trait、`InMemoryTransport`、`MpscTransportAdapter` 的 typed message 行为不变。

### 前序目标保护

保护 Phase 1 的 typed protocol 数据契约和 Phase 2 的 protocol package；不得为了快速通过 WebSocket smoke 恢复 JSON fallback。保护本轮用户边界：不得修改 `studio-app` 的 mpsc transport 语义。

### 验收标准

- `cargo test -p app-server-transport` 通过。
- `cargo test -p app-server-host --test websocket_smoke_roundtrip` 通过。
- `app-server-host` WebSocket 路径不再出现 JSON text frame decode / encode。
- `app-server-transport` 不再导出 text JSON helper，`WebSocketClientTransport` 不再包含 JSON text encode / decode。

## Phase 4：迁移 studio-common / studio-web-wasm / studio-web 到新协议

### 输入

- Phase 3 的 binary WebSocket host。
- `studio-common::ManagedClient` 当前 `Vec<u8>` 队列。
- `crates/studio-web-wasm` 当前 bridge API。
- `packages/studio-web` 当前 WebSocket transport、request resolver、配置页、Playwright harness。
- Phase 2 的 `@budn/app-server-protocol` package。
- Phase 1 的 typed config DTO、invalid workspace entry、export target 和 `HostLocalPath` 数据契约。

### 操作步骤

1. `ManagedClient` 出站 / 入站 envelope bytes 改用 `app-server-protocol` Borsh codec。
2. `studio-web-wasm` 的 browser wasm smoke 改用 Borsh frame 构造 inbound，断言 outbound 可由 Borsh decode。
3. `packages/studio-web` 的 WebSocket transport 移除 string fallback，只处理 `ArrayBuffer` / `Uint8Array`。
4. `packages/studio-web` 的 test harness 用 `@budn/app-server-protocol` decode outgoing binary frame；不再 `TextDecoder + JSON.parse`。
5. 配置页改用 typed config DTO；不得再通过 `ConfigLoadResponse.json`、`ConfigSaveRequest { json }`、`JSON.parse` / `JSON.stringify` 作为 wire payload 编码方案。
6. Web UI 需要新建、重命名、导出目标路径时，只调用 protocol wasm path helper 做预校验；最终校验仍在 server。
7. 导出面板把默认文件名解析为当前源文件所在目录下的 portable export target；不得继续向 protocol 发送裸相对字符串。
8. 当 UI 需要把用户输入的相对链接转换成 protocol file request 时，必须通过 protocol wasm helper 解析为 canonical portable path。本轮不新增 Markdown 文件导航，不解析 OpenSCAD include / use，OpenSCAD include / use 仍由 OpenSCAD CLI 处理。
9. 不可操作 workspace entry 可展示但不可打开、预览、watch 或导出；Web 和 `studio-common` snapshot 不得把 invalid entry 当成合法 path。
10. 配置文件、预设文件和用户文件内容相关 JSON 处理可以保留，但不得作为 protocol envelope 或 command payload 的编码方案。

### 前序目标保护

保护 Phase 2 的 package 边界：TS package 只提供类型和 codec，不复制 `ManagedClient` 状态。保护 Phase 3 的 binary-only wire：浏览器 transport 不得接受 text JSON 作为兼容路径。保护 Phase 1 的数据契约边界：Web 端不得手写路径规则、相对链接解析规则、finite numeric 规则或完整 envelope schema。

### 验收标准

- `cargo test -p studio-common --tests` 通过。
- `cargo test -p studio-web-wasm --tests` 通过。
- `wasm-pack test --headless --chrome crates/studio-web-wasm --test wasm_bridge_smoke` 通过。
- `bun run --cwd packages/studio-web typecheck` 通过。
- `bun run --cwd packages/studio-web test:unit` 通过。
- `bun run web:smoke -- --case browser_smoke` 与 watch / preview / export 相关 smoke 通过。
- `packages/studio-web/src/transport` 不再包含 string WebSocket frame fallback。
- 配置页仍能加载 / 保存配置，但 WebSocket / wasm client API 不再传 `json: String`。
- export smoke 断言 outgoing frame 中 output target 是 portable path，不是裸字符串或 host path。
- invalid workspace entry 显示 smoke 覆盖“可展示但不可打开”行为。

## Phase 5：迁移 npm scope 与删除旧 JSON wire 残留

### 输入

- Phase 1-4 已迁移后的代码。
- 当前 npm package、workspace dependency、tsconfig alias、Vitest alias、README 和注释。
- 当前 Rust / wasm / web 回归矩阵。

### 操作步骤

1. 把已有 npm package scope 从 `@scad-studio` 改为 `@budn`。
2. 删除旧 JSON wire helper、旧 JSON roundtrip 测试、旧 text frame 测试和相关 dependency。
3. 用 grep 检查协议 / transport / host / managed client / wasm bridge smoke 中是否仍有 JSON envelope 处理。
4. 对允许保留的 JSON 做白名单记录，避免把配置文件 / 预设文件 / 工具链文件误判为 wire 残留。
5. 跑完整 Rust + wasm + web 回归矩阵。

### 前序目标保护

保护 Phase 0-4 的所有目标：最终代码不得因为测试便利恢复 JSON frame；不得把 protocol 状态机复制到 TS；不得改变 `studio-app` 的 mpsc transport；不得绕过 portable path 数据契约；不得破坏现有 Web 功能。

### 验收标准

- `cargo check --workspace` 通过。
- `cargo test --workspace --tests` 通过。
- `bun run check:wasm-bindgen` 通过。
- protocol wasm package generated 一致性 smoke 通过。
- `bun run --cwd packages/studio-web typecheck` 通过。
- `bun run --cwd packages/studio-web test:unit` 通过。
- `bun run web:build` 通过。
- `bun run web:smoke` 通过。
- grep 验收：
  - WebSocket wire 路径无 `serde_json::to_vec`、`serde_json::from_slice`、`serde_json::to_string`、`serde_json::from_str`。
  - WebSocket host 无 JSON text frame decode / encode。
  - browser transport 无 `TextEncoder` string fallback。
  - protocol payload 无 `PathBuf` 直接暴露。
  - protocol payload 无 `ConfigLoadResponse { json }`、`ConfigSaveRequest { json }` 或等价 JSON string command payload。
  - protocol payload 无 `ExportRun.output_path: PathBuf`、裸 output string 或 host path export target。
  - protocol payload 无 JSON 字符串承载结构化协议数据。
  - packages 中无 `@scad-studio/*` package name 或 import。
  - 允许保留的 JSON 命中仅位于配置文件持久化、预设文件解析、工具链元数据、测试 fixture 或 wasm-bindgen generated 调试代码。

## 主要风险与处理方式

1. **枚举顺序改变导致 wire 不兼容**：所有 wire enum 必须显式 discriminant，并用 golden bytes 测试锁定。
2. **路径策略过严影响现有 workspace**：策略已允许 CJK、常见 emoji 和 JS 项目常见文件名，但仍会拒绝其他常见问题符号。server 需要返回不可操作条目和明确错误原因，而不是让整个目录列表失败；UI 需要提示用户重命名。
3. **`PathBuf` 被误认为“可序列化所以可入协议”**：计划明确区分“可序列化”和“适合作为跨端 contract”。协议只接受 portable path 或显式 host-local path。
4. **`PathHandle` 校验被 Borsh derive 绕过**：必须手动 decode 或 raw decode 后调用构造器。
5. **配置 / 预设文件格式与 wire protocol 范围混淆**：文件格式不迁移；但 `ConfigLoad` / `ConfigSave` wire payload 必须 typed，不能用 JSON string 表达结构化协议数据。
6. **TS 类型与 Rust 类型不一致**：protocol TS package 必须有 typecheck、import smoke 和 generated 一致性检查；TS 不复制 Borsh schema。
7. **二进制帧调试能力下降**：通过 `@budn/app-server-protocol` decode helper 更新测试 recorder 和开发诊断工具。
8. **浮点 NaN / Inf 导致非法 wire 数据**：mesh、camera、layout 配置等浮点字段需要统一 finite 校验；不能只依赖 Borsh 对 NaN 的失败路径，Inf 也必须拒绝。
9. **Unicode 版本变化造成校验结果变化**：validator 必须固定 Unicode 数据版本，并用 golden tests 覆盖 CJK、emoji sequence、JS route filenames、非法 format control 与大小写冲突。
10. **写路径 symlink escape**：portable segment 不等于文件系统安全。写文件和导出必须使用专门 resolver，校验已存在父目录 canonical path 仍在 workspace root 内。
11. **相对路径链接扩大范围**：本轮只提供 protocol helper 给已有 UI / 请求边界使用，不新增 Markdown 文件导航或 OpenSCAD include / use parser。

## 执行说明

- 本计划讨论确认前不进入实现。
- 实现阶段每个 Phase 完成后必须更新 `plan-00-result.md`。
- 每个 Phase 执行后必须使用独立 subagent review，并在 review 后完成回归验证再进入下一 Phase。
- 若执行中发现无法本轮解决但会影响后续判断的问题，必须同步更新 `docs/known_issues.md`。
