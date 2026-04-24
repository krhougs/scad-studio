// Preset serialization + path derivation. 默认读写桌面端共享的
// `{ presets: { [name]: { [param]: value } } }`，读取时兼容旧版
// `{ version: 1, presets: [{ name, defines[] }] }`。

import {
  presets_parse_shared_file,
  presets_stringify_shared_file,
} from "@budn/studio-web-wasm";
import type { ParameterValue, PresetValueMap } from "./parameter-model";
import type { PresetEntry } from "./presets-panel";

export type PresetFile = {
  presets: PresetEntry[];
};

export function emptyPresetFile(): PresetFile {
  return { presets: [] };
}

export function parsePresetFile(text: string): PresetFile {
  const raw = JSON.parse(text) as unknown;
  if (!raw || typeof raw !== "object") {
    throw new Error("preset file is not an object");
  }
  const outer = raw as Record<string, unknown>;
  if (typeof outer["version"] === "number") {
    return parseLegacyPresetFile(outer);
  }
  const shared = presets_parse_shared_file(text) as Record<string, unknown>;
  const presets = shared["presets"];
  if (!presets || typeof presets !== "object" || Array.isArray(presets)) {
    throw new Error("preset file is missing presets object");
  }
  const out: PresetEntry[] = [];
  for (const [name, values] of Object.entries(presets as Record<string, unknown>)) {
    if (!values || typeof values !== "object" || Array.isArray(values)) continue;
    out.push({ name, values: normalizePresetValues(values) });
  }
  return { presets: out };
}

export function stringifyPresetFile(file: PresetFile): string {
  const presets = Object.fromEntries(
    file.presets.map((entry) => [entry.name, entry.values]),
  );
  return presets_stringify_shared_file({ presets });
}

export function derivePresetPath(sourcePath: unknown): unknown {
  if (!sourcePath || typeof sourcePath !== "object") return null;
  const record = sourcePath as Record<string, unknown>;
  const segments = record["path_segments"];
  if (!Array.isArray(segments) || segments.length === 0) return null;
  const last = segments[segments.length - 1];
  if (typeof last !== "string" || last.length === 0) return null;
  const nextLast = derivePresetFilename(last);
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
  const file = derivePresetFilename(last);
  return parent ? `${parent}/${file}` : file;
}

export function deriveLegacyPresetPath(sourcePath: unknown): unknown {
  const candidates = deriveLegacyPresetPaths(sourcePath);
  return candidates[candidates.length - 1] ?? null;
}

export function deriveLegacyPresetPaths(sourcePath: unknown): unknown[] {
  if (!sourcePath || typeof sourcePath !== "object") return [];
  const record = sourcePath as Record<string, unknown>;
  const segments = record["path_segments"];
  if (!Array.isArray(segments) || segments.length === 0) return [];
  const last = segments[segments.length - 1];
  if (typeof last !== "string" || last.length === 0) return [];
  const dotIndex = last.lastIndexOf(".");
  const stem = dotIndex > 0 ? last.slice(0, dotIndex) : last;
  const parentSegments = segments.slice(0, -1);
  return [
    { ...record, path_segments: parentSegments.concat([`${last}.presets.json`]) },
    { ...record, path_segments: parentSegments.concat([`${stem}.presets.json`]) },
  ];
}

function parseLegacyPresetFile(outer: Record<string, unknown>): PresetFile {
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
    out.push({ name, values: valuesFromDefines(defines) });
  }
  return { presets: out };
}

function derivePresetFilename(sourceName: string): string {
  const dotIndex = sourceName.lastIndexOf(".");
  const stem = dotIndex > 0 ? sourceName.slice(0, dotIndex) : sourceName;
  return `${stem}.scad.json`;
}

function valuesFromDefines(defines: unknown[]): PresetValueMap {
  const values: PresetValueMap = {};
  for (const define of defines) {
    if (typeof define !== "string") continue;
    const eqIndex = define.indexOf("=");
    if (eqIndex <= 0) continue;
    const name = define.slice(0, eqIndex).trim();
    if (!name) continue;
    values[name] = parsePresetValue(define.slice(eqIndex + 1).trim());
  }
  return values;
}

function parsePresetValue(value: string): ParameterValue {
  if (value === "true") return true;
  if (value === "false") return false;
  if (/^-?\d+(?:\.\d+)?$/.test(value)) {
    return Number(value);
  }
  if (
    value.length >= 2 &&
    value.startsWith('"') &&
    value.endsWith('"')
  ) {
    return value.slice(1, -1);
  }
  return value;
}

function normalizePresetValues(values: unknown): PresetValueMap {
  const out: PresetValueMap = {};
  for (const [key, value] of Object.entries(values as Record<string, unknown>)) {
    if (
      typeof value === "number" ||
      typeof value === "boolean" ||
      typeof value === "string"
    ) {
      out[key] = value;
    }
  }
  return out;
}
