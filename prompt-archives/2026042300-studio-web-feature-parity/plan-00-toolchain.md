# Phase 0 契约 · 工具链策略

本文件固定本计划执行期间的 JS 工具链与 Rust 工具链选择，以及 Buddin 设计参考的可获取性兜底流程。

## 1. JS 工具链（强制）

- 仓库采用 **bun 主入口 + pnpm workspace 元数据** 的双层组织方式：
  - `pnpm-workspace.yaml` 仅用作 workspace 描述（声明 `packages/*` 成员）。
  - 本计划**全程不调用** `pnpm install` / `pnpm run` / `pnpm exec` / `pnpm dlx`。
  - JS 安装、运行、构建、测试统一使用 `bun`：
    - `bun install`
    - `bun run web`
    - `bun run web:build`
    - `bun run web:smoke`
    - `bun run web:smoke -- --case <name>`
- 提交的 lockfile 只有两份：
  - `Cargo.lock`
  - `bun.lockb`
- `pnpm-lock.yaml`：
  - 不提交；
  - 在根 `.gitignore` 中显式排除；
  - 若本地不小心生成，执行 `bun install` 前必须删除以免命中 pnpm 自动升级。
- `package.json`:
  - 保留 `workspaces` 字段用于 bun 识别；
  - 脚本入口仍由 bun 驱动；
  - 新增任何 `scripts/*.ts` 必须可由 `bun` 直接执行，不得依赖 `ts-node` / `tsx` 等额外工具。

## 2. Rust 工具链

- `cargo` 继续作为唯一的 Rust 构建入口。
- wasm 构建相关工具与版本锚点：
  - `wasm-bindgen` crate 版本与 `wasm-bindgen-cli` 必须一致（Phase 1 固定版本，见 `plan-00-naming.md` 与 `packages/studio-web-wasm/README.md`）。
  - `wasm-pack` 用于浏览器环境 smoke（S1b）。
  - 不依赖 `wasm-opt` 作为编译必需项（可后续启用，但不作为 Phase 0–5 要求）。
- CI 环境补丁：
  - 增加一项 lockfile drift 校验：`cargo tree --workspace --locked` 必须通过；`bun install --frozen-lockfile` 必须通过。
  - 增加 `wasm-bindgen` crate 与 CLI 版本一致性校验（脚本位于 Phase 1 建立，不属于 Phase 0 范畴）。

## 3. 脚本与二进制约束

- 本仓库**禁止新增** `python` / `python3` 调用；已有 python 调用在相关任务中应优先替换。
- 任何一次性辅助脚本必须放在 `scripts/*.ts`，以 bun 运行。
- 禁止在 npm 包的 `postinstall` / `prepare` 脚本中执行需要网络的命令（例如下载 wasm-bindgen-cli）。如果必须下载，必须在 `scripts/*.ts` 中显式调用，并允许 `SCAD_STUDIO_SKIP_TOOLCHAIN_FETCH=1` 跳过（CI / 离线环境）。

## 4. 切换到 pnpm 主入口的流程

- 本计划期间禁止切换。
- 若未来需要改用 pnpm 主入口：
  - 必须另起计划文档；
  - 先修订 `AGENTS.md` 的工具链约束章节；
  - 再在计划中规划 lockfile 迁移顺序（`bun.lockb` 退出 → `pnpm-lock.yaml` 提交）。
  - 未经上述流程，任何 PR 不得单方面引入 `pnpm install` 调用。

## 5. Buddin 设计参考可获取性兜底

Phase 4 输入文件位于 `/Users/krhougs/LocalCodes/buddin/`：

- `/Users/krhougs/LocalCodes/buddin/README.md`
- `/Users/krhougs/LocalCodes/buddin/SKILL.md`
- `/Users/krhougs/LocalCodes/buddin/ui_kits/app/README.md`
- `/Users/krhougs/LocalCodes/buddin/ui_kits/app/index.html`
- `/Users/krhougs/LocalCodes/buddin/ui_kits/app/colors_and_type.css`

在进入 Phase 4 之前，先检查这些路径是否可读：

- 若不可读：
  - **暂停 Phase 4**，不进入实现；
  - 直接回到用户，请求补充材料（当面粘贴或提供可读路径），再继续；
  - 禁止凭记忆、训练数据或其它外部来源“猜”设计系统规则。
- 若可读：
  - 在 Phase 4 第一步把所引用内容的**摘要**（色板、字号、间距、组件）以及 **license / 来源信息**写入 `docs/design-system/source-notes.md`；
  - Phase 4 完成后，仓库内所有设计系统引用必须指向 `docs/design-system/*`，不再依赖 `/Users/krhougs/LocalCodes/buddin/` 绝对路径；
  - 在 `docs/design-system/source-notes.md` 中明确：后续如果外部 Buddin 仓库更新，本项目如何决定是否同步（默认人工评估，不自动跟随）。

## 6. Phase 0 决策（无需用户确认）

- 默认工具链：bun-only；pnpm 仅作 workspace 描述。
- 默认 lockfile：`Cargo.lock` + `bun.lockb`。
- Phase 0 不需要任何用户确认项，直接按默认推进后续 Phase。
