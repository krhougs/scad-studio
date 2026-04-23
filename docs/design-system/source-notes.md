# 设计系统来源与引用说明

本文件记录 `docs/design-system/` 下设计文档的外部参考来源、引用范围、许可状态与同步策略。

## 外部参考清单

以下文件是 Phase 4 启动前从外部 Buddin 仓库读取的设计参考：

| 外部路径 | 用途 | 引用到的项目内文件 |
|---------|------|------------------|
| `/Users/krhougs/LocalCodes/buddin/README.md` | 品牌定位、内容基调、视觉基础、调色板、字体、间距、动效、排版规则 | `docs/design-system/studio-datasheet-workbench.md` |
| `/Users/krhougs/LocalCodes/buddin/SKILL.md` | 硬性红线（无 emoji、sentence case、无渐变 CTA 等） | `docs/design-system/studio-datasheet-workbench.md` |
| `/Users/krhougs/LocalCodes/buddin/ui_kits/app/README.md` | 五区工作台列宽、chat 回执约束、blur 使用位置 | `docs/design-system/studio-datasheet-workbench.md` §五区工作台 |
| `/Users/krhougs/LocalCodes/buddin/ui_kits/app/index.html` | 五区 grid template、topbar / rail / chat / canvas / inspector 的边界与排版 | `packages/studio-web/src/styles/workbench.css`、`packages/studio-web/src/workbench/*.tsx` |
| `/Users/krhougs/LocalCodes/buddin/ui_kits/app/colors_and_type.css` | CSS token（颜色、字体、间距、阴影、动效） | `packages/studio-web/src/styles/tokens.css` |

## 抽取方式

引用遵循以下原则：

- **不整段照搬外部原文**：项目内文档以本项目术语（app-server、workspace、preview、watch、五区工作台）重写。
- **不复制图像或 svg 资源**：`studio-web` 中不使用 Buddin 的 logo、icon 包、图例 svg；外部参考中展示的 bracket / dimension 图仅作为视觉意图参考，不进仓库。
- **不使用外部 CDN 带来的 emoji / 渐变 CTA / 玻璃化滥用**：项目内 CSS 不引入任何 emoji 字符与渐变背景，blur 仅保留在 canvas 浮动工具栏（与 Buddin 约束一致）。
- **token 重命名**：外部 token 命名偏品牌语义（`--ink-9` / `--live` / `--cad`），项目内落到通用语义前缀（`--bg-page` / `--fg-primary` / `--accent-live` / `--accent-cad`），避免 Studio 代码依赖 Buddin 品牌用词。

## 许可与归属

- 外部 Buddin 仓库未在 README / SKILL.md 中声明独立 license。
- 项目内引用定位为“内部 Buddin 参考，未显式声明许可；引用仅作为项目内设计约束依据”。
- 若未来 Buddin 仓库正式发布 license 或改为外部分发资产，需重新审视本目录下文档的引用合规性。

## 同步策略

- 外部 Buddin 仓库的后续更新**不自动同步**；需由人工发起评审：先在 `prompt-archives/` 中建立对应日期编号的 plan，再回来更新 `docs/design-system/studio-datasheet-workbench.md` 与 `packages/studio-web/src/styles/tokens.css`。
- 本项目设计文档一旦正式落地，视为独立产物；后续 UI 调整应以 `docs/design-system/studio-datasheet-workbench.md` 为准，不再回跳外部绝对路径。
