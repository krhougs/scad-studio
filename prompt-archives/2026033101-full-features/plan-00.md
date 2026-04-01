# Plan-00: scad-studio 全功能实现计划

## Context

scad-studio 的 MVP（Phase 1-5）已完成，涵盖核心管线、文件管理、3D 渲染、轨道相机和基础 UI。feature-roadmap.md 中仍有 50+ 个未完成功能项，涉及渲染模式、相机增强、环境场景、光照系统、交互式截面、参数编辑、预设系统、导出和 GUI 完善。本计划将这些功能组织为 12 个 Phase，按依赖顺序逐步实现，重点关注查看器 GUI 的整体布局与交互设计。

---

## GUI 整体布局方案

```
┌─────────────────────────────────────────────────────────────┐
│  平台菜单栏 (File | View | Help)  [现有，macOS/Windows 原生] │
├─────────────────────────────────────────────────────────────┤
│  工具栏 (TopBottomPanel::top("toolbar"))                     │
│  ┌─────┐ ┌─────────┐ ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐  │
│  │渲染 │ │颜色     │ │投影  │ │环境  │ │阴影  │ │截面  │  │
│  │模式 │ │         │ │      │ │      │ │      │ │      │  │
│  │Solid│ │Mono     │ │Persp │ │Grid  │ │Shadow│ │Clip  │  │
│  │Wire │ │         │ │Ortho │ │Plate │ │      │ │      │  │
│  │XRay │ │         │ │      │ │Axis  │ │      │ │      │  │
│  └─────┘ └─────────┘ └──────┘ └──────┘ └──────┘ └──────┘  │
├────────────────────────────────────────┬────────────────────┤
│                                        │  右侧面板          │
│                                        │  (SidePanel::right)│
│                                        │  可折叠/可调宽     │
│                                        │                    │
│           3D 视口                      │  [参数编辑器]      │
│        (CentralPanel)                  │  CollapsingHeader  │
│                                        │  - 数值滑块        │
│     ┌───┐                              │  - 布尔开关        │
│     │XYZ│  坐标轴指示器                │  - 字符串下拉      │
│     │   │  (左下角 egui overlay)       │                    │
│     └───┘                              │  [预设选择器]      │
│                                        │  CollapsingHeader  │
│                                        │  - 预设列表        │
│                                        │  - 保存/删除       │
│                                        │                    │
│                                        │  [导出]            │
│                                        │  CollapsingHeader  │
│                                        │  - STL / 3MF       │
│                                        │  - 发送到切片      │
├────────────────────────────────────────┴────────────────────┤
│  日志面板 (TopBottomPanel::bottom("log_panel"), 可折叠)      │
│  CLI 输出 | 编译错误 | 滚动区域                              │
├─────────────────────────────────────────────────────────────┤
│  状态栏 (现有 TopBottomPanel::bottom("status_bar"))          │
│  文件名 | 渲染状态 | 三角面数 | 相机模式                     │
└─────────────────────────────────────────────────────────────┘
```

### 面板行为规则

| 面板 | 默认状态 | 交互 |
|------|----------|------|
| 工具栏 | 始终显示 | 按钮组之间有分隔线 |
| 右侧面板 | 隐藏（无文件时）/ 显示（有文件时） | 可通过 View 菜单或快捷键 `]` 切换 |
| 日志面板 | 折叠 | 有错误时自动展开，可通过按钮或快捷键 `` ` `` 切换 |
| 坐标轴指示器 | 始终显示 | 仅展示，不可交互 |

---

## Phase 划分

### Phase 1: GUI 骨架与工具栏框架

**目标**: 建立完整的 GUI 面板布局框架（工具栏、右侧面板、日志面板），为后续功能提供 UI 挂载点。

**输入**: 现有 MVP 代码

**关键文件**:
- `src/app.rs` — 重构 UI 方法，添加工具栏/右侧面板/日志面板
- `src/main.rs` — 添加日志收集到 RuntimeState

**新增文件**:
- `src/ui/mod.rs` — UI 模块入口
- `src/ui/toolbar.rs` — 工具栏组件
- `src/ui/side_panel.rs` — 右侧面板框架
- `src/ui/log_panel.rs` — 日志面板组件
- `src/ui/status_bar.rs` — 从 app.rs 迁出状态栏

**步骤**:
1. 创建 `src/ui/` 模块目录，将现有 `show_menu`/`show_status_bar` 从 `app.rs` 迁移到独立文件
2. 实现工具栏框架（`TopBottomPanel::top("toolbar")`），包含占位按钮组（渲染模式、投影、环境、阴影、截面），每组用 `ui.separator()` 分隔
3. 实现右侧面板框架（`SidePanel::right`），包含空的 `CollapsingHeader` 区域（参数、预设、导出），可通过按钮折叠/展开
4. 实现日志面板（`TopBottomPanel::bottom("log_panel")`），包含日志缓冲区和滚动区域
5. 定义 `ViewerState` 结构体存储所有 UI 状态（渲染模式、投影模式、环境开关等），作为工具栏和渲染器之间的桥梁
6. 实现 `UiActions` 扩展，将工具栏按钮的状态变更通过 action 传递给渲染循环
7. 将 OpenSCAD CLI 的 stdout/stderr 输出收集到日志缓冲区

**验收标准**:
- 启动后可见：工具栏（带占位按钮）、右侧面板（空 section）、可折叠日志面板、状态栏
- 工具栏按钮可点击但暂无实际效果（后续 Phase 接入）
- 日志面板可折叠/展开，OpenSCAD 输出可见
- 右侧面板可通过按钮隐藏/显示
- 现有功能（打开文件、3D 渲染、相机操作、文件监控）不受影响
- `cargo test` 全部通过

**保护**: MVP 所有功能不被破坏（文件打开、渲染、相机、监控）

---

### Phase 2: 渲染模式（Wireframe + X-Ray + 颜色切换）

**目标**: 实现三种渲染模式切换（Solid/Wireframe/X-Ray）和单色/彩色切换，接入工具栏按钮。

**输入**: Phase 1 的 GUI 框架

**关键文件**:
- `src/renderer.rs` — 拆分并添加多管线支持
- `src/shader.wgsl` — 扩展着色器支持颜色模式

**新增文件**:
- `src/pipeline.rs` — 从 renderer.rs 拆出管线创建逻辑
- `src/shader_xray.wgsl` — X-Ray 着色器（Fresnel + alpha blend）
- `tests/pipeline_tests.rs` — 管线配置测试

**步骤**:
1. 从 `renderer.rs` 拆出管线创建相关函数到 `pipeline.rs`（renderer.rs 当前 484 行，加上多管线后会超 500 行限制）
2. 定义 `RenderMode` 枚举：`Solid`、`Wireframe`、`XRay`
3. 创建线框管线：`PolygonMode::Line`（需要 `POLYGON_MODE_LINE` feature，不支持时禁用按钮）
4. 创建 X-Ray 管线：关闭背面剔除 + alpha blend + Fresnel 着色器
5. 在 SceneUniform 中添加 `color_mode` 字段（0=默认颜色, 1=单色），着色器根据此字段选择颜色
6. 将 Phase 1 工具栏中的渲染模式按钮和颜色按钮接入 ViewerState，renderer 根据 ViewerState 选择管线

**验收标准**:
- 工具栏点击 Solid/Wireframe/X-Ray 可实时切换渲染模式
- Wireframe 模式下模型显示为线框
- X-Ray 模式下模型半透明，边缘 Fresnel 效果可见
- 颜色切换按钮可在彩色和单色之间切换
- 如果 GPU 不支持 `POLYGON_MODE_LINE`，Wireframe 按钮自动置灰
- 现有 Solid 渲染效果不变

**保护**: Phase 1 GUI 框架不变；Solid 模式渲染质量不退化

---

### Phase 3: 正交投影与投影切换

**目标**: 实现正交投影模式，工具栏可切换透视/正交。

**输入**: Phase 1 的 GUI 框架

**关键文件**:
- `src/camera.rs` — 添加正交投影矩阵计算
- `tests/camera_tests.rs` — 添加正交投影测试

**步骤**:
1. 在 `OrbitalCamera` 中添加 `ProjectionMode` 枚举（Perspective / Orthographic）
2. 实现 `orthographic_matrices()` 方法，根据 distance 和 aspect_ratio 计算正交投影范围
3. 修改 `matrices()` 方法根据 projection_mode 选择投影方式
4. 修改 `fit_bounds()` 在正交模式下正确计算缩放
5. 接入工具栏投影切换按钮

**验收标准**:
- 工具栏点击可在透视/正交之间切换
- 正交模式下模型无透视畸变
- 正交模式下缩放/旋转/平移操作正常
- 切换投影模式时相机视角不跳变
- 所有现有 camera_tests 仍通过，新增正交投影测试

**保护**: 透视投影的行为和数学不变

---

### Phase 4: 环境场景（Grid + Build Plate + 坐标轴指示器）

**目标**: 实现网格地面、打印平台和坐标轴指示器。

**输入**: Phase 1 GUI 框架 + Phase 3 相机（正交投影下 grid 显示正确）

**关键文件**:
- `src/renderer.rs` (或拆分后的模块) — 添加 grid/plate 渲染
- `src/ui/toolbar.rs` — 环境按钮接入

**新增文件**:
- `src/grid.rs` — Grid 网格生成与渲染
- `src/gizmo.rs` — 坐标轴指示器（egui 2D 绘制）
- `src/shader_grid.wgsl` — Grid 着色器（带淡出效果）
- `tests/grid_tests.rs` — Grid 顶点生成测试
- `tests/gizmo_tests.rs` — 坐标轴投影计算测试

**步骤**:
1. 实现 Grid 网格：在 Y=0 平面生成线段顶点，着色器实现距离淡出
2. 实现 Build Plate：在 Grid 基础上绘制 256mm x 256mm 的矩形边框
3. 实现坐标轴指示器：读取相机旋转矩阵，在视口左下角用 egui `Painter` 绘制红(X)/绿(Y)/蓝(Z) 三轴
4. Grid 和 Build Plate 渲染在模型之前（scene pass 内部），坐标轴在 egui pass 中绘制
5. 接入工具栏环境按钮（Grid/Plate/Axis 各自独立开关）

**验收标准**:
- Grid 在 Y=0 平面显示，随距离淡出
- Build Plate 显示 256mm 方形区域边框
- 坐标轴指示器在左下角正确跟随相机旋转
- 各环境元素可独立开关
- 透视/正交模式下均正常显示

**保护**: Phase 1-3 所有功能不受影响

---

### Phase 5: 光照系统

**目标**: 实现多光源支持（环境光、方向光、聚光灯、点光源）和阴影渲染。

**输入**: Phase 2 渲染管线 + Phase 4 环境场景

**关键文件**:
- `src/shader.wgsl` — 重写光照计算，支持多光源
- `src/renderer.rs` / `src/pipeline.rs` — 添加 shadow map pass

**新增文件**:
- `src/lighting.rs` — 光源定义与管理
- `src/shadow.rs` — Shadow map 生成与采样
- `src/shader_shadow.wgsl` — Shadow map 生成着色器
- `tests/lighting_tests.rs` — 光源参数计算测试

**步骤**:
1. 定义光源数据结构：`AmbientLight`、`DirectionalLight`、`SpotLight`、`PointLight`
2. 创建光照 uniform buffer（最多 4 个光源），更新 SceneUniform
3. 重写 `fs_main` 着色器：遍历光源数组，累加各光源贡献
4. 实现 Shadow Map：为方向光创建 1024x1024 深度纹理，shadow pass 从光源视角渲染深度
5. 在主着色器中采样 shadow map，PCF 3x3 柔化
6. 接入工具栏阴影开关

**验收标准**:
- 默认场景包含 1 个环境光 + 1 个方向光（与现有效果一致）
- 阴影可开关，开启后模型在 Grid 上投射阴影
- 阴影边缘柔和（PCF 生效）
- 性能可接受（无明显帧率下降）

**保护**: Phase 2 渲染模式切换不受影响；无阴影时的渲染效果与之前一致

---

### Phase 6: 指数雾效果

**目标**: 实现指数雾效果，远处物体逐渐融入背景色。

**输入**: Phase 5 光照系统

**关键文件**:
- `src/shader.wgsl` — 在片段着色器末尾添加雾计算

**步骤**:
1. 在 SceneUniform 中添加 `fog_density` 和 `fog_color` 字段
2. 在片段着色器中计算 `fog_factor = exp(-distance * fog_density)`，混合雾色
3. 接入工具栏雾开关
4. fog_density 默认值 0.01，fog_color 与背景色一致

**验收标准**:
- 开启雾效果后远处物体逐渐消融到背景色
- 关闭时渲染效果与之前完全一致
- Grid 和 Build Plate 也受雾影响

**保护**: Phase 5 光照和阴影不受影响

---

### Phase 7: 交互式截面（Cross-Section）

**目标**: 实现切割平面的渲染、选中、平移、旋转和 Stencil Buffer 截面效果。

**输入**: Phase 2 渲染管线 + Phase 1 GUI

**关键文件**:
- `src/renderer.rs` — 深度格式改为 Depth24PlusStencil8，添加 stencil 逻辑
- `src/shader.wgsl` — 添加 clip plane discard

**新增文件**:
- `src/cross_section.rs` — 切割平面定义、变换、交互逻辑
- `src/shader_section.wgsl` — 截面填充着色器
- `tests/cross_section_tests.rs` — 切割平面数学测试

**步骤**:
1. 定义 `ClipPlane` 结构体（法线 + 距离），添加到 SceneUniform
2. 在主着色器中用 `discard` 裁掉平面一侧的片段
3. 修改深度格式为 `Depth24PlusStencil8`，用 stencil buffer 标记截面区域
4. 第二遍渲染 pass 填充截面区域（纯色或条纹）
5. 实现切割平面的半透明可视化渲染（蓝色半透明矩形）
6. 实现鼠标拾取切割平面（光线与平面求交）
7. 实现 W 键平移 / E 键旋转交互，Ctrl 吸附（1mm / 5 度）
8. 接入工具栏截面开关

**验收标准**:
- 开启截面后模型被切割，截面处显示填充色
- 切割平面可视化为半透明蓝色矩形
- W 键进入平移模式，E 键进入旋转模式
- Ctrl 吸附正常工作
- 关闭截面后恢复完整模型
- Stencil buffer 截面效果无伪影

**保护**: Phase 1-6 所有功能不受影响；深度格式变更不影响现有渲染

---

### Phase 8: 参数编辑

**目标**: 解析 .scad 文件中的参数声明，在右侧面板提供 UI 控件，修改后实时重新渲染。

**输入**: Phase 1 GUI 框架（右侧面板）

**关键文件**:
- `src/openscad.rs` — 添加 `-D` 参数传递
- `src/ui/side_panel.rs` — 实现参数编辑器 UI

**新增文件**:
- `src/params.rs` — .scad 参数解析（正则匹配 Customizer 格式）
- `src/ui/param_editor.rs` — 参数编辑器组件
- `tests/params_tests.rs` — 参数解析测试（各种格式）

**新增依赖**: `regex`

**步骤**:
1. 实现参数解析器：用正则匹配 OpenSCAD Customizer 格式
   - `variable = value; // [min:step:max]` → 数值滑块
   - `variable = true; // or false` → 布尔开关
   - `variable = "option"; // [option1, option2, ...]` → 下拉选择
   - `/* [Group Name] */` → 参数分组
   - `/* [Hidden] */` → 隐藏参数
2. 打开 .scad 文件时自动解析参数，存储默认值和当前值
3. 在右侧面板渲染参数编辑器（按分组显示，每组一个 CollapsingHeader）
4. 参数修改后 300ms 去抖动，通过 `-D var=value` 传递给 OpenSCAD CLI 重新渲染
5. 覆盖的参数加粗显示，提供“恢复默认值”按钮
6. 文件变更时重新解析参数，保留用户已修改的值

**验收标准**:
- 打开含 Customizer 注释的 .scad 文件后，右侧面板显示参数控件
- 滑块/开关/下拉控件正常工作
- 修改参数后模型自动重新渲染
- 参数分组正确显示
- 隐藏参数不显示
- 恢复默认值功能正常
- 参数解析测试覆盖各种格式

**保护**: Phase 1-7 所有功能不受影响；无参数的 .scad 文件右侧面板为空

---

### Phase 9: 预设系统

**目标**: 实现 .scad.json 预设文件的读取、保存、删除和热加载。

**输入**: Phase 8 参数编辑

**关键文件**:
- `src/ui/side_panel.rs` — 预设选择器 UI

**新增文件**:
- `src/presets.rs` — 预设文件读写逻辑
- `tests/presets_tests.rs` — 预设序列化/反序列化测试

**新增依赖**: `serde`, `serde_json`

**步骤**:
1. 定义预设文件格式：`{ "presets": { "name": { "param1": value, ... } } }`
2. 打开 .scad 文件时自动查找同名 .scad.json 文件
3. 在右侧面板“预设”区域显示预设列表
4. 实现加载预设：点击预设名 → 参数值覆盖 → 触发重新渲染
5. 实现保存预设：弹出输入框 → 序列化当前参数值 → 写入 .scad.json
6. 实现删除预设：选中预设 → 确认删除 → 更新文件
7. 监控 .scad.json 文件变更（复用 FileWatcher），热加载预设

**验收标准**:
- 有预设文件时自动加载并显示预设列表
- 加载预设后参数控件更新，模型重新渲染
- 保存/删除预设后文件正确更新
- 外部修改 .scad.json 后预设列表自动刷新

**保护**: Phase 8 参数编辑功能不受影响

---

### Phase 10: 导出系统

**目标**: 实现 STL/3MF 导出和一键发送到切片软件。

**输入**: Phase 8 参数编辑（导出时带参数）

**关键文件**:
- `src/ui/side_panel.rs` — 导出面板 UI
- `src/openscad.rs` — 添加导出格式支持

**新增文件**:
- `src/export.rs` — 导出逻辑与切片软件集成
- `tests/export_tests.rs` — 导出路径构建测试

**新增依赖**: `zip`（3MF 是 ZIP 容器）, `dirs`（用户目录检测）

**步骤**:
1. 实现 STL 导出：调用 OpenSCAD CLI 带 `-D` 参数生成最终 STL，保存到用户指定路径
2. 实现 3MF 导出：调用 OpenSCAD CLI 生成 3MF（OpenSCAD 原生支持 `--export-format 3mf`）
3. 在右侧面板“导出”区域添加导出按钮和格式选择
4. 实现切片软件检测：扫描常见安装路径（PrusaSlicer / Bambu Studio / Cura）
5. 实现一键发送到切片：导出 STL/3MF 到临时文件 → 启动切片软件并传入文件路径
6. 实现切片软件路径手动配置（存储在 `~/.config/scad-studio/config.json`）

**验收标准**:
- 点击导出按钮弹出保存对话框，可保存 STL/3MF
- 切片软件列表正确检测已安装的软件
- 点击“发送到切片”按钮可启动切片软件并打开模型
- 切片软件路径可手动配置

**保护**: Phase 1-9 所有功能不受影响

---

### Phase 11: 配置与拖拽

**目标**: 实现 OpenSCAD 路径手动配置和拖拽文件打开。

**输入**: Phase 10 的配置基础设施

**关键文件**:
- `src/main.rs` — 添加 DroppedFile 事件处理
- `src/openscad.rs` — 从配置读取路径

**新增文件**:
- `src/config.rs` — 应用配置管理（JSON 读写）
- `src/ui/settings_dialog.rs` — 设置对话框
- `tests/config_tests.rs` — 配置序列化测试

**步骤**:
1. 实现配置管理模块：读写 `~/.config/scad-studio/config.json`
2. 配置项包含：OpenSCAD 路径、切片软件路径
3. 在菜单中添加“设置”入口，弹出 egui 窗口编辑配置
4. OpenSCAD 路径检测优先级：配置文件 > 环境变量 > 自动检测
5. 实现拖拽文件打开：处理 winit `WindowEvent::DroppedFile`

**验收标准**:
- 拖拽 .scad 文件到窗口可打开并渲染
- 设置对话框可修改 OpenSCAD 路径和切片软件路径
- 配置持久化到文件，重启后生效

**保护**: Phase 1-10 所有功能不受影响

---

### Phase 12: 日志面板集成与收尾

**目标**: 完善日志面板功能，对接所有错误/警告源，全局收尾与测试。

**输入**: Phase 1 日志面板 + 所有前序 Phase

**关键文件**:
- `src/ui/log_panel.rs` — 完善日志显示
- `src/main.rs` — 统一日志路由

**步骤**:
1. 统一所有错误源（OpenSCAD stderr、文件监控错误、渲染错误、参数解析警告）到日志缓冲区
2. 日志分级显示：Info（蓝）/ Warning（黄）/ Error（红）
3. 有 Error 级别日志时自动展开日志面板
4. 日志支持滚动、清除按钮
5. 全面回归测试，确保所有 Phase 功能协同工作
6. 更新 feature-roadmap.md，标记所有已完成项

**验收标准**:
- OpenSCAD 编译错误在日志面板中清晰显示
- 日志面板自动展开/手动折叠正常
- 所有 50+ 功能项均已实现
- `cargo test` 全部通过
- `cargo clippy` 无警告

**保护**: 所有前序 Phase 功能不受影响

---

## 关键技术决策

| 决策 | 方案 | 理由 |
|------|------|------|
| 线框模式 | `PolygonMode::Line` + GPU feature 检测 | wgpu 原生支持，运行时检测兼容性 |
| X-Ray 模式 | Fresnel + alpha blend 单 pass | 无需额外 pass，效果足够好 |
| 坐标轴指示器 | egui Painter 2D 绘制 | 不需要额外 wgpu pipeline |
| 参数解析 | 正则匹配 Customizer 格式 | 比完整语法解析器简单，覆盖主要用例 |
| 阴影 | 方向光 Shadow Map 1024x1024 + PCF | 平衡质量与性能 |
| 截面 | Stencil Buffer + discard | wgpu 原生支持 stencil |
| 配置存储 | `~/.config/scad-studio/config.json` | 跨平台标准位置 |
| renderer.rs 拆分 | 在 Phase 2 拆出 pipeline.rs | 避免超 500 行限制 |

## 新增依赖

| crate | Phase | 用途 |
|-------|-------|------|
| `regex` | 8 | 参数解析 |
| `serde` + `serde_json` | 9 | 预设文件序列化 |
| `zip` | 10 | 3MF 导出（ZIP 容器） |
| `dirs` | 10 | 用户目录检测 |

## 预估代码量

| Phase | 新增文件 | 预估行数 |
|-------|----------|----------|
| 1 | 5 个 .rs | ~400 行 |
| 2 | 2 个 .rs + 1 个 .wgsl | ~350 行 |
| 3 | 0 新文件 | ~100 行 |
| 4 | 2 个 .rs + 1 个 .wgsl + 2 个 test | ~400 行 |
| 5 | 2 个 .rs + 1 个 .wgsl + 1 个 test | ~450 行 |
| 6 | 0 新文件 | ~50 行 |
| 7 | 1 个 .rs + 1 个 .wgsl + 1 个 test | ~400 行 |
| 8 | 2 个 .rs + 1 个 test | ~450 行 |
| 9 | 1 个 .rs + 1 个 test | ~250 行 |
| 10 | 1 个 .rs + 1 个 test | ~300 行 |
| 11 | 2 个 .rs + 1 个 test | ~250 行 |
| 12 | 0 新文件 | ~100 行 |
| **总计** | ~26 个文件 | ~3500 行 |

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| `POLYGON_MODE_LINE` GPU 不支持 | 低 | 运行时检测，不支持时按钮置灰 |
| Shadow Map 性能影响 | 中 | 仅 1024x1024 分辨率，可开关 |
| Stencil Buffer 兼容性 | 低 | `Depth24PlusStencil8` 是 wgpu 广泛支持的格式 |
| renderer.rs 拆分复杂度 | 中 | Phase 2 优先处理，后续 Phase 在拆分后的架构上工作 |
| 参数解析正则覆盖率 | 中 | 参考 OpenSCAD Customizer 标准格式，覆盖主要用例 |
| 3MF 导出 OpenSCAD 支持 | 低 | OpenSCAD 原生支持 `--export-format 3mf` |
| 跨平台配置路径差异 | 低 | `dirs` crate 处理跨平台差异 |

## 执行顺序

Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9 → Phase 10 → Phase 11 → Phase 12

Phase 2/3/4 对 Phase 1 有依赖但彼此独立，实际上可以并行开发，但为避免冲突建议串行执行。
