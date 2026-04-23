// 统一 smoke dispatcher。
// 用法：
//   bun scripts/run_smoke.ts --case <name>
//   bun scripts/run_smoke.ts            # 等价于 --case all
//
// 支持 case：wasm_package_smoke / browser_smoke / browser_watch_smoke / all
//           / markdown_view / image_view / scad_split_view（Phase 6 扩展）
// S1a / S1b / S4 通过直接命令调度。
// all 仅覆盖 S1a-S4 基本用例，不跑 Phase 6 扩展用例（需手动触发）。

import path from "node:path";
import { runWasmPackageSmoke } from "./smoke/wasm_package_smoke";
import { REPO_ROOT } from "./run_websocket_host";

type Case =
  | "wasm_package_smoke"
  | "browser_smoke"
  | "browser_watch_smoke"
  | "markdown_view"
  | "image_view"
  | "scad_split_view"
  | "all";

const VALID_CASES: readonly Case[] = [
  "wasm_package_smoke",
  "browser_smoke",
  "browser_watch_smoke",
  "markdown_view",
  "image_view",
  "scad_split_view",
  "all",
];

function parseCase(argv: string[]): Case {
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--case") {
      const value = argv[i + 1];
      if (!value) fail(`missing value for --case`);
      if (!VALID_CASES.includes(value as Case)) {
        fail(`unknown case: ${value}`);
      }
      return value as Case;
    }
    if (arg === "--") continue;
    if (arg === "-h" || arg === "--help") {
      printUsage(0);
    }
  }
  return "all";
}

function fail(message: string): never {
  console.error(message);
  process.exit(1);
}

function printUsage(code: number): never {
  console.log(
    "usage: bun scripts/run_smoke.ts [--case " + VALID_CASES.join("|") + "]",
  );
  process.exit(code);
}

async function runCommand(cmd: string[], cwd = REPO_ROOT): Promise<number> {
  console.log(`[smoke] $ ${cmd.join(" ")}`);
  const proc = Bun.spawn(cmd, {
    cwd,
    stdout: "inherit",
    stderr: "inherit",
    stdin: "ignore",
  });
  return (await proc.exited) ?? 1;
}

async function runS1aHostTests(): Promise<number> {
  return runCommand(["cargo", "test", "-p", "studio-web-wasm", "--tests"]);
}

async function runS1bWasmBindgen(): Promise<number> {
  if (!(await commandExists("wasm-pack"))) {
    console.error(
      "[smoke] missing wasm-pack; install it with: cargo install wasm-pack",
    );
    return 1;
  }
  return runCommand([
    "wasm-pack",
    "test",
    "--headless",
    "--chrome",
    "crates/studio-web-wasm",
    "--test",
    "wasm_bridge_smoke",
  ]);
}

async function runPlaywrightSpec(
  spec: string,
  grep?: string,
): Promise<number> {
  const cmd = ["bun", "x", "playwright", "test", spec];
  if (grep) {
    cmd.push("--grep", grep);
  }
  return runCommand(cmd, path.join(REPO_ROOT, "packages", "studio-web"));
}

async function runS2BrowserSmoke(): Promise<number> {
  return runPlaywrightSpec("tests/playwright/browser-smoke.spec.ts");
}

async function runS3BrowserWatchSmoke(): Promise<number> {
  return runPlaywrightSpec("tests/playwright/browser-watch-smoke.spec.ts");
}

async function runS4PwaBuild(): Promise<number> {
  return runCommand(["bun", "scripts/build_studio_web.ts"]);
}

async function runS1c(): Promise<number> {
  return runWasmPackageSmoke();
}

async function runPhase6Case(tag: string): Promise<number> {
  return runPlaywrightSpec("tests/playwright/browser-smoke.spec.ts", tag);
}

async function commandExists(command: string): Promise<boolean> {
  const proc = Bun.spawn(["sh", "-lc", `command -v ${command}`], {
    cwd: REPO_ROOT,
    stdout: "ignore",
    stderr: "ignore",
    stdin: "ignore",
  });
  return (await proc.exited) === 0;
}

async function runAll(): Promise<number> {
  const steps: Array<{ label: string; fn: () => Promise<number> }> = [
    { label: "S1a rust_unit_smoke", fn: runS1aHostTests },
    { label: "S1b wasm_bindgen_smoke", fn: runS1bWasmBindgen },
    { label: "S1c wasm_package_smoke", fn: runS1c },
    { label: "S2 browser_smoke", fn: runS2BrowserSmoke },
    { label: "S3 browser_watch_smoke", fn: runS3BrowserWatchSmoke },
    { label: "S4 pwa_build_smoke", fn: runS4PwaBuild },
  ];
  for (const step of steps) {
    console.log(`\n[smoke] ===== ${step.label} =====`);
    const code = await step.fn();
    if (code !== 0) {
      console.error(`[smoke] ${step.label} FAILED (exit ${code})`);
      return code;
    }
  }
  console.log("\n[smoke] all smoke cases passed");
  return 0;
}

async function dispatch(which: Case): Promise<number> {
  switch (which) {
    case "wasm_package_smoke":
      return runS1c();
    case "browser_smoke":
      return runS2BrowserSmoke();
    case "browser_watch_smoke":
      return runS3BrowserWatchSmoke();
    case "markdown_view":
      return runPhase6Case("@markdown");
    case "image_view":
      return runPhase6Case("@image");
    case "scad_split_view":
      return runPhase6Case("@scad-split");
    case "all":
      return runAll();
  }
}

const which = parseCase(Bun.argv.slice(2));
const code = await dispatch(which);
process.exit(code);
