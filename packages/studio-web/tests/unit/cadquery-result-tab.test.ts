import { describe, expect, it } from "vitest";
import {
  cadQueryResultTab,
  cadQueryResultIdFromPath,
  extractCadQueryReadyFromAgentEvent,
} from "../../src/workbench/cadquery-result-tab";

describe("cadquery result tabs", () => {
  it("extracts mesh ready events and creates UI-only tab descriptors", () => {
    const ready = extractCadQueryReadyFromAgentEvent({
      event: "agent.mesh_ready",
      payload: {
        result: {
          result_id: "cq_123",
          build_id: "sha256:abc",
        },
      },
    });

    expect(ready).toEqual({
      result_id: "cq_123",
      build_id: "sha256:abc",
    });
    expect(cadQueryResultTab(ready!)).toEqual({
      id: "cadquery:cq_123",
      kind: "cadquery",
      label: "cq_123",
      path: { type: "cadquery_result", result_id: "cq_123" },
    });
    expect(
      cadQueryResultIdFromPath({
        type: "cadquery_result",
        result_id: "cq_123",
      }),
    ).toBe("cq_123");
  });
});
