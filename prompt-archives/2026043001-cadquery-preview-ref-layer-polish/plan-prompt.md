# CadQuery Preview Ref Layer Polish Prompt Archive

## 背景

本轮基于 `prompt-archives/2026042700-cadquery-mvp-design/plan-00.md` 的实施结果继续查漏补缺。上一轮已经完成 Web Chat 通过 Agent 调用 CadQuery 生成 AirPods 车载无线充电垫模型、`.py` 文件列表打开后进入模型预览、Viewer 选择 Ref 后带入 Chat 后续修改，以及前端显示 `Thinking` reasoning 的基础能力。

上一轮执行结果归档在：

- `prompt-archives/2026043000-cadquery-web-e2e-gapfill/plan-00-result.md`

## 用户最新要求

继续启动 Playwright 调试循环调通：

1. 模型预览右边栏提供一个类似 Photoshop 图层的 section，渲染 Ref 层级树状结构，可以直接在 Ref 页面自由多选任意 Ref。
2. 文件列表选择 `.py` 和 `.step` 打开的应该路由到已经生成好的模型预览，同时需要保证 Agent 在写完模型后保持 step 内容和 py 同步。
3. 保证 PRD 中说的每个模型有一个文字说明他的用途和各种细节，CadQuery Python 代码中的各种元素应该考虑人类交互的情况下进行额外命名，参考 Ref 文档，必要时改 system prompt。
4. 确保预览过程中的 solid / wireframe / xray 渲染和切换正常工作。
5. 确保选择模型后模型更新直接在 `.py` / `.step` 的 tab 中进行预览，而不是打开新的临时文件预览 tab。
6. LLM 输出结束不要显示很大的 done card，和 Claude 网页一样只显示一个图标 logo 作为标识。
7. Agent 各种窗口的 tool 状态（start、running、result 等）应该简化为单独一行，点击弹出 modal 展开详细，而不是显示完整 card。
8. 同一 LLM stream 输出的东西，只在最上面显示一次 `ASSISTANT` 表示来源，中间每个 card 上方的来源名字应该省略。用户输入不受影响。
9. `cadquery-select-dock` 应该处于预览区域的最下方正中间，位于 status bar 上方。
10. 需要提供模式切换：选择模式和预览模式。选择模式保留当前 select dock 的所有选择模式；预览模式保留 axis、底板等预览外观，只隐藏选择用的线框、anchor、hover/selected 高亮和选择 dock/status。

## 用户澄清

2026-04-30 用户澄清：预览模式不是关闭 axis、底板等预览辅助，而是只关闭选择相关覆盖层。此前“只显示模型本身”的表述以后续澄清为准。

## 执行约束

- 自行启动或复用 Web dev server。
- 自行通过 Playwright / 浏览器完成真实页面调试循环。
- 中间不要中断，不向用户询问意见。
- 遇到前端、LLM stream、tool call、CadQuery runner 或 UX 问题时自行定位和修复。
- 遵守根 `AGENTS.md` 中的 CadQuery 架构边界：前端不得绕过 protocol 读取 runner 输出或自行从文件名、路径、instance path 推断 Ref。
- 保持上一轮已经完成的 Chat、CadQuery、文件列表预览、Ref 选择、Thinking 展示能力。
