# Async 后端 / Rig Agent / 模型原生联网搜索执行结果

## 当前状态

- 计划已创建。
- 独立 reviewer 已完成只读审查，未发现阻塞项。
- reviewer 第一轮提出的高风险与普通问题已修订进计划：补齐同步 I/O / 子进程 / 线程模块清单，调整 Phase 2 与 Phase 3 的 Agent worker 顺序，补齐旧路径搜索关键词，补齐 protocol wasm 与 Rust crate 验证。
- reviewer 第二轮提出的范围冲突已修订进计划：旧 Agent / LLM 生产入口按全仓库搜索，后端同步 I/O / 子进程 / 线程关键词只检查 app-server-core 与 app-server-host 生产源码；同时补充 Web smoke 与 browser smoke 验证。
- reviewer 第三轮未发现阻塞项或高风险问题；唯一普通问题已修订：Phase 3 host 侧 Agent 测试输入从不存在的通配路径改为实际测试文件。
- 用户追加要求 Rust 桌面 app 可以完全删除，并要求把删除桌面 app 调整到最前面；计划已重写为 7 个 Phase，Phase 1 先删除 Rust 桌面端与桌面专属生产路径。
- reviewer 第四轮指出 protocol / 生成包桌面 platform 残留风险与 Web smoke `scad-viewer` 标签歧义；已修订 Phase 1 与 Phase 7，纳入 `ClientPlatform::Desktop`、`"desktop"` product platform 和 Web smoke 标签重命名检查。
- reviewer 第五轮未发现阻塞项或高风险问题；普通问题已修订：补充 host 侧 in-process / mpsc 测试输入，补充 `Desktop =` / `ClientPlatform` 搜索关键词，补充删除桌面 UI 后的 workspace 依赖清理。
- reviewer 第六轮指出 `child_terminator`、GUI shutdown example、旧 Chat Completions 精确关键词、跨平台路径文档与 `python3` 测试调用遗漏；已修订 Phase 1、Phase 3 和 Phase 7 输入、操作步骤与最终搜索关键词。
- reviewer 第七轮指出同步文件系统关键词和旧 LLM 配置 / UI 提示检查不足；已扩展 Phase 4 的配置替换要求，并扩展 Phase 7 的 `fs::`、`OpenOptions`、`File::open`、`BUDN_LLM_BASE_URL`、`base_url`、OpenAI-compatible 等搜索关键词。
- reviewer 第八轮指出同步 mpsc / `recv_timeout` 关键词、Web UI 提示输入和历史 fallback 文档关键词遗漏；已补充 Phase 4 输入与 Phase 7 搜索关键词。
- 用户指出同步路径搜索与 Tokio 同名类型存在歧义；已修订 Phase 7，明确候选项必须结合 import 与类型来源判断，`tokio::fs`、`tokio::process::Command`、`tokio::sync::mpsc`、`tokio::task::JoinHandle` 是允许的 async 目标。
- reviewer 第九轮指出 `thread::Builder` / `std::thread::JoinHandle`、不存在的 host 文件输入、Phase 4 Web 验证和 Phase 1 smoke 验证问题；已补充阻塞式线程关键词，移除不存在文件输入，并补充 Web typecheck / unit 与 smoke 验收。
- reviewer 第十轮复审未发现阻塞项、高风险或普通问题。
- Phase 1 已完成实现、验证和独立复审；第四轮 Phase 1 复审未发现阻塞项或高风险问题，准备提交 Phase 1。

## Phase 记录

### Phase 1 — 删除 Rust 桌面端与桌面专属生产路径

- 状态：已完成，准备提交。
- 变更摘要：
  - 删除 `crates/studio-app`、`crates/scad-ui`、`crates/scad-viewer`，并从 workspace 成员与锁文件中移除相关依赖。
  - 删除 `app-server-host` 的 in-process / mpsc 生产入口、导出、runtime、GUI shutdown example 和对应 host 测试。
  - 删除 protocol / TypeScript bridge 中的桌面 product platform，保留 `ClientPlatform::Web = 1` 与 `Other = 2` 的 wire discriminant。
  - 将 Web smoke 中 `scad_viewer` / `@scad-viewer` 重命名为 `scad_preview` / `@scad-preview`。
  - 修正 watcher 事件，使服务端向 Web 发送实际变更路径；Web 端按源文件与设置文件分别刷新，避免设置文件变化触发无关预览重跑。
  - 调整 Playwright smoke harness，使用隔离 host 环境和临时 workspace，避免测试污染共享 fixture。
  - 更新架构、getting started、Web 平台限制、跨平台路径策略、设计系统与已知问题文档，明确当前生产 GUI 端为 Web，旧 Rust GUI / viewer 相关问题转为历史记录或 Web 当前能力问题。
- 验证结果：
  - `bun run protocol:build && bun run protocol:check-generated` 通过。
  - `bun run --cwd packages/studio-web typecheck` 通过。
  - `cargo test --workspace` 通过；仍有既有 dead_code warning，未在本 Phase 扩大处理范围。
  - `bun run --cwd packages/studio-web test:unit` 通过；仍有既有 React `act(...)` warning，未在本 Phase 扩大处理范围。
  - `bun run web:smoke` 通过；构建阶段仍有既有 Vite 大 chunk warning，未在本 Phase 扩大处理范围。
  - `bun run web:smoke:browser` 通过，75 个 Playwright 用例通过。
- 独立复审记录：
  - 第一轮 Phase 1 复审发现两个代码 / 文档阻塞项：`ClientPlatform::Web` discriminant 不得改变，`docs/known_issues.md` 不得伪造归档路径或继续把旧桌面 GUI 回归作为当前风险。已修正。
  - 第二轮 Phase 1 复审发现三个归档阻塞项：`docs/known_issues.md` 仍有不可验证的外部 worktree 路径，部分旧桌面 / viewer 差距仍作为当前目标描述，`plan-00-result.md` 仍写 Phase 1 未执行。已修正。
  - 第三轮 Phase 1 复审发现 `docs/known_issues.md` 的 Markdown 历史记录、`docs/architecture.md` 的硬约束回顾和 `docs/feature-roadmap.md` 的平台菜单说明仍有旧桌面当前目标口吻。已改为 Web / 共享模型 / 历史记录口径。
  - 第四轮 Phase 1 复审未发现阻塞项或高风险问题；普通问题指出 `docs/architecture.md` 对 `scad-scene` 的描述比当前 crate 状态更激进。已改为 Web 生产路径只消费其 mesh / STL / 3MF 纯数据能力，并记录旧 renderer / pipeline / gizmo / 窗口模块仍待后续整理。
- 遗留问题：
  - Phase 1 未处理后续 async service、Rig-only Agent 和模型原生联网搜索目标；这些仍按 Phase 2 到 Phase 6 自动推进。

### Phase 2 — 建立 WebSocket-only async 后端服务边界

- 状态：未执行。

### Phase 3 — 核心 I/O、ChatStore、预览与子进程路径 async 化

- 状态：未执行。

### Phase 4 — Rig 成为唯一生产 Agent 执行引擎

- 状态：未执行。

### Phase 5 — 接入模型原生联网搜索

- 状态：未执行。

### Phase 6 — Protocol、Web 端侧与配置接入

- 状态：未执行。

### Phase 7 — 文档、已知问题、最终验证与独立 review

- 状态：未执行。
