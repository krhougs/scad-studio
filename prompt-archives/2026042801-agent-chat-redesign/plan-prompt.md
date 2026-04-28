# Plan Prompt 存档

## 初始 Prompt

prompt-archives/2026042700-cadquery-mvp-design/plan-00.md 已经标记完成，但是现在还是处于一个不可用的状态：
1. 完全没有接LLM (用 workspace/studio-web/providers.txt 里写的openai endpoint调试，模型用gpt-5.5 variant high，这只是一个给你调试用的账号，你得改成workspace config类似的东西，禁止提交进代码库)
2. 当前没有选择target的情况下agent拒绝聊天，这个在产品层面绝对不能接受
3. 用户可以随时在agent里聊新的东西，ref和selection不应该在agent层面上强耦合，最好的方式是像cursor那样把他塞进聊天内容中
4. 上网搜索参考一下codex app的交互流程，不要照抄，多思考场景和why
5. 上网搜索参考一下cursor的交互流程，不要照抄，多思考场景和why

你现在是一个经验丰富的、懂UIUX的产品经理，当前目标是让这个Agent Chat对于我们所有的目标用户都足够好用，尤其是对于新手能很快上手。
基于docs/cadquery-mvp思考Agent产品流程，开新的plan，别急着干活

## 背景

- 上一个 plan (`prompt-archives/2026042700-cadquery-mvp-design/plan-00.md`) 完成了 CadQuery MVP 的工程管线搭建（协议/服务端/前端），但产品层面不可用
- 需要从产品经理视角重新设计 Agent Chat 交互流程
- 参考了 Codex App 和 Cursor 的交互模式
- 核心转变：从"工具优先"到"对话优先"

## 注意事项

- `workspace/studio-web/providers.txt` 含调试用 OpenAI endpoint，仅供开发调试，禁止提交代码库
- 实现需要遵循 `AGENTS.md` 所有约束，特别是 Python 豁免边界、plan mode 规范
- Agent system prompt (`docs/cadquery-mvp/agent-system-prompt.md`) 已经很好，不需要大改
- 现有协议层结构合理，变更是增量的
