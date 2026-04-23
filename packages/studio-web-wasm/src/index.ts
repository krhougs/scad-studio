// This file is the sole entry point of @scad-studio/studio-web-wasm.
// It re-exports whatever wasm-bindgen has emitted into ../generated/.
// Phase 1 ships with an empty generated/ directory; Phase 2 populates it.

// Phase 2 writes ../generated/studio_web_wasm.{js,d.ts,wasm} during wasm-bindgen
// output. The .d.ts sits next to the .js so TypeScript can resolve exports; if a
// consumer typechecks before wasm-bindgen has run, this import will fail with
// TS2307 — that is the correct signal to run the web:build pipeline first.
export * from "../generated/studio_web_wasm.js";
