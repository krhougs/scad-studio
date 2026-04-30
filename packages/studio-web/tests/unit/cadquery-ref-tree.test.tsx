import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  CadQueryRefTree,
  cadQueryRefRows,
} from "../../src/workbench/cadquery-ref-tree";
import type { CadQueryScenePayload } from "../../src/viewers/cadquery-mesh";
import type { SelectionUpdateRequest } from "@budn/app-server-protocol";

describe("CadQueryRefTree", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("renders the Ref hierarchy from the CadQuery scene payload", () => {
    render(
      <CadQueryRefTree
        scene={sceneFixture()}
        selection={null}
        onSelectionChange={() => undefined}
      />,
    );

    expect(screen.getByTestId("cadquery-ref-tree").textContent).toContain(
      "@part[top_lid]",
    );
    expect(screen.getByTestId("cadquery-ref-tree").textContent).toContain(
      "top_surface",
    );
    expect(screen.getByTestId("cadquery-ref-tree").textContent).toContain(
      "face f_2",
    );
    expect(screen.getByTestId("cadquery-ref-tree").textContent).toContain(
      "edge e_4",
    );
    expect(screen.getByTestId("cadquery-ref-tree").textContent).toContain(
      "vertex v_7",
    );
  });

  it("keeps the root row as a display node instead of a selectable Ref", () => {
    const onSelectionChange = vi.fn();
    const scene = sceneFixture();
    render(
      <CadQueryRefTree
        scene={scene}
        selection={null}
        onSelectionChange={onSelectionChange}
      />,
    );

    fireEvent.click(screen.getByTestId("cadquery-ref-row-root"));

    expect(onSelectionChange).not.toHaveBeenCalled();
    expect(screen.getByTestId("cadquery-ref-row-root").textContent).toContain(
      "root",
    );
    expect(
      cadQueryRefRows(scene).filter(
        (row) => row.ref?.ref_text === scene.rootRefText,
      ),
    ).toHaveLength(0);
  });

  it("renders component and instance refs from the scene payload", () => {
    const rows = cadQueryRefRows(componentInstanceSceneFixture());

    expect(rows.map((row) => row.ref?.kind ?? "root")).toEqual([
      "root",
      "component",
      "instance",
      "feature",
      "face",
    ]);
    expect(rows.map((row) => row.detail)).toContain(
      "@instance[full_adapter/left_pad]",
    );
    expect(rows.map((row) => row.detail)).toContain("@component[pad_insert]");
  });

  it("renders nested assembly refs without merging them into the root assembly", () => {
    const rows = cadQueryRefRows(nestedAssemblySceneFixture());

    expect(rows.map((row) => row.detail)).toContain(
      "@assembly[full_adapter]",
    );
    expect(rows.map((row) => row.detail)).toContain("@assembly[left_module]");
    expect(rows.filter((row) => row.ref?.kind === "assembly")).toHaveLength(1);
  });

  it("allows free multi-select across feature and raw topology refs", () => {
    const onSelectionChange = vi.fn();
    const { rerender } = render(
      <CadQueryRefTree
        scene={sceneFixture()}
        selection={null}
        onSelectionChange={onSelectionChange}
      />,
    );

    fireEvent.click(screen.getByTestId("cadquery-ref-row-feature-top_surface"));
    const first = onSelectionChange.mock.calls[0][0] as SelectionUpdateRequest;
    expect(first.selections.map((item) => item.ref_text)).toEqual([
      "@feature[top_lid.top_surface]",
    ]);
    expect(first.active_index).toBe(0);

    rerender(
      <CadQueryRefTree
        scene={sceneFixture()}
        selection={first}
        onSelectionChange={onSelectionChange}
      />,
    );
    fireEvent.click(screen.getByTestId("cadquery-ref-row-edge-4"));
    const second = onSelectionChange.mock.calls[1][0] as SelectionUpdateRequest;
    expect(second.selections.map((item) => item.ref_text)).toEqual([
      "@feature[top_lid.top_surface]",
      "@edge[top_lid:e_4]",
    ]);
    expect(second.active_index).toBe(1);
  });
});

function sceneFixture(): CadQueryScenePayload {
  return {
    resultId: "cq_123",
    buildId: "sha256:abc",
    rootRefText: "@part[top_lid]",
    rootObjectKind: "part",
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
            positions: new Float32Array(),
            normals: new Float32Array(),
            features: ["top_surface"],
            ambiguous: false,
          },
        ],
        edges: [{ edgeIndex: 4, polyline: new Float32Array(), adjacentFaces: [2] }],
        vertices: [{ vertexIndex: 7, position: [0, 0, 0], adjacentEdges: [4] }],
        featureMap: [{ feature: "top_surface", faceIndices: [2] }],
      },
    ],
  };
}

function componentInstanceSceneFixture(): CadQueryScenePayload {
  return {
    resultId: "cq_456",
    buildId: "sha256:def",
    rootRefText: "@assembly[full_adapter]",
    rootObjectKind: "assembly",
    parts: [
      {
        partIndex: 0,
        name: "pad_insert",
        objectKind: "component",
        refText: "@component[pad_insert]",
        instancePath: "full_adapter/left_pad",
        transform: null,
        faces: [
          {
            faceIndex: 0,
            positions: new Float32Array(),
            normals: new Float32Array(),
            features: ["contact_surface"],
            ambiguous: false,
          },
        ],
        edges: [],
        vertices: [],
        featureMap: [{ feature: "contact_surface", faceIndices: [0] }],
      },
    ],
  };
}

function nestedAssemblySceneFixture(): CadQueryScenePayload {
  return {
    resultId: "cq_789",
    buildId: "sha256:ghi",
    rootRefText: "@assembly[full_adapter]",
    rootObjectKind: "assembly",
    parts: [
      {
        partIndex: 0,
        name: "left_module",
        objectKind: "assembly",
        refText: "@assembly[left_module]",
        instancePath: null,
        transform: null,
        faces: [
          {
            faceIndex: 0,
            positions: new Float32Array(),
            normals: new Float32Array(),
            features: ["module_shell"],
            ambiguous: false,
          },
        ],
        edges: [],
        vertices: [],
        featureMap: [{ feature: "module_shell", faceIndices: [0] }],
      },
    ],
  };
}
