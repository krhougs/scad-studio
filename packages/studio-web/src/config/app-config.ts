import type { AppConfigDto, ConfigSaveRequest } from "@budn/app-server-protocol";

export type SlicerRow = { name: string; path: string };
export type DisplayUnit = "millimeter" | "centimeter" | "inch";

export type AppConfigShape = {
  openscad_path?: string | null;
  slicers?: SlicerRow[];
  recent_workspaces?: string[];
  floating_panel_opacity?: number;
  left_panel_width?: number;
  right_panel_width?: number;
  display_unit?: DisplayUnit;
  camera_overlay_pos?: [number, number] | null;
  camera_overlay_size?: [number, number] | null;
  param_panel_pos?: [number, number] | null;
  param_panel_size?: [number, number] | null;
  log_panel_pos?: [number, number] | null;
  log_panel_size?: [number, number] | null;
};

export type AppConfigState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "error"; message: string }
  | { kind: "ready"; config: AppConfigShape; raw: string; source: "load" | "save" };

export function decodeConfigLoad(response: unknown): {
  config: AppConfigShape;
  raw: string;
} {
  const outer = (response ?? {}) as Record<string, unknown>;
  const inner = (outer["payload"] as Record<string, unknown> | undefined) ?? outer;
  const config = inner["config"];
  if (!config || typeof config !== "object") {
    throw new Error("config is not an object");
  }
  const normalized = normalizeAppConfig(config as AppConfigShape);
  return { config: normalized, raw: encodeConfigRaw(normalized) };
}

export function normalizeAppConfig(config: AppConfigShape): AppConfigShape {
  return {
    ...config,
    openscad_path: normalizePath(config.openscad_path),
    slicers: normalizeSlicers(config.slicers),
    recent_workspaces: Array.isArray(config.recent_workspaces)
      ? config.recent_workspaces.filter((item): item is string => typeof item === "string")
      : [],
    floating_panel_opacity: normalizeOpacity(config.floating_panel_opacity),
    left_panel_width: clampNumber(config.left_panel_width, 280, 640, 360),
    right_panel_width: clampNumber(config.right_panel_width, 280, 640, 320),
    display_unit: normalizeDisplayUnit(config.display_unit),
  };
}

export function configuredSlicerRecords(config: AppConfigShape): SlicerRow[] {
  return normalizeSlicers(config.slicers).filter(
    (item) => item.name.length > 0 && item.path.length > 0,
  );
}

export function toConfigSaveRequest(config: AppConfigShape): ConfigSaveRequest {
  return { config: toAppConfigDto(config) };
}

export function toAppConfigDto(config: AppConfigShape): AppConfigDto {
  const normalized = normalizeAppConfig(config);
  return {
    openscad_path: normalizePath(normalized.openscad_path),
    slicers: normalizeSlicers(normalized.slicers),
    recent_workspaces: normalizeHostLocalPaths(normalized.recent_workspaces),
    floating_panel_opacity: normalized.floating_panel_opacity ?? 0.85,
    left_panel_width: normalized.left_panel_width ?? 360,
    right_panel_width: normalized.right_panel_width ?? 320,
    display_unit: normalizeDisplayUnit(normalized.display_unit),
    camera_overlay_pos: normalizeVec2(normalized.camera_overlay_pos),
    camera_overlay_size: normalizeVec2(normalized.camera_overlay_size),
    param_panel_pos: normalizeVec2(normalized.param_panel_pos),
    param_panel_size: normalizeVec2(normalized.param_panel_size),
    log_panel_pos: normalizeVec2(normalized.log_panel_pos),
    log_panel_size: normalizeVec2(normalized.log_panel_size),
  };
}

export function encodeConfigRaw(config: AppConfigShape): string {
  return JSON.stringify(toAppConfigDto(config));
}

export function describeConfigGaps(config: AppConfigShape): string[] {
  const gaps: string[] = [];
  if (!normalizePath(config.openscad_path)) {
    gaps.push("openscad path missing");
  }
  if (configuredSlicerRecords(config).length === 0) {
    gaps.push("no slicer configured");
  }
  return gaps;
}

function normalizeSlicers(raw: SlicerRow[] | undefined): SlicerRow[] {
  if (!Array.isArray(raw)) return [];
  return raw
    .filter((item): item is SlicerRow => Boolean(item) && typeof item === "object")
    .map((item) => ({
      name: typeof item.name === "string" ? item.name.trim() : "",
      path: typeof item.path === "string" ? item.path.trim() : "",
    }));
}

function normalizePath(value: string | null | undefined): string | null {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function normalizeHostLocalPaths(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value
    .filter((item): item is string => typeof item === "string")
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function normalizeDisplayUnit(value: unknown): DisplayUnit {
  return value === "centimeter" || value === "inch" || value === "millimeter"
    ? value
    : "millimeter";
}

function clampNumber(
  value: unknown,
  min: number,
  max: number,
  fallback: number,
): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return fallback;
  return Math.min(max, Math.max(min, value));
}

function normalizeOpacity(value: unknown): number {
  const clamped = clampNumber(value, 0, 1, 0.85);
  return Math.round(clamped * 100) / 100;
}

function normalizeVec2(value: unknown): [number, number] | null {
  if (!Array.isArray(value) || value.length !== 2) return null;
  const [x, y] = value;
  if (typeof x !== "number" || typeof y !== "number") return null;
  if (!Number.isFinite(x) || !Number.isFinite(y)) return null;
  return [x, y];
}
