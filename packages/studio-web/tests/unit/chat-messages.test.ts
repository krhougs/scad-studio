import { describe, expect, it } from "vitest";
import { findAffectedAssemblies, friendlyErrorMessage } from "../../src/workbench/chat-messages";

describe("friendlyErrorMessage", () => {
  it("returns specific message for llm_error", () => {
    const result = friendlyErrorMessage("llm_error");
    expect(result.title).toBe("AI service error");
    expect(result.hint).toContain("Rig OpenAI Responses configuration");
  });

  it("returns specific message for cadquery_build_error", () => {
    const result = friendlyErrorMessage("cadquery_build_error");
    expect(result.title).toBe("CadQuery build failed");
  });

  it("returns specific message for timeout", () => {
    const result = friendlyErrorMessage("timeout");
    expect(result.title).toBe("Operation timed out");
  });

  it("returns fallback for unknown error type", () => {
    const result = friendlyErrorMessage("some_future_error");
    expect(result.title).toBe("Error");
    expect(result.hint).toContain("unexpected");
  });

  it("returns specific message for each known type", () => {
    const knownTypes = [
      "llm_error", "llm_refused", "cadquery_build_error",
      "python_import_error", "tessellation_error", "topology_mapping_error",
      "export_error", "timeout", "file_conflict", "permission_denied",
    ];
    for (const type of knownTypes) {
      const result = friendlyErrorMessage(type);
      expect(result.title).not.toBe("Error");
      expect(result.hint.length).toBeGreaterThan(0);
    }
  });
});

describe("findAffectedAssemblies", () => {
  it("returns assembly paths", () => {
    const paths = ["parts/lid.py", "assemblies/enclosure.py", "components/pcb.py"];
    expect(findAffectedAssemblies(paths)).toEqual(["assemblies/enclosure.py"]);
  });

  it("returns empty when no assemblies", () => {
    const paths = ["parts/lid.py", "components/pcb.py"];
    expect(findAffectedAssemblies(paths)).toEqual([]);
  });

  it("handles nested assembly paths", () => {
    const paths = ["workspace/assemblies/main.py"];
    expect(findAffectedAssemblies(paths)).toEqual(["workspace/assemblies/main.py"]);
  });

  it("returns multiple assemblies", () => {
    const paths = ["assemblies/a.py", "parts/p.py", "assemblies/b.py"];
    expect(findAffectedAssemblies(paths)).toEqual(["assemblies/a.py", "assemblies/b.py"]);
  });
});
