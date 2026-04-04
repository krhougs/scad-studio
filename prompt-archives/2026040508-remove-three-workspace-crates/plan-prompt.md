## 背景

- 当前仓库根包 `scad-studio` 是实际运行的二进制入口。
- 已确认根包当前不依赖 `crates/scene`、`crates/scad-data`、`crates/scad-ui`。
- 用户要求直接删除上述三个 crate。

## 原始 prompt

1. `当前scad-studio二进制是否以来任何crates目录中的包？`
2. `先删掉这三个包`

## 注意事项

- 工作树当前存在大量未提交修改，其中包含这三个 crate 内的文件；本次按用户明确指令删除这三个 crate，但不改动根目录其他未提交文件。
- 删除后需要同步清理 workspace 成员配置与锁文件，避免残留无效依赖元数据。
- 验证以根目录二进制 `scad-studio` 为准，不以已删除 crate 的历史计划为准。
