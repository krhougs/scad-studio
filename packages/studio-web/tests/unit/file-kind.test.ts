import { describe, expect, it } from "vitest";
import { fileKindLabel } from "../../src/workbench/file-kind";

describe("fileKindLabel", () => {
  it("returns a directory label for folders", () => {
    expect(fileKindLabel({ kind: "directory", label: "models" })).toBe("DIR");
  });

  it("returns explicit labels for supported file types", () => {
    expect(fileKindLabel({ kind: "file", label: "part.scad" })).toBe("SCAD");
    expect(fileKindLabel({ kind: "file", label: "part.py" })).toBe("PY");
    expect(fileKindLabel({ kind: "file", label: "mesh.stl" })).toBe("STL");
    expect(fileKindLabel({ kind: "file", label: "plate.3mf" })).toBe("3MF");
    expect(fileKindLabel({ kind: "file", label: "notes.md" })).toBe("MD");
    expect(fileKindLabel({ kind: "file", label: "photo.jpeg" })).toBe("JPEG");
    expect(fileKindLabel({ kind: "file", label: "photo.png" })).toBe("PNG");
    expect(fileKindLabel({ kind: "file", label: "diagram.svg" })).toBe("SVG");
  });

  it("uses a generic file label only for unknown files", () => {
    expect(fileKindLabel({ kind: "file", label: "LICENSE" })).toBe("FILE");
    expect(fileKindLabel({ kind: "file", label: "archive.zip" })).toBe("ZIP");
  });
});
