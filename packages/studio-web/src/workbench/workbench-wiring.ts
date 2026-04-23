// WorkbenchLayout 的 transport / client / watch 接线辅助函数。
// 抽出目的：保持 workbench-layout.tsx 内部函数 ≤ 50 行，并把 Phase 5 新增的
// watch 订阅 / handshake ack 流程独立成单元。

import { BrowserWebSocketTransport } from "../transport/websocket-transport";
import type { WasmClient, WasmClientCallbacks } from "../wasm-bridge";

export type WireOptions = {
  wsUrl: string;
  client: WasmClient;
  onHandshaking: () => void;
  onTransportError: (message: string) => void;
  onTransportLost: (message: string) => void;
  onTransportReconnecting: (message: string) => void;
};

export function buildHandshakeParams() {
  return {
    capabilities: {
      client_name: "studio-web" as const,
      platform: "web" as const,
      protocol_version: { min: 1, max: 1 },
      file_read: { denied_extensions: [".scad", ".stl", ".3mf"] },
      supported_preview_kinds: ["geometry_artifact"],
    },
  };
}

export function createTransport(opts: WireOptions): BrowserWebSocketTransport {
  const { client, wsUrl } = opts;
  const transport = new BrowserWebSocketTransport(wsUrl, {
    onOpen: () => {
      opts.onHandshaking();
      client.setSender((bytes) => transport.send(bytes));
      try {
        client.beginHandshake(buildHandshakeParams());
        client.pump();
      } catch (err) {
        opts.onTransportError(describeError(err));
      }
    },
    onMessage: (bytes) => {
      try {
        client.receiveInbound(bytes);
      } catch (err) {
        opts.onTransportError(describeError(err));
      }
    },
    onClose: (reason) => {
      client.setSender(null);
      client.markTransportClosed(reason);
      client.pump();
      // Watch registry is retained by ManagedClient and replayed on the next
      // handshake; the React layer must not resubscribe after reconnect or
      // the server accumulates duplicate subscriptions.
      if (reason.was_clean) {
        opts.onTransportReconnecting(
          `transport closed (${reason.code}); reconnecting...`,
        );
      } else {
        opts.onTransportLost(
          `transport lost (${reason.code}${reason.reason ? `: ${reason.reason}` : ""})`,
        );
      }
    },
    onError: (msg) => {
      opts.onTransportError(msg);
    },
  });
  return transport;
}

export function describeError(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "object" && err !== null) {
    try {
      return JSON.stringify(err);
    } catch {
      return String(err);
    }
  }
  return String(err);
}

export function buildClientCallbacks(params: {
  onSnapshotDirty: () => void;
  onHandshakeAccepted: () => void;
  onWatchEvent: () => void;
}): WasmClientCallbacks {
  return {
    onSnapshotDirty: params.onSnapshotDirty,
    onHandshakeAccepted: params.onHandshakeAccepted,
    onWatchEvent: params.onWatchEvent,
  };
}
