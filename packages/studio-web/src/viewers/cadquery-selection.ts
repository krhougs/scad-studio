import type {
  CadQueryObjectKind,
  SelectionKind,
  SelectionRef,
} from "@budn/app-server-protocol";
import type { CadQueryScenePayload } from "./cadquery-mesh";

export type CadQuerySelectionMode =
  | "component"
  | "assembly"
  | "part"
  | "instance"
  | "feature"
  | "face"
  | "edge"
  | "vertex";

export type CadQueryViewerMode = "preview" | CadQuerySelectionMode;

export const CADQUERY_SELECTION_MODES: CadQuerySelectionMode[] = [
  "component",
  "part",
  "assembly",
  "instance",
  "feature",
  "face",
  "edge",
  "vertex",
];

export type CadQueryPickTarget =
  | { kind: "assembly"; partIndex?: number; additive: boolean }
  | { kind: "component"; partIndex: number; additive: boolean }
  | { kind: "part"; partIndex: number; additive: boolean }
  | { kind: "instance"; partIndex: number; additive: boolean }
  | { kind: "feature"; partIndex: number; feature: string; additive: boolean }
  | { kind: "face"; partIndex: number; faceIndex: number; additive: boolean }
  | { kind: "edge"; partIndex: number; edgeIndex: number; additive: boolean }
  | {
      kind: "vertex";
      partIndex: number;
      vertexIndex: number;
      additive: boolean;
    };

export function selectionRefFromCadQueryPick(
  scene: CadQueryScenePayload,
  pick: CadQueryPickTarget,
): SelectionRef {
  if (pick.kind === "assembly") {
    if (typeof pick.partIndex === "number") {
      const assemblyPart = scene.parts[pick.partIndex];
      if (!assemblyPart) {
        throw new Error("cadquery assembly selection out of range");
      }
      return objectSelection(scene, assemblyPart, "assembly");
    }
    return assemblySelection(scene);
  }
  const part = scene.parts[pick.partIndex];
  if (!part) throw new Error("cadquery object selection out of range");
  if (pick.kind === "component" || pick.kind === "part")
    return objectSelection(scene, part, pick.kind);
  if (pick.kind === "instance") return instanceSelection(scene, part);
  if (pick.kind === "feature")
    return selectionRefFromCadQueryFeature(scene, pick.partIndex, pick.feature);
  if (pick.kind === "face") return faceSelection(scene, part, pick.faceIndex);
  if (pick.kind === "edge") return edgeSelection(scene, part, pick.edgeIndex);
  return vertexSelection(scene, part, pick.vertexIndex);
}

export function selectionRefFromCadQueryObject(
  scene: CadQueryScenePayload,
  partIndex: number,
): SelectionRef {
  const part = scene.parts[partIndex];
  if (!part) throw new Error("cadquery object selection out of range");
  return objectSelection(scene, part, part.objectKind);
}

export function selectionRefFromCadQueryInstance(
  scene: CadQueryScenePayload,
  partIndex: number,
): SelectionRef | null {
  const part = scene.parts[partIndex];
  if (!part) throw new Error("cadquery instance selection out of range");
  return part.instancePath ? instanceSelection(scene, part) : null;
}

export function selectionRefFromCadQueryFeature(
  scene: CadQueryScenePayload,
  partIndex: number,
  feature: string,
): SelectionRef {
  const part = scene.parts[partIndex];
  if (!part) throw new Error("cadquery feature selection out of range");
  return baseSelection({
    kind: "feature",
    refText: `@feature[${ownerIdFromRef(part.refText)}.${feature}]`,
    ownerRefText: part.refText,
    ownerObjectKind: part.objectKind,
    instancePath: part.instancePath,
    buildId: scene.buildId,
    resultId: scene.resultId,
  });
}

export function updateCadQuerySelection(
  previous: SelectionRef[],
  next: SelectionRef,
  additive: boolean,
): SelectionRef[] {
  if (!additive) return [next];
  const key = cadQuerySelectionKey(next);
  return [
    ...previous.filter((item) => cadQuerySelectionKey(item) !== key),
    next,
  ];
}

export function toggleCadQuerySelection(
  previous: SelectionRef[],
  next: SelectionRef,
): SelectionRef[] {
  const key = cadQuerySelectionKey(next);
  if (previous.some((item) => cadQuerySelectionKey(item) === key)) {
    return previous.filter((item) => cadQuerySelectionKey(item) !== key);
  }
  return [...previous, next];
}

export function cadQuerySelectionKey(selection: SelectionRef): string {
  return [
    selection.kind,
    selection.ref_text,
    selection.owner_ref_text ?? "",
    selection.owner_object_kind ?? "",
    selection.instance_path ?? "",
    selection.build_id ?? "",
    selection.result_id ?? "",
  ].join("|");
}

export function cadQueryAvailableSelectionModes(
  scene: CadQueryScenePayload | null,
): CadQuerySelectionMode[] {
  if (!scene) return [];
  const available = new Set<CadQuerySelectionMode>();
  for (const part of scene.parts) {
    if (part.refText !== scene.rootRefText) available.add(part.objectKind);
    if (part.instancePath) available.add("instance");
    if (part.featureMap.length > 0) available.add("feature");
    if (part.faces.length > 0) available.add("face");
    if (part.edges.length > 0) available.add("edge");
    if (part.vertices.length > 0) available.add("vertex");
  }
  return CADQUERY_SELECTION_MODES.filter((mode) => available.has(mode));
}

function assemblySelection(scene: CadQueryScenePayload): SelectionRef {
  return baseSelection({
    kind: scene.rootObjectKind,
    refText: scene.rootRefText,
    buildId: scene.buildId,
    resultId: scene.resultId,
  });
}

function objectSelection(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
  expectedKind: CadQueryObjectKind,
): SelectionRef {
  if (part.objectKind !== expectedKind) {
    throw new Error(`cadquery ${expectedKind} selection out of range`);
  }
  return baseSelection({
    kind: part.objectKind,
    refText: part.refText,
    buildId: scene.buildId,
    resultId: scene.resultId,
  });
}

function instanceSelection(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
): SelectionRef {
  if (!part.instancePath) {
    throw new Error("cadquery instance selection out of range");
  }
  return baseSelection({
    kind: "instance",
    refText: `@instance[${part.instancePath}]`,
    ownerRefText: part.refText,
    ownerObjectKind: part.objectKind,
    instancePath: part.instancePath,
    buildId: scene.buildId,
    resultId: scene.resultId,
  });
}

function faceSelection(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
  faceIndex: number,
): SelectionRef {
  const face = part.faces[faceIndex];
  if (!face) throw new Error("cadquery face selection out of range");
  return rawGeometrySelection(scene, part, {
    kind: "face",
    localId: `f_${face.faceIndex}`,
    candidateFeature: featureRef(part.refText, face.features[0]),
    ambiguous: face.ambiguous,
  });
}

function edgeSelection(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
  edgeIndex: number,
): SelectionRef {
  const edge = part.edges[edgeIndex];
  if (!edge) throw new Error("cadquery edge selection out of range");
  return rawGeometrySelection(scene, part, {
    kind: "edge",
    localId: `e_${edge.edgeIndex}`,
  });
}

function vertexSelection(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
  vertexIndex: number,
): SelectionRef {
  const vertex = part.vertices[vertexIndex];
  if (!vertex) throw new Error("cadquery vertex selection out of range");
  return rawGeometrySelection(scene, part, {
    kind: "vertex",
    localId: `v_${vertex.vertexIndex}`,
  });
}

function rawGeometrySelection(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
  input: {
    kind: "face" | "edge" | "vertex";
    localId: string;
    candidateFeature?: string | null;
    ambiguous?: boolean;
  },
): SelectionRef {
  const ownerId = ownerIdFromRef(part.refText);
  return baseSelection({
    kind: input.kind,
    refText: `@${input.kind}[${ownerId}:${input.localId}]`,
    ownerRefText: part.refText,
    ownerObjectKind: part.objectKind,
    instancePath: part.instancePath,
    candidateFeature: input.candidateFeature ?? null,
    ambiguous: input.ambiguous ?? false,
    buildId: scene.buildId,
    resultId: scene.resultId,
  });
}

function featureRef(
  ownerRefText: string,
  feature: string | undefined,
): string | null {
  if (!feature) return null;
  return `@feature[${ownerIdFromRef(ownerRefText)}.${feature}]`;
}

function ownerIdFromRef(refText: string): string {
  const match = /^@[a-z_]+\[([^\]]+)\]$/.exec(refText);
  return match?.[1] ?? refText;
}

function baseSelection(input: {
  kind: SelectionKind;
  refText: string;
  ownerRefText?: string | null;
  ownerObjectKind?: CadQueryObjectKind | null;
  instancePath?: string | null;
  candidateFeature?: string | null;
  ambiguous?: boolean;
  buildId: string;
  resultId: string;
}): SelectionRef {
  return {
    kind: input.kind,
    ref_text: input.refText,
    owner_ref_text: input.ownerRefText ?? null,
    owner_object_kind: input.ownerObjectKind ?? null,
    instance_path: input.instancePath ?? null,
    candidate_feature_ref: input.candidateFeature ?? null,
    build_id: input.buildId,
    result_id: input.resultId,
    ambiguous: input.ambiguous ?? false,
  };
}
