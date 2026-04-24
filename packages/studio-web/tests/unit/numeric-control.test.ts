import { describe, expect, it } from "vitest";
import {
  clamp,
  formatNumber,
  normalizedRange,
  roundToStep,
} from "../../src/workbench/numeric-control";

describe("numeric-control helpers", () => {
  it("normalizes invalid or degenerate ranges", () => {
    expect(normalizedRange(5, 5)).toEqual({ min: 4, max: 6 });
    expect(normalizedRange(Number.NaN, 1)).toEqual({ min: -1, max: 1 });
    expect(normalizedRange(-2, 4)).toEqual({ min: -2, max: 4 });
  });

  it("clamps values to the target range", () => {
    expect(clamp(-4, -2, 2)).toBe(-2);
    expect(clamp(4, -2, 2)).toBe(2);
    expect(clamp(1, -2, 2)).toBe(1);
  });

  it("rounds values to finite positive steps", () => {
    expect(roundToStep(1.24, 0.1)).toBe(1.2);
    expect(roundToStep(1.26, 0.1)).toBe(1.3);
    expect(roundToStep(3, 0)).toBe(3);
  });

  it("formats optional fixed fraction digits", () => {
    expect(formatNumber(3.5)).toBe("3.5");
    expect(formatNumber(3.5, 3)).toBe("3.500");
    expect(formatNumber(Number.NaN)).toBe("0");
  });
});
