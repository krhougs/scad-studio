# Plan-00 Result：3MF 彩色预览

## 当前状态

- 已完成 Phase 1-3 的实现、review 与自动化回归。
- Phase 4 的代码与文档收尾已完成；受本机缺少 OpenSCAD CLI / Nightly 影响，端到端人工验收仍待在具备环境的机器补做。
- 独立复审结果：Phase 1、Phase 2、Phase 3 在修正 reviewer 提出的边界问题后均已复审通过，当前无新增 findings。

## 执行记录模板

### Phase 1

- 完成情况：已完成
- 变更摘要：
  - `src/openscad.rs` 将预览临时产物从 `.stl` 切换到 `.3mf`，新增 `build_preview_job_args`，并统一了预览失败信息与临时文件清理逻辑。
  - `tests/openscad_command_tests.rs` 增加“预览命令必须使用 3mf”“临时文件必须为 .3mf”回归测试。
- 遗留问题：
  - 预览链路已不再依赖 STL，但端到端是否能由本机 OpenSCAD 正常导出 3MF 仍受环境限制，需在具备 CLI / Nightly 的机器补充人工验证。

### Phase 2

- 完成情况：已完成
- 变更摘要：
  - `Cargo.toml` 新增 `zip` 与 `roxmltree`。
  - 新增 `src/three_mf.rs`，支持从 `3D/3dmodel.model` 读取 `mesh`、`basematerials`、`colorgroup`、object 级默认属性、triangle 级 `pid/p1/p2/p3` 以及 build item transform，并对 unsupported resource 明确报错。
  - `src/mesh.rs` 为 `Vertex` / `MeshTriangle` 增加颜色字段，同时保留 STL 路径的 `Z-up -> Y-up` 坐标转换与“无模型颜色”语义。
  - 新增 `tests/three_mf_tests.rs`，并扩展 `tests/mesh_tests.rs` 覆盖 object-level basematerial、逐顶点 colorgroup、同 object 多组三角面颜色和 unsupported resource。
- 遗留问题：
  - 当前 3MF 解析仅覆盖计划范围内的 `mesh + basematerials + colorgroup`；遇到 `texture2d`、`texture2dgroup`、`compositematerials` 等扩展资源会直接失败。

### Phase 3

- 完成情况：已完成
- 变更摘要：
  - `src/pipeline.rs` 为场景顶点布局增加颜色属性。
  - `src/shader.wgsl` 与 `src/shader_xray.wgsl` 接入模型颜色，在 `Color` 模式优先读取顶点颜色，在缺省无模型颜色时保留当前法线推导色；`Mono` 模式继续使用现有单色外观。
  - `tests/pipeline_tests.rs` 增加颜色顶点属性回归测试。
- 遗留问题：
  - Wireframe 仍沿用 polygon mode 输出，没有额外做颜色插值；目前自动化回归未发现该模式回归。

### Phase 4

- 完成情况：已完成代码与文档收尾，Nightly 人工验收待补
- 变更摘要：
  - `src/app.rs` 默认 `color_mode` 改为 `Color`，`src/main.rs` 状态提示改为明确的 3MF 预览文案。
  - 检查 `src/ui/toolbar.rs`，确认现有 `Color / Mono` 文案与交互结构无需额外调整。
  - 更新 `docs/feature-roadmap.md`，同步 3MF 彩色预览的已实现范围与明确不支持的资源类型。
  - 更新 `docs/known_issues.md`，保留“缺少 Nightly 环境”这一真实阻塞，并同步自动化回归已补齐的现状。
- 遗留问题：
  - 尚未在具备 OpenSCAD Nightly 的环境执行 `color(\"red\") / color(\"green\")` 多对象、同 object 不同颜色、`Color / Mono` 切换和错误提示的人工验收。
