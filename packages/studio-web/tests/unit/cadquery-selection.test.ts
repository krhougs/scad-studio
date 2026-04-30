import { describe, expect, it } from "vitest";
import type { CadQueryScenePayload } from "../../src/viewers/cadquery-mesh";
import {
  cadQueryAvailableSelectionModes,
  cadQuerySelectionKey,
  selectionRefFromCadQueryPick,
  updateCadQuerySelection,
} from "../../src/viewers/cadquery-selection";

const scene: CadQueryScenePayload = {
  resultId: "cq_result",
  buildId: "sha256:abc",
  rootRefText: "@assembly[full_enclosure]",
  rootObjectKind: "assembly",
  parts: [
    {
      partIndex: 0,
      name: "top_lid",
      objectKind: "part",
      refText: "@part[top_lid]",
      instancePath: null,
      transform: null,
      faces: [
        {
          faceIndex: 2,
          positions: new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]),
          normals: new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1]),
          features: ["lid_alignment_surface"],
          ambiguous: true,
        },
      ],
      edges: [
        {
          edgeIndex: 4,
          polyline: new Float32Array([0, 0, 0, 1, 0, 0]),
          adjacentFaces: [2],
        },
      ],
      vertices: [
        {
          vertexIndex: 7,
          position: [0, 0, 0],
          adjacentEdges: [4],
        },
      ],
      featureMap: [{ feature: "lid_alignment_surface", faceIndices: [2] }],
    },
  ],
};

describe("cadquery selection refs", () => {
  it("builds raw face refs from payload owner metadata", () => {
    const ref = selectionRefFromCadQueryPick(scene, {
      kind: "face",
      partIndex: 0,
      faceIndex: 0,
      additive: false,
    });

    expect(ref).toEqual({
      kind: "face",
      ref_text: "@face[top_lid:f_2]",
      owner_ref_text: "@part[top_lid]",
      owner_object_kind: "part",
      instance_path: null,
      candidate_feature_ref: "@feature[top_lid.lid_alignment_surface]",
      build_id: "sha256:abc",
      result_id: "cq_result",
      ambiguous: true,
    });
  });

  it("deduplicates additive selections by stable ref key", () => {
    const face = selectionRefFromCadQueryPick(scene, {
      kind: "face",
      partIndex: 0,
      faceIndex: 0,
      additive: false,
    });
    const edge = selectionRefFromCadQueryPick(scene, {
      kind: "edge",
      partIndex: 0,
      edgeIndex: 0,
      additive: true,
    });

    const first = updateCadQuerySelection([], face, false);
    const second = updateCadQuerySelection(first, edge, true);
    const third = updateCadQuerySelection(second, face, true);

    expect(second).toHaveLength(2);
    expect(third).toHaveLength(2);
    expect(third[0]).toBe(edge);
    expect(third[1]).toBe(face);
  });

  it("keeps repeated assembly instance raw selections distinct", () => {
    const repeated = repeatedInstanceScene();
    const first = selectionRefFromCadQueryPick(repeated, {
      kind: "face",
      partIndex: 0,
      faceIndex: 0,
      additive: false,
    });
    const second = selectionRefFromCadQueryPick(repeated, {
      kind: "face",
      partIndex: 1,
      faceIndex: 0,
      additive: true,
    });

    const selections = updateCadQuerySelection([first], second, true);

    expect(first.ref_text).toBe(second.ref_text);
    expect(first.instance_path).toBe("full_enclosure/screw_left");
    expect(second.instance_path).toBe("full_enclosure/screw_right");
    expect(cadQuerySelectionKey(first)).not.toBe(cadQuerySelectionKey(second));
    expect(selections).toEqual([first, second]);
  });

  it("uses root object kind for whole-result selection", () => {
    const partScene: CadQueryScenePayload = {
      ...scene,
      rootRefText: "@part[top_lid]",
      rootObjectKind: "part",
    };

    const ref = selectionRefFromCadQueryPick(partScene, {
      kind: "assembly",
      additive: false,
    });

    expect(ref).toMatchObject({
      kind: "part",
      ref_text: "@part[top_lid]",
      build_id: "sha256:abc",
      result_id: "cq_result",
    });
  });

  it("keeps nested assembly refs distinct from the root assembly", () => {
    const nested = nestedAssemblyScene();

    const ref = selectionRefFromCadQueryPick(nested, {
      kind: "assembly",
      partIndex: 0,
      additive: false,
    });

    expect(ref).toMatchObject({
      kind: "assembly",
      ref_text: "@assembly[inner_frame]",
      build_id: "sha256:abc",
      result_id: "cq_result",
    });
    expect(ref.ref_text).not.toBe(nested.rootRefText);
  });

  it("does not expose root object kind as an object selection mode", () => {
    expect(cadQueryAvailableSelectionModes(scene)).toEqual([
      "part",
      "feature",
      "face",
      "edge",
      "vertex",
    ]);
    expect(cadQueryAvailableSelectionModes(rootPartScene())).toEqual([
      "feature",
      "face",
      "edge",
      "vertex",
    ]);
  });
});

function repeatedInstanceScene(): CadQueryScenePayload {
  return {
    ...scene,
    parts: [
      instancePart(0, "full_enclosure/screw_left"),
      instancePart(1, "full_enclosure/screw_right"),
    ],
  };
}

function nestedAssemblyScene(): CadQueryScenePayload {
  return {
    ...scene,
    rootRefText: "@assembly[full_enclosure]",
    rootObjectKind: "assembly",
    parts: [
      {
        ...scene.parts[0],
        name: "inner_frame",
        objectKind: "assembly",
        refText: "@assembly[inner_frame]",
      },
    ],
  };
}

function rootPartScene(): CadQueryScenePayload {
  return {
    ...scene,
    rootRefText: "@part[top_lid]",
    rootObjectKind: "part",
  };
}

function instancePart(
  partIndex: number,
  instancePath: string,
): CadQueryScenePayload["parts"][number] {
  return {
    ...scene.parts[0],
    partIndex,
    instancePath,
  };
}
