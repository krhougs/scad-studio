// S2 browser smoke. Boots `websocket-host` + Vite dev and drives:
// workspace.current → workspace.list → preview.request end-to-end.
// Service Worker must be disabled in dev; the test also clears any lingering
// registrations or caches before driving the page.

import { spawn, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import { createConnection } from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";

const SPEC_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SPEC_DIR, "..", "..", "..", "..");
const HOST_WORKSPACE = path.join(REPO_ROOT, "tests", "studio-web-smoke-workspace");
const HOST_BIND = process.env.STUDIO_WEB_SMOKE_BIND ?? "127.0.0.1:39180";
const VITE_PORT = Number(process.env.STUDIO_WEB_SMOKE_VITE_PORT ?? 5175);
const BASE_URL = `http://127.0.0.1:${VITE_PORT}`;
const WS_URL = `ws://${HOST_BIND}`;

let hostProc: ChildProcess | null = null;
let viteProc: ChildProcess | null = null;

test.beforeAll(async () => {
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
    { cwd: REPO_ROOT, stdio: ["ignore", "pipe", "pipe"] },
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

test("workspace info appears after handshake", async ({ page }) => {
  await page.goto(`${BASE_URL}/?ws=${encodeURIComponent(WS_URL)}&left-panel=files`);
  const regCount = await page.evaluate(async () => {
    if (!("serviceWorker" in navigator)) return 0;
    const regs = await navigator.serviceWorker.getRegistrations();
    return regs.length;
  });
  expect(regCount).toBe(0);

  // Topbar workspace-name 与 inspector 标题都会渲染 workspace 名；用 topbar 的
  // data-testid 做断言（五区布局把两个位置拆开，但 breadcrumb 仍唯一 data-testid）。
  await expect(page.getByTestId("workspace-name")).not.toHaveText("(loading)", {
    timeout: 30_000,
  });
  await expect(page.getByTestId("phase")).toContainText(/ready|preview/);
});

test("preview request completes or reports error", async ({ page }) => {
  await page.goto(`${BASE_URL}/?ws=${encodeURIComponent(WS_URL)}&left-panel=files`);
  // Open the mesh fixture explicitly; first entry is alphabetically README.md
  // which now routes to the markdown tab and never fires PreviewRequest.
  await page.getByTestId("entry-model.stl").waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId("entry-model.stl").click();
  await expect(page.getByTestId("message")).toContainText(
    /preview ready|preview error/,
    { timeout: 30_000 },
  );
});

test("@markdown clicking README.md opens markdown viewer", async ({ page }) => {
  await page.goto(`${BASE_URL}/?ws=${encodeURIComponent(WS_URL)}&left-panel=files`);
  await page.getByTestId("entry-README.md").waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId("entry-README.md").click();
  await expect(page.getByTestId("tabbar")).toBeVisible();
  await expect(page.getByTestId("tab-README.md")).toBeVisible();
  const body = page.getByTestId("markdown-body");
  await expect(body).toBeVisible({ timeout: 15_000 });
  await expect(body).toContainText("studio-web smoke fixture");
});

test("@image clicking screenshot.png opens image viewer", async ({ page }) => {
  await page.goto(`${BASE_URL}/?ws=${encodeURIComponent(WS_URL)}&left-panel=files`);
  await page.getByTestId("entry-screenshot.png").waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId("entry-screenshot.png").click();
  await expect(page.getByTestId("image-viewer")).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByTestId("image-size")).toBeVisible();
  await expect(page.getByTestId("image-scale")).toContainText("scale");
});

test("@scad-viewer clicking cube.scad opens a real viewer workflow", async ({
  page,
}) => {
  await page.goto(`${BASE_URL}/?ws=${encodeURIComponent(WS_URL)}&left-panel=files`);
  // cube.scad lives under examples/; expand the directory first.
  await page.getByTestId("entry-examples").waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId("entry-examples").click();
  await page.getByTestId("entry-cube.scad").waitFor({
    state: "visible",
    timeout: 15_000,
  });
  await page.getByTestId("entry-cube.scad").click();
  await expect(page.getByTestId("scad-preview-viewer")).toBeVisible();
  await expect(page.getByTestId("scad-source")).toHaveCount(0);
  await expect(page.getByTestId("mesh-canvas")).toBeVisible({
    timeout: 30_000,
  });
  await expect(page.getByTestId("scad-preview-status")).toContainText(
    /preview ready|preview error|preview pending/,
    { timeout: 30_000 },
  );
  await expect(page.getByTestId("export-panel")).toBeVisible();
  await expect(page.getByTestId("slicer-panel")).toBeVisible();
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
