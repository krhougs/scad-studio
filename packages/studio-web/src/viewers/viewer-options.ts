export type MeshRenderMode = "solid" | "wireframe" | "xray";
export type MeshProjectionMode = "perspective" | "orthographic";
export type MeshColorMode = "mono" | "color";

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
};

export const DEFAULT_MESH_VIEWER_OPTIONS: MeshViewerOptions = {
  renderMode: "solid",
  projectionMode: "perspective",
  colorMode: "color",
  showGrid: true,
  showAxis: true,
  showBuildPlate: false,
  shadowsEnabled: false,
  fogEnabled: false,
  clipPlaneEnabled: false,
};
