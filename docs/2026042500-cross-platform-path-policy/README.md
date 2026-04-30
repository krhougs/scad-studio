# 跨平台路径与文件名策略

## 背景

`studio-web` 与 app server 的 wire protocol 将从 JSON 迁移到 Borsh 二进制。路径一旦进入二进制协议，就不能继续依赖 Rust `PathBuf` 或浏览器侧字符串的隐式行为，否则 Windows、macOS、Linux、WebAssembly 与未来云端沙盒会在同一份协议数据上产生不同解释。

本策略把路径定义为协议层自有数据模型：workspace 内路径只允许由一组可移植 path segment 组成；host-local 路径必须用单独类型表达，不能混入 workspace path。

## 适用范围

本策略适用于：

- wire protocol 中客户端提交或 server 返回的 workspace 相对路径。
- `PathHandle`、watch 事件、文件读写、预览源文件、导出目标文件等需要跨端引用 workspace 文件的字段。
- protocol wasm 暴露给 JS 的路径构造、校验、序列化和反序列化能力。

本策略不适用于：

- `config.json`、`*.scad.json`、`package.json`、`tsconfig.json` 等文件格式本身。
- server 本机上由用户配置的 OpenSCAD、切片器等可执行文件路径。此类路径在当前配置、预览、导出 wire payload 中需要表达时，必须使用 `HostLocalPath` 这类独立类型，并在 server 所在平台校验。

## 官方约束依据

- Windows 对文件名限制最严格：保留 `< > : " / \ | ? *`、NUL、控制字符、`CON` / `PRN` / `AUX` / `NUL` / `COM1..COM9` / `LPT1..LPT9` 等设备名，并要求名称不能以空格或句点结尾。参考 Microsoft 文档：[Naming Files, Paths, and Namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file)。
- POSIX 定义了 portable filename character set：英文字母、数字、句点、下划线、连字符，并提醒不要把连字符放在文件名开头。参考 Open Group 文档：[Definitions, 3.264-3.265](https://pubs.opengroup.org/onlinepubs/9799919799/basedefs/V1_chap03.html)。
- Apple 文档说明 macOS 代码读取的是真实文件名，Finder 可能显示 display name；macOS 也存在大小写敏感与大小写不敏感文件系统差异。参考 Apple 文档：[File System Basics](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html) 与 [File System Guidelines](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPFileSystem/Articles/FileSystemGuidelines.html)。
- Unicode 对 emoji 与用户感知字符有单独规范。常见 emoji 采用 Unicode RGI emoji sequence；长度与首尾规则按 extended grapheme cluster 计算，避免把组合 emoji 拆开。参考 Unicode 文档：[UTS #51 Unicode Emoji](https://www.unicode.org/reports/tr51/) 与 [UAX #29 Unicode Text Segmentation](https://www.unicode.org/reports/tr29/)。

结论：budn 的协议层应采用比任一单独平台更严格的交集规则。这样会拒绝一部分某个平台本来能创建的文件名，但能保证同一个 workspace 在三端行为一致。

## 协议模型

推荐协议类型：

```text
PortablePath {
  workspace_id: WorkspaceId,
  segments: Vec<PortablePathSegment>
}

PortablePathSegment(String)
```

约束：

- `segments = []` 只表示 workspace root。
- 非 root 路径必须至少包含一个 segment。
- 协议内部不传平台分隔符；展示路径统一用 `/` join，仅用于 UI 与日志。
- Borsh decode 后必须重新执行同一套校验，不能直接信任二进制输入。
- JS 侧不实现校验规则，只调用 protocol wasm 暴露的构造与校验函数。

## Segment 规则

每个 path segment 必须满足以下规则：

1. 输入先规范化为 NFC，协议只保存规范化后的字符串。
2. 长度为 1 到 80 个 extended grapheme cluster，且 UTF-8 编码不超过 180 字节。
3. 整个 workspace 相对展示路径不超过 240 个 UTF-8 字节。
4. 路径深度不超过 32 个 segment。
5. 允许 Unicode 字母、组合标记、数字；这覆盖 CJK、拉丁扩展、假名、韩文等常见文字。
6. 允许 Unicode RGI emoji sequence；肤色、ZWJ 组合、旗帜等必须作为一个 emoji 字素簇处理。
7. 允许 JS 项目常见 ASCII 标点：内部空格、`.`、`_`、`-`、`@`、`+`、`$`、`[`、`]`、`(`、`)`、`=`。
8. 允许 dotfile：segment 可以用单个 `.` 开头，例如 `.gitignore`、`.env.local`、`.prettierrc`；但不能以 `..` 开头。
9. 第一个字素簇不能是空格、`-`、`]`、`)`、`=` 或独立组合标记。
10. 最后一个字素簇不能是空格或 `.`。
11. segment 不能等于 `.` 或 `..`。
12. segment 不能包含 `/`、`\`、NUL、控制字符、DEL、Windows 保留字符。
13. 拒绝 Unicode format control、bidi control、private-use、noncharacter。`U+200D`、`U+FE0F` 等只允许出现在合法 RGI emoji sequence 内。
14. 拒绝除 JS 常见 ASCII 标点之外的标点、符号和全角符号；emoji 是唯一符号例外。
15. 按 ASCII 大小写不敏感规则检查 Windows 设备名；`CON`、`con.scad`、`NUL.tar.gz`、`COM1`、`LPT9` 都必须拒绝。
16. 同一目录下按 Unicode Default Case Folding 与 NFC 后不能出现重名；`Cube.scad` 与 `cube.scad` 不能同时作为有效路径暴露。

允许路径示例：

- `模型.scad`
- `装配 01.scad`
- `部件_左侧-🔥.scad`
- `テスト/ケース🧪.scad`
- `零件/支架👍🏽.scad`
- `.gitignore`
- `.env.local`
- `@types/node/index.d.ts`
- `app/[id]/page.tsx`
- `app/[[...slug]]/page.tsx`
- `app/(marketing)/page.tsx`
- `app/@modal/(.)photo/page.tsx`
- `src/routes/+page.svelte`
- `src/routes/[page=fruit]/+page.svelte`
- `routes/concerts.$city.tsx`

拒绝 segment 示例：

- `CON.scad`
- `a/b.scad`，原因是 `/` 只能作为相对路径链接的分隔符，不能出现在单个 segment 中。
- `a\b.scad`
- `模型?.scad`
- `支架：左.scad`
- ` name.scad`
- `name .scad`
- `foo#bar.scad`
- `foo%20bar.scad`
- `foo&bar.scad`
- `foo!bar.scad`
- `..hidden`
- `a‍b.scad`，其中零宽连接符不是合法 emoji sequence 的一部分。

## 相对路径链接

canonical protocol path 仍然只保存 `Vec<PortablePathSegment>`，不保存 `.`、`..` 或分隔符。当 UI 或协议命令需要把用户输入的相对链接转换成 workspace file request 时，可以使用本节解析规则。

本策略不新增 Markdown 文件导航，也不解析 OpenSCAD include / use。Markdown 链接当前仍按 Web 端安全策略处理；OpenSCAD include / use 仍由 OpenSCAD CLI 解释。

相对路径链接的解析规则：

1. 输入使用 `/` 作为路径分隔符；`\` 不作为跨平台分隔符接受。
2. 允许 `./`、`../`、尾部 `/` 和多级相对路径。
3. Markdown URL path 可先按 UTF-8 做一次 percent decode；解码失败时返回路径错误。`%20` 这类转义只属于链接输入，不属于 canonical filename。
4. `.` component 被丢弃，`..` component 按 base path 向上解析。
5. 解析后的路径深度仍不能超过 32 个 segment。
6. Markdown fragment 可以保留为 link metadata，例如 `README.md#标题`；fragment 不是文件名的一部分。
7. workspace 文件链接不接受 query string；包含 `?` 的相对链接必须作为外部 URL 或普通文本处理，不能转换成 `PathHandle`。
8. 解析结果必须仍在当前 workspace root 内；越过 root、绝对路径、Windows drive path、UNC path 和 URL scheme 都不能转换成 `PathHandle`。
9. 每个最终 segment 必须通过上一节 segment 校验。
10. 解析成功后输出 canonical `PortablePath`；后续 wire protocol 不再携带原始 link 字符串。

## Unicode 策略

当前策略允许 CJK、常见文字和 Unicode RGI emoji，但不接受任意 UTF-8。原因是：

- Windows、macOS 和 Linux 对大小写、Unicode normalization、显示名和真实名的处理不同。
- Borsh 可以序列化 `String`，但这只保证字节可读，不保证路径在所有平台上有相同文件系统语义。
- 全量允许 Unicode 标点和符号会引入分隔符混淆、Markdown 链接混淆、shell glob、bidi spoofing 和不可见字符问题。
- JS 项目兼容性要求必须覆盖 dotfile、scoped package、SvelteKit / Next.js / Remix 等 file-based routing 命名。因此 `@`、`+`、`$`、`[`、`]`、`(`、`)`、`=` 被列入允许集合，但 `#`、`%`、`&`、`!`、引号、反引号、分号、逗号、花括号等仍拒绝。

实现上需要把 Unicode 数据版本固定在 validator 中，并用测试样例锁定 CJK、组合字符、emoji sequence、非法 format control、非法全角标点和大小写冲突行为。未来 Unicode 版本升级必须作为协议兼容性变更处理。

## `PathBuf` 与 `&str[]` 的边界

`PathBuf` 可以被转换成类似 `&str[]` 的 segment 表示后再序列化；不能直接把 `PathBuf` 作为 wire contract 的原因不是“不能序列化”，而是：

- `PathBuf` 的语义属于所在操作系统，Windows 与 Unix 的分隔符、前缀、绝对路径和设备名规则不同。
- `PathBuf` 内部基于 `OsString`，不能承诺所有平台上的路径都天然是 UTF-8。
- wire protocol 需要声明哪些路径值合法，而不是把校验推迟到某个平台的文件系统 API。

因此正确做法是：协议层使用 `Vec<PortablePathSegment>`，host 边界再把它转换成当前平台的 `PathBuf`。

## 现有不兼容文件的处理

server 扫描 workspace 时不能因为一个非法文件名让整个目录列表失败，也不能静默隐藏。推荐协议暴露“不可操作条目”：

```text
WorkspaceEntry {
  name: String,
  kind: File | Directory | Other,
  handle: Option<PortablePath>,
  path_error: Option<PathValidationError>
}
```

规则：

- 合法条目带 `handle`，client 可以读取、预览、watch 或导出。
- 非法条目不带 `handle`，client 只能展示名称和错误原因，不能发起文件操作。
- 同目录大小写冲突时，冲突条目全部标记为非法，避免不同平台选择不同文件。
- 新建、重命名、导出目标在写入前必须先通过同一套校验。

## 校验归属

- `app-server-protocol`：持有纯校验函数、错误类型、Borsh decode 后校验。
- `app-server-protocol-wasm`：把 Rust 校验暴露给 JS，供 UI 做即时提示。
- `app-server-core`：从真实文件系统读取目录后，按协议策略生成合法 handle 或非法条目。
- `app-server-core`：写文件和导出目标必须使用写路径解析策略，canonicalize 已存在父目录并确认仍在 workspace root 内，拒绝 symlink escape。
- `app-server-host`：收到非法路径 payload 时返回 `InvalidPathHandle`，不能尝试修正。
- `packages/studio-web`：不复制校验规则，不手工构造完整 envelope。

## 测试要求

- protocol 单元测试覆盖合法 segment、非法字符、reserved name、首字符、末尾句点、长度、深度、总路径长度。
- protocol 单元测试覆盖 CJK、内部空格、RGI emoji、ZWJ emoji、dotfile、scoped package、Next.js dynamic route、SvelteKit route、Remix route、非法零宽字符、非法全角符号、percent-decoded 相对链接和越过 root 的相对链接。
- Borsh roundtrip 测试确认非法路径不能通过 decode 绕过校验。
- workspace core 测试覆盖目录中存在非法名称、大小写冲突和合法文件混合的场景。
- workspace core 测试覆盖写入和导出目标的 symlink escape。
- wasm smoke 测试确认 JS 侧只能通过 wasm 获得合法路径 bytes 或错误。
- Web 测试 harness 只能通过 `@budn/app-server-protocol` decode binary frame，不再 `TextDecoder + JSON.parse`。
