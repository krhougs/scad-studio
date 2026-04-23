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
  if (err && typeof err === "object") {
    const any = err as Record<string, unknown>;
    const message = any["message"];
    if (typeof message === "string" && message.length > 0) return message;
    const code = any["code"];
    if (typeof code === "string") return code;
  }
  if (typeof err === "string") return err;
  return "unknown error";
}
