// Browser WebSocket transport. Owns only the socket lifecycle (open / close /
// reconnect with exponential backoff) and funnels bytes into the wasm client.
// No protocol-level state lives here.

export type TransportHandlers = {
  onOpen: () => void;
  onMessage: (bytes: Uint8Array) => void;
  onClose: (reason: { code: number; reason: string; was_clean: boolean }) => void;
  onError: (message: string) => void;
};

const BACKOFF_STEPS_MS = [1000, 2000, 4000, 8000, 15000];

export class BrowserWebSocketTransport {
  private socket: WebSocket | null = null;
  private pendingOutbox: Uint8Array[] = [];
  private stopped = false;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempt = 0;

  constructor(
    private readonly url: string,
    private readonly handlers: TransportHandlers,
  ) {}

  start(): void {
    this.stopped = false;
    this.openSocket();
  }

  send(bytes: Uint8Array): void {
    if (this.socket && this.socket.readyState === WebSocket.OPEN) {
      this.socket.send(bytes);
      return;
    }
    this.pendingOutbox.push(bytes);
  }

  stop(): void {
    this.stopped = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    const current = this.socket;
    this.socket = null;
    if (current && current.readyState !== WebSocket.CLOSED) {
      try {
        current.close(1000, "client-stop");
      } catch {
        // ignore
      }
    }
    this.pendingOutbox = [];
  }

  private openSocket(): void {
    if (this.stopped) return;
    let ws: WebSocket;
    try {
      ws = new WebSocket(this.url);
    } catch (err) {
      this.handlers.onError(
        err instanceof Error ? err.message : String(err),
      );
      this.scheduleReconnect();
      return;
    }
    ws.binaryType = "arraybuffer";
    this.socket = ws;

    ws.addEventListener("open", () => {
      this.reconnectAttempt = 0;
      this.flushPending();
      this.handlers.onOpen();
    });
    ws.addEventListener("message", (ev) => {
      const bytes = coerceToBytes(ev.data);
      if (bytes) this.handlers.onMessage(bytes);
    });
    ws.addEventListener("error", () => {
      this.handlers.onError("websocket error");
    });
    ws.addEventListener("close", (ev) => {
      this.socket = null;
      this.handlers.onClose({
        code: ev.code,
        reason: ev.reason,
        was_clean: ev.wasClean,
      });
      this.scheduleReconnect();
    });
  }

  private flushPending(): void {
    const sock = this.socket;
    if (!sock || sock.readyState !== WebSocket.OPEN) return;
    const buffered = this.pendingOutbox;
    this.pendingOutbox = [];
    for (const frame of buffered) {
      sock.send(frame);
    }
  }

  private scheduleReconnect(): void {
    if (this.stopped) return;
    const delay =
      BACKOFF_STEPS_MS[Math.min(this.reconnectAttempt, BACKOFF_STEPS_MS.length - 1)] ?? 15000;
    this.reconnectAttempt += 1;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.openSocket();
    }, delay);
  }
}

function coerceToBytes(data: unknown): Uint8Array | null {
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  if (data instanceof Uint8Array) return data;
  if (typeof data === "string") return new TextEncoder().encode(data);
  return null;
}
