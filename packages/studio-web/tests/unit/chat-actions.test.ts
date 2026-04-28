import { describe, expect, it } from "vitest";
import type { AgentPlanProposedEvent } from "@budn/app-server-protocol";
import { confirmPlan, parseSlashCommand } from "../../src/workbench/chat-actions";
import type { ChatSnapshot } from "../../src/workbench/chat-zone";
import type { WasmClient } from "../../src/wasm-bridge";

describe("parseSlashCommand", () => {
  it("returns auto for plain text", () => {
    const result = parseSlashCommand("design a phone case");
    expect(result).toEqual({ operation: "auto", prompt: "design a phone case" });
  });

  it("parses /plan with prompt", () => {
    const result = parseSlashCommand("/plan design a sliding lid");
    expect(result).toEqual({ operation: "plan", prompt: "design a sliding lid" });
  });

  it("parses /execute with prompt", () => {
    const result = parseSlashCommand("/execute apply the fillet");
    expect(result).toEqual({ operation: "execute", prompt: "apply the fillet" });
  });

  it("parses /inform with prompt", () => {
    const result = parseSlashCommand("/inform explain CadQuery loft");
    expect(result).toEqual({ operation: "inform", prompt: "explain CadQuery loft" });
  });

  it("handles /plan with no prompt", () => {
    const result = parseSlashCommand("/plan");
    expect(result).toEqual({ operation: "plan", prompt: "" });
  });

  it("ignores partial matches like /planning", () => {
    const result = parseSlashCommand("/planning something");
    expect(result).toEqual({ operation: "auto", prompt: "/planning something" });
  });

  it("handles leading whitespace before command", () => {
    const result = parseSlashCommand("  /execute do it");
    expect(result).toEqual({ operation: "execute", prompt: "do it" });
  });

  it("treats unknown slash as plain text", () => {
    const result = parseSlashCommand("/help me");
    expect(result).toEqual({ operation: "auto", prompt: "/help me" });
  });

  it("confirms a proposed plan with the server-provided plan scope", async () => {
    const calls: unknown[] = [];
    const client = {
      dispatchAgentPlanConfirm: async (params: unknown) => {
        calls.push(params);
        return {};
      },
    } as unknown as WasmClient;
    const plan: AgentPlanProposedEvent = {
      session_id: "chat-1",
      run_id: "run-1",
      plan_ref: pathHandle(["plans", "add-lid-vents.md"]),
      target_path: pathHandle(["parts", "top_lid.py"]),
      target_type: "part",
      affected_files: [
        pathHandle(["parts", "top_lid.py"]),
        pathHandle(["parts", "top_lid.md"]),
      ],
      new_files: [pathHandle(["parts", "top_lid_ref_map.md"])],
      change_description: "Add lid vents",
      export_targets: [pathHandle(["outputs", "top_lid.step"])],
    };
    const snapshot: ChatSnapshot = {
      workspace_current: { workspace_id: "ws" },
      current_selection: null,
    };

    await confirmPlan(client, plan, snapshot);

    expect(calls).toHaveLength(1);
    expect(calls[0]).toEqual({
      session_id: "chat-1",
      run_id: "run-1",
      confirmed_cadquery: {
        request: {
          target_path: pathHandle(["parts", "top_lid.py"]),
          target_type: "part",
          code: "",
          export_formats: ["step"],
          params_json: "{}",
        },
        plan_ref: pathHandle(["plans", "add-lid-vents.md"]),
        affected_files: [
          pathHandle(["parts", "top_lid.py"]),
          pathHandle(["parts", "top_lid.md"]),
        ],
        new_files: [pathHandle(["parts", "top_lid_ref_map.md"])],
        export_targets: [pathHandle(["outputs", "top_lid.step"])],
      },
    });
  });

  it("derives export formats from proposed export targets", async () => {
    const calls: unknown[] = [];
    const client = {
      dispatchAgentPlanConfirm: async (params: unknown) => {
        calls.push(params);
        return {};
      },
    } as unknown as WasmClient;
    const plan: AgentPlanProposedEvent = {
      session_id: "chat-1",
      run_id: "run-1",
      plan_ref: pathHandle(["plans", "assembly.md"]),
      target_path: pathHandle(["assemblies", "case.py"]),
      target_type: "assembly",
      affected_files: [pathHandle(["assemblies", "case.py"])],
      new_files: [],
      change_description: "Export assembly",
      export_targets: [
        pathHandle(["outputs", "case.stl"]),
        pathHandle(["outputs", "case.step"]),
      ],
    };

    await confirmPlan(client, plan, {
      workspace_current: { workspace_id: "ws" },
      current_selection: null,
    });

    expect(calls[0]).toEqual(
      expect.objectContaining({
        confirmed_cadquery: expect.objectContaining({
          request: expect.objectContaining({
            export_formats: ["stl", "step"],
          }),
        }),
      }),
    );
  });
});

function pathHandle(pathSegments: string[]) {
  return { workspace_id: "ws", path_segments: pathSegments };
}
