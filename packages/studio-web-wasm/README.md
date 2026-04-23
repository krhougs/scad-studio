# @scad-studio/studio-web-wasm

Internal npm workspace package that distributes the `studio-web-wasm` Rust crate's wasm-bindgen output to the `packages/studio-web` React PWA. This package contains **no business logic**; it is only a transport for generated wasm bindings.

## Layout

```
packages/studio-web-wasm/
├── package.json        # npm metadata
├── README.md           # this file
├── src/index.ts        # re-exports from ./generated/
└── generated/          # committed wasm-bindgen output (see build steps)
```

## Build

From repo root:

```bash
cargo build -p studio-web-wasm --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/studio_web_wasm.wasm \
  --target bundler \
  --out-dir packages/studio-web-wasm/generated \
  --out-name studio_web_wasm
```

The commands above are the sole source of truth for regenerating `generated/`. See `prompt-archives/2026042300-studio-web-feature-parity/plan-00-naming.md` for the locked build recipe.

## Version pinning

The `wasm-bindgen` crate version (in `crates/studio-web-wasm/Cargo.toml`) and the `wasm-bindgen-cli` binary used locally must match exactly. Current pin: **0.2.117**.

### Pinning strategy

Phase 0 toolchain 契约 (`plan-00-toolchain.md`) allowed either (a) pinning `wasm-bindgen-cli` as an npm devDependency, or (b) pinning via a local script + README. This project deliberately picks **option (b)** — the npm devDependency route is avoided because no first-party npm publisher for `wasm-bindgen-cli` exists at the required version, and bundling a post-install downloader would add a non-trivial cargo/network dependency to every `bun install`.

### How pinning is enforced

1. **Crate side**: `crates/studio-web-wasm/Cargo.toml` declares `wasm-bindgen = "=0.2.117"` (the `=` is significant — `Cargo.lock` refuses to upgrade).
2. **Host binary side**: developers install the CLI once with `cargo install wasm-bindgen-cli --version 0.2.117 --locked`.
3. **Drift check**: `bun run check:wasm-bindgen` parses `crates/studio-web-wasm/Cargo.toml`, invokes `wasm-bindgen --version`, and exits non-zero on mismatch. Run it locally before commits; CI runs it as part of S1 acceptance.
4. **Regenerate drift check**: the S1c smoke (`scripts/smoke/wasm_package_smoke.ts`) snapshots `generated/`, regenerates with the current CLI, and byte-diffs every file. Any matching-version toolchain difference (rustc, feature flags, envs) surfaces here.

If the CLI is missing or the wrong version:

```bash
cargo install wasm-bindgen-cli --version 0.2.117 --locked
```

## Consumers

- `packages/studio-web` (React PWA) imports `@scad-studio/studio-web-wasm`.
- Nothing else imports this package.

## What must NOT live here

- Protocol state machines, WebSocket transport, React components, CSS, business logic.

If a future change requires any of the above, fix the boundary in `crates/studio-web-wasm` (for Rust-side bridge logic) or `packages/studio-web` (for TS transport and UI) — **not** here.
