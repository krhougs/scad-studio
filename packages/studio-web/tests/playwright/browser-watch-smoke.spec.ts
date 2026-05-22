// S3 browser_watch_smoke: watch 推送 → `client_drain_events` → React 重新
// 渲染目录列表。
// 流程：
//   1. 启动 websocket-host（共享 smoke workspace）+ Vite dev
//   2. Playwright 打开 workbench，等 workspace_list.entries 渲染出 README.md
//   3. Node fs 直接写 watch-smoke-generated.txt 到 workspace 目录
//   4. 等 Files tab 里出现 `entry-watch-smoke-generated.txt`

import { spawn, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import { cpSync, mkdtempSync, rmSync } from "node:fs";
import { createConnection } from "node:net";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import {
  isolatedHostEnvWithTestCadqueryRunner,
  type HostEnvHandle,
} from "./_smoke-harness";

const SPEC_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SPEC_DIR, "..", "..", "..", "..");
const SOURCE_WORKSPACE = path.join(REPO_ROOT, "tests", "studio-web-smoke-workspace");
const HOST_WORKSPACE = mkdtempSync(
  path.join(tmpdir(), "scad-studio-watch-workspace-"),
);
cpSync(SOURCE_WORKSPACE, HOST_WORKSPACE, { recursive: true });
const HOST_BIND = process.env.STUDIO_WEB_SMOKE_BIND ?? "127.0.0.1:39181";
const VITE_PORT = Number(process.env.STUDIO_WEB_SMOKE_VITE_PORT ?? 5176);
const BASE_URL = `http://127.0.0.1:${VITE_PORT}`;
const WS_URL = `ws://${HOST_BIND}`;
const WATCH_SMOKE_FILE = path.join(
  HOST_WORKSPACE,
  "watch-smoke-generated.txt",
);
const AUTORENDER_SCAD = path.join(
  HOST_WORKSPACE,
  "watch-smoke-scad.scad",
);
const README_FILE = path.join(HOST_WORKSPACE, "README.md");
const IMAGE_FILE = path.join(HOST_WORKSPACE, "screenshot.png");
const MODEL_FILE = path.join(HOST_WORKSPACE, "model.stl");
const PRESET_FILE = path.join(HOST_WORKSPACE, "examples", "params-cube.scad.json");

const ALT_IMAGE_BASE64 =
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADUlEQVR42mP8/5+hHgAHggJ/PWdwRwAAAABJRU5ErkJggg==";
const MODEL_STL_SINGLE = `solid single
facet normal 0 0 1
  outer loop
    vertex 0 0 0
    vertex 1 0 0
    vertex 0 1 0
  endloop
endfacet
endsolid single
`;
const MODEL_STL_DOUBLE = `solid double
facet normal 0 0 1
  outer loop
    vertex 0 0 0
    vertex 1 0 0
    vertex 0 1 0
  endloop
endfacet
facet normal 0 0 1
  outer loop
    vertex 1 0 0
    vertex 1 1 0
    vertex 0 1 0
  endloop
endfacet
endsolid double
`;

let hostProc: ChildProcess | null = null;
let viteProc: ChildProcess | null = null;
let hostEnv: HostEnvHandle | null = null;
let originalReadme = "";
let originalImage = Buffer.alloc(0);
let originalModel = "";

test.beforeAll(async () => {
  hostEnv = isolatedHostEnvWithTestCadqueryRunner();
  await mkdir(HOST_WORKSPACE, { recursive: true });
  await rm(WATCH_SMOKE_FILE, { force: true });
  await rm(PRESET_FILE, { force: true });
  originalReadme = await readFile(README_FILE, "utf-8");
  originalImage = await readFile(IMAGE_FILE);
  originalModel = await readFile(MODEL_FILE, "utf-8");

  const host = spawn(
    "cargo",
    [
      "run",
      "-p",
      "app-server-host",
      "--bin",
      "websocket-host",
      "--",
      "--workspace",
      HOST_WORKSPACE,
      "--bind",
      HOST_BIND,
    ],
    {
      cwd: REPO_ROOT,
      stdio: ["ignore", "pipe", "pipe"],
      env: hostEnv.env,
    },
  );
  hostProc = host;
  host.stdout?.on("data", () => {});
  host.stderr?.on("data", () => {});
  const [hostName, hostPortText] = HOST_BIND.split(":");
  await waitForPort(hostName, Number(hostPortText), 60_000);

  const vite = spawn(
    "bun",
    ["x", "vite", "--port", String(VITE_PORT), "--host", "127.0.0.1", "--strictPort"],
    {
      cwd: path.join(REPO_ROOT, "packages", "studio-web"),
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, VITE_WS_URL: WS_URL },
    },
  );
  viteProc = vite;
  vite.stdout?.on("data", () => {});
  vite.stderr?.on("data", () => {});
  await waitForPort("127.0.0.1", VITE_PORT, 60_000);
});

test.afterAll(async () => {
  for (const proc of [viteProc, hostProc]) {
    if (!proc) continue;
    proc.kill("SIGINT");
    try {
      await Promise.race([once(proc, "exit"), delay(2000)]);
    } catch {
      // ignore
    }
    if (proc.exitCode === null) proc.kill("SIGKILL");
  }
  viteProc = null;
  hostProc = null;
  await rm(WATCH_SMOKE_FILE, { force: true });
  await rm(AUTORENDER_SCAD, { force: true });
  await rm(PRESET_FILE, { force: true });
  await writeFile(README_FILE, originalReadme);
  await writeFile(IMAGE_FILE, originalImage);
  await writeFile(MODEL_FILE, originalModel);
  rmSync(HOST_WORKSPACE, { recursive: true, force: true });
  hostEnv?.cleanup();
  hostEnv = null;
});

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    if ("serviceWorker" in navigator) {
      void navigator.serviceWorker
        .getRegistrations()
        .then((regs) => Promise.all(regs.map((r) => r.unregister())));
    }
    if (typeof caches !== "undefined") {
      void caches
        .keys()
        .then((keys) => Promise.all(keys.map((k) => caches.delete(k))));
    }
  });
});

test("@scad-autorerender writing to an open .scad triggers rerender", async ({
  page,
}) => {
  // seed the scad file before navigation so the Files tab lists it
  await writeFile(AUTORENDER_SCAD, "cube([10, 10, 10]);\n");
  await page.goto(`${BASE_URL}/?ws=${encodeURIComponent(WS_URL)}&left-panel=files`);
  await expect(page.getByTestId("entry-watch-smoke-scad.scad")).toBeVisible({
    timeout: 30_000,
  });
  await page.getByTestId("entry-watch-smoke-scad.scad").click();
  await expect(page.getByTestId("scad-workbench")).toBeVisible();
  await expect(page.getByTestId("scad-preview-status")).toContainText(
    /preview pending|preview ready|preview error/,
    { timeout: 30_000 },
  );

  const existing = await readFile(AUTORENDER_SCAD, "utf-8");
  await writeFile(AUTORENDER_SCAD, `${existing}\n// rerender sentinel\n`);

  await expect(page.getByTestId("log-panel")).toHaveCount(0);
  await page.getByTestId("rail-log").click();
  await expect(page.getByTestId("log-panel")).toBeVisible();
  await expect
    .poll(
      async () => {
        const text = (await page.getByTestId("log-list").textContent()) ?? "";
        return /document refresh triggered by|auto rerender triggered by/.test(
          text,
        );
      },
      { timeout: 15_000 },
    )
    .toBe(true);
});

test("@watch markdown body refreshes when the open file changes", async ({
  page,
}) => {
  await page.goto(`${BASE_URL}/?ws=${encodeURIComponent(WS_URL)}&left-panel=files`);
  await page.getByTestId("entry-README.md").waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId("entry-README.md").click();
  await expect(page.getByTestId("markdown-body")).toContainText(
    "studio-web smoke fixture",
    { timeout: 15_000 },
  );

  await writeFile(
    README_FILE,
    "# watch markdown\n\nupdated from playwright watch test\n",
  );
  await expect(page.getByTestId("markdown-body")).toContainText(
    "updated from playwright watch test",
    { timeout: 15_000 },
  );
});

test("@watch image viewer refreshes when the open file changes", async ({
  page,
}) => {
  await page.goto(`${BASE_URL}/?ws=${encodeURIComponent(WS_URL)}&left-panel=files`);
  await page.getByTestId("entry-screenshot.png").waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId("entry-screenshot.png").click();
  const image = page.getByTestId("image-element");
  await image.waitFor({ state: "visible", timeout: 15_000 });
  const originalSrc = await image.getAttribute("src");
  if (!originalSrc) throw new Error("image viewer did not expose a blob src");

  await writeFile(IMAGE_FILE, Buffer.from(ALT_IMAGE_BASE64, "base64"));
  await expect
    .poll(async () => page.getByTestId("image-element").getAttribute("src"), {
      timeout: 15_000,
    })
    .not.toBe(originalSrc);
});

test("@watch mesh viewer refreshes when the open file changes", async ({
  page,
}) => {
  await writeFile(MODEL_FILE, MODEL_STL_SINGLE);
  await page.goto(`${BASE_URL}/?ws=${encodeURIComponent(WS_URL)}&left-panel=files`);
  await page.getByTestId("entry-model.stl").waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId("entry-model.stl").click();
  await expect(page.getByTestId("mesh-canvas")).toBeVisible({
    timeout: 30_000,
  });
  const originalStatus = await page.getByTestId("mesh-status").textContent();
  if (!originalStatus) throw new Error("mesh status should not be empty");

  await writeFile(MODEL_FILE, MODEL_STL_DOUBLE);
  await expect
    .poll(async () => page.getByTestId("mesh-status").textContent(), {
      timeout: 15_000,
    })
    .not.toBe(originalStatus);
});

test("@watch preset list refreshes when the open preset file changes", async ({
  page,
}) => {
  await rm(PRESET_FILE, { force: true });
  await page.goto(`${BASE_URL}/?ws=${encodeURIComponent(WS_URL)}&left-panel=files`);
  await page.getByTestId("entry-examples").waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId("entry-examples").click();
  await page.getByTestId("entry-params-cube.scad").waitFor({
    state: "visible",
    timeout: 15_000,
  });
  await page.getByTestId("entry-params-cube.scad").click();
  await expect(page.getByTestId("presets-panel")).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByTestId("preset-row-external")).toHaveCount(0);

  await writeFile(
    PRESET_FILE,
    JSON.stringify(
      {
        presets: {
          external: {
            size: 18,
            wall: 3,
          },
        },
      },
      null,
      2,
    ),
  );

  await expect(page.getByTestId("preset-row-external")).toBeVisible({
    timeout: 15_000,
  });
});

test("watch push triggers Files tab re-render with new file", async ({ page }) => {
  await page.goto(`${BASE_URL}/?ws=${encodeURIComponent(WS_URL)}&left-panel=files`);

  const regCount = await page.evaluate(async () => {
    if (!("serviceWorker" in navigator)) return 0;
    const regs = await navigator.serviceWorker.getRegistrations();
    return regs.length;
  });
  expect(regCount).toBe(0);

  // 确认 handshake + workspace_list 完成：已知文件 README.md 出现
  await expect(page.getByTestId("entry-README.md")).toBeVisible({
    timeout: 30_000,
  });

  // watch 触发前，生成文件不应存在
  await expect(page.getByTestId("entry-watch-smoke-generated.txt")).toHaveCount(0);

  // 直接在 workspace 写文件。websocket-host 的 DirectoryWatch 观察文件系统
  // 事件，推送给 client；ManagedClient 触发 watch event；WorkbenchLayout
  // 在 onWatchEvent 里重新请求 workspace_list，Files tab 渲染出新文件。
  await writeFile(WATCH_SMOKE_FILE, "watch smoke mutation\n");

  await expect(page.getByTestId("entry-watch-smoke-generated.txt")).toBeVisible({
    timeout: 15_000,
  });
});

async function waitForPort(host: string, port: number, timeoutMs: number): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const ok = await new Promise<boolean>((resolve) => {
      const sock = createConnection({ host, port });
      sock.once("connect", () => {
        sock.end();
        resolve(true);
      });
      sock.once("error", () => resolve(false));
    });
    if (ok) return;
    await delay(250);
  }
  throw new Error(`port ${host}:${port} not ready after ${timeoutMs}ms`);
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
