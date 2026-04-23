// Phase 3 build entry: cargo → wasm-bindgen → vite build.
// Does not launch websocket-host; that's handled by run_studio_web_dev.ts / smoke.

import path from "node:path";

const root = path.resolve(import.meta.dir, "..");

async function run(cmd: string[], cwd = root): Promise<void> {
  const proc = Bun.spawn(cmd, {
    cwd,
    stdout: "inherit",
    stderr: "inherit",
    stdin: "ignore",
  });
  const code = await proc.exited;
  if (code !== 0) {
    throw new Error(`${cmd.join(" ")} failed with exit code ${code}`);
  }
}

const wasmPath = path.join(
  root,
  "target",
  "wasm32-unknown-unknown",
  "release",
  "studio_web_wasm.wasm",
);
const outDir = path.join(root, "packages", "studio-web-wasm", "generated");

await run([
  "cargo",
  "build",
  "-p",
  "studio-web-wasm",
  "--target",
  "wasm32-unknown-unknown",
  "--release",
]);
await run([
  "wasm-bindgen",
  wasmPath,
  "--target",
  "bundler",
  "--out-dir",
  outDir,
  "--out-name",
  "studio_web_wasm",
]);
await run(["bun", "run", "build"], path.join(root, "packages", "studio-web"));
