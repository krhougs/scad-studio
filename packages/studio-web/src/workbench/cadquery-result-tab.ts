import type { CadQueryResultReady } from "@budn/app-server-protocol";

export type CadQueryResultTabPath = {
  type: "cadquery_result";
  result_id: string;
};

export function extractCadQueryReadyFromAgentEvent(
  event: unknown,
): CadQueryResultReady | null {
  if (!event || typeof event !== "object") return null;
  const record = event as Record<string, unknown>;
  if (record["event"] !== "agent.mesh_ready") return null;
  const payload = objectRecord(record["payload"]);
  const result = objectRecord(payload?.["result"]);
  const resultId = result?.["result_id"];
  if (typeof resultId !== "string" || resultId.length === 0) return null;
  return result as unknown as CadQueryResultReady;
}

export function cadQueryResultIdFromPath(path: unknown): string | null {
  const record = objectRecord(path);
  const resultId = record?.["result_id"];
  return record?.["type"] === "cadquery_result" && typeof resultId === "string"
    ? resultId
    : null;
}

function objectRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object"
    ? (value as Record<string, unknown>)
    : null;
}
