# Phase 0 契约 · 命名矩阵

本文件固定后续所有 Phase 引用的包名、产物路径、构建命令、默认环境变量。任何 Phase 禁止私自更名；如需调整，先改本文件再改代码。

## 1. Cargo / wasm 产物命名

| 维度 | 取值 |
|------|------|
| Cargo package name | `studio-web-wasm` |
| Rust lib crate name（`[lib].name`） | `studio_web_wasm` |
| `crate-type` | `["cdylib", "rlib"]` |
| 默认编译目标 | `wasm32-unknown-unknown` |
| 构建 profile | `release` |
| wasm-bindgen target | `bundler` |
| wasm-bindgen 输出目录 | `packages/studio-web-wasm/generated/` |
| wasm-bindgen `--out-name` | `studio_web_wasm` |
| wasm js wrapper 文件名 | `studio_web_wasm.js` |
| wasm 二进制文件名 | `studio_web_wasm_bg.wasm` |
| wasm TypeScript d.ts 文件名 | `studio_web_wasm.d.ts` |
| Cargo feature（过渡期默认壳） | `legacy-shell`（默认不启用） |

## 2. npm / pnpm 包命名

| 维度 | 取值 |
|------|------|
| npm package name（wasm 包） | `@scad-studio/studio-web-wasm` |
| npm package name（React PWA） | `@scad-studio/studio-web` |
| pnpm workspace 成员路径 | `packages/*` |
| wasm 包在 PWA 中的 import path | `@scad-studio/studio-web-wasm` |
| wasm 包主入口 | `src/index.ts`（仅 re-export `generated/`） |
| React PWA 入口 | `index.html` → `src/main.tsx` |

## 3. 运行时 / 默认值

| 维度 | 取值 |
|------|------|
| `websocket-host` 启动归属 | `scripts/run_studio_web.ts`（dev / smoke 共用） |
| `SCAD_STUDIO_WS_URL` 默认值 | `ws://127.0.0.1:38421`（完整 URL；端口由该 URL 解析而来，不另设 `SCAD_STUDIO_WS_PORT`） |
| Vite dev server 端口 | `5173`（Vite 默认） |
| Service Worker dev 模式 | 禁用（`vite-plugin-pwa` `devOptions.enabled = false`） |
| Service Worker prod 模式 | 启用，wasm 资源走 hashed filename |

## 4. 构建命令（按执行顺序）

```bash
# 1. Rust → wasm
cargo build -p studio-web-wasm --target wasm32-unknown-unknown --release

# 2. wasm-bindgen 产物生成（写入 packages/studio-web-wasm/generated/）
wasm-bindgen target/wasm32-unknown-unknown/release/studio_web_wasm.wasm \
  --target bundler \
  --out-dir packages/studio-web-wasm/generated \
  --out-name studio_web_wasm

# 3. Vite 构建 React PWA
bun run web:build

# 4. Vite dev server
bun run web
```

上述命令的调用方统一为 `scripts/run_studio_web.ts`；其他脚本禁止复制粘贴这些命令，必须 `import` 或 `exec` 同一入口以避免漂移。

## 5. 测试命令（按 smoke 编号）

```bash
cargo test -p studio-web-wasm                          # S1a
wasm-pack test --headless --chrome crates/studio-web-wasm  # S1b
bun run web:smoke -- --case wasm_package_smoke         # S1c
bun run web:smoke -- --case browser_smoke              # S2
bun run web:smoke -- --case browser_watch_smoke        # S3
bun run web:build                                       # S4
```

## 6. lockfile 策略

| 文件 | 是否提交 |
|------|----------|
| `Cargo.lock` | 提交 |
| `bun.lockb` | 提交 |
| `pnpm-lock.yaml` | **不提交**，进入 `.gitignore` |

## 7. 命名硬约束（违反视为 PR block）

- Rust 侧（crate / lib / feature / 产物文件名）与 npm 侧（package name / import path / generated 路径）都必须使用上表中的原文；禁止出现 `studio-web`（旧名）、`studio_web`（旧 snake_case）、`studio-app-wasm` 等别名。
- `packages/studio-web-wasm/generated/` 是 wasm-bindgen 唯一输出目的地；禁止在其它位置再生成 wrapper。
- `@scad-studio` scope 不得更换；若后续需要切换 scope，另起计划并同步更新本文件。
