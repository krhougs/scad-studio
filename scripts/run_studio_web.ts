import { existsSync, statSync } from "node:fs";
import { mkdir, rm } from "node:fs/promises";
import path from "node:path";
import { waitForPort } from "./wait_for_port";

type Args = {
  workspace: string;
  bind: string;
  webPort: number;
  outDir: string;
};

const root = path.resolve(import.meta.dir, "..");
const defaultWorkspace = path.join(root, "workspace", "studio-web");

function parseArgs(argv: string[]): Args {
  let workspace = process.env.STUDIO_WEB_WORKSPACE ?? defaultWorkspace;
  let bind = process.env.STUDIO_WEB_BIND ?? "127.0.0.1:39180";
  let webPort = Number(process.env.STUDIO_WEB_PORT ?? 8010);
  let outDir = path.resolve(
    root,
    process.env.STUDIO_WEB_OUT_DIR ?? path.join("target", "studio-web-shell"),
  );

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    switch (arg) {
      case "--workspace":
        workspace = argv[++i] ?? "";
        break;
      case "--bind":
        bind = argv[++i] ?? bind;
        break;
      case "--web-port":
        webPort = Number(argv[++i] ?? webPort);
        break;
      case "--out-dir":
        outDir = path.resolve(root, argv[++i] ?? outDir);
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

  const workspacePath = path.resolve(root, workspace);
  if (existsSync(workspacePath) && !isDirectory(workspacePath)) {
    console.error(`workspace path exists but is not a directory: ${workspacePath}`);
    process.exit(1);
  }

  const [, portText] = bind.split(":");
  const hostPort = Number(portText);
  if (!Number.isInteger(hostPort) || hostPort <= 0) {
    console.error(`invalid --bind value: ${bind}`);
    process.exit(1);
  }

  if (!Number.isInteger(webPort) || webPort <= 0) {
    console.error(`invalid --web-port value: ${webPort}`);
    process.exit(1);
  }

  return {
    workspace: workspacePath,
    bind,
    webPort,
    outDir,
  };
}

function printUsage(code: number): never {
  console.log(
    "usage: bun scripts/run_studio_web.ts [--workspace <path>] [--bind 127.0.0.1:39180] [--web-port 8010] [--out-dir target/studio-web-shell]",
  );
  console.log(`default workspace: ${defaultWorkspace}`);
  console.log(
    "env overrides: STUDIO_WEB_WORKSPACE, STUDIO_WEB_BIND, STUDIO_WEB_PORT, STUDIO_WEB_OUT_DIR",
  );
  process.exit(code);
}

async function runCommand(cmd: string[], cwd = root) {
  const proc = Bun.spawn(cmd, {
    cwd,
    stdout: "inherit",
    stderr: "inherit",
    stdin: "ignore",
  });
  const exitCode = await proc.exited;
  if (exitCode !== 0) {
    throw new Error(`${cmd.join(" ")} failed with exit code ${exitCode}`);
  }
}

async function buildShell(outDir: string) {
  const wasmPath = path.join(
    root,
    "target",
    "wasm32-unknown-unknown",
    "debug",
    "studio_web_wasm.wasm",
  );
  const indexPath = path.join(root, "crates", "studio-web-wasm", "web", "index.html");

  await runCommand([
    "cargo",
    "build",
    "-p",
    "studio-web-wasm",
    "--features",
    "legacy-shell",
    "--target",
    "wasm32-unknown-unknown",
  ]);

  await rm(outDir, { recursive: true, force: true });
  await mkdir(outDir, { recursive: true });

  await runCommand(["wasm-bindgen", wasmPath, "--target", "web", "--out-dir", outDir]);
  await Bun.write(path.join(outDir, "index.html"), Bun.file(indexPath));
}

function createStaticServer(outDir: string, webPort: number) {
  const server = Bun.serve({
    port: webPort,
    async fetch(request) {
      const url = new URL(request.url);
      const pathname = url.pathname === "/" ? "/index.html" : url.pathname;
      const filePath = path.join(outDir, pathname.replace(/^\/+/, ""));
      if (!existsSync(filePath)) {
        return new Response("Not Found", { status: 404 });
      }
      return new Response(Bun.file(filePath));
    },
  });
  return server;
}

async function main() {
  const args = parseArgs(Bun.argv.slice(2));
  const [hostName, portText] = args.bind.split(":");
  const hostPort = Number(portText);

  if (!(await commandExists("wasm-bindgen"))) {
    throw new Error("missing wasm-bindgen CLI; install it with: cargo install wasm-bindgen-cli");
  }

  await mkdir(args.workspace, { recursive: true });

  console.log(`[studio-web] building shell into ${args.outDir}`);
  await buildShell(args.outDir);

  console.log(`[studio-web] starting websocket host on ${args.bind}`);
  const hostProc = Bun.spawn(
    [
      "cargo",
      "run",
      "-p",
      "app-server-host",
      "--bin",
      "websocket-host",
      "--",
      "--workspace",
      args.workspace,
      "--bind",
      args.bind,
    ],
    {
      cwd: root,
      stdout: "inherit",
      stderr: "inherit",
      stdin: "ignore",
    },
  );

  const stop = async () => {
    try {
      hostProc.kill();
    } catch {}
    await staticServer.stop(true);
  };

  const staticServer = createStaticServer(args.outDir, args.webPort);

  process.on("SIGINT", async () => {
    await stop();
    process.exit(0);
  });
  process.on("SIGTERM", async () => {
    await stop();
    process.exit(0);
  });

  hostProc.exited.then(async (code) => {
    if (code !== 0) {
      console.error(`[studio-web] websocket host exited unexpectedly with code ${code}`);
      await staticServer.stop(true);
      process.exit(code ?? 1);
    }
  });

  await waitForPort(hostName, hostPort);

  const appUrl = `http://127.0.0.1:${args.webPort}/index.html?ws=ws://${args.bind}`;
  console.log(`[studio-web] static server ready at http://127.0.0.1:${args.webPort}`);
  console.log(`[studio-web] open ${appUrl}`);

  await new Promise(() => {});
}

function isDirectory(filePath: string) {
  try {
    return statSync(filePath).isDirectory();
  } catch {
    return false;
  }
}

async function commandExists(command: string) {
  const proc = Bun.spawn(["sh", "-lc", `command -v ${command}`], {
    cwd: root,
    stdout: "ignore",
    stderr: "ignore",
    stdin: "ignore",
  });
  return (await proc.exited) === 0;
}

try {
  await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
