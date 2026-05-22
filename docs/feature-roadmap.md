# scad-studio 功能路线图

状态说明：

- `[x]` 已完成
- `[ ]` 未开始
- `[~]` 进行中

---

## 核心管线

- [x] 调用 OpenSCAD CLI 将 .scad 转换为 STL
- [x] OpenSCAD 可执行文件路径自动检测（macOS / Linux / Windows）
- [x] OpenSCAD 可执行文件路径手动配置
- [x] STL 文件解析（二进制 + ASCII）
- [x] 3MF 文件解析（支持颜色信息）
- [x] CLI 进程管理（新请求到来时终止旧进程，防止堆积）
- [x] CLI 错误信息捕获与 UI 展示

## 文件管理

- [x] 文件对话框打开 .scad 文件
- [x] 文件变更监控（notify），外部修改后自动重新渲染
- [x] 变更去抖动（debounce），避免频繁调用 CLI
- [x] 切换文件时取消旧监控、注册新监控
- [x] 拖拽文件到窗口打开

## 3D 渲染

- [x] wgpu 渲染管线（顶点 + 法线 + 深度测试）
- [x] Blinn-Phong 光照着色器
- [x] 渲染模式：实体（Solid）
- [x] 渲染模式：线框（Wireframe）
- [x] 渲染模式：X 光 / 半透明（X-Ray）
- [x] 颜色开关（彩色 / 单色切换）

## 相机

- [x] 轨道相机：鼠标左键拖拽旋转
- [x] 轨道相机：滚轮缩放（带最小/最大距离限制）
- [x] 轨道相机：鼠标中键/右键平移
- [x] 加载新模型时自动 fit 到包围盒
- [x] 透视投影（Perspective）
- [x] 正交投影（Orthographic）
- [x] 透视/正交切换

## 环境与场景

- [x] 空白背景环境
- [x] 网格地面（Grid）
- [x] 打印平台（Build Plate，256mm²）
- [x] 坐标轴指示器（角落 XYZ 小部件）
- [x] 指数雾效果（Exponential Fog）

## 光照

- [x] 环境光（Ambient Light）
- [x] 方向光（Directional Light）
- [x] 聚光灯（Spotlight）
- [x] 点光源（Point Light）
- [x] 阴影渲染（Shadow Map）
- [x] 阴影开关

## 交互式截面（Cross-Section）

- [x] 切割平面渲染
- [x] 点击选中切割平面
- [x] 平移切割平面（W 键）
- [x] 旋转切割平面（E 键）
- [x] Ctrl 吸附网格（平移 1mm / 旋转 5°）
- [x] Stencil Buffer 截面渲染
- [x] 截面开关

## 参数编辑

- [x] 从 .scad 文件解析顶层变量声明
- [x] 数值类型：滑块控件（支持 min/step/max 注释）
- [x] 布尔类型：开关控件
- [x] 字符串类型：下拉选择控件
- [x] 参数分组（按 `/* [Group] */` 注释）
- [x] 隐藏参数支持（`/* [Hidden] */`）
- [x] 修改参数后实时调用 CLI 重新渲染（通过 `-D` 传参）
- [x] 参数覆盖标记（加粗显示）
- [x] 单个参数恢复默认值

## 预设系统

- [x] 读取 .scad.json 配套预设文件
- [x] 监控 .scad.json 文件变更并热加载
- [x] 加载预设
- [x] 保存预设
- [x] 删除预设

## 导出

- [x] 导出为 STL 文件
- [x] 导出为 3MF 文件
- [x] 一键发送到切片软件（PrusaSlicer / Bambu Studio / Cura 等）
- [x] 切片软件路径配置

## UI

- [x] 菜单栏（File > Open）
- [x] 状态栏（文件名、渲染状态）
- [x] 工具栏（渲染模式、环境、阴影等切换按钮）
- [x] 日志面板（CLI 输出、编译错误）
- [x] 窗口 resize 处理

补充说明：

- 旧原生 GUI 中的 macOS / Windows 平台菜单和 `winit` 接线已随 Rust 桌面端删除；当前生产 GUI 端为 Web。
- OpenSCAD 可执行文件当前由 app server 统一管理，支持自动检测、环境变量 `OPENSCAD_PATH` 覆盖，以及 Web 设置页中的手动路径配置。
- 预览链路现已改为优先输出并解析 3MF，保留 `mesh`、`basematerials`、`colorgroup` 与三角面级 `pid/p1/p2/p3` 的颜色语义。
- 当前 3MF 预览仍不支持 `texture2d`、`texture2dgroup`、`compositematerials` 等扩展资源；遇到这些类型会明确报错，不做静默降级。

---

## 最小可用版本（MVP）范围

以下功能属于 MVP，对应 plan-00 的 Phase 1-5：

- 核心管线：CLI 调用、STL 解析、CLI 进程管理、错误展示
- 文件管理：文件对话框、文件变更监控、去抖动、监控切换
- 3D 渲染：wgpu 管线、Blinn-Phong 光照、实体渲染模式
- 相机：旋转、缩放、平移、自动 fit 包围盒
- UI：菜单栏、状态栏、窗口 resize
