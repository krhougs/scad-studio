// Preset serialization + path derivation for the Phase 7 parameters/presets
// flow. The on-disk shape is intentionally thin: a `version` sentinel and an
// array of { name, defines: string[] }. Defines are the same `name=value`
// strings a user types into the parameters panel; the preview pipeline hands
// them to PreviewRequest.defines verbatim.

import type { PresetEntry } from "./presets-panel";

export type PresetFile = {
  version: 1;
  presets: PresetEntry[];
};

export function emptyPresetFile(): PresetFile {
  return { version: 1, presets: [] };
}

export function parsePresetFile(text: string): PresetFile {
  const raw = JSON.parse(text) as unknown;
  if (!raw || typeof raw !== "object") {
    throw new Error("preset file is not an object");
  }
  const outer = raw as Record<string, unknown>;
  if (outer["version"] !== 1) {
    throw new Error(`unsupported preset version: ${String(outer["version"])}`);
  }
  const presets = outer["presets"];
  if (!Array.isArray(presets)) {
    throw new Error("preset file is missing presets array");
  }
  const out: PresetEntry[] = [];
  for (const entry of presets) {
    if (!entry || typeof entry !== "object") continue;
    const row = entry as Record<string, unknown>;
    const name = row["name"];
    const defines = row["defines"];
    if (typeof name !== "string" || !Array.isArray(defines)) continue;
    const strings = defines.filter((item): item is string => typeof item === "string");
    out.push({ name, defines: strings });
  }
  return { version: 1, presets: out };
}

export function stringifyPresetFile(file: PresetFile): string {
  return `${JSON.stringify(file, null, 2)}\n`;
}

// `<source>.presets.json` on the same workspace-relative path. Mirrors the
// desktop convention in crates/studio-common for preset paths.
export function derivePresetPath(sourcePath: unknown): unknown {
  if (!sourcePath || typeof sourcePath !== "object") return null;
  const record = sourcePath as Record<string, unknown>;
  const segments = record["path_segments"];
  if (!Array.isArray(segments) || segments.length === 0) return null;
  const last = segments[segments.length - 1];
  if (typeof last !== "string" || last.length === 0) return null;
  const nextLast = `${last}.presets.json`;
  const nextSegments = segments.slice(0, -1).concat([nextLast]);
  return { ...record, path_segments: nextSegments };
}

export function derivePresetPathLabel(sourcePath: unknown): string {
  if (!sourcePath || typeof sourcePath !== "object") return "";
  const record = sourcePath as Record<string, unknown>;
  const segments = record["path_segments"];
  if (!Array.isArray(segments)) return "";
  const strings = segments.filter((item): item is string => typeof item === "string");
  if (strings.length === 0) return "";
  const last = strings[strings.length - 1];
  const parent = strings.slice(0, -1).join("/");
  const file = `${last}.presets.json`;
  return parent ? `${parent}/${file}` : file;
}
