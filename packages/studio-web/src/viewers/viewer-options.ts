export type MeshRenderMode = "solid" | "wireframe" | "xray";
export type MeshProjectionMode = "perspective" | "orthographic";
export type MeshColorMode = "mono" | "color";

export type PreviewAppearance = {
  backgroundColor: string;
  gridMajorColor: string;
  gridMinorColor: string;
  lightingIntensity: number;
};

export type MeshViewerOptions = {
  renderMode: MeshRenderMode;
  projectionMode: MeshProjectionMode;
  colorMode: MeshColorMode;
  showGrid: boolean;
  showAxis: boolean;
  showBuildPlate: boolean;
  shadowsEnabled: boolean;
  fogEnabled: boolean;
  clipPlaneEnabled: boolean;
} & PreviewAppearance;

export const DEFAULT_PREVIEW_APPEARANCE: PreviewAppearance = {
  backgroundColor: "#181b20",
  gridMajorColor: "#5a6573",
  gridMinorColor: "#343b45",
  lightingIntensity: 1.25,
};

export const DEFAULT_MESH_VIEWER_OPTIONS: MeshViewerOptions = {
  renderMode: "solid",
  projectionMode: "perspective",
  colorMode: "color",
  showGrid: true,
  showAxis: false,
  showBuildPlate: false,
  shadowsEnabled: false,
  fogEnabled: false,
  clipPlaneEnabled: false,
  ...DEFAULT_PREVIEW_APPEARANCE,
};

export function normalizePreviewAppearance(input: unknown): PreviewAppearance {
  if (!input || typeof input !== "object" || Array.isArray(input)) {
    return { ...DEFAULT_PREVIEW_APPEARANCE };
  }
  const record = input as Record<string, unknown>;
  return {
    backgroundColor: normalizeHexColor(record["backgroundColor"], DEFAULT_PREVIEW_APPEARANCE.backgroundColor),
    gridMajorColor: normalizeHexColor(record["gridMajorColor"], DEFAULT_PREVIEW_APPEARANCE.gridMajorColor),
    gridMinorColor: normalizeHexColor(record["gridMinorColor"], DEFAULT_PREVIEW_APPEARANCE.gridMinorColor),
    lightingIntensity: normalizeLightingIntensity(record["lightingIntensity"]),
  };
}

function normalizeHexColor(value: unknown, fallback: string): string {
  if (typeof value !== "string") return fallback;
  const trimmed = value.trim().toLowerCase();
  return /^#[0-9a-f]{6}$/.test(trimmed) ? trimmed : fallback;
}

function normalizeLightingIntensity(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return DEFAULT_PREVIEW_APPEARANCE.lightingIntensity;
  }
  return Math.min(3, Math.max(0.25, value));
}
