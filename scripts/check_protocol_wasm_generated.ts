import path from "node:path";

const root = path.resolve(import.meta.dir, "..");
const generatedDir = "packages/app-server-protocol/generated";
const expectedGeneratedFiles = [
  "app_server_protocol_wasm.d.ts",
  "app_server_protocol_wasm.js",
  "app_server_protocol_wasm_bg.wasm",
  "app_server_protocol_wasm_bg.wasm.d.ts",
];

async function run(cmd: string[]): Promise<void> {
  const proc = Bun.spawn(cmd, {
    cwd: root,
    stdout: "inherit",
    stderr: "inherit",
    stdin: "ignore",
  });
  const code = await proc.exited;
  if (code !== 0) {
    throw new Error(`${cmd.join(" ")} failed with exit code ${code}`);
  }
}

await run(["bun", "run", "protocol:build"]);
await run(["git", "diff", "--exit-code", "--", generatedDir]);

for (const file of expectedGeneratedFiles) {
  const trackedCheck = Bun.spawn(
    ["git", "ls-files", "--cached", "--error-unmatch", `${generatedDir}/${file}`],
    {
      cwd: root,
      stdout: "ignore",
      stderr: "ignore",
      stdin: "ignore",
    },
  );
  if ((await trackedCheck.exited) !== 0) {
    throw new Error(`protocol wasm generated file is not staged or tracked: ${generatedDir}/${file}`);
  }
}

const status = Bun.spawn(
  ["git", "ls-files", "--others", "--exclude-standard", "--", generatedDir],
  {
    cwd: root,
    stdout: "pipe",
    stderr: "inherit",
    stdin: "ignore",
  },
);
const output = await new Response(status.stdout).text();
const code = await status.exited;
if (code !== 0) {
  throw new Error(`git ls-files failed with exit code ${code}`);
}
if (output.trim().length > 0) {
  console.error(output.trimEnd());
  throw new Error("protocol wasm generated output contains untracked files");
}
