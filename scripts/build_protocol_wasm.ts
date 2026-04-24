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
  "app_server_protocol_wasm.wasm",
);
const outDir = path.join(root, "packages", "app-server-protocol", "generated");

await run([
  "cargo",
  "build",
  "-p",
  "app-server-protocol-wasm",
  "--target",
  "wasm32-unknown-unknown",
  "--release",
]);
await run([
  "wasm-bindgen",
  wasmPath,
  "--target",
  "web",
  "--out-dir",
  outDir,
  "--out-name",
  "app_server_protocol_wasm",
]);
