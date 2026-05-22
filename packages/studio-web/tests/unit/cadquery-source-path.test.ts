import { describe, expect, it } from "vitest";
import {
  cadQueryArtifactTabPathForFile,
  cadQueryPreviewSourcePath,
  cadQueryTabMatchesReady,
} from "../../src/workbench/cadquery-source-path";

describe("cadQueryPreviewSourcePath", () => {
  it("uses CadQuery Python sources directly", () => {
    const path = { workspace_id: "ws", path_segments: ["parts", "pad.py"] };
    expect(cadQueryPreviewSourcePath(path, "pad.py")).toBe(path);
  });

  it("does not infer CadQuery sources from generated STEP file names", () => {
    const path = { workspace_id: "ws", path_segments: ["outputs", "pad.step"] };
    expect(cadQueryPreviewSourcePath(path, "pad.step")).toBe(path);
  });

  it("uses explicit artifact tab source paths", () => {
    const source = { workspace_id: "ws", path_segments: ["parts", "pad.py"] };
    const artifact = {
      type: "cadquery_artifact",
      workspace_id: "ws",
      path_segments: ["outputs", "pad.step"],
      artifact_path: { workspace_id: "ws", path_segments: ["outputs", "pad.step"] },
      source_path: source,
      result_id: "cq_1",
    };
    expect(cadQueryPreviewSourcePath(artifact, "pad.step")).toBe(source);
  });
});

describe("cadQueryArtifactTabPathForFile", () => {
  const ready = {
    result_id: "cq_1",
    build_id: "sha256:abc",
    part_count: 1,
    face_count: 1,
    edge_count: 0,
    vertex_count: 0,
    artifact_relation: {
      source_path: "parts/pad.py",
      exports: [
        {
          name: "step",
          path: "outputs/pad.step",
          hash: "sha256:def",
        },
      ],
    },
  };

  it("creates an explicit artifact tab path from protocol relation", () => {
    expect(
      cadQueryArtifactTabPathForFile(
        { workspace_id: "ws", path_segments: ["outputs", "pad.step"] },
        [ready],
      ),
    ).toEqual({
      type: "cadquery_artifact",
      workspace_id: "ws",
      path_segments: ["outputs", "pad.step"],
      artifact_path: {
        workspace_id: "ws",
        path_segments: ["outputs", "pad.step"],
      },
      source_path: { workspace_id: "ws", path_segments: ["parts", "pad.py"] },
      result_id: "cq_1",
    });
  });

  it("returns null when no explicit export relation matches", () => {
    expect(
      cadQueryArtifactTabPathForFile(
        { workspace_id: "ws", path_segments: ["vendor", "pad.step"] },
        [ready],
      ),
    ).toBeNull();
  });
});

describe("cadQueryTabMatchesReady", () => {
  const ready = {
    result_id: "cq_2",
    build_id: "sha256:abc",
    part_count: 1,
    face_count: 1,
    edge_count: 0,
    vertex_count: 0,
    artifact_relation: {
      source_path: "parts/pad.py",
      exports: [
        {
          name: "step",
          path: "outputs/pad.step",
          hash: "sha256:def",
        },
      ],
    },
  };

  it("matches corresponding source and artifact tabs", () => {
    expect(
      cadQueryTabMatchesReady(
        { workspace_id: "ws", path_segments: ["parts", "pad.py"] },
        ready,
      ),
    ).toBe(true);
    expect(
      cadQueryTabMatchesReady(
        {
          type: "cadquery_artifact",
          workspace_id: "ws",
          path_segments: ["outputs", "pad.step"],
          artifact_path: {
            workspace_id: "ws",
            path_segments: ["outputs", "pad.step"],
          },
          source_path: { workspace_id: "ws", path_segments: ["parts", "pad.py"] },
          result_id: "cq_1",
        },
        ready,
      ),
    ).toBe(true);
  });

  it("does not match unrelated CadQuery tabs", () => {
    expect(
      cadQueryTabMatchesReady(
        { workspace_id: "ws", path_segments: ["parts", "other.py"] },
        ready,
      ),
    ).toBe(false);
  });
});
