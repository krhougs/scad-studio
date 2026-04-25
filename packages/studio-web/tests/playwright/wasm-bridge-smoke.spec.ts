// Browser-level wasm bridge smoke. This replaces the old wasm-pack Chrome
// smoke in the aggregate web smoke path so Playwright owns browser startup.

import { readFileSync } from "node:fs";
import path from "node:path";
import { expect, test } from "@playwright/test";
import {
  initSync as initProtocolWasmSync,
  protocol_decode_client_frame,
} from "@budn/app-server-protocol";
import {
  clearServiceWorkerState,
  createHarness,
  installProtocolRecorder,
  REPO_ROOT,
} from "./_smoke-harness";

const HARNESS = createHarness({
  bindPort: 39190,
  vitePort: 5190,
});
let protocolWasmReady = false;

test.beforeAll(async () => {
  await HARNESS.start();
});

test.afterAll(async () => {
  await HARNESS.stop();
});

test.beforeEach(async ({ page }) => {
  await clearServiceWorkerState(page);
  await installProtocolRecorder(page);
});

test("@wasm-bridge browser client emits decodable Borsh frames", async ({
  page,
}) => {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}`);

  await expect(page.getByTestId("workspace-name")).not.toHaveText("(loading)", {
    timeout: 30_000,
  });

  const frames = await page.evaluate(() => {
    return (
      window as Window & {
        __scadOutgoingFrames?: Array<{ bytes: number[] }>;
      }
    ).__scadOutgoingFrames ?? [];
  });

  expect(frames.length).toBeGreaterThanOrEqual(2);
  const decoded = await decodeFrames(frames);
  expect(decoded.some((frame) => frame.kind === "handshake")).toBe(true);
  expect(
    decoded.some((frame) => {
      if (frame.kind !== "request") return false;
      const payload = frame.payload as Record<string, unknown>;
      const command = payload["command"] as Record<string, unknown> | undefined;
      return command?.["command"] === "workspace.current";
    }),
  ).toBe(true);
});

async function decodeFrames(
  frames: Array<{ bytes: number[] }>,
): Promise<Array<Record<string, unknown>>> {
  ensureProtocolWasm();
  return frames.map((frame) =>
    protocol_decode_client_frame(new Uint8Array(frame.bytes)) as Record<string, unknown>,
  );
}

function ensureProtocolWasm(): void {
  if (protocolWasmReady) return;
  initProtocolWasmSync({
    module: readFileSync(
      path.join(
        REPO_ROOT,
        "packages",
        "app-server-protocol",
        "generated",
        "app_server_protocol_wasm_bg.wasm",
      ),
    ),
  });
  protocolWasmReady = true;
}
