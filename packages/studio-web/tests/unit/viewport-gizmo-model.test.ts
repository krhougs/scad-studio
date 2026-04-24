import { describe, expect, it } from "vitest";
import {
  projectViewportGizmoAxes,
  type ViewportGizmoAxis,
} from "../../src/workbench/viewport-gizmo-model";
import { PRESET_STATES } from "../../src/canvas/camera-state";

describe("viewport-gizmo-model", () => {
  it("projects all CAD axes from the front camera", () => {
    const axes = projectViewportGizmoAxes(PRESET_STATES.front, 72);
    const x = axis(axes, "x");
    const y = axis(axes, "y");
    const z = axis(axes, "z");

    expect(x.end[0]).toBeGreaterThan(x.start[0]);
    expect(Math.abs(x.end[1] - x.start[1])).toBeLessThan(0.001);
    expect(y.end).toEqual(y.start);
    expect(z.end[1]).toBeLessThan(z.start[1]);
    expect(Math.abs(z.end[0] - z.start[0])).toBeLessThan(0.001);
  });

  it("changes projected axes when camera view changes", () => {
    const front = projectViewportGizmoAxes(PRESET_STATES.front, 72);
    const top = projectViewportGizmoAxes(PRESET_STATES.top, 72);

    expect(front.find((axis) => axis.id === "z")?.end).not.toEqual(
      top.find((axis) => axis.id === "z")?.end,
    );
  });

  it("projects six preset directions with project-coordinate screen up", () => {
    const cases = [
      {
        preset: "top" as const,
        horizontalAxis: "x" as const,
        horizontalSign: 1,
        verticalAxis: "y" as const,
        verticalSign: -1,
      },
      {
        preset: "bottom" as const,
        horizontalAxis: "x" as const,
        horizontalSign: 1,
        verticalAxis: "y" as const,
        verticalSign: 1,
      },
      {
        preset: "front" as const,
        horizontalAxis: "x" as const,
        horizontalSign: 1,
        verticalAxis: "z" as const,
        verticalSign: -1,
      },
      {
        preset: "back" as const,
        horizontalAxis: "x" as const,
        horizontalSign: -1,
        verticalAxis: "z" as const,
        verticalSign: -1,
      },
      {
        preset: "right" as const,
        horizontalAxis: "y" as const,
        horizontalSign: 1,
        verticalAxis: "z" as const,
        verticalSign: -1,
      },
      {
        preset: "left" as const,
        horizontalAxis: "y" as const,
        horizontalSign: -1,
        verticalAxis: "z" as const,
        verticalSign: -1,
      },
    ];

    for (const item of cases) {
      const axes = projectViewportGizmoAxes(PRESET_STATES[item.preset], 72);
      const horizontal = axis(axes, item.horizontalAxis);
      const vertical = axis(axes, item.verticalAxis);
      expect(axes).toHaveLength(3);
      expect(Math.sign(horizontal.end[0] - horizontal.start[0])).toBe(
        item.horizontalSign,
      );
      expect(Math.sign(vertical.end[1] - vertical.start[1])).toBe(
        item.verticalSign,
      );
    }
  });
});

function axis(
  axes: ViewportGizmoAxis[],
  id: ViewportGizmoAxis["id"],
): ViewportGizmoAxis {
  const found = axes.find((candidate) => candidate.id === id);
  if (!found) throw new Error(`missing ${id} axis`);
  return found;
}
