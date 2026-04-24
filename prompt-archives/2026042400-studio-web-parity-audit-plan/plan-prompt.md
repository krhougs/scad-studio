# Plan prompt 存档

本目录对应任务：**重新审计 `prompt-archives/2026042300-studio-web-feature-parity` 的真实完成度，不采信其 `plan-00-result.md` 自述；以 `studio-app` 的实际实现为基线，核对 `studio-web` 仍缺哪些功能，并据此整理修复计划。**

## 背景

- 旧归档 `prompt-archives/2026042300-studio-web-feature-parity/plan-00-result.md` 把多个 Phase 标记为“已完成”，但用户验收发现存在“smoke 通过、实际功能未完成”的情况。
- 用户明确要求：**“审，不要相信他的 result。”**
- 因此本轮输入优先级为：
  1. 当前仓库里的真实代码与测试；
  2. `studio-app` 的实际行为与能力边界；
  3. 旧 plan / result 仅作为“声称做过什么”的对照材料，不能作为完成事实。

## 这轮任务的目标

1. 审计 `studio-web` 当前实现与 `studio-app` 实现之间的真实差距。
2. 找出“归档声称已完成、但代码实际上未完成或只做了占位/降级”的项目。
3. 将确认后的缺口按严重度归类，并转换成新的可执行修复计划。

## 审计原则

- 以源码、测试、实际调用链为准，不以 `plan-00-result.md` 的描述为准。
- 重点检查用户能直接感知的功能是否真正成立，而不是只看是否有 smoke / unit test 覆盖。
- 若桌面端已有明确实现，而 Web 端只剩静态文案、空壳按钮、未接线的状态或不兼容的数据格式，应记为未完成。
- 若发现会影响后续开发判断、但当前轮次不准备立即修复的问题，必须同步维护 `docs/known_issues.md`。

## 当前已确认的审计输入

- Web 主实现：
  - `packages/studio-web/src/`
  - `crates/studio-web-wasm/src/`
- 桌面端对照实现：
  - `crates/studio-app/src/`
  - `crates/scad-viewer/src/ui/`
  - `crates/studio-common/src/`
- 旧归档对照：
  - `prompt-archives/2026042300-studio-web-feature-parity/plan-00.md`
  - `prompt-archives/2026042300-studio-web-feature-parity/plan-00-result.md`

## 已知注意事项

- 当前工作树已存在未归属本轮任务的改动：`AGENTS.md`、`crates/app-server-protocol/src/protocol.rs`、`crates/studio-web-wasm/src/wasm_bridge/mesh.rs`、`packages/studio-web/src/workbench/workbench-wiring.ts`。规划与审计过程中不得覆盖或回退这些改动。
- 本轮先做审计与计划整理，不默认进入实现。

---

## 2026-04-24 继续执行

用户指令：

```text
prompt-archives/2026042400-studio-web-parity-audit-plan/plan-00.md 继续干活，不要停直到全部完成
```

执行要求：从当前 `plan-00-result.md` 记录的未完成项继续，完成 Phase 4、Phase 6、Phase 7，并保持前序 Phase 已满足的验收边界。
## 2026-04-24 12:48:56 CST 用户补充

> 还是有问题，在当前plan目录中开一个新的plan-01：
> 1. `preview error: 启动 OpenSCAD CLI 失败: No such file or directory (os error 2)`说明你的openscad二进制detection没有和studio-app保持一致，同时也没有fallback
> 2. 尝试使用默认的预览区域错误提示和工具栏重合，不可用，你需要着重注意工具栏在不同尺寸屏幕下的
> 3. 请在大尺寸屏幕中渲染`/Users/krhougs/LocalCodes/buddin/ui_kits/app/index.html`并分析我的设计，3d预览区域应该默认占满中间全部空间，参数和preset应该为右边栏的section
> 4. 文件列表不应该在右边栏而是作为左边栏的tab，同时文件列表也没有和studio-app一样展示明显的文件类型，现在是写死的"FILE"
> 5. 设置不应该是页面而应该是侧边栏的一个Tab
> 6. 右边栏所有section应该按照设计可以展开收起（请整理共享组件）

## 2026-04-24 用户补充（二）

> plan-01问题补充：
> 1. status bar应该占满可用宽度，固定高度，整体固定在中间预览区域的最底部，且不应该浮在文档流中而是作为页面框架固定存在。status bar不应该挡住预览区域任何内容
> 2. markdown预览直接使用 `@uiw/react-markdown-preview` 且开启mermaid支持，“注意安全问题”
> 3. 整体图标库使用 `@phosphor-icons/react`，这个写进本地维护的设计系统中，同时在侧边栏中使用 bold 这个weight
> 4. 在app状态中维护浏览器标题栏，标题栏需要展示当前展示的文件名和我们本身的产品名
> 5. 我们的产品名已经定下来为 `budn'`，在代码中使用 `budn` 作为名称，请写进整体的AGENTS.md和README中
> 6. Log从section进化为左边栏的一个Tab，入口按钮和设置按钮放在一起（最底部），log按钮放在设置按钮的正上方
> 7. 浏览器URL中应该有类似 `#left-panel=chat|files|settings`的路由控制联动(我没有规定必须这么写，但是必须有类似的机制保证路由和panel tab联动)

## 2026-04-24 13:10:33 CST 用户补充（三）

> 1. 这个markdown库应该自带mermaid支持，你看看有没有必要引入新的依赖
> 2. md中所有的链接都应该在新的浏览器tab中打开
> 3. markdown预览记得adopt当前的设计系统

## 2026-04-24 plan-02 discussion prompt

用户反馈 plan-01 后仍需修改，要求开启 plan-02 讨论：

1. 右边栏 section 展开按照原设计稿，应该是第一行的 +/- 来展示状态。
2. 重复打开已经打开的文件，应该跳转到已经打开的 tab 中。
3. 左边文件列表、Log 以及类似 UI 应该直接占满宽度而不是再套一层框，log 的 xx entries 那一行应该直接删掉，信息整合到上方 title block 中。
4. 左边栏 log 入口换个图标。
5. 右侧边栏缺少和 studio-app 同样完整功能的精确相机控制，预览区域缺少 studio-app 中左下角快速拖动视角的三线 handle，同时各个预设视角和默认视角的距离应该通过模型实际大小动态计算。
6. 缺少 studio-app 中的各种渲染模式：mono/color、剖切、fog。
7. 底板大小应该通过模型实际大小动态计算，默认视角应该是正面朝上斜 45 度且可以看到全貌。
8. 预览区域背景太黑且光照不足，看不清模型；需要保持黑色高端风格，同时调整颜色和光照参数以便正常使用。
9. parameters 修改数值应直接生效并重新生成预览，但要节流；保留恢复默认按钮，删除 apply 按钮；save preset 移到 parameters section 中。
10. preview section 应显示模型整体长宽高；设置中增加单位选项，可选毫米/厘米或对应英制单位。
11. parameters 如果识别是数字，应展示拖动条，允许负数，范围需要从当前值动态分析。

## 2026-04-24 plan-02 supplement

用户继续补充：

1. 修改 parameters 之后重新 render 后，摄像机不应该被重置。
2. 启动 dev server 应该默认听 `0.0.0.0` 且保证配好了 WebSocket 反向代理，外部可以方便连进来测试。
3. 两个侧边栏宽度可拖动并记忆，需要有默认宽度和最小宽度。
4. Tab 栏打开项目较多之后鼠标滚轮可以滚动，需要注意 Windows 用户滚动条样式。
5. 预览区鼠标手势不能实现 360 度调整，需要修改。
6. 模型和图片需要正在加载状态。parameters 修改之后也会触发加载状态；该加载状态不能破坏已经打开的文件预览。
