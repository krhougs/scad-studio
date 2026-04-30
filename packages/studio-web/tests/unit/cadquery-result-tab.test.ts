import { describe, expect, it } from "vitest";
import {
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
          artifact_relation: {
            source_path: "parts/model.py",
            exports: [
              {
                name: "step",
                path: "outputs/model.step",
                hash: "sha256:def",
              },
            ],
          },
        },
      },
    });

    expect(ready).toEqual({
      result_id: "cq_123",
      build_id: "sha256:abc",
      artifact_relation: {
        source_path: "parts/model.py",
        exports: [
          {
            name: "step",
            path: "outputs/model.step",
            hash: "sha256:def",
          },
        ],
      },
    });
    expect(
      cadQueryResultIdFromPath({
        type: "cadquery_result",
        result_id: "cq_123",
      }),
    ).toBe("cq_123");
  });
});
