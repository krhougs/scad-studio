# Phase 0 契约 · 状态归属表

本文件固定每类状态的**唯一归属 crate / package**，避免 Phase 2 / Phase 3 实现时把共享 client 状态机重复塞回 web 专属 crate。归属由上到下从“跨端共享”到“web 专属壳”递进。

## 1. 归属矩阵

| 状态种类 | 归属 crate / package | 备注 |
|----------|-----------------------|------|
| `AppServerTransportPort` trait 定义 | `studio-common` | 当前已存在 |
| `AppServerClient<T>` 结构体 | `studio-common` | 当前已存在 |
| request id 分配 (`RequestId` 生成) | `studio-common` | 由 `AppServerClient::allocate_request_id` 提供 |
| pending request registry（id → 期望响应类型 / 截止时间） | `studio-common`（Phase 2 新增 `ManagedClient`） | JS 与 wasm 均不重复维护 |
| watch 订阅 registry（用于 reconnect 重订阅） | `studio-common`（Phase 2 新增 `ManagedClient`） | 同上 |
| 超时计时（基于 `tick(now_ms)`） | `studio-common`（Phase 2 新增 `ManagedClient`） | 不使用 `setTimeout` |
| watch 事件节流窗口 | `studio-common`（Phase 2 新增 `ManagedClient`） | TS 不重复节流 |
| `WorkspaceSession` / 当前 workspace / 当前目录文件列表 | `studio-common` | 跨端共享业务状态 |
| 预览任务状态 / 当前激活预览目标 / 预览错误 | `studio-common` | 跨端共享业务状态 |
| `DirectoryWatchLifecycle` | `studio-common` | 跨端共享业务状态 |
| ClientEvent / ClientError / ClientSnapshot 类型定义 | `studio-common` | 由 wasm 侧 `#[wasm_bindgen]` 再导出 |
| wasm outbound / inbound 字节队列 | `crates/studio-web-wasm` | 仅 wasm transport 适配器可见 |
| mesh decode 结果（中间表示） | `crates/studio-web-wasm` | 使用 `scad-scene` 已有类型 |
| renderer handle / camera state | `crates/studio-web-wasm` | 使用 `scad-scene` 渲染栈 |
| wasm_bindgen 导出的 `ClientHandle` / `RendererHandle` / `MeshHandle` | `crates/studio-web-wasm` | 仅作为 JS 与 wasm 之间的句柄 |
| WebSocket 实例与生命周期 | `packages/studio-web`（TS） | wasm 禁止持有 WebSocket |
| 指数退避与重连调度 | `packages/studio-web`（TS） | wasm 被动等待 `client_mark_transport_closed` |
| React Router 路由状态 | `packages/studio-web`（TS / Zustand） | UI 壳状态 |
| 面板开关 / modal 状态 / 侧边栏折叠 | `packages/studio-web`（TS / Zustand） | UI 壳状态 |
| 选中标签页 / 打开标签列表 | `packages/studio-web`（TS / Zustand） | UI 壳状态 |
| 输入草稿（未提交的表单、编辑器未保存内容） | `packages/studio-web`（TS / Zustand） | UI 壳状态 |
| canvas DOM ref / renderer 绑定 | `packages/studio-web`（TS / React） | UI 壳状态 |
| 设计系统 CSS token / 布局 | `packages/studio-web`（TS / CSS） | 不下沉到 wasm |

## 2. 禁止项（违反即视为边界违规）

- **禁止** 在 `packages/studio-web` 的 Zustand store 中出现如下类型或同名状态机：
  - `WorkspaceSession`
  - `PreviewState`
  - `DirectoryWatchLifecycle`
  - `AppServerClient`
  - 任何 per-request `requestId` registry 或 pending map
- **禁止** 在 `packages/studio-web-wasm`（npm 产物包）中写业务代码；该包**仅**包含：
  - `package.json`
  - `README.md`
  - `generated/`（wasm-bindgen 输出）
  - `src/index.ts`（仅 re-export `generated/`）
- **禁止** 在 `crates/studio-web-wasm` 中持有 WebSocket 实例、`web_sys::WebSocket`、`gloo-net::websocket` 等 transport 具体实现。
- **禁止** 在 `studio-common` 中出现 `egui::Context` 驱动逻辑、页面级 widget 组装、浏览器 API、renderer / GPU 生命周期相关能力。
- **禁止** 在 `studio-app` 与 `studio-web`（TS）之间**分别**维护实现不同但语义相同的状态机；若发现，必须先收敛到 `studio-common`。

## 3. Phase 3 grep 验收语句（参考）

Phase 3 验收时运行以下命令，**人工确认**无协议状态机实现：

```bash
rg "WorkspaceSession|PreviewState|DirectoryWatchLifecycle|AppServerClient|requestId" packages/studio-web/src
```

允许命中场景（不视为违规）：

- 类型 import 自 `@scad-studio/studio-web-wasm`（wasm 产物 TS 签名）。
- 注释中出现这些名字用于“禁止复制到 TS 侧”的说明。
- 测试夹具中 mock wasm API。
- TS 适配层中以 `RequestId` 为 key 的 Promise / callback resolver table（仅用于把 wasm 事件派发回发起方调用点，不保存任何命令载荷 / workspace / preview 业务状态；收到对应事件后立即 `delete`）。该文件**必须**命名为 `packages/studio-web/src/wasm-bridge/request-resolvers.ts`（或同等单一文件），不得在多个 store 中复制。

不允许命中场景：

- TS 文件里出现同名 class / interface / function 定义。
- Zustand store 中定义 `pendingRequests: Map<...>`、`watchLifecycle: ...` 等业务状态字段。

## 4. 未来边界变化规则

- 任何新增“跨端共享状态”默认归 `studio-common`；
- 任何新增“web 壳专属 UI 状态”默认归 `packages/studio-web`；
- 任何新增“wasm 内部句柄 / 渲染资源”默认归 `crates/studio-web-wasm`；
- 若某类状态不属于以上三者（例如 desktop 菜单），归 `studio-app`；
- 不得在 `packages/studio-web-wasm`（npm 产物包）中新增任何业务逻辑。
