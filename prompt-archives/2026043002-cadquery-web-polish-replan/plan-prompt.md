# CadQuery Web Polish Replan Prompt

## 背景

本计划接续以下上下文：

- `prompt-archives/2026042700-cadquery-mvp-design/plan-00.md` 的 CadQuery MVP 方向。
- `prompt-archives/2026043000-cadquery-web-e2e-gapfill/` 的执行结果：Web Chat 已能触发 CadQuery 建模，LLM reasoning 已在前端显示 `Thinking`，CadQuery `.py` 预览和 Ref selection 已完成基础链路。
- `prompt-archives/2026043001-cadquery-preview-ref-layer-polish/` 的部分执行结果：已开始实现 Ref tree、`.step` 路由、聊天事件轻量化和预览模式，但尚未完成完整 Playwright 调试循环与真实网页验收。

## 用户需求原文摘要

用户要求继续启动 Playwright 调试循环并调通：

1. 模型预览右边栏提供类似 Photoshop 图层的 section，渲染 Ref 层级树，可以在 Ref 页面自由多选任意 Ref。
2. 文件列表选择 `.py` 和 `.step` 打开时路由到已经生成好的模型预览，并保证 Agent 写完模型后 `.py` 和 `.step` 同步。
3. 每个模型有文字说明用途和细节，CadQuery 代码元素需要面向人类交互额外命名，必要时调整 system prompt。
4. 预览中的 solid / wireframe / xray 渲染和切换正常。
5. 选择模型后，模型更新直接在 `.py` / `.step` tab 中预览，而不是打开新的临时文件 tab。
6. LLM 输出结束不要显示大 done card，只显示一个图标或 logo 标识。
7. Agent tool 的 start / running / result 等状态简化为单独一行，点击弹出 modal 展开详情。
8. 同一 LLM stream 的内容只在最上面显示一次 `ASSISTANT` 来源，用户输入不受影响。
9. `cadquery-select-dock` 位于预览区域最下方正中间、status bar 上方。
10. 提供模式切换：选择模式与预览模式。

用户随后澄清：

1. 选择模式需要保留之前已有的多种选择方式。
2. 预览模式需要保留 axis、底板等预览辅助，只隐藏选择用的线框和 anchor。

用户继续补充：

1. 模式集合应包含一个独立的预览模式。
2. 其他选择模式应按此前 Ref 层级设计提供多个选择模式，而不是只有一个笼统的选择模式。选择模式应覆盖用户可见 Ref 层级中的 component / part / assembly、instance、feature、face、edge、vertex 等层级；具体可用项以当前模型和 MVP protocol 暴露的数据为准。

用户还指出：

- `app-server` 通用代码不能与 AirPods 垫子这类特定测试 case 耦合。
- 已更新 `AGENTS.md`，明确禁止测试场景污染产品代码。

用户继续补充：

- 整个计划需要增加一个前置步骤：清理之前 Agent 把具体建模 case 和任务相关内容直接耦合进前后端代码的问题。

## 当前已知状态

- 当前工作树存在上一轮和本轮的未提交改动，执行前必须先审计已有改动，不得盲目覆盖。
- `bun run --cwd packages/studio-web test:e2e tests/playwright/cadquery-viewer-selection.spec.ts` 在中断前实际完成，4 个 CadQuery viewer 选择用例通过；这只能作为基线证据，不代表完整任务完成。
- Web dev server 可能仍在后台运行，执行时应确认状态并复用或重启。

## 关键约束

- 继续执行前必须先完成本计划并归档。
- 不得把 AirPods、车载无线充电板或当前验收 prompt 的具体对象名称固化进 `app-server`、protocol、transport 或通用 tool schema 代码。
- 前端不得绕过 protocol 读取 runner 输出或通过文件名、路径、instance path 自行推断 Ref。
- CadQuery `.py` 模型生成和修改必须通过结构化 CadQuery tool call，不能通过普通文件写入工具直接改写。
- 模式切换必须表达为“预览模式 + Ref 层级选择模式集合”。预览模式不是“纯空白模型模式”：必须保留 axis、底板、gizmo 等预览外观，只隐藏选择相关覆盖层和选择 UI。
- 执行任何新功能前，必须先审计并清理前端、后端、`app-server`、protocol、tool schema 和产品 prompt 中的具体建模 case / 当前任务语义耦合。测试 fixture、prompt archive 和真实验收记录可以包含具体 case，但不能影响产品代码路径或通用契约。
