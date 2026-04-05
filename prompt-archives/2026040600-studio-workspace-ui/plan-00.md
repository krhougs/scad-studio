# Plan-00: SCAD Studio Workspace UI 构建

## 背景

当前项目是一个单 crate 的 OpenSCAD 文件查看器（"SCAD Viewer"），约 14000 行 Rust 代码，基于 egui 0.33 + wgpu 27 + winit 0.30。目标是将其演化为完整的 "SCAD Studio" IDE，包含 Workspace 目录管理、多标签页工作区、Agent Chat 面板。

本计划将项目从单一 Viewer 二进制重构为多 crate workspace，并在此基础上构建 Studio 应用。

## 已确认的设计决策

| 决策项 | 结论 | 补充说明 |
|--------|------|----------|
| Viewer 定位 | 长期独立产品 | Viewer 和 Studio 作为两个独立可发布的应用 |
| Viewer UI 归属 | scad-viewer lib+bin 双模式 | Studio 依赖 scad-viewer 的 lib 部分复用 UI 组件 |
| 左侧面板 Tab 样式 | 水平图标 Tab | 面板顶部水平排列图标按钮，点击切换内容 |
| 工作区标签页交互 | 含拖拽排序 | 支持拖拽排序，不含 split view |
| 欢迎界面 | 居中卡片式 | Logo + 打开文件夹按钮 + 最近打开列表 |
| 配色方案 | 沿用 Viewer 深色主题 | BG_PANEL #0e0e0e, BG_WINDOW #141414, ACCENT #3764a0 |
| 日志面板 | 底部独立面板 | 类似 VS Code Terminal/Output 区域，可折叠 |

## 目标架构

### Crate 结构

```
scad-studio/                     # Workspace 根目录
├── Cargo.toml                   # Workspace 定义 + Studio 二进制 (根 crate)
├── src/                         # scad-studio 二进制源码
│   ├── main.rs                  # Studio 入口 + 事件循环
│   ├── app.rs                   # StudioApp 状态
│   ├── workspace.rs             # Workspace 目录管理
│   ├── layout.rs                # Studio 主布局编排
│   ├── left_panel.rs            # 左侧面板（tab 切换容器）
│   ├── work_area.rs             # 右侧标签页工作区
│   ├── viewer_tab.rs            # 3D Viewer 标签页
│   ├── markdown_tab.rs          # Markdown 标签页
│   ├── welcome.rs               # 欢迎页面
│   ├── log_panel.rs             # 底部日志面板
│   └── platform_menu.rs         # Studio 菜单栏
│
├── crates/
│   ├── scad-scene/              # 3D 场景渲染引擎（库）
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── renderer.rs      # wgpu 渲染器
│   │       ├── pipeline.rs      # 渲染管线
│   │       ├── mesh.rs          # 网格数据
│   │       ├── camera.rs        # 轨道相机
│   │       ├── lighting.rs, shadow.rs, grid.rs, gizmo.rs
│   │       ├── scene_bindings.rs, section.rs, cross_section.rs
│   │       ├── three_mf.rs      # 3MF 解析
│   │       ├── system_fonts.rs
│   │       └── shader*.wgsl
│   │
│   ├── scad-data/               # OpenSCAD 数据集成层（库）
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── openscad.rs      # CLI 运行器
│   │       ├── params.rs        # 参数解析
│   │       ├── presets.rs       # 预设管理
│   │       ├── document.rs      # 文档状态
│   │       ├── export.rs        # 导出逻辑
│   │       ├── config.rs        # 应用配置
│   │       └── watcher.rs       # 文件监控
│   │
│   ├── scad-ui/                 # 共享 UI 框架（库）
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── theme.rs         # 深色主题（沿用现有配色）
│   │       ├── tab_system.rs    # 标签页框架（trait + 管理器 + 拖拽排序）
│   │       ├── file_tree.rs     # 目录树组件
│   │       ├── chat_panel.rs    # Agent Chat UI 组件
│   │       ├── markdown.rs      # Markdown 渲染组件
│   │       └── widgets.rs       # 通用小组件
│   │
│   └── scad-viewer/             # 独立 Viewer 应用（lib+bin）
│       └── src/
│           ├── lib.rs           # 导出可复用的 Viewer UI 组件
│           ├── main.rs          # Viewer 独立入口
│           ├── app.rs           # ViewerApp 状态
│           ├── viewer_ui.rs     # Viewer 完整 UI 编排（供 lib 导出）
│           ├── toolbar.rs       # Viewer 工具栏
│           ├── side_panel.rs    # 参数/预设/导出面板
│           ├── log_panel.rs     # 日志面板
│           ├── camera_overlay.rs
│           ├── param_editor.rs
│           ├── settings_dialog.rs
│           ├── status_bar.rs
│           └── platform_menu.rs
```

### 依赖关系图（无循环依赖）

```
scad-scene (无 UI 依赖)
    ↑
scad-data (依赖 scene 的 MeshData 等类型)
    ↑
scad-ui (依赖 scene 的 Camera 类型用于 gizmo 等)
    ↑
scad-viewer [lib+bin] (依赖 scene + data + ui)
    ↑
scad-studio [bin] (依赖 scene + data + ui + viewer-lib)
```

### Studio UI 布局

```
┌──────────────────────────────────────────────────────────────────┐
│  菜单栏 (File | View | Help)                                     │
├───────────────┬──────────────────────────────────────────────────┤
│  左侧面板      │  工作区标签栏  [model.scad ×] [README.md ×]      │
│  ┌───────────┐│  (支持拖拽排序)                                   │
│  │[💬][📁]   ││──────────────────────────────────────────────────│
│  │ 水平图标Tab ││                                                 │
│  ├───────────┤│              标签页内容区域                        │
│  │           ││                                                   │
│  │  当前 Tab  ││   3D Viewer / Markdown / Welcome                 │
│  │  内容区域  ││                                                   │
│  │           ││                                                   │
│  │  (Chat 或  ││                                                   │
│  │   Files)  ││                                                   │
│  │           ││                                                   │
│  └───────────┘│                                                   │
├───────────────┴──────────────────────────────────────────────────┤
│  底部日志面板（可折叠）— OpenSCAD 输出、系统日志                     │
├──────────────────────────────────────────────────────────────────┤
│  状态栏                                                           │
└──────────────────────────────────────────────────────────────────┘
```

**左侧面板**：
- 顶部水平图标 Tab 栏：Chat 图标、Files 图标（后续可扩展更多图标）
- 切换图标只改变面板内容，不影响右侧工作区
- 面板宽度可拖拽调整，有最小宽度限制

**右侧工作区**：
- 顶部标签栏：点击切换、中键或 × 关闭、拖拽排序
- 无标签页时显示 WelcomeTab

**底部日志面板**：
- 可折叠/展开（默认折叠）
- 显示 OpenSCAD CLI 输出和系统日志
- 有错误时自动展开
- 清除按钮

**欢迎页面**（无 Workspace 时）：
- 居中卡片式布局
- "SCAD Studio" 标题
- "打开文件夹" 按钮
- 最近打开的 Workspace 列表（存储在 config 中）

---

## Phase 1: Workspace 重构 — 拆分 scad-scene crate

### 目标

将 3D 渲染引擎相关代码从根 crate 提取到 `crates/scad-scene/` 库 crate，建立 workspace 多 crate 基础。

### 输入

- 当前单 crate 代码（`src/` 目录下所有 .rs 和 .wgsl 文件）
- 现有 `Cargo.toml` 依赖列表

### 需要保护的目标与边界

- 根二进制 `scad-studio`（即当前 Viewer）必须仍能编译运行，功能完全不变
- 不修改任何业务逻辑，仅做文件移动和 `use` 路径调整
- 现有测试全部通过

### 操作步骤

1. 修改根 `Cargo.toml`，添加 `workspace.members = ["crates/scad-scene"]`
2. 创建 `crates/scad-scene/Cargo.toml`，声明为库 crate，将以下依赖从根 crate 移入：wgpu, glam, bytemuck, egui, egui-wgpu, fontdb, ttf-parser, stl_io, roxmltree, zip, log，以及平台相关的字体依赖（core-foundation, objc2-app-kit 等）
3. 将以下文件从 `src/` 移入 `crates/scad-scene/src/`：
   - renderer.rs, pipeline.rs, mesh.rs, camera.rs
   - lighting.rs, shadow.rs, grid.rs, gizmo.rs, scene_bindings.rs
   - section.rs, cross_section.rs
   - three_mf.rs, system_fonts.rs
   - shader.wgsl, shader_grid.wgsl, shader_section.wgsl, shader_shadow.wgsl, shader_xray.wgsl
4. 创建 `crates/scad-scene/src/lib.rs`，声明 public 模块并 re-export 关键类型
5. 根 `Cargo.toml` 添加 `scad-scene` 为 path 依赖，更新 `src/main.rs` 和所有引用方的 `use` 路径（`crate::renderer` → `scad_scene::renderer` 等）
6. 将依赖 scad-scene 内部类型的测试文件迁移到 `crates/scad-scene/tests/`
7. 运行 `cargo check --workspace` 和 `cargo test --workspace` 验证

### 验收标准

- `cargo check -p scad-scene` 通过
- `cargo check -p scad-studio` 通过（根 crate 当前仍叫 scad-studio）
- `cargo test --workspace` 全部通过
- 应用启动并正常渲染 3D 模型

---

## Phase 2: 拆分 scad-data crate

### 目标

将 OpenSCAD 数据集成层提取到 `crates/scad-data/` 库 crate。

### 输入

- Phase 1 完成后的 workspace 结构

### 需要保护的前序目标与边界

- Phase 1 的 scad-scene crate 结构和接口不变
- 根二进制功能完全不变

### 操作步骤

1. 创建 `crates/scad-data/Cargo.toml`，依赖：regex, serde, serde_json, notify, rfd, dirs, log, stl_io, scad-scene（path 引用，用于 mesh::MeshData 等共享类型）
2. 将以下文件移入 `crates/scad-data/src/`：
   - openscad.rs, params.rs, presets.rs, document.rs
   - export.rs, config.rs, watcher.rs
3. 创建 `crates/scad-data/src/lib.rs`，声明 public 模块
4. 根 crate 添加 `scad-data` path 依赖，更新 `use` 路径
5. 将依赖 scad-data 内部类型的测试文件迁移到 `crates/scad-data/tests/`
6. 运行 `cargo check --workspace` 和 `cargo test --workspace` 验证

### 验收标准

- `cargo check -p scad-data` 通过
- `cargo check -p scad-studio` 通过
- `cargo test --workspace` 全部通过
- 应用功能不变

---

## Phase 3: 拆分 scad-ui crate + scad-viewer 独立二进制

### 目标

1. 创建 `crates/scad-ui/` 共享 UI 框架 crate
2. 创建 `crates/scad-viewer/` 作为 lib+bin 双模式 crate，将当前 Viewer 的入口和 UI 代码完整迁移过去
3. 根 crate 清空为 Studio 占位，准备后续 Phase 填充

### 输入

- Phase 2 完成后的 workspace

### 需要保护的前序目标与边界

- scad-scene 和 scad-data 的 crate 结构和公共接口不变
- Viewer 所有功能在 `scad-viewer` 二进制中完整保留
- 现有测试全部通过

### 操作步骤

**3a: scad-ui crate**
1. 创建 `crates/scad-ui/Cargo.toml`，依赖：egui, log
2. 从当前根 crate 的 `src/ui/theme.rs` 移入 `crates/scad-ui/src/theme.rs`
3. 创建 `crates/scad-ui/src/lib.rs`，导出 theme 模块
4. 后续 Phase 会向此 crate 逐步添加 tab_system、file_tree、chat_panel、markdown 等组件

**3b: scad-viewer lib+bin crate**
1. 创建 `crates/scad-viewer/Cargo.toml`：
   - `[lib]` section 导出可复用组件
   - `[[bin]]` section 定义 `scad-viewer` 二进制
   - 依赖：scad-scene, scad-data, scad-ui, egui, egui-wgpu, egui-winit, winit, wgpu, glam, log, env_logger, muda, rfd，以及平台依赖
2. 将以下文件移入 `crates/scad-viewer/src/`：
   - main.rs（当前入口）, app.rs
   - ui/ 目录下的所有文件（toolbar, side_panel, log_panel, camera_overlay, param_editor, settings_dialog, status_bar, mod.rs）—— 注意 theme.rs 已移至 scad-ui
   - platform_menu.rs
   - bin/font_probe.rs（作为 scad-viewer 的额外 binary 或 example）
3. 创建 `crates/scad-viewer/src/lib.rs`，导出可复用模块：
   - `pub mod ui` — 导出 toolbar、param_editor、side_panel、log_panel、camera_overlay 等
   - `pub mod app` — 导出 ViewerState、UiActions 等类型
4. 更新所有 `use` 路径：`crate::renderer` → `scad_scene::renderer` 等；`crate::ui::theme` → `scad_ui::theme` 等

**3c: 根 crate 清空为 Studio 占位**
1. 根 `src/` 清空，仅保留占位 `src/main.rs`（简单窗口 + "SCAD Studio" 文字）
2. 根 `Cargo.toml`：
   - name 保持 `scad-studio`
   - workspace.members 更新为 `["crates/scad-scene", "crates/scad-data", "crates/scad-ui", "crates/scad-viewer"]`
   - 依赖仅保留 Studio 需要的最小集（egui, winit, wgpu, log, env_logger）+ scad-scene, scad-data, scad-ui, scad-viewer
3. `default-run` 改为 `scad-studio`

### 验收标准

- `cargo build -p scad-viewer` 生成可执行文件，启动后功能与重构前完全一致
- `cargo run -p scad-viewer` 可正常打开和渲染 .scad 文件
- `cargo build -p scad-studio` 生成占位 Studio 窗口
- `cargo test --workspace` 全部通过
- `cargo clippy --workspace` 无错误

---

## Phase 4: Studio 应用骨架与 Workspace 机制

### 目标

实现 Studio 应用的基础框架：窗口创建、事件循环、Workspace 打开机制、欢迎页面、主布局骨架。

### 输入

- Phase 3 完成后的 workspace，根 crate 为 Studio 占位

### 需要保护的前序目标与边界

- scad-viewer 二进制功能不受影响
- scad-scene / scad-data / scad-ui 公共接口不变

### 操作步骤

1. 在根 `src/main.rs` 中实现 Studio 事件循环（参考 scad-viewer 的 winit ApplicationHandler 模式，但使用 Studio 自己的状态管理）
2. 创建 `src/app.rs`（StudioApp），管理：
   - `workspace_path: Option<PathBuf>` — 当前打开的 Workspace 路径
   - `left_panel_tab: LeftPanelTab` — 枚举：Chat / Files
   - `left_panel_width: f32` — 左侧面板宽度
   - `log_panel_open: bool` — 底部日志面板展开状态
   - `recent_workspaces: Vec<PathBuf>` — 最近打开的 Workspace 列表
3. 创建 `src/welcome.rs`，实现欢迎页面：
   - 居中卡片布局："SCAD Studio" 标题
   - "打开文件夹" 按钮（调用 `rfd::FileDialog::pick_folder`）
   - 最近打开的 Workspace 列表（从 config 读取），点击直接打开
4. 创建 `src/layout.rs`，用 egui 编排主布局：
   - `egui::SidePanel::left("left_panel")` — 左侧面板（可拖拽调整宽度）
   - 左侧面板顶部：水平图标 Tab 栏（Chat 图标、Files 图标）
   - `egui::TopBottomPanel::bottom("log_panel")` — 底部日志面板（可折叠）
   - `egui::TopBottomPanel::bottom("status_bar")` — 状态栏
   - `egui::TopBottomPanel::top("tab_bar")` —  工作区标签栏（占位）
   - `egui::CentralPanel` — 标签页内容区域（此阶段显示欢迎页或占位）
5. 创建 `src/left_panel.rs`，渲染左侧面板：
   - 图标 Tab 栏渲染（选中高亮）
   - 根据当前 tab 显示对应内容（此阶段为占位文字）
6. 创建 `src/log_panel.rs`，实现底部日志面板（参考 scad-viewer 的 log_panel 但适配 Studio 布局）
7. 创建 `src/platform_menu.rs`，实现 Studio 菜单栏：
   - File: Open Folder / Recent Workspaces / Quit
   - View: 左侧面板显示/隐藏、日志面板显示/隐藏
   - Help: About
8. 窗口标题根据状态动态更新：
   - 无 workspace 时："SCAD Studio"
   - 有 workspace 时："SCAD Studio — [workspace_name]"
9. 将最近打开的 Workspace 列表持久化到 config（复用 scad-data 的 AppConfig 或 Studio 独立 config）

### 验收标准

- `cargo run -p scad-studio` 启动后显示 Studio 窗口和欢迎页面
- 可通过菜单或欢迎页按钮打开文件夹
- 打开 Workspace 后，左侧面板可见（Chat / Files 图标 Tab 可切换，内容为占位）
- 底部日志面板可折叠/展开
- 状态栏显示 workspace 路径
- 窗口标题正确更新
- 最近打开列表持久化并在欢迎页面正确显示

---

## Phase 5: Tab 系统框架（含拖拽排序）

### 目标

在 scad-ui 中实现可扩展的标签页框架（含拖拽排序），Studio 集成标签栏和标签页管理。

### 输入

- Phase 4 的 Studio 骨架

### 需要保护的前序目标与边界

- Phase 4 的 Studio 布局结构不变
- scad-viewer 不受影响

### 操作步骤

1. 在 `crates/scad-ui/src/tab_system.rs` 中定义核心接口：
   ```rust
   pub type TabId = u64;

   pub trait WorkTab {
       fn id(&self) -> TabId;
       fn title(&self) -> &str;
       fn is_closable(&self) -> bool;
       fn show(&mut self, ui: &mut egui::Ui, ctx: &mut TabContext);
   }

   pub struct TabContext {
       // 传递给 Tab 内容的上下文（wgpu device/queue 等）
   }

   pub struct TabManager { ... }
   ```
2. `TabManager` 实现：
   - `open_tab(tab: Box<dyn WorkTab>)` — 打开新标签页；若同 ID 已存在则切换到该标签，不重复创建
   - `close_tab(id: TabId)` — 关闭标签页，自动切换到相邻标签
   - `set_active(id: TabId)` — 切换激活标签
   - `show_tab_bar(&mut self, ui: &mut egui::Ui)` — 渲染标签栏 UI
   - `show_active_content(&mut self, ui: &mut egui::Ui, ctx: &mut TabContext)` — 渲染当前激活标签内容
3. 标签栏视觉设计：
   - 激活标签与内容区域背景色连续（Chrome 风格无底边框）
   - 非激活标签背景稍暗
   - 关闭按钮 × 在 hover 时显示
   - 标签栏沿用 Viewer 深色主题配色
4. **拖拽排序实现**：
   - 利用 egui 的 drag/drop API（`egui::Sense::drag`）
   - 拖拽开始时记录源标签索引
   - 拖拽过程中在目标位置显示插入指示器（竖线）
   - 释放时重排 tabs 数组
5. 在 Studio 的 `src/work_area.rs` 中集成 `TabManager`，替换 Phase 4 的占位
6. 创建 `src/welcome.rs` 中的 WelcomeTab（实现 `WorkTab`，不可关闭），在无其他标签页时显示

### 验收标准

- 标签栏可显示多个标签，点击切换
- 可关闭标签页（WelcomeTab 除外），关闭后自动切换到相邻标签
- 拖拽排序标签页顺序正常工作，有视觉指示器
- `open_tab` 对已存在的 tab ID 不重复创建，直接切换
- 标签栏视觉风格与深色主题一致
- `TabManager` 有完整的单元测试（open/close/reorder 逻辑）

---

## Phase 6: 文件树组件

### 目标

在 scad-ui 中实现目录树组件，Studio 左侧面板 Files tab 中集成文件树，双击文件触发打开标签页。

### 输入

- Phase 4 的 Workspace 机制 + Phase 5 的 Tab 系统

### 需要保护的前序目标与边界

- Phase 5 的 Tab 系统接口不变
- Phase 4 的布局结构不变
- scad-viewer 不受影响

### 操作步骤

1. 在 `crates/scad-ui/src/file_tree.rs` 中实现：
   ```rust
   pub struct FileTree {
       root: PathBuf,
       expanded: HashSet<PathBuf>,
       selected: Option<PathBuf>,
       children_cache: HashMap<PathBuf, Vec<DirEntry>>,
   }
   ```
2. `FileTree::show(&mut self, ui: &mut egui::Ui) -> Option<FileTreeAction>` 递归渲染目录结构：
   - 目录节点：▶/▼ 展开折叠图标，点击展开/折叠
   - 文件节点：单击选中，双击返回 `FileTreeAction::OpenFile(PathBuf)`
   - 按文件类型区分显示（.scad 文件、.md 文件、其他文件用不同颜色或前缀标记）
   - 目录排在文件前面，各自按字母排序
3. **懒加载**：首次展开目录时读取子目录内容并缓存到 `children_cache`，避免大目录初始化卡顿
4. 文件树通过 scad-data 的 `FileWatcher` 监控 workspace 目录变更：
   - 监控到变更时清除受影响目录的 cache，下次渲染时重新读取
5. 在 Studio 的 `src/left_panel.rs` 中，Files tab 显示 `FileTree`：
   - 将 `FileTreeAction::OpenFile` 事件传递给 StudioApp
   - StudioApp 根据文件扩展名调用 `TabManager::open_tab` 打开对应类型标签页：
     - `.scad` / `.stl` / `.3mf` → ViewerTab（Phase 7 实现，此阶段为占位 Tab）
     - `.md` → MarkdownTab（Phase 8 实现，此阶段为占位 Tab）
     - 其他 → 暂不处理或显示"不支持的文件类型"

### 验收标准

- 打开 workspace 后，左侧 Files tab 显示完整目录树
- 目录可展开/折叠，层级缩进正确
- 文件可单击选中（高亮），双击触发打开事件
- 双击文件后在工作区出现新标签页（此阶段内容为占位）
- 已打开文件再次双击不重复创建标签页，而是切换到已有标签
- 文件系统变更后目录树自动更新
- 懒加载生效：仅展开的目录才读取内容

---

## Phase 7: Viewer Tab — 集成 3D 模型查看器

### 目标

实现 ViewerTab，复用 scad-viewer lib 的 UI 组件，将 3D 渲染器嵌入 Studio 标签页中。

### 输入

- Phase 5 的 Tab 系统 + Phase 6 的文件树

### 需要保护的前序目标与边界

- scad-viewer 独立二进制不受影响
- Tab 系统接口不变
- 文件树功能不受影响

### 操作步骤

1. 创建 `src/viewer_tab.rs`，实现 `WorkTab` trait：
   - `ViewerTab` 持有：OrbitalCamera、DocumentState、ViewerState（来自 scad-viewer lib 的类型）
   - 使用 scad-viewer lib 导出的 UI 组件：精简版工具栏（渲染模式、颜色、投影切换，不含文件打开/导出），参数面板
2. 3D 渲染集成方案：
   - Studio 主循环持有 wgpu Device/Queue（与 egui_wgpu::Renderer 共享）
   - ViewerTab 通过 `TabContext` 获取 Device/Queue 引用
   - 使用 `egui::PaintCallback` 在标签页内容区域渲染 wgpu 3D 场景（与现有 Viewer 方式一致）
3. 打开流程：文件树双击 .scad → 创建 ViewerTab → 调用 OpenScadRunner 生成预览 → 加载 mesh → 渲染
4. 每个 ViewerTab 独立状态：
   - 独立的 OrbitalCamera（切换标签时保留各自相机视角）
   - 独立的 ViewerState（渲染模式等设置）
   - 独立的 DocumentState（参数值）
5. 文件变更监控：workspace 级 FileWatcher 检测到 .scad 文件变更时，通知对应 ViewerTab 重新渲染
6. ViewerTab 内嵌工具栏精简：去除文件打开、导出等操作（由 Studio 菜单承担），保留渲染控制

### 验收标准

- 在文件树中双击 .scad 文件，工作区打开 Viewer Tab 并显示 3D 模型预览
- 相机操作（旋转、平移、缩放）在标签页内正常工作
- 渲染模式切换（Solid / Wireframe / X-Ray）正常
- 多个 Viewer Tab 可同时打开，各自独立的相机和渲染状态
- 切换标签时渲染状态保持
- .scad 文件外部修改后自动更新对应 Viewer Tab 的预览

---

## Phase 8: Markdown Tab

### 目标

实现 MarkdownTab，支持在标签页中渲染 Markdown 文件。

### 输入

- Phase 5 的 Tab 系统

### 需要保护的前序目标与边界

- Phase 7 的 Viewer Tab 不受影响
- Tab 系统接口不变

### 操作步骤

1. 在根 `Cargo.toml` 和 `crates/scad-ui/Cargo.toml` 添加 Markdown 解析依赖（`pulldown-cmark`）
2. 在 `crates/scad-ui/src/markdown.rs` 中实现 Markdown 渲染组件：
   - 解析 Markdown 为事件流（pulldown-cmark）
   - 用 egui 原生组件渲染：
     - 标题（h1-h6 不同字号和粗细）
     - 段落（正常文本）
     - 代码块（等宽字体 + 深色背景区域）
     - 行内代码（等宽字体 + 背景色）
     - 列表（有序/无序，缩进 + 项目符号）
     - 链接（蓝色文字，暂不支持点击跳转）
     - 粗体、斜体
   - 使用 egui::ScrollArea 支持滚动浏览
3. 创建 `src/markdown_tab.rs`，实现 `WorkTab` trait：
   - 打开时读取文件内容 → 解析 Markdown → 缓存渲染数据
   - 文件变更时重新读取和解析
   - 标签标题为文件名
4. 从文件树双击 .md 文件 → 创建 MarkdownTab → 渲染

### 验收标准

- 双击 .md 文件在工作区打开 Markdown Tab
- 标题、段落、代码块、列表等基本 Markdown 元素正确渲染
- 长文档支持滚动
- 文件修改后内容自动更新
- 视觉风格与深色主题一致（代码块背景略深、标题有明显层级区分）

---

## Phase 9: Agent Chat UI

### 目标

在左侧面板 Chat tab 中实现 Agent Chat 的 UI 布局和交互，不对接后端。

### 输入

- Phase 4 的左侧面板框架

### 需要保护的前序目标与边界

- 所有前序 Phase 功能不受影响
- 文件树功能不受影响（Chat 和 Files 是左侧面板的两个独立 tab）

### 操作步骤

1. 在 `crates/scad-ui/src/chat_panel.rs` 中实现 Chat UI 组件：
   ```rust
   pub struct ChatPanel {
       messages: Vec<ChatMessage>,
       input_text: String,
       scroll_to_bottom: bool,
   }

   pub struct ChatMessage {
       pub role: MessageRole,   // User / Assistant
       pub content: String,
       pub timestamp: String,
   }

   pub enum MessageRole { User, Assistant }
   ```
2. UI 布局（从上到下）：
   - **顶部**：标题 "Agent Chat"（紧凑，不占用过多空间）
   - **中部**：消息列表（egui::ScrollArea），新消息自动滚动到底部
     - User 消息：右对齐，深色背景气泡（ACCENT 色调）
     - Assistant 消息：左对齐，浅灰背景气泡
     - 气泡内支持多行文本
     - 消息间有适当间距
   - **底部**：输入区域
     - 多行文本输入框（`egui::TextEdit::multiline`）
     - Shift+Enter 换行，Enter 发送
     - 发送按钮（图标或文字），输入为空时置灰
3. 预置模拟数据用于 UI 调试：
   - 3-5 条示例对话展示排版效果
   - 发送消息时将用户输入添加到消息列表，自动生成占位 Assistant 回复（如"[Agent 功能开发中...]"）
4. 在 Studio 的 `src/left_panel.rs` 中，Chat tab 显示 `ChatPanel`
5. `ChatPanel` 预留回调接口，供后续对接真实 Agent 后端：
   ```rust
   pub enum ChatAction {
       SendMessage(String),
   }
   ```

### 验收标准

- 左侧面板 Chat tab 显示完整的聊天界面
- 消息气泡左右对齐正确，User 消息右侧，Assistant 消息左侧
- 样式与深色主题一致（气泡颜色协调，文字清晰可读）
- 输入框支持多行输入，Enter 发送，Shift+Enter 换行
- 消息列表支持滚动，新消息自动滚到底部
- 发送消息后出现在消息列表中，并有占位的 Assistant 回复
- 无后端对接，仅 UI 层面完成

---

## 关键技术决策

| 决策 | 方案 | 理由 |
|------|------|------|
| scad-viewer 模式 | lib+bin 双模式 crate | Viewer 为长期独立产品，lib 部分供 Studio 复用 UI 组件 |
| 3D 渲染嵌入标签页 | `egui::PaintCallback` + 共享 wgpu Device/Queue | 与现有 Viewer 渲染方式一致，无需额外 Surface 管理 |
| Tab 系统 | trait object (`Box<dyn WorkTab>`) + 拖拽排序 | 扩展新 tab 类型只需实现 trait；egui drag API 实现排序 |
| 文件树懒加载 | 展开目录时按需读取 + HashMap 缓存 | 避免大 workspace 初始化卡顿 |
| Markdown 渲染 | pulldown-cmark + egui 原生组件 | 纯 Rust 依赖，无需 WebView；egui 渲染保持一致视觉风格 |
| Chat UI 布局 | 气泡式对话 + 底部输入框 | 符合常见 Chat 交互直觉 |
| 配色 | 沿用 Viewer 深色主题 | 产品视觉一致性 |
| 左侧面板 Tab | 水平图标按钮 | 紧凑、可扩展，不占用过多垂直空间 |

## 新增依赖

| crate | Phase | 用途 |
|-------|-------|------|
| pulldown-cmark | 8 | Markdown 解析 |

其余依赖已存在于项目中，仅在 crate 间重新分配。

## 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| scad-scene crate 公共接口设计 | 高 | Phase 1 仔细设计暴露面，优先暴露类型而非实现细节 |
| scad-viewer lib 导出的 UI 组件在 Studio 中复用时的上下文差异 | 高 | Viewer UI 组件通过参数化接收上下文，不硬编码布局假设 |
| 多 Viewer Tab 共享 wgpu Device 的资源管理 | 中 | Studio 主循环单线程渲染，Tab 切换时串行渲染当前激活 Tab |
| egui PaintCallback 在 Tab 切换时的 viewport 管理 | 中 | 每次渲染前根据 CentralPanel 的 available_rect 更新 viewport |
| 文件树性能（大目录 >1000 文件） | 中 | 懒加载 + 缓存 + 仅展开节点读取内容 |
| crate 拆分导致编译时间增加 | 低 | workspace 级增量编译；库 crate 稳定后很少重新编译 |
| 拖拽排序在 egui 中的交互体验 | 中 | egui 内置 drag/drop 支持；需要仔细调整视觉反馈（插入指示器） |

## 执行顺序

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9
```

- Phase 1-3 是基础重构，必须严格串行（每步依赖前一步的 crate 结构）
- Phase 4-5 建立 Studio 骨架和 Tab 系统，必须串行
- Phase 6 依赖 Phase 4（Workspace） + Phase 5（Tab 系统）
- Phase 7 依赖 Phase 5（Tab 系统） + Phase 6（文件树触发打开）
- Phase 8 依赖 Phase 5（Tab 系统），可与 Phase 7 并行但建议串行避免冲突
- Phase 9 依赖 Phase 4（左侧面板），可与 Phase 6-8 并行但建议串行
