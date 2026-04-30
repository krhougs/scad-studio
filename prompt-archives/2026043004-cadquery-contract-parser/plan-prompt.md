# Prompt Archive

## User Context

用户在 `workspace/budn-web/chats/main.jsonl` 中观察到 CadQuery agent 几乎每次生成模型时都会看到类似：

```text
has_model_description: false
It's not detecting my MODEL_DESCRIPTION or MODEL_DETAILS.
```

用户要求确认 agent 中有哪些位置与 `MODEL_DESCRIPTION`、`MODEL_DETAILS`、`has_model_description` 有关，并结合 chat history 判断问题来源；此前用户已明确要求“找出来问题要修”。

## Investigation Notes

`workspace/budn-web/chats/main.jsonl` 中的失败样例显示：

- `msg-20`、`msg-27`、`msg-41`、`msg-43` 使用了 Python 合法的括号包裹字符串拼接：
  - `MODEL_DESCRIPTION = ("..." "...")`
  - `MODEL_DETAILS` 字段值也可能是 `("..." "...")`
- `msg-22` 把 `MODEL_DESCRIPTION` 和 `MODEL_DETAILS` 作为 docstring 文本写入，不是顶层变量，这种形态应继续判定为不合规。
- `msg-29` 使用单行字符串后通过 `has_model_description: true`。

## Relevant Files

- `crates/app-server-core/src/agent/tools/cadquery/support.rs`
- `crates/app-server-core/tests/agent_tool_tests.rs`
- `docs/known_issues.md`（仅当发现本轮无法完成的遗留问题时更新）

