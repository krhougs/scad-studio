# Plan prompt 存档

本目录对应任务：**Studio 统一 App Server 架构、多端统一协议、删除独立 Viewer 边界、浏览器完整文件浏览与 fake chatbox UI**。

## 锁定基线

- 锁定提交：`7b232bdbdb751da84adbe6ec7d4fa28175b8cf97`（短哈希 `7b232bd`）
- 本轮所有目标与计划都必须显式保护该提交中**所有已完成的功能、行为、构建结果与测试结果**，不得出现功能回退。

## 用户原始请求（按时间顺序）

1. **回滚当前工作树改动，重新规划**
   - 要求先清理当前未提交改动，再重新定义目标与计划。

2. **沿用最新 Web/WASM plan 的目标，但要升级为更长期的统一架构**
   - 不是一次性 Web 适配，而是长期可扩展的多端架构。

3. **新增要求：Studio 与 Preview 重构**
   - 将所有 I/O 与外部调用能力拆分到 `app server`。
   - `app server` 是未来扩展到本地客户端、云 Agent、沙盒的统一能力层。

4. **新增要求：浏览器中完整实现 fake chatbox 与目录内文件列表**
   - fake chatbox 先做纯前端假 UI。
   - 文件浏览采用“目录树 + 当前目录文件列表”。
   - fake chatbox 当前阶段只保留在 `studio-web`，不进入 `studio-common`。

5. **统一 server / protocol / transport 约束（用户补充拍板）**
   - GUI 与网页必须走**同一套 app server 代码**。
   - GUI 与网页必须走**同一个 server 和同一个 protocol**。
   - GUI 场景下，在同进程中启动新的 Tokio task 作为 app server。
   - GUI 与 app server 的通信走 `tokio::mpsc` channel。
   - 网页或未来其他 client 走 WebSocket 或其他任意 transport。
    - transport 必须写成 trait，便于后续维护。
    - transport 与 protocol 必须彻底分离。
    - protocol 设计必须考虑平台差异和 I/O 差异。

6. **根 crate 与多包拆分约束（用户补充拍板）**
   - 根 crate `scad-studio` 不应包含任何业务代码，只作为 workspace 根使用。
   - 以当前目标看，现有 Studio 代码至少要拆成：`studio-app`、`studio-web`、`studio-common`。
   - 后续继续讨论这些包的能力边界，并以此修订计划。
   - `studio-common` 允许少量 `egui` 基础类型和无平台共享 UI 状态，但不承载页面级布局、widget 组装、`egui::Context` 驱动逻辑或平台事件接线。
   - `studio-common` 管共享状态与行为，`scad-ui` 管可复用组件与呈现；若某段代码主要负责“画出来”，优先归入 `scad-ui`，而不是 `studio-common`。

7. **Viewer 范围拍板**
   - 删除独立 `Viewer` 应用或 crate。
   - `Studio` 已包含 Viewer 的全部功能。本轮要做的是删除独立 Viewer 产品边界与重复接线，而不是把预览“迁回” Studio。
   - 预览状态机放入 `studio-common`；桌面与网页只保留各自的预览 UI 呈现层。

8. **多端会话与界面结构偏好**
   - 单窗口单 workspace session。
   - 左侧目录树，主区域或侧区域展示当前目录文件列表。

9. **当前回合要求**
    - 先把目标与计划正式存档，再继续细化和执行。

10. **2026-04-23 最小 Phase 6 web slice 执行请求**
    - `studio-web` 不再停留在 placeholder，落地最小可运行 wasm 浏览器切片。
    - `app-server-transport` 提供可复用的 WebSocket wire / wasm client transport。
    - `app-server-host` 提供稳定的 repo-local WebSocket host 启动命令。
    - 浏览器 smoke 需真实加载 wasm、连接 WebSocket host、拉取 workspace 列表、请求一次 `preview.request` 并验证成功。
    - 保持范围最小：允许先用简单 fixture 与 `.stl` 预览打通链路；fake chat 状态只留在 `studio-web`；`studio-common` 不引入浏览器 API。

## 执行时注意

- 以锁定提交 `7b232bd` 的代码、测试和已完成功能为基线，不得破坏。
- 若后续实现发现旧 plan 与当前锁定基线不一致，以**锁定基线的真实行为**为准修订计划。
- 每个 Phase 完成后按项目规范更新 `plan-00-result.md`，记录完成情况、变更摘要、遗留问题。
