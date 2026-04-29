import { describe, expect, it } from "vitest";
import {
  initProtocolWasm,
  protocol_decode_server_frame,
  type AgentInvokeRequest,
  type AgentPlanSavedEvent,
  type CadQueryMeshPayload,
  type CadQueryResultReady,
  type CommandSuccess,
  type AppConfigDto,
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
      protocol_version: { min: 4, max: 4 },
      reconnect_window_ms: 30_000,
      supports_watch: true,
      supported_preview_kinds: ["geometry_artifact"],
      supports_session_reclaim: true,
      cadquery: true,
      agent: false,
      selection_sync: false,
      llm_configured: false,
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
    const invoke: AgentInvokeRequest = {
      session_id: "chat-1",
      prompt: "run plan",
      mode: "agent",
      plan_ref: {
        workspace_id: "ws",
        path_segments: ["plans", "2026050100-add-lid-vents"],
      },
      context_refs: ["@part[top_lid]"],
    };
    const savedPlan: AgentPlanSavedEvent = {
      session_id: "chat-1",
      run_id: "run-1",
      package: {
        plan_id: "2026050100-add-lid-vents",
        plan_ref: {
          workspace_id: "ws",
          path_segments: ["plans", "2026050100-add-lid-vents"],
        },
        request_path: {
          workspace_id: "ws",
          path_segments: ["plans", "2026050100-add-lid-vents", "request.md"],
        },
        plan_path: {
          workspace_id: "ws",
          path_segments: ["plans", "2026050100-add-lid-vents", "plan.md"],
        },
        result_path: {
          workspace_id: "ws",
          path_segments: [
            "plans",
            "2026050100-add-lid-vents",
            "plan-result.md",
          ],
        },
      },
      title: "Add lid vents",
      status: "planned",
      target_path: {
        workspace_id: "ws",
        path_segments: ["parts", "top_lid.py"],
      },
      target_type: "part",
      affected_files: [
        { workspace_id: "ws", path_segments: ["parts", "top_lid.py"] },
      ],
      new_files: [
        { workspace_id: "ws", path_segments: ["parts", "top_lid.md"] },
      ],
      change_description: "Add lid vents",
      export_targets: [
        { workspace_id: "ws", path_segments: ["outputs", "top_lid.step"] },
      ],
    };
    const planModeInvoke: AgentInvokeRequest = {
      session_id: "chat-1",
      prompt: "draft a plan",
      mode: "plan",
      plan_ref: null,
      context_refs: [],
    };

    expect(capabilities.cadquery).toBe(true);
    expect(mesh.root_object_kind).toBe("part");
    expect(success.payload.result_id).toBe("cq_1");
    expect(invoke.mode).toBe("agent");
    expect(planModeInvoke.mode).toBe("plan");
    expect(savedPlan.package.plan_ref.path_segments).toEqual([
      "plans",
      "2026050100-add-lid-vents",
    ]);
  });
});
