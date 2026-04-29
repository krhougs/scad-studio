// Studio web dev 启动器：websocket-host + WASM watch + Vite dev。

import path from "node:path";
import { watch } from "node:fs";
import { launchWebsocketHost, REPO_ROOT, resolveWsUrl } from "./run_websocket_host";

type Args = {
  workspace?: string;
  webPort: number;
  wsUrl?: string;
};

function parseArgs(argv: string[]): Args {
  let workspace: string | undefined;
  let webPort = Number(process.env.STUDIO_WEB_PORT ?? 5173);
  let wsUrl: string | undefined;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    switch (arg) {
      case "--workspace":
        workspace = argv[++i];
        break;
      case "--web-port":
        webPort = Number(argv[++i] ?? webPort);
        break;
      case "--ws-url":
        wsUrl = argv[++i];
        break;
      case "--help":
      case "-h":
        printUsage(0);
        break;
      default:
        console.error(`unknown argument: ${arg}`);
        printUsage(1);
    }
  }

  if (!Number.isInteger(webPort) || webPort <= 0) {
    console.error(`invalid --web-port value: ${webPort}`);
    process.exit(1);
  }
  return { workspace, webPort, wsUrl };
}

function printUsage(code: number): never {
  console.log(
    "usage: bun scripts/run_studio_web_dev.ts [--workspace <path>] [--web-port 5173] [--ws-url ws://host:port]",
  );
  console.log(
    "env overrides: STUDIO_WEB_WORKSPACE, STUDIO_WEB_PORT, SCAD_STUDIO_WS_URL",
  );
  process.exit(code);
}

const WASM_WATCH_DIRS = [
  path.join(REPO_ROOT, "crates", "app-server-protocol", "src"),
  path.join(REPO_ROOT, "crates", "app-server-protocol-wasm", "src"),
  path.join(REPO_ROOT, "crates", "studio-web-wasm", "src"),
  path.join(REPO_ROOT, "crates", "studio-common", "src"),
  path.join(REPO_ROOT, "crates", "scad-scene", "src"),
];

function startWasmWatch(): { stop: () => void; initialBuild: Promise<void> } {
  let building = false;
  let pending = false;
  let resolveInitial: () => void;
  const initialBuild = new Promise<void>((resolve) => {
    resolveInitial = resolve;
  });

  const buildStudioWebWasm = async (): Promise<boolean> => {
    const cargo = Bun.spawn(
      ["cargo", "build", "-p", "studio-web-wasm", "--target", "wasm32-unknown-unknown", "--release"],
      { cwd: REPO_ROOT, stdout: "inherit", stderr: "inherit", stdin: "ignore" },
    );
    if ((await cargo.exited) !== 0) return false;
    const bindgen = Bun.spawn(
      [
        "wasm-bindgen",
        path.join(REPO_ROOT, "target", "wasm32-unknown-unknown", "release", "studio_web_wasm.wasm"),
        "--target", "bundler",
        "--out-dir", path.join(REPO_ROOT, "packages", "studio-web-wasm", "generated"),
        "--out-name", "studio_web_wasm",
      ],
      { cwd: REPO_ROOT, stdout: "inherit", stderr: "inherit", stdin: "ignore" },
    );
    return (await bindgen.exited) === 0;
  };

  const build = async () => {
    if (building) {
      pending = true;
      return;
    }
    building = true;
    console.log("[wasm-watch] rebuilding wasm packages...");
    try {
      const protocolProc = Bun.spawn(
        ["bun", "run", "protocol:build"],
        { cwd: REPO_ROOT, stdout: "inherit", stderr: "inherit", stdin: "ignore" },
      );
      const [protocolCode, studioOk] = await Promise.all([
        protocolProc.exited,
        buildStudioWebWasm(),
      ]);
      if (protocolCode === 0 && studioOk) {
        console.log("[wasm-watch] build complete");
      } else {
        console.error("[wasm-watch] build failed (protocol=" + protocolCode + " studio-web=" + studioOk + ")");
      }
    } catch (err) {
      console.error("[wasm-watch] build error:", err instanceof Error ? err.message : String(err));
    } finally {
      building = false;
      resolveInitial();
      if (pending) {
        pending = false;
        build();
      }
    }
  };

  // 初始构建
  build();

  const watchers = WASM_WATCH_DIRS.map((dir) => {
    try {
      return watch(dir, { recursive: true }, (_event, filename) => {
        if (filename && filename.endsWith(".rs")) {
          build();
        }
      });
    } catch {
      console.warn(`[wasm-watch] cannot watch ${dir}`);
      return null;
    }
  });

  console.log("[wasm-watch] watching for Rust source changes");

  return {
    stop: () => {
      for (const w of watchers) {
        try {
          w?.close();
        } catch {}
      }
    },
    initialBuild,
  };
}

async function main() {
  const args = parseArgs(Bun.argv.slice(2));
  const parsed = resolveWsUrl(args.wsUrl);
  const directWsOverride = Boolean(args.wsUrl ?? process.env.SCAD_STUDIO_WS_URL);

  const wasmWatch = startWasmWatch();
  await wasmWatch.initialBuild;

  console.log(`[studio-web] starting websocket-host on ${parsed.bind}`);
  const host = await launchWebsocketHost({
    workspace: args.workspace,
    wsUrl: parsed.url,
  });

  const viteCwd = path.join(REPO_ROOT, "packages", "studio-web");
  console.log(`[studio-web] starting vite dev on http://0.0.0.0:${args.webPort}`);
  console.log(
    directWsOverride
      ? `[studio-web] frontend websocket override: ${parsed.url}`
      : "[studio-web] frontend websocket proxy: /app-server/ws",
  );
  const vite = Bun.spawn(
    [
      "bun",
      "x",
      "vite",
      "--port",
      String(args.webPort),
      "--host",
      "0.0.0.0",
      "--strictPort",
    ],
    {
      cwd: viteCwd,
      stdout: "inherit",
      stderr: "inherit",
      stdin: "ignore",
      env: {
        ...process.env,
        VITE_WS_PROXY_TARGET: parsed.url,
        ...(directWsOverride ? { VITE_WS_URL: parsed.url } : {}),
      },
    },
  );

  const shutdown = async () => {
    wasmWatch.stop();
    try {
      vite.kill();
    } catch {}
    await host.stop();
    try {
      await vite.exited;
    } catch {}
  };

  process.on("SIGINT", async () => {
    await shutdown();
    process.exit(0);
  });
  process.on("SIGTERM", async () => {
    await shutdown();
    process.exit(0);
  });

  const hostExit = host.proc.exited.then((code) => ({ source: "host", code }));
  const viteExit = vite.exited.then((code) => ({ source: "vite", code }));
  const first = await Promise.race([hostExit, viteExit]);
  console.error(
    `[studio-web] ${first.source} exited with code ${first.code}; shutting down`,
  );
  await shutdown();
  process.exit(first.code ?? 1);
}

try {
  await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
