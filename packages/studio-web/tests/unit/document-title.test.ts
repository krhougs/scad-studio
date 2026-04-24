import { describe, expect, it } from "vitest";
import { documentTitleForFile } from "../../src/workbench/document-title";

describe("documentTitleForFile", () => {
  it("uses the product name when no file is active", () => {
    expect(documentTitleForFile(null)).toBe("budn'");
  });

  it("includes the active file name before the product name", () => {
    expect(documentTitleForFile("models/cube.scad")).toBe("cube.scad · budn'");
    expect(documentTitleForFile("README.md")).toBe("README.md · budn'");
  });
});
