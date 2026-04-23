// Decodes the CommandSuccess::FileRead envelope produced by the wasm client.
// Keeping the shape-handling localized here prevents viewers from reaching
// into protocol internals.

export type FileReadUtf8 = { kind: "utf8"; text: string; mediaType: string };
export type FileReadBinary = {
  kind: "binary";
  bytes: Uint8Array;
  mediaType: string;
};
export type FileReadDecoded = FileReadUtf8 | FileReadBinary;

export function decodeFileRead(response: unknown): FileReadDecoded | null {
  if (!response || typeof response !== "object") return null;
  const outer = response as Record<string, unknown>;
  const inner =
    (outer["payload"] as Record<string, unknown> | undefined) ?? outer;
  const contents = inner["contents"] as Record<string, unknown> | undefined;
  const mediaType =
    typeof inner["media_type"] === "string"
      ? (inner["media_type"] as string)
      : "application/octet-stream";
  if (!contents) return null;
  const contentKind = contents["kind"];
  const contentPayload = contents["payload"];
  if (contentKind === "utf8_text" && typeof contentPayload === "string") {
    return { kind: "utf8", text: contentPayload, mediaType };
  }
  if (contentKind === "binary") {
    if (contentPayload instanceof Uint8Array) {
      return { kind: "binary", bytes: contentPayload, mediaType };
    }
    if (Array.isArray(contentPayload)) {
      return {
        kind: "binary",
        bytes: Uint8Array.from(contentPayload as number[]),
        mediaType,
      };
    }
  }
  return null;
}

export function describeFileReadError(err: unknown): string {
  // ClientError serde shape: { "type": "protocol_error", "payload": { code, message } }
  // for variants that carry data, or { "type": "cancelled" } / "invalid_handle" / etc.
  // Earlier versions only looked at the outer level and missed the nested message.
  if (err && typeof err === "object") {
    const outer = err as Record<string, unknown>;
    const nested = outer["payload"];
    if (nested && typeof nested === "object") {
      const inner = nested as Record<string, unknown>;
      const innerMessage = inner["message"];
      if (typeof innerMessage === "string" && innerMessage.length > 0) {
        return innerMessage;
      }
      const innerCode = inner["code"];
      if (typeof innerCode === "string" && innerCode.length > 0) {
        return innerCode;
      }
    }
    const topMessage = outer["message"];
    if (typeof topMessage === "string" && topMessage.length > 0) {
      return topMessage;
    }
    const topType = outer["type"];
    if (typeof topType === "string" && topType.length > 0) return topType;
    const topCode = outer["code"];
    if (typeof topCode === "string" && topCode.length > 0) return topCode;
  }
  if (typeof err === "string") return err;
  return "unknown error";
}
