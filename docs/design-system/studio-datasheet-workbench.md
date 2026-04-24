# budn' Datasheet Workbench 设计规范

本文件是 `packages/studio-web` React PWA 的设计系统总纲。用户可见产品名固定为 `budn'`，代码和配置标识符使用 `budn`。本文覆盖视觉、排版、交互、五区工作台布局和禁用项。执行 UI 改动前必须通读本文件；token 实际值在 `packages/studio-web/src/styles/tokens.css`，布局在 `packages/studio-web/src/styles/workbench.css`。

外部参考来源与引用规则见 `docs/design-system/source-notes.md`。

## 一、定位与声音

Studio web 的视觉定位是**工业技术数据手册（technical datasheet）**：冷静、精确、工整；可以像工程图纸一样折好放进档案盒。

基调要求：

- 专业而不花哨；直接而不套话；具体而不抽象。
- 友好来自于“表达清楚”，而不是“显得可爱”。
- 文案大小写一律使用 sentence case；按钮、菜单、标签、标题同理。唯一例外是 11 px 以下的 mono 元数据标签，允许全大写 + `+0.08em` tracking（如 `WORKSPACE`、`PREVIEW`、`§1 · STATUS`）。

## 二、色板与语义

**暗色单色基调（dark-only, monochrome-first）**。没有浅色面板，没有亮色品牌主色，没有彩色 CTA。

### 2.1 面与文字语义 token

| 语义 | token | 用途 |
|------|-------|-----|
| `--bg-page` | 页面外层背景；五区工作台 grid 外的“纸面” |
| `--bg-surface` | 主要内容区背景（rail / chat / inspector 默认面） |
| `--bg-surface-raised` | 激活行、列表 hover、小面板抬起态 |
| `--bg-canvas-well` | canvas（预览区）更深的背景 |
| `--fg-primary` | 主文字、按钮主色填充 |
| `--fg-body` | 正文默认文字 |
| `--fg-muted` | 次级文字、说明 |
| `--fg-subtle` | 元数据、标签、提示 |
| `--fg-dim` | 禁用、最弱文字 |
| `--border-hairline` | 1 px 边框（**所有分隔与边界都走这个**） |
| `--border-strong` | 强分隔线（很少用，用于突出当前选项） |

### 2.2 口音色（极少使用）

仅两个口音色，各自承担**单一职责**，**永远不用在 CTA 上**：

- `--accent-live`（暖铁锈红）：表示“在线 / 活跃 / 正在工作”——watch 活跃、预览正在进行、连接 online 的小圆点。
- `--accent-cad`（冷蓝灰）：仅用于 CAD 类标注（尺寸线、引线、坐标）。UI chrome（按钮、输入、导航）禁止使用。

### 2.3 信号色

仅用于状态，不做装饰：

- `--signal-ok`：成功、在量程内、构建通过
- `--signal-warn`：接近上限
- `--signal-err`：失败、错误、超量程

### 2.4 禁用项

- 不用浅色/米色/骨色背景。
- 不用 lavender、pastel、紫色、安全橙、SaaS 蓝紫渐变。
- 不用霓虹色除非确实是“材料样色”。
- 不用彩色 CTA；主按钮是 `--fg-primary` 填充在 `--bg-page` 上。

## 三、字体规则

只有两个字体家族——sans 与 mono，绝不引入衬线体。

- 正文 / UI：`Geist`，加载 weight 400 / 500 / 600。
- 数字 / 元数据 / 代码：`Geist Mono`，加载 weight 400 / 500。
- 不用斜体正文；斜体仅保留给极少数展示排版里的次级子句。
- Hero / 大标题使用 weight 400（轻），视觉重量来自尺寸与 tracking，而不是粗体。

### 字号阶梯

| token | px | 用途 |
|-------|----|-----|
| `--fs-xxs` | 10 | 标题块字段、极小 mono 元数据 |
| `--fs-xs`  | 11 | `.label` —— 全大写 mono，`+0.08em` tracking |
| `--fs-sm`  | 13 | 次级 UI、导航、行内元数据 |
| `--fs-base`| 15 | 默认正文 |
| `--fs-lg`  | 18 | 引子段落、大正文 |
| `--fs-xl`  | 24 | H3 |
| `--fs-2xl` | 34 | H2 |
| `--fs-3xl` | 48 | H1 |

### Tracking

- H1 / H2：`-0.03em`
- H3 / H4：`-0.02em`
- 正文：`-0.015em`
- 全大写 mono 元数据：`+0.08em`

### Fallback 栈

CSS 中直接写：

```css
--font-body: "Geist", ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, sans-serif;
--font-mono: "Geist Mono", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
```

Geist 通过 `packages/studio-web/index.html` 的 Google Fonts CDN 加载；离线打包方案延后到 Phase 5+ 讨论。

## 四、间距与栅格

- 4 px 基线：`--space-1` = 4, `--space-2` = 8, `--space-3` = 12, `--space-4` = 16, `--space-5` = 20, `--space-6` = 24, `--space-8` = 32, `--space-10` = 40。
- 五区工作台不走内容居中栏（不套 `--content-max: 1280px` 这种尺寸），整屏铺满，用 1 px hairline 分隔每区。
- 工作台内部 section 内 padding 常用 `--space-4` / `--space-5`；chat 气泡、参数行、inspector section 使用 `--space-6` 做底部留白。

## 五、边框与分隔

- **所有圆角为零**：按钮、输入、tab、面板、卡片、图像框。唯一的圆是 live 状态小圆点（6 px）。
- 默认边框：`1px solid var(--border-hairline)`。不能用更粗的边框做强调，强调请改用 `--border-strong` 或加 `§` mono 前缀。
- 不用阴影做层级；阴影只留给：
  - `--shadow-low`：1 px 下降影，几乎不可见，用作 topbar 下方边线的轻微抬起效果（本项目默认不启用）。
  - 模态遮罩 / 弹窗悬浮才允许引入更强阴影。
- 禁止使用彩色光晕 / halo / glow。

## 六、五区工作台布局

> 参考：`/Users/krhougs/LocalCodes/buddin/ui_kits/app/index.html`。

工作台使用 CSS Grid，列宽与区域严格对齐：

```
grid-template-columns: 52px 360px 1fr 320px;
grid-template-rows: 44px 1fr;
grid-template-areas:
  "topbar topbar   topbar  topbar"
  "rail   chat     canvas  inspector";
```

分区职责：

- **Topbar（44 px 高）**：logo / 产品名 / workspace breadcrumb / 连接状态 / 主操作按钮。背景 `--bg-page`，底部 1 px hairline。元数据用 `--font-mono` + `--fs-xxs` + `+0.08em`。不在 topbar 上放业务动作按钮——业务入口在 canvas 底部或 inspector 里。
- **Rail（52 px 宽）**：纯 icon 导航列。按钮 40 px 高，默认 `--fg-subtle`，hover `--fg-body`，active `--fg-primary` 并带 2 px 左侧边线（`--fg-primary`）。每个按钮必须带 `aria-label`；icon-only 控件必须有 tooltip。
- **Chat（360 px 宽）**：会话区。用户消息以左侧 2 px hairline 分隔的纯文本呈现；agent 的操作输出必须以“receipt 卡片”形式（mono 字体、具体数值），而不是“好的我做了”之类的口语确认。底部输入框用 1 px 边框；focus 时边框变 `--fg-primary`。
- **Canvas（flex 1）**：预览主区，背景 `--bg-canvas-well`。允许的浮动元素：上边的视图切换 pills、右上角的 canvas 信息小卡、底部的 part-meta + 主操作按钮。浮动元素使用 `rgba(...,0.78)` + `backdrop-filter: blur(18px)` —— **全站唯一允许使用 blur 的地方**。
- **Inspector（320 px 宽）**：右侧详情面板。顶部有 `§` 大写 mono kicker + 选中对象标题。下方按 section 分组（Features / Parameters / Material / Build / Preview 状态等），每个 section 以 1 px top rule 分隔。参数行遵循 “label + value + unit” 三列结构，unit 用 mono 小字。

### 可折叠策略（后续）

- 默认 rail / chat / inspector 均展开；Phase 4 交付不做响应式折叠。
- 移动端适配与区块折叠推到 Phase 7+，届时需新增 `docs/design-system/` 内对应补充文件。

## 七、组件规则

### 7.0 图标

- Web 端统一使用 `@phosphor-icons/react`。
- Rail 侧边栏图标使用 `weight="bold"`，尺寸保持 18 px。
- 图标组件必须通过语义化 React 组件导入，不再新增 `lucide-react` 使用点。
- icon-only 控件必须有 `aria-label`；必要时保留 title 或 tooltip。

### 7.1 按钮

- 高度阶梯：32 px 默认，26 px `sm`，40 px `lg`。
- `btn-solid`（主操作）：`--fg-primary` 背景、`--bg-page` 文字，1 px `--fg-primary` 边框。hover 背景变纯白。active 有 0.5 px 向下位移，**没有阴影变化**。
- `btn-line`（次操作）：透明背景，1 px `--border-hairline`，文字 `--fg-body`。hover 边框变 `--border-strong`，文字变 `--fg-primary`。
- `btn-ghost`：透明、无边框，仅文字色 `--fg-subtle` → hover `--fg-primary`。
- 不加渐变、不加彩色背景、不加 pill 圆角。

### 7.2 输入

- 高度 32 px；1 px `--border-hairline`；focus 时边框变 `--fg-primary`；无 box-shadow、无 glow。
- placeholder 用 `--fg-subtle`。
- textarea 放在一个 1 px 边框的外壳里（wrap），内部无边框；focus-within 触发外壳边框变色。

### 7.3 列表与树

- 行高 28–32 px；行间用 1 px hairline 分隔；active 行背景 `--bg-surface-raised`，左侧图标换 `--accent-live`。
- 目录条目右侧挂一个 mono 小字尺寸 / 计数（`--fs-xxs` + `+0.08em`）。

### 7.4 标签（chip）

- 高度 22 px；mono 全大写；1 px 边框与文字色匹配（通过 `color-mix` 与基色做 42 % 透明度 fallback）。
- 语义变体：`--accent-live`（活跃）、`--signal-ok`、`--signal-warn`、`--signal-err`、`--accent-cad`。

### 7.5 面板

- 面板 = 1 px 边框 + 零圆角 + `--bg-surface` 背景 + 无阴影。
- 面板左上角放 mono 大写 kicker（`§1 · NAME` 形式），右上角放轻操作按钮。
- 面板之间不留 gap；用 1 px hairline 相邻分隔。

## 八、动效与反馈

- 缓动：`cubic-bezier(0.2, 0.6, 0.15, 1)`。
- `--motion-fast`：120 ms（hover / focus / 边框色切换）。
- `--motion-base`：200 ms（菜单 / 抽屉 / side panel）。
- `--motion-slow`：400 ms（canvas 相机动画，放在 canvas 控制器内使用）。
- 悬停不做放大、不做旋转；只变边框色与文字色。
- focus-visible：1 px `--fg-primary` 实线，`outline-offset: 2px`，**不加模糊、不加光晕**。
- loading 使用 mono 计数或细进度线，不用旋转 spinner。

## 九、禁用项（硬性红线）

- 不用 emoji（任何文案、标签、图标都不能包含）。
- 不用彩色 / 渐变 CTA。
- 不滥用玻璃化（backdrop-blur 仅限 canvas 浮动工具栏）。
- 不用阴影做视觉分层。
- 不把 CSS 字符串拼进 React 组件（这是 Phase 2 已清理的 anti-pattern）。
- 不在设计系统路径 (`.claude/skills/` / `agents/`) 下新增文件。

## 十、跨端共享评估结论

本节是 Phase 4 强制交付的书面结论（plan-00.md §Phase 4 步骤 3）。

**结论：不把任何 token / 组件放入 `scad-ui`。**

理由：

1. `scad-ui` 是 egui-based desktop UI 共享层，自己的视觉栈基于 `egui::Style` 与 `eframe` 的 painter；不消费 CSS 自定义属性。把 `--bg-page` / `--fg-primary` 之类的 CSS token 抽到 Rust 常量，desktop 不会真的用这些字符串去构造颜色，反而引入死代码与双向漂移的维护面。
2. Buddin 设计系统的核心表达（1 px hairline、零圆角、mono 大写元数据、blur 仅限 canvas 浮动工具栏）直接依赖 DOM / CSS 渲染语义：`backdrop-filter`、`text-transform`、`letter-spacing`、`color-mix(in oklab, ...)`；egui 的等价实现需要另写一遍，共享不了。
3. 字体 fallback（`"Geist", system-ui, ...`）理论上可以做成 `scad-ui::GEIST_FAMILY_FALLBACK` 字符串常量，但 `scad-ui` 当前（`crates/scad-ui/src/lib.rs`）并没有可接入的字体配置出口；为了本次 Phase 4 就在 `scad-ui` 增设一个空的挂点，违反 AGENTS.md “只写解决问题所需的最少代码”。保留为后续工作建议即可。
4. toolbar / statusbar / panel 等命名约定在 egui 端有自己的约定（`work_area_frame`、`panel_switcher`、`widgets`），语义不等价；强行统一命名会让 desktop 端现有代码失去语义，收益为零。

**未来若 desktop 端要统一视觉表达**，流程如下：

1. 在 `crates/scad-ui/` 下先为字体家族、口音色、spacing 基线增加共享常量 / 挂点（需要修改 `studio-app`、`scad-ui` 的公共 API）。
2. 修订本文件与 `packages/studio-web/src/styles/tokens.css`，把字体 / 口音色 / spacing 基线从“web CSS 自定义属性”升级为“跨端语义常量 + web CSS 引用”。
3. 新开 `prompt-archives/` 计划并独立 review 通过后再合入；不在本 Phase 隐式推进。

作为折中保障：`studio-datasheet-workbench.md` 与 `tokens.css` 的字段命名使用通用语义（`bg-page` / `fg-primary` / `accent-live`），不绑定“Buddin 品牌专有词”，降低未来跨端对齐时的文字改名成本。
