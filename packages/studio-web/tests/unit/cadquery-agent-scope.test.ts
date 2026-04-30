import { describe, expect, it } from "vitest";
import type { SelectionRef } from "@budn/app-server-protocol";
import {
  DEFAULT_CADQUERY_TARGET_PATH,
  buildCadQueryConfirmation,
  cadQuerySelectionSummary,
  preferredRefText,
} from "../../src/workbench/cadquery-agent-scope";
import type { ChatSnapshot } from "../../src/workbench/chat-zone";

describe("cadquery agent scope", () => {
  it("maps selected part face to feature ref and part target", () => {
    const confirmation = buildCadQueryConfirmation(
      chatSnapshot([faceSelection()]),
      DEFAULT_CADQUERY_TARGET_PATH,
      "add a vent on this face",
    );

    expect(cadQuerySelectionSummary(chatSnapshot([faceSelection()]))).toEqual(
      "@feature[top_lid.lid_alignment_surface]",
    );
    expect(confirmation.request.target_type).toBe("part");
    expect(confirmation.request.target_path.path_segments).toEqual([
      "parts",
      "top_lid.py",
    ]);
    expect(confirmation.affected_files).toEqual([
      { workspace_id: "ws", path_segments: ["parts", "top_lid.py"] },
    ]);
    expect(confirmation.export_targets).toEqual([
      { workspace_id: "ws", path_segments: ["outputs", "top_lid.step"] },
    ]);
  });

  it("does not infer instance movement from prompt words", () => {
    const confirmation = buildCadQueryConfirmation(
      chatSnapshot([instanceSelection()]),
      DEFAULT_CADQUERY_TARGET_PATH,
      "move this screw 5mm right",
    );

    expect(confirmation.request.target_type).toBe("component");
    expect(confirmation.request.target_path.path_segments).toEqual([
      "components",
      "screw.py",
    ]);
  });

  it("does not infer instance replacement from prompt words", () => {
    const confirmation = buildCadQueryConfirmation(
      chatSnapshot([instanceSelection()]),
      DEFAULT_CADQUERY_TARGET_PATH,
      "replace this screw with a countersunk version",
    );

    expect(confirmation.request.target_type).toBe("component");
    expect(confirmation.request.target_path.path_segments).toEqual([
      "components",
      "screw.py",
    ]);
    expect(confirmation.affected_files).toEqual([
      { workspace_id: "ws", path_segments: ["components", "screw.py"] },
    ]);
  });

  it("does not reject movement words for component selection", () => {
    const confirmation = buildCadQueryConfirmation(
      chatSnapshot([componentSelection()]),
      DEFAULT_CADQUERY_TARGET_PATH,
      "move this component 5mm right",
    );

    expect(confirmation.request.target_type).toBe("component");
    expect(confirmation.request.target_path.path_segments).toEqual([
      "components",
      "pcb.py",
    ]);
  });

  it("maps component replacement without requiring an assembly instance", () => {
    const confirmation = buildCadQueryConfirmation(
      chatSnapshot([componentSelection()]),
      DEFAULT_CADQUERY_TARGET_PATH,
      "replace this component with a stronger version",
    );

    expect(confirmation.request.target_type).toBe("component");
    expect(confirmation.request.target_path.path_segments).toEqual([
      "components",
      "pcb.py",
    ]);
  });

  it("does not promote ambiguous raw face selection to feature ref", () => {
    const snapshot = chatSnapshot([ambiguousFaceSelection()]);

    expect(cadQuerySelectionSummary(snapshot)).toEqual("@face[top_lid:f_1]");
  });

  it("preferredRefText returns feature ref for non-ambiguous face", () => {
    expect(preferredRefText(faceSelection())).toBe(
      "@feature[top_lid.lid_alignment_surface]",
    );
  });

  it("preferredRefText returns raw ref for ambiguous face", () => {
    expect(preferredRefText(ambiguousFaceSelection())).toBe(
      "@face[top_lid:f_1]",
    );
  });

  it("preferredRefText returns ref_text when no candidate feature", () => {
    expect(preferredRefText(instanceSelection())).toBe(
      "@instance[full_enclosure/screw_1]",
    );
  });

  it("keeps explicit execute target instead of replacing it from selection", () => {
    const confirmation = buildCadQueryConfirmation(
      chatSnapshot([faceSelection()]),
      "parts/manual.py",
      "modify this selected face",
    );

    expect(confirmation.request.target_path.path_segments).toEqual([
      "parts",
      "manual.py",
    ]);
  });
});

function chatSnapshot(selections: SelectionRef[]): ChatSnapshot {
  return {
    workspace_current: { workspace_id: "ws" },
    current_selection: { selections, active_index: 0 },
  };
}

function faceSelection(): SelectionRef {
  return {
    kind: "face",
    ref_text: "@face[top_lid:f_0]",
    owner_ref_text: "@part[top_lid]",
    owner_object_kind: "part",
    instance_path: null,
    candidate_feature_ref: "@feature[top_lid.lid_alignment_surface]",
    build_id: "sha256:build",
    result_id: "cq_1",
    ambiguous: false,
  };
}

function instanceSelection(): SelectionRef {
  return {
    kind: "instance",
    ref_text: "@instance[full_enclosure/screw_1]",
    owner_ref_text: "@component[screw]",
    owner_object_kind: "component",
    instance_path: "full_enclosure/screw_1",
    candidate_feature_ref: null,
    build_id: "sha256:build",
    result_id: "cq_1",
    ambiguous: false,
  };
}

function componentSelection(): SelectionRef {
  return {
    kind: "component",
    ref_text: "@component[pcb]",
    owner_ref_text: null,
    owner_object_kind: null,
    instance_path: null,
    candidate_feature_ref: null,
    build_id: "sha256:build",
    result_id: "cq_1",
    ambiguous: false,
  };
}

function ambiguousFaceSelection(): SelectionRef {
  return {
    kind: "face",
    ref_text: "@face[top_lid:f_1]",
    owner_ref_text: "@part[top_lid]",
    owner_object_kind: "part",
    instance_path: null,
    candidate_feature_ref: "@feature[top_lid.ambiguous_surface]",
    build_id: "sha256:build",
    result_id: "cq_1",
    ambiguous: true,
  };
}
