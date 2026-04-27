import type { CadQueryObjectKind } from "@budn/app-server-protocol";

export type CadQueryMeshHandleLike = {
  metadata(): unknown;
  face_positions(partIndex: number, faceIndex: number): Float32Array | number[];
  face_normals(partIndex: number, faceIndex: number): Float32Array | number[];
  edge_polyline(partIndex: number, edgeIndex: number): Float32Array | number[];
  vertex_position(
    partIndex: number,
    vertexIndex: number,
  ): Float32Array | number[];
};

export type CadQueryScenePayload = {
  resultId: string;
  buildId: string;
  rootRefText: string;
  rootObjectKind: CadQueryObjectKind;
  parts: CadQueryScenePart[];
};

export type CadQueryScenePart = {
  partIndex: number;
  name: string;
  objectKind: CadQueryObjectKind;
  refText: string;
  instancePath: string | null;
  transform: number[] | null;
  faces: CadQuerySceneFace[];
  edges: CadQuerySceneEdge[];
  vertices: CadQuerySceneVertex[];
  featureMap: CadQueryFeatureFaces[];
};

export type CadQuerySceneFace = {
  faceIndex: number;
  positions: Float32Array;
  normals: Float32Array;
  features: string[];
  ambiguous: boolean;
};

export type CadQuerySceneEdge = {
  edgeIndex: number;
  polyline: Float32Array;
  adjacentFaces: number[];
};

export type CadQuerySceneVertex = {
  vertexIndex: number;
  position: [number, number, number];
  adjacentEdges: number[];
};

export type CadQueryFeatureFaces = {
  feature: string;
  faceIndices: number[];
};

export function cadQuerySceneFromHandle(
  handle: CadQueryMeshHandleLike,
): CadQueryScenePayload {
  const metadata = record(handle.metadata(), "cadquery metadata");
  const parts = arrayField(metadata, "parts").map((part, partIndex) =>
    parsePart(record(part, "cadquery part"), partIndex, handle),
  );
  return {
    resultId: stringField(metadata, "result_id"),
    buildId: stringField(metadata, "build_id"),
    rootRefText: stringField(metadata, "root_ref_text"),
    rootObjectKind: objectKindField(metadata, "root_object_kind"),
    parts,
  };
}

function parsePart(
  part: Record<string, unknown>,
  partIndex: number,
  handle: CadQueryMeshHandleLike,
): CadQueryScenePart {
  return {
    partIndex,
    name: stringField(part, "name"),
    objectKind: objectKindField(part, "object_kind"),
    refText: stringField(part, "ref_text"),
    instancePath: nullableString(part["instance_path"]),
    transform: nullableNumberArray(part["transform"], 16),
    faces: parseFaces(part, partIndex, handle),
    edges: parseEdges(part, partIndex, handle),
    vertices: parseVertices(part, partIndex, handle),
    featureMap: parseFeatureMap(part),
  };
}

function parseFaces(
  part: Record<string, unknown>,
  partIndex: number,
  handle: CadQueryMeshHandleLike,
): CadQuerySceneFace[] {
  return arrayField(part, "faces").map((raw, faceIndex) => {
    const face = record(raw, "cadquery face");
    return {
      faceIndex: numberField(face, "face_idx"),
      positions: toFloat32(handle.face_positions(partIndex, faceIndex)),
      normals: toFloat32(handle.face_normals(partIndex, faceIndex)),
      features: stringArray(face["features"]),
      ambiguous: face["ambiguous"] === true,
    };
  });
}

function parseEdges(
  part: Record<string, unknown>,
  partIndex: number,
  handle: CadQueryMeshHandleLike,
): CadQuerySceneEdge[] {
  return arrayField(part, "edges").map((raw, edgeIndex) => {
    const edge = record(raw, "cadquery edge");
    return {
      edgeIndex: numberField(edge, "edge_idx"),
      polyline: toFloat32(handle.edge_polyline(partIndex, edgeIndex)),
      adjacentFaces: numberArray(edge["adjacent_faces"]),
    };
  });
}

function parseVertices(
  part: Record<string, unknown>,
  partIndex: number,
  handle: CadQueryMeshHandleLike,
): CadQuerySceneVertex[] {
  return arrayField(part, "vertices").map((raw, vertexIndex) => {
    const vertex = record(raw, "cadquery vertex");
    return {
      vertexIndex: numberField(vertex, "vertex_idx"),
      position: vec3(handle.vertex_position(partIndex, vertexIndex)),
      adjacentEdges: numberArray(vertex["adjacent_edges"]),
    };
  });
}

function parseFeatureMap(
  part: Record<string, unknown>,
): CadQueryFeatureFaces[] {
  return arrayField(part, "feature_map").map((raw) => {
    const item = record(raw, "cadquery feature");
    return {
      feature: stringField(item, "feature"),
      faceIndices: numberArray(item["face_indices"]),
    };
  });
}

function record(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== "object") throw new Error(`${label} invalid`);
  return value as Record<string, unknown>;
}

function arrayField(record: Record<string, unknown>, key: string): unknown[] {
  const value = record[key];
  return Array.isArray(value) ? value : [];
}

function stringField(record: Record<string, unknown>, key: string): string {
  const value = record[key];
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${key} missing`);
  }
  return value;
}

function numberField(record: Record<string, unknown>, key: string): number {
  const value = record[key];
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`${key} missing`);
  }
  return value;
}

function objectKindField(
  record: Record<string, unknown>,
  key: string,
): CadQueryObjectKind {
  const value = stringField(record, key);
  if (value === "part" || value === "component" || value === "assembly") {
    return value;
  }
  throw new Error(`${key} invalid`);
}

function nullableString(value: unknown): string | null {
  return typeof value === "string" && value.length > 0 ? value : null;
}

function nullableNumberArray(value: unknown, length: number): number[] | null {
  if (!Array.isArray(value) || value.length !== length) return null;
  const numbers = value.map(Number);
  return numbers.every(Number.isFinite) ? numbers : null;
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function numberArray(value: unknown): number[] {
  return Array.isArray(value)
    ? value.map(Number).filter((item) => Number.isFinite(item))
    : [];
}

function toFloat32(value: Float32Array | number[]): Float32Array {
  return value instanceof Float32Array ? value : new Float32Array(value);
}

function vec3(value: Float32Array | number[]): [number, number, number] {
  const raw = toFloat32(value);
  return [raw[0] ?? 0, raw[1] ?? 0, raw[2] ?? 0];
}
