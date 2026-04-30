import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CadQueryViewer } from "../../src/viewers/cadquery-viewer";
import {
  cadQueryAvailableSelectionModes,
  type CadQueryPickTarget,
} from "../../src/viewers/cadquery-selection";

const viewer = {
  setCadQueryMesh: vi.fn(),
  setCadQuerySelectionMode: vi.fn(),
  setCadQuerySelectionEnabled: vi.fn(),
  setCadQuerySelectedKeys: vi.fn(),
  setOptions: vi.fn(),
  resize: vi.fn(),
  onCameraChange: vi.fn(),
  dispose: vi.fn(),
};

vi.mock("../../src/viewers/mesh-three", () => ({
  createMeshViewer: vi.fn(() => viewer),
}));

describe("CadQueryViewer", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("takes cadquery mesh from the explicit side buffer and renders it", async () => {
    const client = fakeClient();

    render(
      <CadQueryViewer
        resultId="cq_123"
        client={client}
        label="top_lid"
        selectionMode="face"
        onPreviewStatus={() => undefined}
        onInfo={() => undefined}
      />,
    );

    await waitFor(() => expect(viewer.setCadQueryMesh).toHaveBeenCalled());
    expect(client.dispatchCadQueryResultGet).toHaveBeenCalledWith({
      result_id: "cq_123",
    });
    expect(client.takeCadQueryMesh).toHaveBeenCalledWith("cq_123");
  });

  it("confirms ambiguous face selection before dispatching selection.update", async () => {
    const client = fakeClient();

    render(
      <CadQueryViewer
        resultId="cq_123"
        client={client}
        label="top_lid"
        selectionMode="face"
        onPreviewStatus={() => undefined}
        onInfo={() => undefined}
      />,
    );

    await waitFor(() => expect(viewer.setCadQueryMesh).toHaveBeenCalled());
    const pick = viewer.setCadQueryMesh.mock.calls[0][1].onPick as (
      pick: CadQueryPickTarget,
    ) => void;
    act(() => {
      pick({ kind: "face", partIndex: 0, faceIndex: 0, additive: false });
    });

    expect(client.dispatchSelectionUpdate).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "confirm" }));

    await waitFor(() =>
      expect(client.dispatchSelectionUpdate).toHaveBeenCalled(),
    );
    expect(client.dispatchSelectionUpdate).toHaveBeenCalledWith({
      selections: [
        expect.objectContaining({
          kind: "face",
          ref_text: "@face[top_lid:f_2]",
          candidate_feature_ref: "@feature[top_lid.lid_alignment_surface]",
          ambiguous: true,
        }),
      ],
      active_index: 0,
    });
  });

  it("shows the selected feature ref after non-ambiguous face selection", async () => {
    const client = fakeClient({ ambiguous: false });

    render(
      <CadQueryViewer
        resultId="cq_123"
        client={client}
        label="top_lid"
        selectionMode="face"
        onPreviewStatus={() => undefined}
        onInfo={() => undefined}
      />,
    );

    await waitFor(() => expect(viewer.setCadQueryMesh).toHaveBeenCalled());
    const pick = viewer.setCadQueryMesh.mock.calls[0][1].onPick as (
      pick: CadQueryPickTarget,
    ) => void;
    act(() => {
      pick({ kind: "face", partIndex: 0, faceIndex: 0, additive: false });
    });

    await waitFor(() =>
      expect(
        screen.getByTestId("cadquery-selection-status").textContent,
      ).toContain("@feature[top_lid.lid_alignment_surface]"),
    );
  });

  it("hides selection chrome and disables CadQuery picking in preview mode", async () => {
    const client = fakeClient({ ambiguous: false });

    render(
      <CadQueryViewer
        resultId="cq_123"
        client={client}
        label="top_lid"
        selectionMode="face"
        interactionMode="preview"
        onPreviewStatus={() => undefined}
        onInfo={() => undefined}
      />,
    );

    await waitFor(() => expect(viewer.setCadQueryMesh).toHaveBeenCalled());
    expect(viewer.setCadQuerySelectionEnabled).toHaveBeenCalledWith(false);
    expect(screen.queryByTestId("cadquery-select-dock")).toBeNull();
    expect(screen.queryByTestId("cadquery-selection-status")).toBeNull();
  });

  it("renders available RefKind selection modes in the bottom dock", async () => {
    const client = fakeClient({ scene: "assembly" });

    render(
      <CadQueryViewer
        resultId="cq_123"
        client={client}
        label="full_adapter"
        selectionMode="face"
        onPreviewStatus={() => undefined}
        onInfo={() => undefined}
      />,
    );

    await waitFor(() => expect(viewer.setCadQueryMesh).toHaveBeenCalled());

    for (const mode of [
      "component",
      "instance",
      "feature",
      "face",
      "edge",
      "vertex",
    ]) {
      expect(screen.getByTestId(`cadquery-select-mode-${mode}`)).toBeTruthy();
    }
    expect(screen.queryByTestId("cadquery-select-mode-assembly")).toBeNull();
    expect(screen.queryByTestId("cadquery-select-mode-preview")).toBeNull();
  });

  it("renders assembly mode when the scene contains a non-root assembly", async () => {
    const client = fakeClient({ scene: "nested-assembly" });

    render(
      <CadQueryViewer
        resultId="cq_123"
        client={client}
        label="full_adapter"
        selectionMode="face"
        onPreviewStatus={() => undefined}
        onInfo={() => undefined}
      />,
    );

    await waitFor(() => expect(viewer.setCadQueryMesh).toHaveBeenCalled());

    expect(screen.getByTestId("cadquery-select-mode-assembly")).toBeTruthy();
  });

  it("derives available selection modes from protocol RefKinds", async () => {
    const scene = cadQueryScenePayloadForTest("assembly");

    expect(cadQueryAvailableSelectionModes(scene)).toEqual([
      "component",
      "instance",
      "feature",
      "face",
      "edge",
      "vertex",
    ]);
  });

  it("delegates bottom dock mode changes when mode is controlled", async () => {
    const client = fakeClient({ scene: "assembly" });
    const onMode = vi.fn();

    render(
      <CadQueryViewer
        resultId="cq_123"
        client={client}
        label="full_adapter"
        selectionMode="face"
        mode="face"
        onMode={onMode}
        onPreviewStatus={() => undefined}
        onInfo={() => undefined}
      />,
    );

    await waitFor(() => expect(viewer.setCadQueryMesh).toHaveBeenCalled());
    fireEvent.click(screen.getByTestId("cadquery-select-mode-edge"));

    expect(onMode).toHaveBeenCalledWith("edge");
  });
});

function fakeClient(
  options: { ambiguous?: boolean; scene?: "part" | "assembly" | "nested-assembly" } = {},
) {
  const ambiguous = options.ambiguous ?? true;
  const metadata = cadQueryMetadataForTest(options.scene ?? "part", ambiguous);
  const mesh = {
    metadata: () => metadata,
    face_positions: () => new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0]),
    face_normals: () => new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1]),
    edge_polyline: () => new Float32Array(),
    vertex_position: () => new Float32Array(),
  };
  return {
    dispatchCadQueryResultGet: vi.fn().mockResolvedValue({
      type: "cad_query_result_ready",
      payload: { result_id: "cq_123" },
    }),
    takeCadQueryMesh: vi.fn(() => mesh),
    dispatchSelectionUpdate: vi.fn().mockResolvedValue({ accepted_count: 1 }),
  };
}

function cadQueryScenePayloadForTest(kind: "part" | "assembly" | "nested-assembly") {
  const metadata = cadQueryMetadataForTest(kind, false);
  return {
    resultId: metadata.result_id,
    buildId: metadata.build_id,
    rootRefText: metadata.root_ref_text,
    rootObjectKind: metadata.root_object_kind,
    parts: metadata.parts.map((part, partIndex) => ({
      partIndex,
      name: part.name,
      objectKind: part.object_kind,
      refText: part.ref_text,
      instancePath: part.instance_path,
      transform: part.transform,
      faces: part.faces.map((face) => ({
        faceIndex: face.face_idx,
        positions: new Float32Array(),
        normals: new Float32Array(),
        features: face.features,
        ambiguous: face.ambiguous,
      })),
      edges: part.edges.map((edge) => ({
        edgeIndex: edge.edge_idx,
        polyline: new Float32Array(),
        adjacentFaces: edge.adjacent_faces,
      })),
      vertices: part.vertices.map((vertex) => ({
        vertexIndex: vertex.vertex_idx,
        position: vertex.position as [number, number, number],
        adjacentEdges: vertex.adjacent_edges,
      })),
      featureMap: part.feature_map.map((feature) => ({
        feature: feature.feature,
        faceIndices: feature.face_indices,
      })),
    })),
  };
}

function cadQueryMetadataForTest(
  kind: "part" | "assembly" | "nested-assembly",
  ambiguous: boolean,
) {
  if (kind === "nested-assembly") {
    return {
      result_id: "cq_123",
      build_id: "sha256:abc",
      root_ref_text: "@assembly[full_adapter]",
      root_object_kind: "assembly" as const,
      parts: [
        {
          name: "left_module",
          object_kind: "assembly" as const,
          ref_text: "@assembly[left_module]",
          instance_path: null,
          transform: null,
          faces: [{ face_idx: 2, features: ["module_shell"], ambiguous }],
          edges: [{ edge_idx: 4, adjacent_faces: [2] }],
          vertices: [{ vertex_idx: 7, position: [0, 0, 0], adjacent_edges: [4] }],
          feature_map: [{ feature: "module_shell", face_indices: [2] }],
        },
      ],
    };
  }
  if (kind === "assembly") {
    return {
      result_id: "cq_123",
      build_id: "sha256:abc",
      root_ref_text: "@assembly[full_adapter]",
      root_object_kind: "assembly" as const,
      parts: [
        {
          name: "pad_insert",
          object_kind: "component" as const,
          ref_text: "@component[pad_insert]",
          instance_path: "full_adapter/left_pad",
          transform: null,
          faces: [{ face_idx: 2, features: ["contact_surface"], ambiguous }],
          edges: [{ edge_idx: 4, adjacent_faces: [2] }],
          vertices: [{ vertex_idx: 7, position: [0, 0, 0], adjacent_edges: [4] }],
          feature_map: [{ feature: "contact_surface", face_indices: [2] }],
        },
      ],
    };
  }
  return {
    result_id: "cq_123",
    build_id: "sha256:abc",
    root_ref_text: "@part[top_lid]",
    root_object_kind: "part" as const,
    parts: [
      {
        name: "top_lid",
        object_kind: "part" as const,
        ref_text: "@part[top_lid]",
        instance_path: null,
        transform: null,
        faces: [{ face_idx: 2, features: ["lid_alignment_surface"], ambiguous }],
        edges: [],
        vertices: [],
        feature_map: [{ feature: "lid_alignment_surface", face_indices: [2] }],
      },
    ],
  };
}
