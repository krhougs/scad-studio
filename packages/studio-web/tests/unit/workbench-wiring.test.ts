import { describe, expect, it } from "vitest";
import { buildHandshakeParams } from "../../src/workbench/workbench-wiring";

describe("workbench wiring", () => {
  it("requests current app-server protocol version 3", () => {
    expect(buildHandshakeParams().capabilities.protocol_version).toEqual({
      min: 3,
      max: 3,
    });
  });
});
