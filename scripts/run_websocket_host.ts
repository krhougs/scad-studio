// 共享的 websocket-host 启动器。
// 所有 smoke / dev 入口都必须通过 `launchWebsocketHost` 启动，不得自行 cargo run。
//
// 端口与 URL 由 `SCAD_STUDIO_WS_URL` 控制；默认 `ws://127.0.0.1:38421`。
// `--workspace` 可通过参数或 `STUDIO_WEB_WORKSPACE` 覆盖，默认 `workspace/studio-web`。

import { existsSync, statSync } from "node:fs";
import { mkdir } from "node:fs/promises";
import path from "node:path";
import { waitForPort } from "./wait_for_port";

export const REPO_ROOT = path.resolve(import.meta.dir, "..");
export const DEFAULT_WS_URL = "ws://127.0.0.1:38421";
export const DEFAULT_WORKSPACE = path.join(REPO_ROOT, "workspace", "studio-web");

export type ParsedWsUrl = {
  hostname: string;
  port: number;
  bind: string;
  url: string;
};

export function resolveWsUrl(override?: string): ParsedWsUrl {
  const raw = override ?? process.env.SCAD_STUDIO_WS_URL ?? DEFAULT_WS_URL;
  const url = new URL(raw);
  if (url.protocol !== "ws:" && url.protocol !== "wss:") {
    throw new Error(`SCAD_STUDIO_WS_URL must be ws:// or wss://, got: ${raw}`);
  }
  const hostname = url.hostname;
  const port = Number(url.port || (url.protocol === "wss:" ? 443 : 80));
  if (!Number.isInteger(port) || port <= 0) {
    throw new Error(`invalid port in SCAD_STUDIO_WS_URL: ${raw}`);
  }
  return {
    hostname,
    port,
    bind: `${hostname}:${port}`,
    url: raw,
  };
}

export type LaunchedHost = {
  proc: ReturnType<typeof Bun.spawn>;
  bind: string;
  url: string;
  workspace: string;
  stop: () => Promise<void>;
};

export type LaunchOptions = {
  workspace?: string;
  wsUrl?: string;
  silent?: boolean;
  stdout?: "inherit" | "pipe" | "ignore";
  stderr?: "inherit" | "pipe" | "ignore";
  waitTimeoutMs?: number;
};

export async function launchWebsocketHost(
  options: LaunchOptions = {},
): Promise<LaunchedHost> {
  const parsed = resolveWsUrl(options.wsUrl);
  const workspace = path.resolve(
    REPO_ROOT,
    options.workspace ?? process.env.STUDIO_WEB_WORKSPACE ?? DEFAULT_WORKSPACE,
  );
  if (existsSync(workspace) && !statSync(workspace).isDirectory()) {
    throw new Error(`workspace path exists but is not a directory: ${workspace}`);
  }
  await mkdir(workspace, { recursive: true });

  const stdout = options.stdout ?? (options.silent ? "ignore" : "inherit");
  const stderr = options.stderr ?? (options.silent ? "ignore" : "inherit");

  const proc = Bun.spawn(
    [
      "cargo",
      "run",
      "-p",
      "app-server-host",
      "--bin",
      "websocket-host",
      "--",
      "--workspace",
      workspace,
      "--bind",
      parsed.bind,
    ],
    {
      cwd: REPO_ROOT,
      stdout,
      stderr,
      stdin: "ignore",
    },
  );

  const timeoutMs = options.waitTimeoutMs ?? 120_000;
  try {
    await waitForPort(parsed.hostname, parsed.port, { timeoutMs });
  } catch (err) {
    try {
      proc.kill();
    } catch {}
    throw err;
  }

  const stop = async () => {
    try {
      proc.kill();
    } catch {}
    try {
      await proc.exited;
    } catch {}
  };

  return {
    proc,
    bind: parsed.bind,
    url: parsed.url,
    workspace,
    stop,
  };
}

async function cliMain() {
  let workspace: string | undefined;
  let wsUrl: string | undefined;
  const argv = Bun.argv.slice(2);
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--workspace") workspace = argv[++i];
    else if (arg === "--ws-url") wsUrl = argv[++i];
    else if (arg === "-h" || arg === "--help") {
      console.log(
        "usage: bun scripts/run_websocket_host.ts [--workspace <path>] [--ws-url ws://host:port]",
      );
      process.exit(0);
    }
  }
  const host = await launchWebsocketHost({ workspace, wsUrl });
  console.log(`[websocket-host] listening on ${host.url} (workspace: ${host.workspace})`);

  const shutdown = async () => {
    await host.stop();
  };
  process.on("SIGINT", async () => {
    await shutdown();
    process.exit(0);
  });
  process.on("SIGTERM", async () => {
    await shutdown();
    process.exit(0);
  });
  const code = await host.proc.exited;
  process.exit(code ?? 0);
}

if (import.meta.main) {
  try {
    await cliMain();
  } catch (err) {
    console.error(err instanceof Error ? err.message : String(err));
    process.exit(1);
  }
}
