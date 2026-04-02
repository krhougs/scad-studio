# 3MF 彩色预览 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 `.scad` 预览链路强制输出 `3MF`，解析其中的逐面 / 逐顶点颜色信息，并在查看器中按 `Color / Mono` 开关正确渲染，默认进入 `Color`。

**Architecture:** 预览阶段不再复用 STL 导入链路，而是在 Rust 侧新增精简的 3MF ZIP/XML 解析模块，只覆盖当前所需的 `mesh + basematerials + colorgroup` 颜色语义，并把解析结果写入扩展后的 `MeshData`。渲染器继续保留现有 `wgpu` 管线，但要扩展顶点格式和着色器，使 `Color` 模式读取模型颜色，`Mono` 模式忽略模型颜色并回退到现有单色外观。

**Tech Stack:** Rust 2024、OpenSCAD CLI、3MF（ZIP + XML）、`zip`、`roxmltree`、`wgpu`、`egui`

---

## Context

当前预览路径固定为：

`.scad` -> OpenSCAD CLI `binstl` -> `src/mesh.rs` 解析 STL -> `MeshData` -> `wgpu`

这条路径天然丢失颜色。仓库当前已存在 3MF 导出能力，但只用于导出，不用于预览。参考仓库 `thijsdaniels/vscode-openscad-preview` 的做法是：预览优先走 3MF，STL 仅作为兼容回退；其中 3MF 颜色保留能力依赖 OpenSCAD Nightly。

本计划只覆盖查看器预览，不改变“导出到文件”的现有 UI 语义；也不在本轮引入贴图、PBR 或完整 3MF 材质生态。计划执行时必须保护当前工作树尚未提交的 3 项修复目标：

- 相机已支持完整轨道旋转
- XYZ 轴指示器已锚定真实视口
- STL 导入已做 OpenSCAD `Z-up` -> 查看器 `Y-up` 坐标转换

---

## 范围与非目标

### 本轮范围

- 预览强制使用 `3MF`
- OpenSCAD 预览失败时给出明确错误，而不是静默回退 STL
- 解析 3MF 中的：
  - `mesh` 顶点 / 三角面
  - `basematerials`
  - `colorgroup`
  - 三角面级 `pid/p1/p2/p3`
- 在查看器中支持：
  - 逐对象纯色
  - 逐三角面颜色
  - 逐顶点颜色插值
- 保留 `Color / Mono` 开关，默认改为 `Color`

### 非目标

- 不支持 3MF 贴图、纹理坐标、PBR、metallic、composite materials
- 不修改现有导出 UI 的格式选择逻辑
- 不在本轮把整个查看器内部坐标系从 `Y-up` 全量切换到 `Z-up`

### 失败策略

- 预览阶段若 3MF 文件缺失、XML 无法解析、或引用了本轮未支持的材质资源类型，应返回明确错误并终止本次预览更新
- 禁止为了“先显示一个模型”而静默回退 STL，避免再次引入“颜色悄悄丢失”

---

## 涉及文件与职责

### 修改文件

- `src/openscad.rs`
  - 把预览输出从 `BinaryStl` 切换到 `ThreeMf`
  - 维护临时 3MF 文件生命周期
  - 将预览解析错误改写为面向用户的明确信息
- `src/mesh.rs`
  - 扩展 `Vertex` / `MeshData`，承载颜色属性
  - 保留现有 STL 坐标变换逻辑
- `src/renderer.rs`
  - 上传扩展后的顶点缓冲
  - 更新 scene pass 所需的顶点布局
- `src/scene_bindings.rs`
  - 视需要扩展 uniform，声明模型颜色开关或插值模式相关字段
- `src/shader.wgsl`
  - 读取模型颜色并在 `Color` 模式下参与光照
- `src/shader_xray.wgsl`
  - 与主着色器保持一致的颜色语义
- `src/app.rs`
  - 将默认 `color_mode` 从 `Mono` 改为 `Color`
- `src/ui/toolbar.rs`
  - 仅检查现有 `Color / Mono` 开关文案是否仍准确；默认状态变更不需要改交互结构

### 新增文件

- `src/three_mf.rs`
  - 负责 3MF ZIP/XML 解析
  - 将 3MF 语义转换为仓库内部 `MeshTriangle` / `MeshData` 所需的中间结构
  - 控制 unsupported material group 的错误边界
- `tests/three_mf_tests.rs`
  - 3MF 解析回归测试
  - 覆盖 basematerials、colorgroup、逐三角面 / 逐顶点颜色、unsupported resource

### 补强测试文件

- `tests/mesh_tests.rs`
  - 增加颜色属性在导入后的断言
- `tests/openscad_command_tests.rs`
  - 补预览输出格式固定为 `3MF` 的命令组装测试
- `tests/pipeline_tests.rs`
  - 若 shader/uniform 协议变化，补颜色模式对应断言

---

## Phase 划分

### Phase 1：预览输出协议切换到 3MF

**目标**：让预览链路从 OpenSCAD CLI 输出 3MF，而不是 STL，并在失败时清晰报错。

**输入**：

- 当前 `src/openscad.rs` 固定输出 `BinaryStl`
- 当前仓库已有导出 3MF 的 CLI 参数组装逻辑

**关键文件**：

- `src/openscad.rs`
- `tests/openscad_command_tests.rs`

**本 Phase 要保护的前序目标 / 边界**：

- 不破坏现有“导出 STL / 3MF 到文件”的功能
- 不覆盖当前工作树里关于 STL 坐标修复的本地改动
- 不在本 Phase 引入任何颜色渲染改动，只改预览协议

**步骤**：

1. 梳理 `OpenScadRunner::build_job` 到 `finalize_job` 的预览产物路径，确认哪些逻辑与 STL 扩展名和清理流程耦合。
2. 将预览临时文件后缀从 `.stl` 改为 `.3mf`，并将 `build_cli_args` 调用固定为 `CliOutputFormat::ThreeMf`。
3. 调整成功与失败路径中的临时文件清理逻辑，确保预览异常退出时不会残留临时 3MF。
4. 将“预览输出不存在 / OpenSCAD 返回非零 / 后续解析失败”的错误消息改成面向用户的明确描述，明确指出预览需要可用的 3MF 输出。
5. 在 `tests/openscad_command_tests.rs` 中补充失败用例：
   - 预览命令必须使用 `3mf`
   - 临时文件名必须为 `.3mf`
6. 运行针对性测试并确认红绿过程成立。

**验收标准**：

- 预览命令固定输出 3MF
- 预览链路不再隐式依赖 STL
- 预览错误提示中能看出是“3MF 预览失败”，而不是模糊的“模型解析失败”
- `tests/openscad_command_tests.rs` 通过

---

### Phase 2：新增 3MF 解析器并输出带颜色的内部网格

**目标**：在 Rust 侧解析 3MF 中的 mesh 与颜色资源，生成仓库内部可消费的彩色网格数据。

**输入**：

- Phase 1 已能稳定得到 OpenSCAD 输出的临时 3MF
- 当前 `MeshData` 只承载位置和法线，不承载颜色

**关键文件**：

- `src/three_mf.rs`
- `src/mesh.rs`
- `tests/three_mf_tests.rs`
- `tests/mesh_tests.rs`

**本 Phase 要保护的前序目标 / 边界**：

- Phase 1 的 3MF 预览协议不得回退为 STL
- 当前 STL 导入的坐标变换逻辑必须保留
- 不在本 Phase 触碰渲染器和 shader，只完成“带颜色网格”的数据生产

**步骤**：

1. 在 `Cargo.toml` 中补充最小依赖：
   - `zip`
   - `roxmltree`
2. 新建 `src/three_mf.rs`，封装：
   - ZIP 容器打开
   - `/3D/3dmodel.model` 读取
   - XML 资源与对象查找
3. 设计中间数据结构，只覆盖本轮所需的 3MF 语义：
   - 顶点列表
   - 三角面列表
   - `basematerials`
   - `colorgroup`
   - 三角面上的 `pid/p1/p2/p3`
4. 明确颜色语义映射：
   - `basematerials`：三角面纯色
   - `colorgroup + p1/p2/p3`：逐顶点颜色
   - 未带颜色的三角面：标记为“无模型颜色”，由后续渲染层决定默认色
5. 明确 unsupported 行为：
   - 遇到 `texture2d`、`texture2dgroup`、`compositematerials` 等本轮未支持资源时，直接返回错误，禁止静默降级
6. 扩展 `src/mesh.rs`：
   - `Vertex` 增加颜色字段
   - `MeshTriangle` 增加颜色来源信息
   - 保留 STL 路径，让 STL 仍能生成“无模型颜色”的网格
7. 在 3MF -> 内部网格转换中保留 OpenSCAD `Z-up` -> 查看器 `Y-up` 的坐标变换，确保 3MF 与当前 STL 预览姿态一致。
8. 在 `tests/three_mf_tests.rs` 中为以下场景提供 fixture 级回归：
   - basematerials 纯色
   - colorgroup 逐顶点颜色
   - 同一 object 下多组三角面颜色
   - unsupported resource 明确报错
9. 在 `tests/mesh_tests.rs` 中补断言，确认导入后的颜色字段与坐标变换同时成立。

**验收标准**：

- Rust 侧能从 3MF 中得到带颜色的 `MeshData`
- 支持逐三角面和逐顶点颜色
- 不支持的 3MF 材质资源会明确失败，而不是静默丢色
- 3MF 导入后的模型姿态与当前查看器地面约定一致
- `tests/three_mf_tests.rs`、`tests/mesh_tests.rs` 全部通过

---

### Phase 3：把模型颜色接入 wgpu 渲染管线

**目标**：让 `Color` 模式读取模型颜色，`Mono` 模式忽略模型颜色，同时兼容 Solid / X-Ray / Wireframe。

**输入**：

- Phase 2 已能提供带颜色的 `MeshData`
- 当前 `shader.wgsl` / `shader_xray.wgsl` 的颜色来源仍是法线推导色或固定单色

**关键文件**：

- `src/renderer.rs`
- `src/mesh.rs`
- `src/shader.wgsl`
- `src/shader_xray.wgsl`
- `src/pipeline.rs`
- `tests/pipeline_tests.rs`

**本 Phase 要保护的前序目标 / 边界**：

- Phase 2 的 3MF 颜色解析语义不被改弱
- 已有相机、gizmo、截面、shadow、fog 测试必须继续通过
- Mono 模式下的整体视觉风格要尽量与当前版本一致，不能因为引入彩色顶点就让 Mono 外观漂移

**步骤**：

1. 扩展 GPU 顶点布局，为 mesh 顶点增加颜色属性；必要时把 scene pass 与 shadow pass 的顶点布局拆开，避免影子管线被无关颜色字段污染。
2. 调整 `Renderer::set_mesh` 和相关 buffer 上传逻辑，确保颜色缓冲与位置 / 法线一起进入 GPU。
3. 更新 `shader.wgsl`：
   - 顶点阶段向片段阶段传递模型颜色
   - `Color` 模式下使用模型颜色作为 base color 参与光照
   - `Mono` 模式下维持现有单色逻辑
4. 更新 `shader_xray.wgsl`，使其颜色语义与主 shader 对齐。
5. 校验 Wireframe：
   - 若线框仍由 polygon mode 输出，则不必额外做颜色插值
   - 但要确认 `Mono` / `Color` 切换不会导致线框模式出错
6. 在 `tests/pipeline_tests.rs` 或新增渲染协议测试中补断言，覆盖：
   - 顶点布局中颜色属性存在
   - `ColorMode::Color` 与 `ColorMode::Mono` 对应的 shader / uniform 行为仍可区分

**验收标准**：

- Solid 模式下彩色 3MF 能按模型颜色渲染
- X-Ray 模式下仍保留模型颜色，只是叠加透明度 / Fresnel
- Mono 模式下忽略模型颜色，视觉回到当前单色表现
- Wireframe 模式不因彩色属性引入回归
- `tests/pipeline_tests.rs` 与全量 `cargo test` 通过

---

### Phase 4：UI 默认值、错误体验与验收闭环

**目标**：把 UI 默认值和日志体验收拢到最终产品行为，并为缺少 OpenSCAD Nightly 的环境提供可执行的验收路径。

**输入**：

- Phase 1-3 已完成 3MF 预览与颜色渲染主链路
- 当前 `ViewerState::default()` 仍默认 `Mono`

**关键文件**：

- `src/app.rs`
- `src/ui/toolbar.rs`
- `src/main.rs`
- `docs/feature-roadmap.md`
- `docs/known_issues.md`

**本 Phase 要保护的前序目标 / 边界**：

- 前三 Phase 的解析与渲染逻辑不被 UI 层再度弱化
- 不为了“兼容旧环境”恢复 STL 预览回退
- 当前工作树已有的相机 / gizmo / 坐标修复必须保持

**步骤**：

1. 将 `ViewerState::default().color_mode` 改为 `Color`。
2. 检查工具栏 `Color / Mono` 文案与默认高亮是否仍准确；若需要，仅做最小文本调整，不重做工具栏结构。
3. 在预览失败路径中补清晰日志：
   - OpenSCAD 未安装
   - OpenSCAD 不支持 3MF 预览
   - 3MF 解析失败
   - 3MF 中引用了本轮未支持的资源类型
4. 更新 `docs/feature-roadmap.md` 中与“3MF 文件解析（支持颜色信息）”相关的条目，确保 roadmap 与实现范围一致。
5. 在 `plan-00-result.md` 中预留执行时的记录模板，要求后续执行每个 Phase 完成后实时回填。
6. 在具备 OpenSCAD Nightly 的环境完成一次人工验收：
   - `color("red")` / `color("green")` 多对象
   - 同 object 不同颜色
   - `Color` / `Mono` 切换
   - 错误环境下的提示

**验收标准**：

- 默认打开模型即进入 `Color`
- 用户在不支持 3MF 彩色预览的环境中能看到明确错误，而不是单色“看起来还能用”
- roadmap 与本轮实现边界一致
- 计划执行结果文档具备后续断点续做所需信息

---

## Phase 执行要求

- 每个 Phase 必须遵循：干活 -> review -> 回归
- 每个 Phase 的 review 必须使用独立 subagent，且 review 输入必须包含：
  - 当前 Phase 目标与验收标准
  - 本文全文
  - 本次变更 diff 或文件清单
- 每个 Phase 完成后必须立即回填 `plan-00-result.md`
- 除非遇到真正需要用户拍板的 blocker，否则 Phase 之间自动推进，不等待额外确认

---

## 风险与处理

### 风险 1：本机没有 OpenSCAD Nightly，无法做端到端彩色验证

- 影响：实现阶段只能依赖 fixture 和单元测试，无法在本机完成“`.scad` -> OpenSCAD -> 3MF -> 彩色预览”的闭环
- 处理：本轮计划强制要求先补 3MF fixture 解析测试，并把 Nightly 联调列为独立验收步骤

### 风险 2：3MF 材质语义超出本轮边界

- 影响：若 OpenSCAD 生成的 3MF 使用了本轮未支持资源类型，可能导致再次丢色
- 处理：对 unsupported resource 直接报错，避免静默错误

### 风险 3：扩展顶点格式后影响 shadow / xray / section 管线

- 影响：多管线共用顶点结构时容易引入布局不一致
- 处理：在计划中明确把 scene pass 与 shadow pass 的顶点布局检查列为独立步骤，并用回归测试覆盖

---

## 执行顺序建议

1. Phase 1：先把预览协议切到 3MF，锁住产品方向
2. Phase 2：再补 3MF 解析与颜色数据模型
3. Phase 3：最后把颜色真正接进渲染器
4. Phase 4：收口默认值、错误信息和文档
