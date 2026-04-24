import { describe, expect, it, vi } from "vitest";

vi.mock("@budn/app-server-protocol", () => ({
  initProtocolWasm: vi.fn(() => Promise.resolve()),
  protocol_path_handle: vi.fn((workspaceId: string, segments: string[]) => {
    for (const segment of segments) {
      if (segment.includes("/") || segment.includes("\\")) {
        throw { code: "native_separator", message: "native separator" };
      }
      if (segment.includes("#")) {
        throw { code: "disallowed_character", message: "disallowed character" };
      }
    }
    return { workspace_id: workspaceId, path_segments: segments };
  }),
}));

const { resolveSiblingOutputPath } = await import("../../src/workbench/protocol-paths");

describe("protocol path helpers", () => {
  it("resolves export output paths relative to the source file", async () => {
    const output = await resolveSiblingOutputPath(
      { workspace_id: "ws", path_segments: ["examples", "part.scad"] },
      "part.stl",
    );

    expect(output).toEqual({
      workspace_id: "ws",
      path_segments: ["examples", "part.stl"],
    });
  });

  it("rejects output names with path separators", async () => {
    await expect(
      resolveSiblingOutputPath(
        { workspace_id: "ws", path_segments: ["part.scad"] },
        "../part.stl",
      ),
    ).rejects.toMatchObject({ code: "native_separator" });
  });

  it("rejects fragment characters instead of stripping them", async () => {
    await expect(
      resolveSiblingOutputPath(
        { workspace_id: "ws", path_segments: ["part.scad"] },
        "part#draft.stl",
      ),
    ).rejects.toMatchObject({ code: "disallowed_character" });
  });
});
