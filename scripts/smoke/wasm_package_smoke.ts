// S1c wasm 包 generated 一致性校验。
// 流程（plan-00-smoke.md §3.3）：
//   1. 把 packages/studio-web-wasm/generated/ 复制到 target/smoke-snapshot/wasm-generated/
//   2. cargo build + wasm-bindgen 重新生成到原目录
//   3. 逐文件 diff（fs readdir 兜底所有文件）
//   4. 把 snapshot 还原回 generated/，避免污染工作树

import { existsSync } from "node:fs";
import {
  mkdir,
  readFile,
  readdir,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { REPO_ROOT } from "../run_websocket_host";

const GENERATED_DIR = path.join(
  REPO_ROOT,
  "packages",
  "studio-web-wasm",
  "generated",
);
const SNAPSHOT_DIR = path.join(
  REPO_ROOT,
  "target",
  "smoke-snapshot",
  "wasm-generated",
);
const RELEASE_WASM = path.join(
  REPO_ROOT,
  "target",
  "wasm32-unknown-unknown",
  "release",
  "studio_web_wasm.wasm",
);

type FileEntry = { rel: string; abs: string };

// 收集所有文件（包括 .gitkeep），用于 snapshot 拷贝和还原。
async function collectAllFiles(root: string): Promise<FileEntry[]> {
  const out: FileEntry[] = [];
  async function walk(dir: string, prefix: string) {
    const entries = await readdir(dir, { withFileTypes: true });
    for (const entry of entries) {
      const abs = path.join(dir, entry.name);
      const rel = prefix ? path.join(prefix, entry.name) : entry.name;
      if (entry.isDirectory()) {
        await walk(abs, rel);
      } else if (entry.isFile()) {
        out.push({ rel, abs });
      }
    }
  }
  await walk(root, "");
  out.sort((a, b) => a.rel.localeCompare(b.rel));
  return out;
}

// diff 时跳过 .gitkeep（wasm-bindgen 不会重新生成它）。
async function collectDiffableFiles(root: string): Promise<FileEntry[]> {
  const all = await collectAllFiles(root);
  return all.filter((entry) => path.basename(entry.rel) !== ".gitkeep");
}

async function copyAll(src: string, dst: string) {
  await rm(dst, { recursive: true, force: true });
  await mkdir(dst, { recursive: true });
  const files = await collectAllFiles(src);
  for (const entry of files) {
    const target = path.join(dst, entry.rel);
    await mkdir(path.dirname(target), { recursive: true });
    const data = await readFile(entry.abs);
    await writeFile(target, data);
  }
}

async function run(cmd: string[], cwd = REPO_ROOT) {
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

async function regenerateWasmPackage() {
  await run([
    "cargo",
    "build",
    "-p",
    "studio-web-wasm",
    "--target",
    "wasm32-unknown-unknown",
    "--release",
  ]);
  if (!existsSync(RELEASE_WASM)) {
    throw new Error(
      `expected release wasm at ${RELEASE_WASM}, not found after cargo build`,
    );
  }
  await run([
    "wasm-bindgen",
    RELEASE_WASM,
    "--target",
    "bundler",
    "--out-dir",
    GENERATED_DIR,
    "--out-name",
    "studio_web_wasm",
  ]);
}

type DiffResult =
  | { kind: "equal" }
  | { kind: "missing_in_new"; files: string[] }
  | { kind: "missing_in_snapshot"; files: string[] }
  | {
      kind: "content_diff";
      diffs: Array<{
        rel: string;
        snapshotLen: number;
        regeneratedLen: number;
        firstDifferenceOffset: number;
      }>;
    };

async function diffTrees(
  snapshotRoot: string,
  currentRoot: string,
): Promise<DiffResult[]> {
  const snap = await collectDiffableFiles(snapshotRoot);
  const curr = await collectDiffableFiles(currentRoot);
  const snapSet = new Set(snap.map((e) => e.rel));
  const currSet = new Set(curr.map((e) => e.rel));
  const missingInNew = snap.filter((e) => !currSet.has(e.rel)).map((e) => e.rel);
  const missingInSnapshot = curr
    .filter((e) => !snapSet.has(e.rel))
    .map((e) => e.rel);

  const diffs: Array<{
    rel: string;
    snapshotLen: number;
    regeneratedLen: number;
    firstDifferenceOffset: number;
  }> = [];
  for (const entry of snap) {
    if (!currSet.has(entry.rel)) continue;
    const a = await readFile(entry.abs);
    const b = await readFile(path.join(currentRoot, entry.rel));
    if (a.length !== b.length || !a.equals(b)) {
      const limit = Math.min(a.length, b.length);
      let off = 0;
      while (off < limit && a[off] === b[off]) off += 1;
      diffs.push({
        rel: entry.rel,
        snapshotLen: a.length,
        regeneratedLen: b.length,
        firstDifferenceOffset: off,
      });
    }
  }

  const out: DiffResult[] = [];
  if (missingInNew.length > 0) {
    out.push({ kind: "missing_in_new", files: missingInNew });
  }
  if (missingInSnapshot.length > 0) {
    out.push({ kind: "missing_in_snapshot", files: missingInSnapshot });
  }
  if (diffs.length > 0) {
    out.push({ kind: "content_diff", diffs });
  }
  if (out.length === 0) {
    out.push({ kind: "equal" });
  }
  return out;
}

function printDiffSummary(results: DiffResult[]) {
  for (const result of results) {
    if (result.kind === "equal") {
      console.log("[wasm_package_smoke] generated tree is byte-identical");
      return;
    }
    if (result.kind === "missing_in_new") {
      console.error("[wasm_package_smoke] missing in regenerated:");
      for (const rel of result.files) console.error(`  - ${rel}`);
    }
    if (result.kind === "missing_in_snapshot") {
      console.error(
        "[wasm_package_smoke] missing in snapshot (newly generated files):",
      );
      for (const rel of result.files) console.error(`  - ${rel}`);
    }
    if (result.kind === "content_diff") {
      console.error("[wasm_package_smoke] content diff:");
      for (const diff of result.diffs) {
        console.error(
          `  - ${diff.rel}: snapshot=${diff.snapshotLen}B, regenerated=${diff.regeneratedLen}B, first diff @ byte ${diff.firstDifferenceOffset}`,
        );
      }
    }
  }
}

async function verifyPackageImport() {
  // Minimal static check: @budn/studio-web-wasm 应指向 src/index.ts
  // 再确认 src/index.ts 能 re-export generated/。
  const pkgJson = JSON.parse(
    await readFile(
      path.join(REPO_ROOT, "packages", "studio-web-wasm", "package.json"),
      "utf8",
    ),
  );
  if (pkgJson.name !== "@budn/studio-web-wasm") {
    throw new Error(
      `unexpected package name: ${pkgJson.name}; expected @budn/studio-web-wasm`,
    );
  }
  const indexText = await readFile(
    path.join(REPO_ROOT, "packages", "studio-web-wasm", "src", "index.ts"),
    "utf8",
  );
  if (!indexText.includes("generated/")) {
    throw new Error(
      "packages/studio-web-wasm/src/index.ts must re-export from ./generated/",
    );
  }
}

async function restore(snapshotRoot: string, targetRoot: string) {
  await rm(targetRoot, { recursive: true, force: true });
  await mkdir(targetRoot, { recursive: true });
  // restore 把 snapshot 完整拷回去（包括 .gitkeep，保持字节不变）
  const files = await collectAllFiles(snapshotRoot);
  for (const entry of files) {
    const target = path.join(targetRoot, entry.rel);
    await mkdir(path.dirname(target), { recursive: true });
    const data = await readFile(entry.abs);
    await writeFile(target, data);
  }
}

export async function runWasmPackageSmoke(): Promise<number> {
  if (!existsSync(GENERATED_DIR)) {
    console.error(
      `[wasm_package_smoke] generated dir not found: ${GENERATED_DIR}`,
    );
    return 1;
  }
  const snapshotStat = await stat(GENERATED_DIR);
  if (!snapshotStat.isDirectory()) {
    console.error(`[wasm_package_smoke] not a directory: ${GENERATED_DIR}`);
    return 1;
  }

  console.log(`[wasm_package_smoke] snapshotting generated/ → ${SNAPSHOT_DIR}`);
  await copyAll(GENERATED_DIR, SNAPSHOT_DIR);

  console.log("[wasm_package_smoke] verifying package layout");
  await verifyPackageImport();

  console.log("[wasm_package_smoke] regenerating wasm bundle");
  try {
    await regenerateWasmPackage();
  } catch (err) {
    console.error("[wasm_package_smoke] regeneration failed; restoring snapshot");
    await restore(SNAPSHOT_DIR, GENERATED_DIR);
    throw err;
  }

  const diffs = await diffTrees(SNAPSHOT_DIR, GENERATED_DIR);
  printDiffSummary(diffs);

  await restore(SNAPSHOT_DIR, GENERATED_DIR);

  const drift = diffs.some((result) => result.kind !== "equal");
  if (drift) {
    console.error("[wasm_package_smoke] drift detected; see diff summary above");
    return 1;
  }
  return 0;
}

if (import.meta.main) {
  const code = await runWasmPackageSmoke();
  process.exit(code);
}
