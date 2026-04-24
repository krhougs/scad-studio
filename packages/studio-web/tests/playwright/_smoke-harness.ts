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
  workspacePath?: string;
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
  const workspacePath = opts.workspacePath ?? HOST_WORKSPACE;
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
          workspacePath,
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

export async function installProtocolRecorder(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.addInitScript(() => {
    const win = window as Window & {
      __scadOutgoingFrames?: Array<{ raw: string; parsed: unknown }>;
      __scadDispatchedCommands?: Array<{ type: string; payload: unknown }>;
      __scadProtocolRecorderInstalled?: boolean;
    };
    win.__scadOutgoingFrames = [];
    win.__scadDispatchedCommands = [];
    if (win.__scadProtocolRecorderInstalled) {
      return;
    }
    win.__scadProtocolRecorderInstalled = true;
    const decoder = new TextDecoder();
    const originalSend = WebSocket.prototype.send;
    WebSocket.prototype.send = function patchedSend(data: Parameters<WebSocket["send"]>[0]) {
      try {
        let text: string | null = null;
        if (typeof data === "string") {
          text = data;
        } else if (data instanceof ArrayBuffer) {
          text = decoder.decode(new Uint8Array(data));
        } else if (ArrayBuffer.isView(data)) {
          text = decoder.decode(
            new Uint8Array(data.buffer, data.byteOffset, data.byteLength),
          );
        }
        if (text) {
          win.__scadOutgoingFrames?.push({
            raw: text,
            parsed: JSON.parse(text),
          });
        }
      } catch {
        // 忽略非 JSON 帧；当前协议走 text JSON，这里只作为测试观测点。
      }
      return originalSend.call(this, data);
    };
  });
}

export async function clearRecordedClientCommands(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.evaluate(() => {
    (
      window as Window & {
        __scadOutgoingFrames?: Array<{ raw: string; parsed: unknown }>;
        __scadDispatchedCommands?: Array<{ type: string; payload: unknown }>;
      }
    ).__scadOutgoingFrames = [];
    (
      window as Window & {
        __scadOutgoingFrames?: Array<{ raw: string; parsed: unknown }>;
        __scadDispatchedCommands?: Array<{ type: string; payload: unknown }>;
      }
    ).__scadDispatchedCommands = [];
  });
}

export async function latestRecordedClientCommand(
  page: import("@playwright/test").Page,
  commandType: string,
): Promise<unknown | null> {
  return page.evaluate((type) => {
    const dispatched =
      (
        window as Window & {
          __scadDispatchedCommands?: Array<{ type: string; payload: unknown }>;
        }
      ).__scadDispatchedCommands ?? [];
    const directMatches = dispatched
      .filter((entry) => entry.type === type)
      .map((entry) => entry.payload);
    if (directMatches.length > 0) {
      return directMatches[directMatches.length - 1] ?? null;
    }
    const frames =
      (
        window as Window & {
          __scadOutgoingFrames?: Array<{ raw: string; parsed: unknown }>;
        }
      ).__scadOutgoingFrames ?? [];
    const matches = frames
      .map((frame) => frame.parsed as Record<string, unknown>)
      .filter((envelope) => envelope["type"] === "request")
      .map((envelope) => envelope["payload"] as Record<string, unknown> | undefined)
      .filter((payload): payload is Record<string, unknown> => Boolean(payload))
      .map((payload) => payload["command"] as Record<string, unknown> | undefined)
      .filter((command): command is Record<string, unknown> => Boolean(command))
      .filter((command) => command["type"] === type)
      .map((command) => command["payload"] ?? null);
    return matches.length > 0 ? matches[matches.length - 1] : null;
  }, commandType);
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
