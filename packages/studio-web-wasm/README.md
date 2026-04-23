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

Verify both sides in one step from the repo root:

```bash
bun run check:wasm-bindgen
```

That script parses `crates/studio-web-wasm/Cargo.toml`, invokes `wasm-bindgen --version`, and fails with a non-zero exit code on any mismatch. CI invokes the same script. If the CLI is missing or the wrong version, install the pinned one:

```bash
cargo install wasm-bindgen-cli --version 0.2.117 --locked
```

The S1c smoke additionally diffs regenerated wrapper output against what is committed under `generated/`, catching drift that even a matching version could introduce.

## Consumers

- `packages/studio-web` (React PWA) imports `@scad-studio/studio-web-wasm`.
- Nothing else imports this package.

## What must NOT live here

- Protocol state machines, WebSocket transport, React components, CSS, business logic.

If a future change requires any of the above, fix the boundary in `crates/studio-web-wasm` (for Rust-side bridge logic) or `packages/studio-web` (for TS transport and UI) — **not** here.
