export type SlicerRow = { name: string; path: string };

export type AppConfigShape = {
  openscad_path?: string | null;
  slicers?: SlicerRow[];
  recent_workspaces?: string[];
  floating_panel_opacity?: number;
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
  const json = inner["json"];
  if (typeof json !== "string") {
    throw new Error("ConfigLoadResponse.json missing");
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    throw new Error("ConfigLoadResponse.json is not valid JSON");
  }
  if (!parsed || typeof parsed !== "object") {
    throw new Error("config is not an object");
  }
  return { config: normalizeAppConfig(parsed as AppConfigShape), raw: json };
}

export function normalizeAppConfig(config: AppConfigShape): AppConfigShape {
  return {
    ...config,
    openscad_path: normalizePath(config.openscad_path),
    slicers: normalizeSlicers(config.slicers),
    recent_workspaces: Array.isArray(config.recent_workspaces)
      ? config.recent_workspaces.filter((item): item is string => typeof item === "string")
      : [],
    floating_panel_opacity:
      typeof config.floating_panel_opacity === "number"
        ? config.floating_panel_opacity
        : 0.85,
  };
}

export function configuredSlicerRecords(config: AppConfigShape): SlicerRow[] {
  return normalizeSlicers(config.slicers).filter(
    (item) => item.name.length > 0 && item.path.length > 0,
  );
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
