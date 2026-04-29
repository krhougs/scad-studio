import { describe, expect, it } from "vitest";
import { planRunTargetForPath } from "../../src/viewers/plan-preview-path";

describe("planRunTargetForPath", () => {
  it("returns a run target for plan package documents", () => {
    const path = {
      workspace_id: "ws",
      path_segments: ["plans", "2026050100-add-lid-vents", "plan.md"],
    };

    expect(planRunTargetForPath(path)).toEqual({
      planId: "2026050100-add-lid-vents",
      planRef: {
        workspace_id: "ws",
        path_segments: ["plans", "2026050100-add-lid-vents"],
      },
    });
  });

  it("maps request and result documents to the same plan package", () => {
    const requestPath = {
      workspace_id: "ws",
      path_segments: ["plans", "2026050100-add-lid-vents", "request.md"],
    };
    const resultPath = {
      workspace_id: "ws",
      path_segments: ["plans", "2026050100-add-lid-vents", "plan-result.md"],
    };

    expect(planRunTargetForPath(requestPath)?.planId).toBe(
      "2026050100-add-lid-vents",
    );
    expect(planRunTargetForPath(resultPath)?.planId).toBe(
      "2026050100-add-lid-vents",
    );
  });

  it("ignores ordinary markdown files and malformed plan paths", () => {
    expect(
      planRunTargetForPath({
        workspace_id: "ws",
        path_segments: ["docs", "README.md"],
      }),
    ).toBeNull();
    expect(
      planRunTargetForPath({
        workspace_id: "ws",
        path_segments: ["plans", "draft", "plan.md"],
      }),
    ).toBeNull();
  });
});
