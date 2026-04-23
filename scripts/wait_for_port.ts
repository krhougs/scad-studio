import { setTimeout as sleep } from "node:timers/promises";
import net from "node:net";

export async function waitForPort(
  hostname: string,
  port: number,
  options: { timeoutMs?: number; intervalMs?: number } = {},
) {
  const timeoutMs = options.timeoutMs ?? 30_000;
  const intervalMs = options.intervalMs ?? 200;
  const deadline = Date.now() + timeoutMs;

  while (Date.now() < deadline) {
    try {
      await new Promise<void>((resolve, reject) => {
        const socket = net.connect({ host: hostname, port });
        socket.once("connect", () => {
          socket.end();
          resolve();
        });
        socket.once("error", (error) => {
          socket.destroy();
          reject(error);
        });
      });
      return;
    } catch {
      await sleep(intervalMs);
    }
  }

  throw new Error(`port ${hostname}:${port} did not become ready within ${timeoutMs}ms`);
}

if (import.meta.main) {
  const [, , hostname, portText, timeoutText, intervalText] = Bun.argv;
  if (!hostname || !portText) {
    console.error(
      "usage: bun scripts/wait_for_port.ts <hostname> <port> [timeoutMs] [intervalMs]",
    );
    process.exit(1);
  }

  const port = Number(portText);
  const timeoutMs = timeoutText ? Number(timeoutText) : undefined;
  const intervalMs = intervalText ? Number(intervalText) : undefined;

  if (!Number.isInteger(port) || port <= 0) {
    console.error(`invalid port: ${portText}`);
    process.exit(1);
  }

  try {
    await waitForPort(hostname, port, { timeoutMs, intervalMs });
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  }
}
