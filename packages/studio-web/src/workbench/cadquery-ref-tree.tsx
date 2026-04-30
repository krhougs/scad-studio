import type {
  SelectionRef,
  SelectionUpdateRequest,
} from "@budn/app-server-protocol";
import type React from "react";
import type { CadQueryScenePayload } from "../viewers/cadquery-mesh";
import {
  cadQuerySelectionKey,
  selectionRefFromCadQueryFeature,
  selectionRefFromCadQueryPick,
  toggleCadQuerySelection,
} from "../viewers/cadquery-selection";

type CadQueryRefTreeProps = {
  scene: CadQueryScenePayload | null;
  selection: SelectionUpdateRequest | null;
  onSelectionChange: (next: SelectionUpdateRequest) => void;
};

type RefTreeRow = {
  key: string;
  testId: string;
  label: string;
  detail: string;
  depth: number;
  ref: SelectionRef;
};

export function CadQueryRefTree(props: CadQueryRefTreeProps) {
  if (!props.scene) {
    return (
      <div className="cadquery-ref-tree is-empty" data-testid="cadquery-ref-tree">
        No CadQuery refs loaded.
      </div>
    );
  }
  const rows = cadQueryRefRows(props.scene);
  const selected = new Set(
    (props.selection?.selections ?? []).map(cadQuerySelectionKey),
  );
  const toggle = (ref: SelectionRef) => {
    const next = toggleCadQuerySelection(props.selection?.selections ?? [], ref);
    props.onSelectionChange({
      selections: next,
      active_index: next.length > 0 ? next.length - 1 : null,
    });
  };
  return (
    <div className="cadquery-ref-tree" data-testid="cadquery-ref-tree">
      {rows.map((row) => (
        <button
          key={row.key}
          type="button"
          className="cadquery-ref-row"
          style={{ "--ref-depth": row.depth } as React.CSSProperties}
          aria-pressed={selected.has(row.key)}
          data-testid={row.testId}
          onClick={() => toggle(row.ref)}
        >
          <span className="cadquery-ref-row__check" aria-hidden="true" />
          <span className="cadquery-ref-row__label">{row.label}</span>
          <code>{row.detail}</code>
        </button>
      ))}
    </div>
  );
}

export function cadQueryRefRows(scene: CadQueryScenePayload): RefTreeRow[] {
  const rows: RefTreeRow[] = [rootRow(scene)];
  for (const part of scene.parts) {
    const partDepth = part.refText === scene.rootRefText ? 1 : 1;
    if (part.refText !== scene.rootRefText || scene.parts.length > 1) {
      rows.push(partRow(scene, part, partDepth));
    }
    rows.push(...featureRows(scene, part, partDepth + 1));
    rows.push(...faceRows(scene, part, partDepth + 1));
    rows.push(...edgeRows(scene, part, partDepth + 1));
    rows.push(...vertexRows(scene, part, partDepth + 1));
  }
  return rows;
}

function rootRow(scene: CadQueryScenePayload): RefTreeRow {
  const ref = selectionRefFromCadQueryPick(scene, {
    kind: "assembly",
    additive: false,
  });
  return row(ref, "root", scene.rootObjectKind, scene.rootRefText, 0);
}

function partRow(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
  depth: number,
): RefTreeRow {
  const ref = selectionRefFromCadQueryPick(scene, {
    kind: "part",
    partIndex: part.partIndex,
    additive: false,
  });
  return row(ref, `part-${part.partIndex}`, part.name, part.refText, depth);
}

function featureRows(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
  depth: number,
): RefTreeRow[] {
  return part.featureMap.map((feature) => {
    const ref = selectionRefFromCadQueryFeature(scene, part.partIndex, feature.feature);
    return row(
      ref,
      `feature-${safeId(feature.feature)}`,
      `feature ${feature.feature}`,
      ref.ref_text,
      depth,
    );
  });
}

function faceRows(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
  depth: number,
): RefTreeRow[] {
  return part.faces.map((face) => {
    const ref = selectionRefFromCadQueryPick(scene, {
      kind: "face",
      partIndex: part.partIndex,
      faceIndex: part.faces.indexOf(face),
      additive: false,
    });
    return row(ref, `face-${face.faceIndex}`, `face f_${face.faceIndex}`, ref.ref_text, depth);
  });
}

function edgeRows(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
  depth: number,
): RefTreeRow[] {
  return part.edges.map((edge, edgeIndex) => {
    const ref = selectionRefFromCadQueryPick(scene, {
      kind: "edge",
      partIndex: part.partIndex,
      edgeIndex,
      additive: false,
    });
    return row(ref, `edge-${edge.edgeIndex}`, `edge e_${edge.edgeIndex}`, ref.ref_text, depth);
  });
}

function vertexRows(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
  depth: number,
): RefTreeRow[] {
  return part.vertices.map((vertex, vertexIndex) => {
    const ref = selectionRefFromCadQueryPick(scene, {
      kind: "vertex",
      partIndex: part.partIndex,
      vertexIndex,
      additive: false,
    });
    return row(
      ref,
      `vertex-${vertex.vertexIndex}`,
      `vertex v_${vertex.vertexIndex}`,
      ref.ref_text,
      depth,
    );
  });
}

function row(
  ref: SelectionRef,
  id: string,
  label: string,
  detail: string,
  depth: number,
): RefTreeRow {
  return {
    key: cadQuerySelectionKey(ref),
    testId: `cadquery-ref-row-${id}`,
    label,
    detail,
    depth,
    ref,
  };
}

function safeId(value: string): string {
  return value.replace(/[^A-Za-z0-9_-]+/g, "_");
}
