import { describe, expect, it } from "vitest";
import { resolveWorkbenchWsUrl } from "../../src/workbench/ws-url";

describe("workbench websocket url", () => {
  it("uses query override before env and fallback", () => {
    const url = resolveWorkbenchWsUrl(
      new URLSearchParams("ws=ws%3A%2F%2Fexample.test%3A9000"),
      {
        envUrl: "ws://env.test:38421",
        location: { protocol: "http:", host: "localhost:5173" },
      },
    );

    expect(url).toBe("ws://example.test:9000");
  });

  it("uses env override before same-origin fallback", () => {
    const url = resolveWorkbenchWsUrl(new URLSearchParams(), {
      envUrl: "ws://env.test:38421",
      location: { protocol: "http:", host: "localhost:5173" },
    });

    expect(url).toBe("ws://env.test:38421");
  });

  it("builds a same-origin websocket proxy url by default", () => {
    const url = resolveWorkbenchWsUrl(new URLSearchParams(), {
      location: { protocol: "https:", host: "lan-device.test:5173" },
    });

    expect(url).toBe("wss://lan-device.test:5173/app-server/ws");
  });
});
