export type MeshRenderMode = "solid" | "wireframe" | "xray";
export type MeshProjectionMode = "perspective" | "orthographic";

export type MeshViewerOptions = {
  renderMode: MeshRenderMode;
  projectionMode: MeshProjectionMode;
  showGrid: boolean;
  showAxis: boolean;
  showBuildPlate: boolean;
  shadowsEnabled: boolean;
};

export const DEFAULT_MESH_VIEWER_OPTIONS: MeshViewerOptions = {
  renderMode: "solid",
  projectionMode: "perspective",
  showGrid: true,
  showAxis: true,
  showBuildPlate: false,
  shadowsEnabled: false,
};
