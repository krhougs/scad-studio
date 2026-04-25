import { readFileSync } from "node:fs";
import path from "node:path";
import { expect, test } from "@playwright/test";
import {
  initSync as initProtocolWasmSync,
  protocol_decode_client_frame,
  protocol_decode_server_frame,
} from "@budn/app-server-protocol";
import {
  clearServiceWorkerState,
  createHarness,
  REPO_ROOT,
} from "./_smoke-harness";

type RecordedFrame = {
  bytes: number[];
  t: number;
};

type PreviewRequestRecord = {
  key: string;
  requestId: string;
  source: unknown;
  defines: unknown;
  configuredOpenscadPath: unknown;
  t: number;
};

const HARNESS = createHarness({
  bindPort: 39214,
  vitePort: 5214,
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
  await installBidirectionalProtocolRecorder(page);
});

test("@preview-dedup opening scad emits one equivalent preview request", async ({
  page,
}) => {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
  await page.getByTestId("entry-examples").waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId("entry-examples").click();
  await page.getByTestId("entry-cube.scad").waitFor({
    state: "visible",
    timeout: 30_000,
  });

  await clearRecordedFrames(page);
  await page.getByTestId("entry-cube.scad").click();
  await waitForPreviewResponse(page);
  await page.waitForTimeout(500);

  const requests = await recordedPreviewRequests(page);
  const duplicates = duplicatePreviewRequests(requests);

  expect(duplicates, formatDuplicatePreviewRequests(duplicates)).toEqual([]);
});

async function installBidirectionalProtocolRecorder(
  page: import("@playwright/test").Page,
): Promise<void> {
  await page.addInitScript(() => {
    window.__scadOutgoingFrames = [];
    window.__scadIncomingFrames = [];
    if (window.__scadBidirectionalRecorderInstalled) return;
    window.__scadBidirectionalRecorderInstalled = true;

    const OriginalWebSocket = window.WebSocket;
    class PatchedWebSocket extends OriginalWebSocket {
      constructor(url: string | URL, protocols?: string | string[]) {
        super(url, protocols);
        this.addEventListener("message", (ev) => {
          const record = (bytes: Uint8Array) => {
            window.__scadIncomingFrames?.push({
              bytes: Array.from(bytes),
              t: performance.now(),
            });
          };
          if (ev.data instanceof ArrayBuffer) {
            record(new Uint8Array(ev.data));
          } else if (ArrayBuffer.isView(ev.data)) {
            record(new Uint8Array(ev.data.buffer, ev.data.byteOffset, ev.data.byteLength));
          } else if (ev.data instanceof Blob) {
            void ev.data.arrayBuffer().then((buffer) => record(new Uint8Array(buffer)));
          }
        });
      }

      send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void {
        let bytes: Uint8Array | null = null;
        if (data instanceof ArrayBuffer) {
          bytes = new Uint8Array(data);
        } else if (ArrayBuffer.isView(data)) {
          bytes = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
        }
        if (bytes) {
          window.__scadOutgoingFrames?.push({
            bytes: Array.from(bytes),
            t: performance.now(),
          });
        }
        return super.send(data);
      }
    }

    Object.defineProperty(PatchedWebSocket, "CONNECTING", {
      value: OriginalWebSocket.CONNECTING,
    });
    Object.defineProperty(PatchedWebSocket, "OPEN", {
      value: OriginalWebSocket.OPEN,
    });
    Object.defineProperty(PatchedWebSocket, "CLOSING", {
      value: OriginalWebSocket.CLOSING,
    });
    Object.defineProperty(PatchedWebSocket, "CLOSED", {
      value: OriginalWebSocket.CLOSED,
    });
    window.WebSocket = PatchedWebSocket;
  });
}

async function clearRecordedFrames(page: import("@playwright/test").Page): Promise<void> {
  await page.evaluate(() => {
    window.__scadOutgoingFrames = [];
    window.__scadIncomingFrames = [];
  });
}

async function recordedPreviewRequests(
  page: import("@playwright/test").Page,
): Promise<PreviewRequestRecord[]> {
  const frames = await page.evaluate(() => window.__scadOutgoingFrames ?? []);
  ensureProtocolWasm();
  return frames.flatMap((frame) => {
    const decoded = protocol_decode_client_frame(new Uint8Array(frame.bytes));
    const payload = previewRequestPayload(decoded);
    if (!payload) return [];
    return [{
      key: stablePreviewRequestKey(payload),
      requestId: payload.requestId,
      source: payload.source,
      defines: payload.defines,
      configuredOpenscadPath: payload.configuredOpenscadPath,
      t: frame.t,
    }];
  });
}

async function waitForPreviewResponse(
  page: import("@playwright/test").Page,
): Promise<void> {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const frames = await page.evaluate(() => window.__scadIncomingFrames ?? []);
    ensureProtocolWasm();
    if (
      frames.some((frame) => {
        const decoded = protocol_decode_server_frame(new Uint8Array(frame.bytes));
        return serverResponseType(decoded) === "preview_ready";
      })
    ) {
      return;
    }
    await page.waitForTimeout(100);
  }
  throw new Error("timed out waiting for preview_ready response");
}

function duplicatePreviewRequests(
  requests: PreviewRequestRecord[],
): PreviewRequestRecord[][] {
  const groups = new Map<string, PreviewRequestRecord[]>();
  for (const request of requests) {
    const group = groups.get(request.key) ?? [];
    group.push(request);
    groups.set(request.key, group);
  }
  return [...groups.values()].filter((group) => group.length > 1);
}

function formatDuplicatePreviewRequests(
  duplicates: PreviewRequestRecord[][],
): string {
  return duplicates
    .map((group) => {
      const first = group[0];
      const requestIds = group.map((item) => item.requestId).join(", ");
      const deltas = group.map((item) => Math.round(item.t - first.t)).join(", ");
      return [
        `duplicate preview.request key=${first.key}`,
        `requestIds=[${requestIds}]`,
        `timeDeltasMs=[${deltas}]`,
        `configuredOpenscadPath=${stableJson(first.configuredOpenscadPath)}`,
      ].join(" ");
    })
    .join("\n");
}

function previewRequestPayload(decoded: unknown): {
  requestId: string;
  source: unknown;
  defines: unknown;
  configuredOpenscadPath: unknown;
} | null {
  const root = asRecord(decoded);
  if (root?.["kind"] !== "request") return null;
  const payload = asRecord(root["payload"]);
  const command = asRecord(payload?.["command"]);
  if ((command?.["command"] ?? command?.["type"]) !== "preview.request") return null;
  const body = asRecord(command["payload"]);
  if (!body) return null;
  return {
    requestId: requestIdString(payload?.["request_id"]),
    source: body["source"],
    defines: body["defines"],
    configuredOpenscadPath: body["configured_openscad_path"] ?? null,
  };
}

function stablePreviewRequestKey(payload: {
  source: unknown;
  defines: unknown;
  configuredOpenscadPath: unknown;
}): string {
  return stableJson({
    source: payload.source,
    defines: Array.isArray(payload.defines) ? payload.defines : [],
    configuredOpenscadPath: payload.configuredOpenscadPath ?? null,
  });
}

function serverResponseType(decoded: unknown): string | null {
  const root = asRecord(decoded);
  if (root?.["kind"] !== "response") return null;
  const payload = asRecord(root["payload"]);
  const result = asRecord(payload?.["result"]);
  const ok = asRecord(result?.["Ok"]);
  return typeof ok?.["type"] === "string" ? ok["type"] : null;
}

function stableJson(value: unknown): string {
  return JSON.stringify(value, (_key, item) => {
    if (typeof item === "bigint") return item.toString();
    if (!item || typeof item !== "object" || Array.isArray(item)) return item;
    return Object.fromEntries(
      Object.entries(item as Record<string, unknown>).sort(([left], [right]) =>
        left.localeCompare(right),
      ),
    );
  });
}

function requestIdString(value: unknown): string {
  if (typeof value === "bigint") return value.toString();
  if (typeof value === "number") return String(value);
  const record = asRecord(value);
  const inner = record?.["0"];
  if (typeof inner === "bigint") return inner.toString();
  if (typeof inner === "number") return String(inner);
  return String(value);
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object"
    ? (value as Record<string, unknown>)
    : null;
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

declare global {
  interface Window {
    __scadOutgoingFrames?: RecordedFrame[];
    __scadIncomingFrames?: RecordedFrame[];
    __scadBidirectionalRecorderInstalled?: boolean;
  }
}
