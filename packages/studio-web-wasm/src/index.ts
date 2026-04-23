// This file is the sole entry point of @scad-studio/studio-web-wasm.
// It re-exports whatever wasm-bindgen has emitted into ../generated/.
// Phase 1 ships with an empty generated/ directory; Phase 2 populates it.

// @ts-expect-error Phase 2 writes ../generated/studio_web_wasm.js during wasm-bindgen output.
export * from "../generated/studio_web_wasm.js";
