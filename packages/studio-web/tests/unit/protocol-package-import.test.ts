import { describe, expect, it } from "vitest";
import {
  initProtocolWasm,
  protocol_decode_server_frame,
  type AppConfigDto,
  type CadQueryMeshPayload,
  type CadQueryResultReady,
  type CommandSuccess,
  type ServerCapabilities,
} from "@budn/app-server-protocol";

describe("protocol package import", () => {
  it("exposes wasm init and shared config DTO types", () => {
    const config: AppConfigDto = {
      openscad_path: null,
      slicers: [],
      recent_workspaces: [],
      floating_panel_opacity: 0.85,
      left_panel_width: 360,
      right_panel_width: 320,
      display_unit: "millimeter",
      camera_overlay_pos: null,
      camera_overlay_size: null,
      param_panel_pos: null,
      param_panel_size: null,
      log_panel_pos: null,
      log_panel_size: null,
    };

    expect(typeof initProtocolWasm).toBe("function");
    expect(typeof protocol_decode_server_frame).toBe("function");
    expect(config.display_unit).toBe("millimeter");
  });

  it("exposes CadQuery protocol types from the package entrypoint", () => {
    const capabilities: ServerCapabilities = {
      protocol_version: { min: 2, max: 2 },
      reconnect_window_ms: 30_000,
      supports_watch: true,
      supported_preview_kinds: ["geometry_artifact"],
      supports_session_reclaim: true,
      cadquery: true,
      agent: false,
      selection_sync: false,
    };
    const ready: CadQueryResultReady = {
      result_id: "cq_1",
      build_id: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      part_count: 1,
      face_count: 1,
      edge_count: 1,
      vertex_count: 1,
    };
    const mesh: CadQueryMeshPayload = {
      result_id: ready.result_id,
      build_id: ready.build_id,
      unit: "millimeter",
      root_ref_text: "@part[top_lid]",
      root_object_kind: "part",
      parts: [],
    };
    const success: CommandSuccess = {
      type: "cad_query_result_ready",
      payload: ready,
    };

    expect(capabilities.cadquery).toBe(true);
    expect(mesh.root_object_kind).toBe("part");
    expect(success.payload.result_id).toBe("cq_1");
  });
});
