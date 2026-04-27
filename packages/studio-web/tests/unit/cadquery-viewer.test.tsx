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
import type { CadQueryPickTarget } from "../../src/viewers/cadquery-selection";

const viewer = {
  setCadQueryMesh: vi.fn(),
  setCadQuerySelectionMode: vi.fn(),
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
          candidate_feature_ref: "@feature[top_lid.top_surface]",
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
      ).toContain("@feature[top_lid.top_surface]"),
    );
  });
});

function fakeClient(options: { ambiguous?: boolean } = {}) {
  const ambiguous = options.ambiguous ?? true;
  const mesh = {
    metadata: () => ({
      result_id: "cq_123",
      build_id: "sha256:abc",
      root_ref_text: "@part[top_lid]",
      root_object_kind: "part",
      parts: [
        {
          name: "top_lid",
          object_kind: "part",
          ref_text: "@part[top_lid]",
          instance_path: null,
          transform: null,
          faces: [{ face_idx: 2, features: ["top_surface"], ambiguous }],
          edges: [],
          vertices: [],
          feature_map: [{ feature: "top_surface", face_indices: [2] }],
        },
      ],
    }),
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
