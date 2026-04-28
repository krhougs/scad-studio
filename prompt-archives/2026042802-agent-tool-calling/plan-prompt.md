# Agent Tool Calling

## 背景

Agent Chat Phase A-D 全部完成，LLM 已接入且能流式响应。但 Agent 在 Inform/Plan/Auto 模式下完全不可用——LLM 回复"当前会话没有可用的文件读写或 CadQuery 运行工具可调用"。

根因：
1. `build_request_body` 不包含 `tools` 字段
2. `LlmProvider::stream_chat` 不支持 tool calling
3. SSE 解析只处理 content delta，不处理 tool_calls delta
4. 无 workspace 文件读取工具，LLM 无法了解项目状态

## 目标

为 text agent（Inform/Plan/Auto）提供 `read_file` 和 `list_directory` 两个只读工具，让 LLM 能探索 workspace 后给出有意义的回答和计划。

## 约束

- Execute 路径保持不变（已有 plan confirm → cadquery execute 管线）
- 工具执行沙箱在 workspace 根目录内，禁止路径穿越
- 新文件不超过 500 行，新函数不超过 50 行
- 纯函数必须有单元测试
