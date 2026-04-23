// Shared harness used by the Phase 7 Playwright specs. Every spec that imports
// this must pick a unique {bindPort, vitePort} pair to avoid port clashes when
// the dispatcher runs specs in parallel.

import { spawn, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import { createConnection } from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SPEC_DIR = path.dirname(fileURLToPath(import.meta.url));
export const REPO_ROOT = path.resolve(SPEC_DIR, "..", "..", "..", "..");
export const HOST_WORKSPACE = path.join(
  REPO_ROOT,
  "tests",
  "studio-web-smoke-workspace",
);

export type HarnessOptions = {
  bindPort: number;
  vitePort: number;
  /**
   * Extra env overrides applied to the `websocket-host` subprocess. Used by
   * `@config-settings` to point `dirs::config_dir()` at a throwaway `HOME`
   * so the test never writes to the developer's real scad-studio config.
   */
  hostEnv?: Record<string, string>;
};

export type HarnessHandle = {
  baseUrl: string;
  wsUrl: string;
  start: () => Promise<void>;
  stop: () => Promise<void>;
};

export function createHarness(opts: HarnessOptions): HarnessHandle {
  const hostBind = `127.0.0.1:${opts.bindPort}`;
  const baseUrl = `http://127.0.0.1:${opts.vitePort}`;
  const wsUrl = `ws://${hostBind}`;
  let hostProc: ChildProcess | null = null;
  let viteProc: ChildProcess | null = null;

  return {
    baseUrl,
    wsUrl,
    async start() {
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
          hostBind,
        ],
        {
          cwd: REPO_ROOT,
          stdio: ["ignore", "pipe", "pipe"],
          env: { ...process.env, ...(opts.hostEnv ?? {}) },
        },
      );
      hostProc = host;
      host.stdout?.on("data", () => {});
      host.stderr?.on("data", () => {});
      await waitForPort("127.0.0.1", opts.bindPort, 60_000);

      const vite = spawn(
        "bun",
        [
          "x",
          "vite",
          "--port",
          String(opts.vitePort),
          "--host",
          "127.0.0.1",
          "--strictPort",
        ],
        {
          cwd: path.join(REPO_ROOT, "packages", "studio-web"),
          stdio: ["ignore", "pipe", "pipe"],
          env: { ...process.env, VITE_WS_URL: wsUrl },
        },
      );
      viteProc = vite;
      vite.stdout?.on("data", () => {});
      vite.stderr?.on("data", () => {});
      await waitForPort("127.0.0.1", opts.vitePort, 60_000);
    },
    async stop() {
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
      hostProc = null;
      viteProc = null;
    },
  };
}

export async function clearServiceWorkerState(
  page: import("@playwright/test").Page,
): Promise<void> {
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
}

export async function waitForPort(
  host: string,
  port: number,
  timeoutMs: number,
): Promise<void> {
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

export function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
