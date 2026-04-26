import { cpSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
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
  HOST_WORKSPACE,
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

const TEST_WORKSPACE = mkdtempSync(
  path.join(tmpdir(), "scad-studio-preview-dedup-"),
);
cpSync(HOST_WORKSPACE, TEST_WORKSPACE, { recursive: true });
const PARAMS_CUBE_SCAD_JSON = path.join(
  TEST_WORKSPACE,
  "examples",
  "params-cube.scad.json",
);

const HARNESS = createHarness({
  bindPort: 39214,
  vitePort: 5214,
  workspacePath: TEST_WORKSPACE,
});

let protocolWasmReady = false;

test.beforeAll(async () => {
  await HARNESS.start();
});

test.afterAll(async () => {
  await HARNESS.stop();
  rmSync(TEST_WORKSPACE, { recursive: true, force: true });
});

test.beforeEach(async ({ page }) => {
  rmSync(PARAMS_CUBE_SCAD_JSON, { force: true });
  await clearServiceWorkerState(page);
  await installBidirectionalProtocolRecorder(page);
});

test("@preview-dedup opening scad emits one equivalent preview request", async ({
  page,
}) => {
  await clearRecordedFrames(page);
  await openExampleScad(page, "cube.scad");
  await waitForPreviewResponse(page);
  await page.waitForTimeout(500);

  const requests = await recordedPreviewRequests(page);
  singleRequestForSource(requests, "cube.scad");
  const duplicates = duplicatePreviewRequests(requests);

  expect(duplicates, formatDuplicatePreviewRequests(duplicates)).toEqual([]);
});

test("@preview-dedup parameter changes still emit updated preview request", async ({
  page,
}) => {
  await clearRecordedFrames(page);
  await openExampleScad(page, "params-cube.scad");
  await waitForPreviewResponse(page);
  await page.waitForTimeout(500);

  const initial = singleRequestForSource(
    await recordedPreviewRequests(page),
    "params-cube.scad",
  );
  expect(initial.defines).toEqual([
    "size=10",
    "wall=2",
    "enabled=true",
    'mode="draft"',
  ]);

  await clearRecordedFrames(page);
  const inspector = page.getByTestId("workbench-inspector");
  const sizeField = inspector
    .getByTestId("parameter-number-field-size")
    .getByRole("spinbutton");
  await sizeField.fill("24");
  await waitForPreviewResponse(page);
  await page.waitForTimeout(500);

  const updated = singleRequestForSource(
    await recordedPreviewRequests(page),
    "params-cube.scad",
  );
  expect(updated.requestId).not.toBe(initial.requestId);
  expect(updated.defines).toEqual([
    "size=24",
    "wall=2",
    "enabled=true",
    'mode="draft"',
  ]);
});

test("@preview-dedup appearance changes do not emit preview request", async ({
  page,
}) => {
  await clearRecordedFrames(page);
  await openExampleScad(page, "params-cube.scad");
  await waitForPreviewResponse(page);
  await page.waitForTimeout(500);

  await clearRecordedFrames(page);
  const inspector = page.getByTestId("workbench-inspector");
  await inspector.getByTestId("preview-background-color").fill("#20242b");
  await expect(page.getByTestId("mesh-canvas")).toHaveAttribute(
    "data-preview-background",
    "#20242b",
  );
  await inspector.getByTestId("preview-point-light-mode-manual").click();
  await expect(page.getByTestId("mesh-canvas")).toHaveAttribute(
    "data-point-light-mode",
    "manual",
  );
  await inspector.getByTestId("preview-point-light-mode-auto").click();
  await expect(page.getByTestId("mesh-canvas")).toHaveAttribute(
    "data-point-light-mode",
    "auto",
  );
  await inspector.getByTestId("preview-point-light-mode-manual").click();
  await expect(page.getByTestId("mesh-canvas")).toHaveAttribute(
    "data-point-light-mode",
    "manual",
  );
  await inspector
    .getByTestId("preview-point-light-x-number-field")
    .getByRole("spinbutton")
    .fill("11");
  await inspector
    .getByTestId("preview-point-light-y-number-field")
    .getByRole("spinbutton")
    .fill("22");
  await inspector
    .getByTestId("preview-point-light-z-number-field")
    .getByRole("spinbutton")
    .fill("33");
  await inspector.getByTestId("preview-point-light-reset").click();
  await expect(page.getByTestId("mesh-canvas")).toHaveAttribute(
    "data-effective-point-light-mode",
    "manual",
  );
  await inspector.getByTestId("preview-point-light-mode-auto").click();
  await expect(page.getByTestId("mesh-canvas")).toHaveAttribute(
    "data-point-light-mode",
    "auto",
  );
  await inspector.getByTestId("preview-point-light-mode-off").click();
  await expect(page.getByTestId("mesh-canvas")).toHaveAttribute(
    "data-point-light-mode",
    "off",
  );
  await page.waitForTimeout(700);

  expect(await recordedPreviewRequests(page)).toEqual([]);
});

test("@preview-dedup external scad settings refresh does not emit preview request", async ({
  page,
}) => {
  await clearRecordedFrames(page);
  await openExampleScad(page, "params-cube.scad");
  await waitForPreviewResponse(page);
  await page.waitForTimeout(500);

  await clearRecordedFrames(page);
  await writeFile(
    PARAMS_CUBE_SCAD_JSON,
    JSON.stringify(
      {
        presets: {
          external: {
            size: 18,
          },
        },
        previewAppearance: {
          backgroundColor: "#20242b",
          gridMajorColor: "#7c8795",
          gridMinorColor: "#46505d",
          lightingIntensity: 1.6,
        },
      },
      null,
      2,
    ),
  );
  await expect(page.getByTestId("preset-row-external")).toBeVisible({
    timeout: 15_000,
  });
  await page.waitForTimeout(700);

  expect(await recordedPreviewRequests(page)).toEqual([]);
});

test("@preview-dedup scad refresh emits one equivalent preview request", async ({
  page,
}) => {
  await clearRecordedFrames(page);
  await openExampleScad(page, "cube.scad");
  await waitForPreviewResponse(page);
  await page.waitForTimeout(500);

  await clearRecordedFrames(page);
  const cubePath = path.join(TEST_WORKSPACE, "examples", "cube.scad");
  const source = await readFile(cubePath, "utf-8");
  await writeFile(cubePath, `${source}\n// refresh ${Date.now()}\n`);
  await waitForPreviewResponse(page);
  await page.waitForTimeout(700);

  const requests = await recordedPreviewRequests(page);
  singleRequestForSource(requests, "cube.scad");
  const duplicates = duplicatePreviewRequests(requests);

  expect(duplicates, formatDuplicatePreviewRequests(duplicates)).toEqual([]);
});

async function openExampleScad(
  page: import("@playwright/test").Page,
  filename: string,
): Promise<void> {
  await page.goto(`${HARNESS.baseUrl}/?ws=${encodeURIComponent(HARNESS.wsUrl)}&left-panel=files`);
  await page.getByTestId("entry-examples").waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId("entry-examples").click();
  await page.getByTestId(`entry-${filename}`).waitFor({
    state: "visible",
    timeout: 30_000,
  });
  await page.getByTestId(`entry-${filename}`).click();
}

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

function singleRequestForSource(
  requests: PreviewRequestRecord[],
  filename: string,
): PreviewRequestRecord {
  const matches = requests.filter((request) => {
    const source = asRecord(request.source);
    const segments = source?.["path_segments"];
    return Array.isArray(segments) && segments.at(-1) === filename;
  });
  expect(matches, `preview requests for ${filename}`).toHaveLength(1);
  return matches[0];
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
  if (!command) return null;
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
