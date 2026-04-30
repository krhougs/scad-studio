import { describe, expect, it } from "vitest";
import { cadQueryPreviewSourcePath } from "../../src/workbench/cadquery-source-path";

describe("cadQueryPreviewSourcePath", () => {
  it("uses CadQuery Python sources directly", () => {
    const path = { workspace_id: "ws", path_segments: ["parts", "pad.py"] };
    expect(cadQueryPreviewSourcePath(path, "pad.py")).toBe(path);
  });

  it("routes generated STEP outputs to the matching CadQuery source path", () => {
    expect(
      cadQueryPreviewSourcePath(
        { workspace_id: "ws", path_segments: ["outputs", "pad.step"] },
        "pad.step",
      ),
    ).toEqual({ workspace_id: "ws", path_segments: ["parts", "pad.py"] });
  });

  it("leaves unknown STEP locations unchanged", () => {
    const path = { workspace_id: "ws", path_segments: ["vendor", "pad.step"] };
    expect(cadQueryPreviewSourcePath(path, "pad.step")).toBe(path);
  });
});
