import {
  initProtocolWasm,
  protocol_path_handle,
  type PathHandle,
} from "@budn/app-server-protocol";
import protocolWasmUrl from "@budn/app-server-protocol/generated/app_server_protocol_wasm_bg.wasm?url";
import { pathSegments } from "./path-utils";

let protocolWasmReady: Promise<void> | null = null;

export async function resolveSiblingOutputPath(
  source: unknown,
  filename: string,
): Promise<PathHandle> {
  await ensureProtocolWasm();
  const workspaceId = workspaceIdFromPath(source);
  const parent = pathSegments(source).slice(0, -1);
  return protocol_path_handle(workspaceId, parent.concat([filename])) as PathHandle;
}

async function ensureProtocolWasm(): Promise<void> {
  protocolWasmReady ??= initProtocolWasm({
    module_or_path: new URL(protocolWasmUrl, window.location.href),
  }).then(() => undefined);
  await protocolWasmReady;
}

function workspaceIdFromPath(path: unknown): string {
  if (!path || typeof path !== "object") {
    throw new Error("source path missing workspace_id");
  }
  const raw = (path as Record<string, unknown>)["workspace_id"];
  if (typeof raw === "string") return raw;
  if (raw && typeof raw === "object") {
    const inner = (raw as Record<string, unknown>)["0"];
    if (typeof inner === "string") return inner;
  }
  throw new Error("source path missing workspace_id");
}
