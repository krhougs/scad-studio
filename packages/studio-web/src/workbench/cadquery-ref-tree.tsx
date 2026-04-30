import type {
  SelectionRef,
  SelectionUpdateRequest,
} from "@budn/app-server-protocol";
import type React from "react";
import type { CadQueryScenePayload } from "../viewers/cadquery-mesh";
import {
  cadQuerySelectionKey,
  selectionRefFromCadQueryFeature,
  selectionRefFromCadQueryInstance,
  selectionRefFromCadQueryObject,
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
  ref: SelectionRef | null;
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
      {rows.map((row) => {
        const ref = row.ref;
        return (
          <RefTreeRowView
            key={row.key}
            row={row}
            selected={ref ? selected.has(row.key) : false}
            onToggle={ref ? () => toggle(ref) : undefined}
          />
        );
      })}
    </div>
  );
}

export function cadQueryRefRows(scene: CadQueryScenePayload): RefTreeRow[] {
  const rows: RefTreeRow[] = [rootDisplayRow(scene)];
  const seen = new Set<string>();
  for (const part of scene.parts) {
    const partDepth = 1;
    if (part.refText !== scene.rootRefText) {
      pushUnique(rows, seen, partRow(scene, part, partDepth));
    }
    const instance = instanceRow(scene, part, partDepth);
    if (instance) pushUnique(rows, seen, instance);
    rows.push(...featureRows(scene, part, partDepth + 1));
    rows.push(...faceRows(scene, part, partDepth + 1));
    rows.push(...edgeRows(scene, part, partDepth + 1));
    rows.push(...vertexRows(scene, part, partDepth + 1));
  }
  return rows;
}

function RefTreeRowView(props: {
  row: RefTreeRow;
  selected: boolean;
  onToggle?: () => void;
}) {
  const style = { "--ref-depth": props.row.depth } as React.CSSProperties;
  const content = (
    <>
      <span className="cadquery-ref-row__check" aria-hidden="true" />
      <span className="cadquery-ref-row__label">{props.row.label}</span>
      <code>{props.row.detail}</code>
    </>
  );
  if (!props.onToggle) {
    return (
      <div
        className="cadquery-ref-row is-root"
        style={style}
        data-testid={props.row.testId}
      >
        {content}
      </div>
    );
  }
  return (
    <button
      type="button"
      className="cadquery-ref-row"
      style={style}
      aria-pressed={props.selected}
      data-testid={props.row.testId}
      onClick={props.onToggle}
    >
      {content}
    </button>
  );
}

function rootDisplayRow(scene: CadQueryScenePayload): RefTreeRow {
  return row(null, "root", "root", scene.rootRefText, 0);
}

function partRow(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
  depth: number,
): RefTreeRow {
  const ref = selectionRefFromCadQueryObject(scene, part.partIndex);
  return row(
    ref,
    `${part.objectKind}-${part.partIndex}`,
    part.name,
    part.refText,
    depth,
  );
}

function instanceRow(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
  depth: number,
): RefTreeRow | null {
  const ref = selectionRefFromCadQueryInstance(scene, part.partIndex);
  if (!ref) return null;
  return row(
    ref,
    `instance-${safeId(part.instancePath ?? "")}`,
    part.name,
    ref.ref_text,
    depth,
  );
}

function featureRows(
  scene: CadQueryScenePayload,
  part: CadQueryScenePayload["parts"][number],
  depth: number,
): RefTreeRow[] {
  return part.featureMap.map((feature) => {
    const ref = selectionRefFromCadQueryFeature(
      scene,
      part.partIndex,
      feature.feature,
    );
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
  return part.faces.map((face, faceIndex) => {
    const ref = selectionRefFromCadQueryPick(scene, {
      kind: "face",
      partIndex: part.partIndex,
      faceIndex,
      additive: false,
    });
    return row(
      ref,
      `face-${face.faceIndex}`,
      `face f_${face.faceIndex}`,
      ref.ref_text,
      depth,
    );
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
    return row(
      ref,
      `edge-${edge.edgeIndex}`,
      `edge e_${edge.edgeIndex}`,
      ref.ref_text,
      depth,
    );
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
  ref: SelectionRef | null,
  id: string,
  label: string,
  detail: string,
  depth: number,
): RefTreeRow {
  return {
    key: ref ? cadQuerySelectionKey(ref) : id,
    testId: `cadquery-ref-row-${id}`,
    label,
    detail,
    depth,
    ref,
  };
}

function pushUnique(rows: RefTreeRow[], seen: Set<string>, row: RefTreeRow) {
  if (seen.has(row.key)) return;
  seen.add(row.key);
  rows.push(row);
}

function safeId(value: string): string {
  return value.replace(/[^A-Za-z0-9_-]+/g, "_");
}
