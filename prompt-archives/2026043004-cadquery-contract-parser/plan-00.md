# CadQuery Contract Parser Plan

## Background

CadQuery agent 的源码契约要求模型源码包含顶层 `MODEL_DESCRIPTION` 和 `MODEL_DETAILS`。当前 chat history 显示，LLM 经常生成 Python 合法的括号包裹字符串拼接，但 checker 只接受赋值右侧直接以引号开头的字符串，因此出现 `has_model_description: false` 的误判。

## Goal

让 CadQuery contract checker 通过 `budn_cad_runner` 内的 Python `ast` 分析 `MODEL_DESCRIPTION` / `MODEL_DETAILS`，接受 Python 中常见的括号包裹字符串拼接，同时保留现有安全边界：

- 仍然只接受顶层 `MODEL_DESCRIPTION` 和 `MODEL_DETAILS`。
- 仍然要求 `MODEL_DETAILS` 包含六个固定字段。
- 仍然拒绝 docstring、普通字符串、函数内部变量和空字段。
- 不引入 Python 辅助脚本。
- 不新增项目通用 Python 调用；该能力只作为 `budn_cad_runner` 外部 CAD 工具边界的一部分存在。

## Phase 1: Reproduce The False Negative

**Input**

- chat history 中的失败形态：
  - `MODEL_DESCRIPTION = ("..." "...")`
  - `MODEL_DETAILS` 字段值为括号包裹字符串拼接

**Actions**

1. 在现有 agent tool tests 中增加最小复现测试。
2. 运行目标测试，确认新增测试因 `has_model_description: false` 失败。

**Acceptance**

- 测试失败原因指向 contract checker 未接受括号包裹字符串拼接。

**Protect Previous Phase Goals**

- 本 Phase 是首个 Phase，无前序目标。

## Phase 2: Move Model Contract Analysis To Runner AST

**Input**

- Phase 1 的失败测试。
- `budn_cad_runner` 作为唯一允许的 CadQuery Python 边界。

**Actions**

1. 在 `budn_cad_runner` 内新增 AST contract 分析入口。
2. Rust host 通过 CadQuery runtime 调用该入口，`cadquery_check_source` 和 `cadquery_execute` 在有 runtime 时使用 AST 结果。
3. Rust core 保留静态回退路径，供无 runtime 的单元测试和非 host 场景使用。
4. 不改变 docstring 跳过逻辑、顶层赋值判断和字典字段要求。

**Acceptance**

- Phase 1 新增测试通过。
- 现有负向测试继续通过。
- runner contract 分析测试覆盖括号字符串拼接和 tuple / 表达式拒绝。

**Protect Previous Phase Goals**

- 保护 Phase 1 的失败样例，不用放宽为接受任意表达式或非顶层变量。

## Phase 3: Verification And Archive Result

**Input**

- Phase 2 的实现。

**Actions**

1. 运行相关 Rust 测试。
2. 运行格式检查。
3. 记录执行结果。

**Acceptance**

- 相关测试通过。
- `cargo fmt --check` 通过。
- `plan-00-result.md` 准确记录根因、改动和验证结果。

**Protect Previous Phase Goals**

- 不改变 CadQuery 执行、导出、runner 环境和 LLM tool schema 的既有行为。
