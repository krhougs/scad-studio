import { afterEach, describe, expect, it, vi } from "vitest";
import { BrowserWebSocketTransport } from "../../src/transport/websocket-transport";

const OriginalWebSocket = globalThis.WebSocket;
let lastSocket: FakeWebSocket | null = null;

class FakeWebSocket extends EventTarget {
  static readonly OPEN = 1;
  static readonly CLOSED = 3;

  readonly url: string;
  readyState = FakeWebSocket.OPEN;
  binaryType: BinaryType = "blob";

  constructor(url: string) {
    super();
    this.url = url;
    lastSocket = this;
  }

  send(): void {}

  close(): void {
    this.readyState = FakeWebSocket.CLOSED;
  }
}

function installFakeWebSocket(): void {
  lastSocket = null;
  globalThis.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
}

afterEach(() => {
  globalThis.WebSocket = OriginalWebSocket;
  lastSocket = null;
});

describe("BrowserWebSocketTransport", () => {
  it("accepts ArrayBuffer messages as inbound bytes", () => {
    installFakeWebSocket();
    const onMessage = vi.fn();
    const transport = new BrowserWebSocketTransport("ws://127.0.0.1:1", {
      onOpen: vi.fn(),
      onMessage,
      onClose: vi.fn(),
      onError: vi.fn(),
    });

    transport.start();
    lastSocket?.dispatchEvent(
      new MessageEvent("message", {
        data: new Uint8Array([1, 2, 3]).buffer,
      }),
    );

    expect(onMessage).toHaveBeenCalledWith(new Uint8Array([1, 2, 3]));
  });

  it("ignores text messages instead of converting them to bytes", () => {
    installFakeWebSocket();
    const onMessage = vi.fn();
    const transport = new BrowserWebSocketTransport("ws://127.0.0.1:1", {
      onOpen: vi.fn(),
      onMessage,
      onClose: vi.fn(),
      onError: vi.fn(),
    });

    transport.start();
    lastSocket?.dispatchEvent(new MessageEvent("message", { data: "json" }));

    expect(onMessage).not.toHaveBeenCalled();
  });
});
