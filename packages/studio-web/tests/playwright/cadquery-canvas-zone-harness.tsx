import { createRoot } from "react-dom/client";
import { CanvasZone } from "../../src/workbench/canvas-zone";
import type { ScadWorkbenchState } from "../../src/workbench/scad-workbench";
import "../../src/styles/tokens.css";
import "../../src/styles/workbench.css";
import "../../src/styles/workbench-zones.css";
import "../../src/styles/viewers.css";

export function mountCadQueryCanvasZone() {
  document.body.innerHTML = '<div id="root"></div>';
  document.documentElement.style.width = "100%";
  document.documentElement.style.height = "100%";
  document.body.style.margin = "0";
  document.body.style.width = "100%";
  document.body.style.height = "100%";
  const root = document.getElementById("root");
  if (!root) throw new Error("root missing");
  root.className = "app";
  root.style.width = "1000px";
  root.style.height = "700px";
  root.style.minWidth = "0";
  root.style.gridTemplateColumns = "0 0 1fr 0";
  root.style.gridTemplateRows = "0 1fr";
  createRoot(root).render(
    <CanvasZone
      phase="ready"
      message="cadquery ready"
      previewTargetLabel="lid.py"
      tabs={[
        {
          id: "cadquery",
          label: "lid.py",
          kind: "cadquery",
          path: { type: "cadquery_result", result_id: "cq_canvas" },
        },
      ]}
      activeTabId="cadquery"
      onActivateTab={() => undefined}
      onCloseTab={() => undefined}
      onPreviewStatus={() => undefined}
      client={fakeClient() as never}
      refreshSignal={0}
      config={null}
      meshInfo={null}
      activeView="iso"
      onMeshInfo={() => undefined}
      onCadQueryScene={() => undefined}
      cadQueryScene={null}
      cadQuerySelection={null}
      cameraState={null}
      cameraOverride={null}
      onCameraChange={() => undefined}
      scadWorkbenchState={fakeScadWorkbenchState()}
      planRunDisabled
      onRunPlan={() => undefined}
    />,
  );
}

function fakeClient() {
  return {
    dispatchCadQueryResultGet: () =>
      Promise.resolve({
        type: "cad_query_result_ready",
        payload: { result_id: "cq_canvas" },
      }),
    takeCadQueryMesh: () => cadQueryMesh(),
    dispatchSelectionUpdate: () => Promise.resolve({ accepted_count: 1 }),
  };
}

function cadQueryMesh() {
  const metadata = {
    result_id: "cq_canvas",
    build_id: "sha256:canvas",
    root_ref_text: "@part[fixture_panel]",
    root_object_kind: "part",
    parts: [
      {
        name: "fixture_panel",
        object_kind: "part",
        ref_text: "@part[fixture_panel]",
        instance_path: null,
        transform: null,
        faces: [
          {
            face_idx: 0,
            features: ["lid_alignment_surface"],
            ambiguous: false,
          },
        ],
        edges: [{ edge_idx: 0, adjacent_faces: [0] }],
        vertices: [{ vertex_idx: 0, adjacent_edges: [0] }],
        feature_map: [
          { feature: "lid_alignment_surface", face_indices: [0] },
        ],
      },
    ],
  };
  return {
    metadata: () => metadata,
    face_positions: () => new Float32Array([-18, -16, 0, 18, -16, 0, 0, 18, 0]),
    face_normals: () => new Float32Array([0, 0, 1, 0, 0, 1, 0, 0, 1]),
    edge_polyline: () => new Float32Array([-18, -16, 0, 18, -16, 0]),
    vertex_position: () => new Float32Array([0, 18, 0]),
  };
}

function fakeScadWorkbenchState(): ScadWorkbenchState {
  return {
    previewAppearance: {},
    setPreviewPointLightAutoPosition: () => undefined,
  } as unknown as ScadWorkbenchState;
}
