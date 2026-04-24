export type Vec3 = [number, number, number];

export type MeshBounds = {
  min: Vec3;
  max: Vec3;
};

export type MeshInfo = {
  vertices: number;
  indices: number;
  bounds: MeshBounds;
  center: Vec3;
  dimensions: Vec3;
  radius: number;
};

export function computeMeshInfo(
  positions: Float32Array,
  indices: Uint32Array | null,
): MeshInfo | null {
  if (positions.length === 0 || positions.length % 3 !== 0) return null;

  const min: Vec3 = [Infinity, Infinity, Infinity];
  const max: Vec3 = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < positions.length; i += 3) {
    const x = positions[i];
    const y = positions[i + 1];
    const z = positions[i + 2];
    if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(z)) {
      return null;
    }
    min[0] = Math.min(min[0], x);
    min[1] = Math.min(min[1], y);
    min[2] = Math.min(min[2], z);
    max[0] = Math.max(max[0], x);
    max[1] = Math.max(max[1], y);
    max[2] = Math.max(max[2], z);
  }

  const dimensions: Vec3 = [
    Math.max(0, max[0] - min[0]),
    Math.max(0, max[1] - min[1]),
    Math.max(0, max[2] - min[2]),
  ];
  const center: Vec3 = [
    min[0] + dimensions[0] / 2,
    min[1] + dimensions[1] / 2,
    min[2] + dimensions[2] / 2,
  ];
  const radius = Math.hypot(dimensions[0], dimensions[1], dimensions[2]) / 2;

  return {
    vertices: positions.length / 3,
    indices: indices ? indices.length : positions.length / 3,
    bounds: { min, max },
    center,
    dimensions,
    radius,
  };
}

export function meshBuildPlateSize(info: MeshInfo | null): number {
  if (!info) return 200;
  const maxDim = Math.max(...info.dimensions, info.radius * 2, 1);
  return Math.max(80, maxDim * 1.8);
}
