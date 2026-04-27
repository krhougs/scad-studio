import { describe, expect, it } from "vitest";
import { buildHandshakeParams } from "../../src/workbench/workbench-wiring";

describe("workbench wiring", () => {
  it("requests app-server protocol version 2", () => {
    expect(buildHandshakeParams().capabilities.protocol_version).toEqual({
      min: 2,
      max: 2,
    });
  });
});
