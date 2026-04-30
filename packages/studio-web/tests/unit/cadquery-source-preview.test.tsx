import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import {
  CadQuerySourcePreview,
  cadQueryReadyResultId,
} from "../../src/workbench/cadquery-source-preview";

vi.mock("../../src/viewers/cadquery-viewer", () => ({
  CadQueryViewer: ({ resultId, label }: { resultId: string; label: string }) => (
    <div data-testid="mock-cadquery-viewer">{`${label}:${resultId}`}</div>
  ),
}));

describe("cadQueryReadyResultId", () => {
  it("reads result ids from protocol success payloads", () => {
    expect(
      cadQueryReadyResultId({
        type: "cad_query_result_ready",
        payload: { result_id: "cq_123" },
      }),
    ).toBe("cq_123");
    expect(cadQueryReadyResultId({ result_id: "cq_direct" })).toBe(
      "cq_direct",
    );
    expect(cadQueryReadyResultId({ payload: {} })).toBeNull();
  });
});

describe("CadQuerySourcePreview", () => {
  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("dispatches cadquery.preview for a source file and renders the result viewer", async () => {
    const client = fakeClient();
    const onPreviewStatus = vi.fn();
    render(
      <CadQuerySourcePreview
        sourcePath={{ workspace_id: "ws", path_segments: ["parts", "pad.py"] }}
        client={client}
        label="pad.py"
        selectionMode="face"
        onPreviewStatus={onPreviewStatus}
      />,
    );

    await waitFor(() =>
      expect(screen.getByTestId("mock-cadquery-viewer").textContent).toBe(
        "pad.py:cq_123",
      ),
    );
    expect(client.dispatchCadQueryPreview).toHaveBeenCalledWith({
      target_path: { workspace_id: "ws", path_segments: ["parts", "pad.py"] },
      export_formats: [],
      params_json: "{}",
    });
    expect(onPreviewStatus).toHaveBeenCalledWith("cadquery pending");
  });
});

function fakeClient() {
  return {
    dispatchCadQueryPreview: vi.fn().mockResolvedValue({
      type: "cad_query_result_ready",
      payload: { result_id: "cq_123" },
    }),
    dispatchCadQueryResultGet: vi.fn(),
    takeCadQueryMesh: vi.fn(),
    dispatchSelectionUpdate: vi.fn(),
  };
}
