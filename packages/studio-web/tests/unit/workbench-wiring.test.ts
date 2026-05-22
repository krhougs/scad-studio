import { describe, expect, it } from "vitest";
import { CURRENT_PROTOCOL_VERSION } from "@budn/app-server-protocol";
import { buildHandshakeParams } from "../../src/workbench/workbench-wiring";

describe("workbench wiring", () => {
  it("requests the current app-server protocol version", () => {
    expect(buildHandshakeParams().capabilities.protocol_version).toEqual({
      min: CURRENT_PROTOCOL_VERSION,
      max: CURRENT_PROTOCOL_VERSION,
    });
  });
});
