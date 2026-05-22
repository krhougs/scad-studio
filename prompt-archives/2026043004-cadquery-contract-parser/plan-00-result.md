# CadQuery Contract Parser Result

## Status

已完成。

## Root Cause

`workspace/budn-web/chats/main.jsonl` 中的失败样例不是 LLM 完全没写 `MODEL_DESCRIPTION` / `MODEL_DETAILS`，而是写成了 Python 合法的相邻字符串字面量，例如：

```python
MODEL_DESCRIPTION = (
    "Contract "
    "model"
)
```

原 Rust 轻量 scanner 只接受赋值右侧直接以引号开头的字符串，因此把这类合法 Python 误判为 `has_model_description: false`。

## Changes

- 在 `budn_cad_runner/contract.py` 新增基于 Python `ast.parse` 的 model contract 分析。
- 在 `budn_cad_runner/__main__.py` 新增 `--contract-file` 入口；该入口在导入 CadQuery schema 之前返回，只依赖 Python AST。
- 在 `app-server-core` 新增 `run_cadquery_contract`，通过 `budn_cad_runner --contract-file` 获取 `has_model_description`。
- `CadQueryToolRuntime` 新增可选 `model_contract` 方法；host runtime 实现该方法。
- `cadquery_check_source`、`cadquery_analyze_source`、`cadquery_execute` 在 host runtime 可用时使用 runner AST 结果覆盖旧 scanner 的 `has_model_description`。
- 保留无 runtime 时的旧静态回退，避免非 host 单元测试和工具执行器被强制绑定本机 Python。
- Rust contract 临时文件使用独占创建，Unix 下权限为 `0600`，并在 Drop 时清理。
- runner 进程设置 `PYTHONDONTWRITEBYTECODE=1`，避免继续生成 Python bytecode。

## Verification

- `cargo test -p app-server-core --test agent_tool_tests`：141 passed。
- `cargo test -p app-server-core --test cadquery_tests`：11 passed。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests cadquery`：6 passed。
- `bun test tests/cadquery_runner.test.ts --timeout 30000`：13 passed。
- `cargo fmt --check`：passed。
- `git diff --check`：passed。

## Review Notes

- 独立 review 指出的 `analyze_source` warning 矛盾已修复。
- 独立 review 指出的 `--contract-file` 提前导入 CadQuery/OCP 问题已修复。
- 独立 review 指出的临时文件非独占创建风险已修复。
- 独立 review 指出的重复顶层赋值语义已改为最终赋值。
- 独立 review 指出的真实 runner CLI 负向覆盖不足已补充。
- 最终独立 review 未发现阻塞问题；补充处理了非阻塞风险中的裸类型标注、contract 分支延迟导入和执行校验顺序测试。

## Remaining Notes

- `app-server-core` 中旧 scanner 仍作为无 runtime 回退存在；host/web agent 路径使用 `budn_cad_runner` AST 结果。
