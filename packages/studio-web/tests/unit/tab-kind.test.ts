// Unit tests for the file-extension → tab kind router.

import { describe, expect, it } from "vitest";
import { extensionOf, resolveTabKind } from "../../src/workbench/tab-kind";

describe("resolveTabKind", () => {
  it("routes markdown variants", () => {
    expect(resolveTabKind("README.md")).toBe("markdown");
    expect(resolveTabKind("NOTES.MARKDOWN")).toBe("markdown");
  });

  it("routes image variants case-insensitively", () => {
    expect(resolveTabKind("shot.PNG")).toBe("image");
    expect(resolveTabKind("photo.jpg")).toBe("image");
    expect(resolveTabKind("photo.jpeg")).toBe("image");
    expect(resolveTabKind("sticker.webp")).toBe("image");
    expect(resolveTabKind("animation.GIF")).toBe("image");
    expect(resolveTabKind("scan.bmp")).toBe("image");
    expect(resolveTabKind("plate.tif")).toBe("image");
    expect(resolveTabKind("plate.tiff")).toBe("image");
    expect(resolveTabKind("favicon.ico")).toBe("image");
  });

  it("routes scad and meshes", () => {
    expect(resolveTabKind("cube.scad")).toBe("scad");
    expect(resolveTabKind("airpods_charging_pad.py")).toBe("cadquery");
    expect(resolveTabKind("airpods_charging_pad.step")).toBe("cadquery");
    expect(resolveTabKind("part.stl")).toBe("mesh");
    expect(resolveTabKind("assembly.3mf")).toBe("mesh");
  });

  it("returns null for unsupported extensions", () => {
    expect(resolveTabKind("notes.txt")).toBeNull();
    expect(resolveTabKind("blob")).toBeNull();
  });
});

describe("extensionOf", () => {
  it("extracts the trailing extension including the dot", () => {
    expect(extensionOf("foo.bar.zip")).toBe(".zip");
    expect(extensionOf("README.md")).toBe(".md");
  });

  it("returns an empty string when no extension is present", () => {
    expect(extensionOf("Makefile")).toBe("");
  });
});
