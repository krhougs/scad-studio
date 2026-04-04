# 删除三个 workspace crate 执行计划

## 背景

当前根包 `scad-studio` 才是实际运行的二进制入口。`crates/scene`、`crates/scad-data`、`crates/scad-ui` 是先前拆分 workspace 时引入的子包，但根包当前并未依赖它们。用户要求直接删除这三个包。

## 目标

删除上述三个 crate 目录及其 workspace 配置，确保根二进制仍能通过依赖解析与编译检查，且不修改用户在其他路径上的未提交工作。

## Phase 1：移除 crate 目录与 workspace 引用

### 输入

- 根目录 `Cargo.toml`
- `crates/scene/`
- `crates/scad-data/`
- `crates/scad-ui/`
- 当前工作树状态

### 需要保护的前序目标与边界

- 保护当前根二进制 `scad-studio` 的源码与运行路径，不把删除动作扩散到 `src/` 与 `tests/` 的既有未提交修改。
- 只删除用户明确点名的三个 crate 及其直接工作区引用，不额外整理历史存档或其他无关文件。

### 操作步骤

1. 从根 `Cargo.toml` 删除这三个 crate 的 workspace 成员声明。
2. 删除 `crates/scene/`、`crates/scad-data/`、`crates/scad-ui/` 目录下的源码、测试与清单文件。
3. 自查删除范围，确认没有误删根目录源码、测试或其他文档。

### 验收标准

- `Cargo.toml` 中不再出现这三个 crate 的 workspace 成员条目。
- `crates/scene`、`crates/scad-data`、`crates/scad-ui` 目录不存在。
- `git diff --stat` 中本次变更仅覆盖计划存档、根 `Cargo.toml`、锁文件和上述三个目录。

## Phase 2：回归依赖元数据并验证根二进制

### 输入

- Phase 1 后的根 `Cargo.toml`
- 现有 `Cargo.lock`
- 根包源码与测试

### 需要保护的前序目标与边界

- 保护 Phase 1 已完成的删除范围，不为了通过验证重新引入任何 crate 目录或相关依赖。
- 保护根目录其他未提交修改，只做依赖元数据回归和构建验证，不擅自重写无关文件。

### 操作步骤

1. 重新生成或更新 `Cargo.lock`，移除已删除 crate 的残留元数据。
2. 运行 `cargo check -p scad-studio` 验证根二进制仍可完成依赖解析与编译检查。
3. 运行 `cargo tree -p scad-studio`，确认依赖树中不再出现这三个 crate。
4. 自查最终 diff 与验证结果，整理执行结果记录。

### 验收标准

- `Cargo.lock` 中不再保留 `scene`、`scad-data`、`scad-ui` 包条目。
- `cargo check -p scad-studio` 成功。
- `cargo tree -p scad-studio` 输出中不含已删除 crate。
