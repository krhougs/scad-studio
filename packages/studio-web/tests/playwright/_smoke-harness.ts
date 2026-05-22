// Shared harness used by the Phase 7 Playwright specs. Every spec that imports
// this must pick a unique {bindPort, vitePort} pair to avoid port clashes when
// the dispatcher runs specs in parallel.

import { spawn, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createConnection } from "node:net";
import { homedir, tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  initSync as initProtocolWasmSync,
  protocol_decode_client_frame,
} from "@budn/app-server-protocol";

const SPEC_DIR = path.dirname(fileURLToPath(import.meta.url));
export const REPO_ROOT = path.resolve(SPEC_DIR, "..", "..", "..", "..");
export const HOST_WORKSPACE = path.join(
  REPO_ROOT,
  "tests",
  "studio-web-smoke-workspace",
);
const TEST_CADQUERY_RUNNER = path.join(
  REPO_ROOT,
  "target",
  "playwright-fake-cadquery-python.sh",
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
  /**
   * When true, the host subprocess inherits `process.env` directly — no
   * isolated HOME, no fake CadQuery runner.  Matches `bun run dev` behaviour.
   */
  rawEnv?: boolean;
};

export type HarnessHandle = {
  baseUrl: string;
  wsUrl: string;
  start: () => Promise<void>;
  stop: () => Promise<void>;
};

export type HostEnvHandle = {
  env: NodeJS.ProcessEnv;
  cleanup: () => void;
};

let protocolWasmReady = false;

export function createHarness(opts: HarnessOptions): HarnessHandle {
  const hostBind = `127.0.0.1:${opts.bindPort}`;
  const baseUrl = `http://127.0.0.1:${opts.vitePort}`;
  const wsUrl = `ws://${hostBind}`;
  const workspace = isolatedWorkspace(opts.workspacePath);
  const workspacePath = workspace.path;
  const hostEnv: HostEnvHandle = opts.rawEnv
    ? { env: loadRepoRootEnv(), cleanup: () => {} }
    : isolatedHostEnvWithTestCadqueryRunner(opts.hostEnv);
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
          env: hostEnv.env,
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
          env: { ...process.env, NODE_ENV: "development", VITE_WS_URL: wsUrl },
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
      hostEnv.cleanup();
      workspace.cleanup();
    },
  };
}

function loadRepoRootEnv(): NodeJS.ProcessEnv {
  const dotenv = path.join(REPO_ROOT, ".env");
  if (!existsSync(dotenv)) return { ...process.env };
  const merged: NodeJS.ProcessEnv = { ...process.env };
  for (const line of readFileSync(dotenv, "utf-8").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    const eq = trimmed.indexOf("=");
    if (eq < 1) continue;
    const key = trimmed.slice(0, eq);
    const val = trimmed.slice(eq + 1);
    if (!(key in merged)) merged[key] = val;
  }
  return merged;
}

function isolatedWorkspace(explicitPath?: string): { path: string; cleanup: () => void } {
  if (explicitPath) {
    return { path: explicitPath, cleanup: () => {} };
  }
  const tempWorkspace = mkdtempSync(
    path.join(tmpdir(), "scad-studio-smoke-workspace-"),
  );
  cpSync(HOST_WORKSPACE, tempWorkspace, { recursive: true });
  return {
    path: tempWorkspace,
    cleanup: () => rmSync(tempWorkspace, { recursive: true, force: true }),
  };
}

export function isolatedHostEnvWithTestCadqueryRunner(
  overrides: Record<string, string> = {},
): HostEnvHandle {
  const configEnv = isolatedHostConfigEnv(overrides);
  return {
    env: hostEnvWithTestCadqueryRunner(configEnv.env),
    cleanup: configEnv.cleanup,
  };
}

function isolatedHostConfigEnv(
  overrides: Record<string, string> = {},
): { env: Record<string, string>; cleanup: () => void } {
  if ("HOME" in overrides || "XDG_CONFIG_HOME" in overrides) {
    return { env: overrides, cleanup: () => {} };
  }
  const tempHome = mkdtempSync(path.join(tmpdir(), "scad-studio-smoke-home-"));
  const realHome = homedir();
  return {
    env: {
      ...overrides,
      HOME: tempHome,
      XDG_CONFIG_HOME: path.join(tempHome, ".config"),
      CARGO_HOME: process.env.CARGO_HOME ?? path.join(realHome, ".cargo"),
      RUSTUP_HOME: process.env.RUSTUP_HOME ?? path.join(realHome, ".rustup"),
    },
    cleanup: () => rmSync(tempHome, { recursive: true, force: true }),
  };
}

export function hostEnvWithTestCadqueryRunner(
  overrides: Record<string, string> = {},
): NodeJS.ProcessEnv {
  ensureTestCadqueryRunner();
  return {
    ...process.env,
    CADQUERY_RUNNER_PYTHON: TEST_CADQUERY_RUNNER,
    ...overrides,
  };
}

function ensureTestCadqueryRunner(): void {
  if (!existsSync(TEST_CADQUERY_RUNNER)) {
    writeFileSync(TEST_CADQUERY_RUNNER, "#!/bin/sh\nexit 0\n");
    chmodSync(TEST_CADQUERY_RUNNER, 0o755);
  }
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
      __scadOutgoingFrames?: Array<{ bytes: number[] }>;
      __scadProtocolRecorderInstalled?: boolean;
    };
    win.__scadOutgoingFrames = [];
    if (win.__scadProtocolRecorderInstalled) {
      return;
    }
    win.__scadProtocolRecorderInstalled = true;
    const originalSend = WebSocket.prototype.send;
    WebSocket.prototype.send = function patchedSend(data: Parameters<WebSocket["send"]>[0]) {
      let bytes: Uint8Array | null = null;
      if (data instanceof ArrayBuffer) {
        bytes = new Uint8Array(data);
      } else if (ArrayBuffer.isView(data)) {
        bytes = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
      }
      if (bytes) {
        win.__scadOutgoingFrames?.push({ bytes: Array.from(bytes) });
      }
      return originalSend.call(this, data);
    };
  });
}

export async function injectStoreState(
  page: import("@playwright/test").Page,
  patch: Record<string, unknown>,
): Promise<void> {
  await page.evaluate(async (state) => {
    const mod = await import(/* @vite-ignore */ "/src/state/protocol-store.ts");
    const store = (mod as Record<string, unknown>)["useProtocolStore"] as {
      setState: (patch: Record<string, unknown>) => void;
    };
    store.setState(state);
  }, patch);
}

export async function clearRecordedClientCommands(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.evaluate(() => {
    (
      window as Window & {
        __scadOutgoingFrames?: Array<{ bytes: number[] }>;
      }
    ).__scadOutgoingFrames = [];
  });
}

export async function latestRecordedClientCommand(
  page: import("@playwright/test").Page,
  commandType: string,
): Promise<unknown | null> {
  const frames = await page.evaluate(() => {
    const frames =
      (
        window as Window & {
          __scadOutgoingFrames?: Array<{ bytes: number[] }>;
        }
      ).__scadOutgoingFrames ?? [];
    return frames;
  });
  ensureProtocolWasm();
  const matches = frames
    .map((frame) => protocol_decode_client_frame(new Uint8Array(frame.bytes)))
    .map((envelope) => commandPayloadFromClientEnvelope(envelope, commandType))
    .filter((payload): payload is unknown => payload !== undefined);
  return matches.length > 0 ? matches[matches.length - 1] : null;
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

function ensureProtocolWasm(): void {
  if (protocolWasmReady) return;
  const wasmPath = path.join(
    REPO_ROOT,
    "packages",
    "app-server-protocol",
    "generated",
    "app_server_protocol_wasm_bg.wasm",
  );
  initProtocolWasmSync({ module: readFileSync(wasmPath) });
  protocolWasmReady = true;
}

function commandPayloadFromClientEnvelope(
  envelope: unknown,
  commandType: string,
): unknown | undefined {
  const root = asRecord(envelope);
  if (root?.["kind"] !== "request" && root?.["type"] !== "request") return undefined;
  const payload = asRecord(root["payload"]);
  const command = asRecord(payload?.["command"]);
  if (!command) return undefined;
  const observed = command?.["command"] ?? command?.["type"];
  if (observed !== commandType && observed !== commandType.replaceAll("_", ".")) {
    return undefined;
  }
  return command["payload"] ?? null;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object"
    ? (value as Record<string, unknown>)
    : null;
}

export async function waitForTestClient(
  page: import("@playwright/test").Page,
  timeoutMs = 30_000,
): Promise<void> {
  await page.waitForFunction(
    () => !!(window as Window & { __budn_test_client?: unknown }).__budn_test_client,
    { timeout: timeoutMs },
  );
}

export async function dispatchSelectionUpdateInPage(
  page: import("@playwright/test").Page,
  params: unknown,
): Promise<void> {
  await waitForTestClient(page);
  await page.evaluate(async (selectionParams) => {
    const client = (window as Window & { __budn_test_client?: { dispatchSelectionUpdate(p: unknown): Promise<unknown> } })
      .__budn_test_client;
    if (!client) throw new Error("__budn_test_client not available (dev mode only)");
    await client.dispatchSelectionUpdate(selectionParams);
  }, params);
}

export async function waitForAssistantMessage(
  page: import("@playwright/test").Page,
  timeoutMs = 120_000,
  minLength = 10,
): Promise<string> {
  const selector = '[data-testid="chat-body"] .msg.agent .bubble';
  let text = "";
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const locator = page.locator(selector).last();
    const visible = await locator.isVisible().catch(() => false);
    if (visible) {
      text = await locator.innerText().catch(() => "");
      if (text.length >= minLength) return text;
    }
    await delay(500);
  }
  if (text.length > 0) return text;
  throw new Error(`no assistant message with >=${minLength} chars after ${timeoutMs}ms`);
}
