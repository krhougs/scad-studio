// Thin adapter that wraps the wasm-bindgen client_* functions in a React-friendly
// shape. State ownership is strict: pending request resolvers live in
// RequestResolverMap, snapshot data lives inside wasm, and this class owns no
// business state of its own.

import * as Wasm from "@budn/studio-web-wasm";
import type { ClientEventShape } from "./event-stream";
import { dispatchClientEvents } from "./event-stream";
import { RequestResolverMap } from "./request-resolvers";

export type HandshakeParams = {
  capabilities: {
    client_name: string;
    platform: "desktop" | "web" | "other";
    protocol_version: { min: number; max: number };
    file_read: { denied_extensions: string[] };
    supported_preview_kinds: string[];
  };
};

export type TransportSender = (bytes: Uint8Array) => void;

export type WasmClientCallbacks = {
  onSnapshotDirty: () => void;
  onHandshakeAccepted?: (payload: Record<string, unknown>) => void;
  onTransportOpen?: () => void;
  onTransportClosed?: (reason: unknown) => void;
  onWatchEvent?: (requestId: bigint, payload: unknown) => void;
  onWatchResubscribed?: (requestId: bigint) => void;
};

export class WasmClient {
  private handle: Wasm.ClientHandle | null;
  private readonly resolvers = new RequestResolverMap();
  private destroyed = false;
  private sender: TransportSender | null = null;

  constructor(private readonly callbacks: WasmClientCallbacks) {
    this.handle = Wasm.client_create();
  }

  setSender(sender: TransportSender | null): void {
    this.sender = sender;
  }

  beginHandshake(params: HandshakeParams): void {
    this.requireHandle();
    Wasm.client_begin_handshake(this.handle!, params);
  }

  markTransportClosed(reason: {
    code: number;
    reason: string;
    was_clean: boolean;
  }): void {
    if (!this.handle) return;
    Wasm.client_mark_transport_closed(this.handle, reason);
  }

  receiveInbound(bytes: Uint8Array): void {
    this.requireHandle();
    Wasm.client_receive_inbound(this.handle!, bytes);
    this.pump();
  }

  pump(nowMs?: number): void {
    if (!this.handle || this.destroyed) return;
    const tickMs =
      typeof nowMs === "number" ? nowMs : Date.now();
    Wasm.client_tick(this.handle, BigInt(Math.floor(tickMs)));
    // drain outbound first
    while (true) {
      const frame = Wasm.client_next_outbound(this.handle);
      if (!frame) break;
      this.sender?.(frame);
    }
    // then drain events
    const raw = Wasm.client_drain_events(this.handle) as ClientEventShape[] | null;
    if (raw && raw.length > 0) {
      dispatchClientEvents(raw, {
        resolvers: this.resolvers,
        onSnapshotDirty: this.callbacks.onSnapshotDirty,
        onHandshakeAccepted: this.callbacks.onHandshakeAccepted,
        onTransportOpen: this.callbacks.onTransportOpen,
        onTransportClosed: this.callbacks.onTransportClosed,
        onWatchEvent: this.callbacks.onWatchEvent,
        onWatchResubscribed: this.callbacks.onWatchResubscribed,
      });
    }
  }

  snapshot(): unknown {
    if (!this.handle) return null;
    return Wasm.client_snapshot(this.handle);
  }

  cancel(targetRequestId: bigint): bigint {
    this.requireHandle();
    const cancelId = Wasm.client_cancel(this.handle!, targetRequestId);
    this.pump();
    return cancelId;
  }

  dispatchWorkspaceCurrent(): Promise<unknown> {
    return this.dispatchWithId((h) => Wasm.client_dispatch_workspace_current(h));
  }

  dispatchWorkspaceList(params: { directory: unknown | null }): Promise<unknown> {
    return this.dispatchWithId((h) => Wasm.client_dispatch_workspace_list(h, params));
  }

  dispatchPreviewRequest(params: unknown): Promise<unknown> {
    const result = this.dispatchWithId((h) =>
      Wasm.client_dispatch_preview_request(h, params),
    );
    const delayMs = readTestPreviewDelayMs();
    return delayMs > 0 ? delayPromise(result, delayMs) : result;
  }

  dispatchFileRead(params: unknown): Promise<unknown> {
    const result = this.dispatchWithId((h) => Wasm.client_dispatch_file_read(h, params));
    const delayMs = readTestFileReadDelayMs();
    return delayMs > 0 ? delayPromise(result, delayMs) : result;
  }

  dispatchFileWriteText(params: unknown): Promise<unknown> {
    return this.dispatchWithId((h) => Wasm.client_dispatch_file_write_text(h, params));
  }

  dispatchConfigLoad(): Promise<unknown> {
    return this.dispatchWithId((h) => Wasm.client_dispatch_config_load(h));
  }

  dispatchConfigSave(params: unknown): Promise<unknown> {
    return this.dispatchWithId((h) => Wasm.client_dispatch_config_save(h, params));
  }

  dispatchSlicerList(params: unknown): Promise<unknown> {
    return this.dispatchWithId((h) => Wasm.client_dispatch_slicer_list(h, params));
  }

  dispatchExportRun(params: unknown): Promise<unknown> {
    return this.dispatchWithId((h) => Wasm.client_dispatch_export_run(h, params));
  }

  subscribeDirectoryWatch(params: unknown): bigint {
    this.requireHandle();
    const id = Wasm.client_subscribe_directory_watch(this.handle!, params);
    this.pump();
    return id;
  }

  destroy(): void {
    if (this.destroyed) return;
    this.destroyed = true;
    this.resolvers.clear({ type: "transport_closed" });
    if (this.handle) {
      Wasm.client_destroy(this.handle);
      this.handle = null;
    }
  }

  private dispatchWithId(
    invoke: (handle: Wasm.ClientHandle) => bigint,
  ): Promise<unknown> {
    this.requireHandle();
    return new Promise((resolve, reject) => {
      let requestId: bigint;
      try {
        requestId = invoke(this.handle!);
      } catch (err) {
        reject(err);
        return;
      }
      this.resolvers.register(requestId, { resolve, reject });
      this.pump();
    });
  }

  private requireHandle(): void {
    if (this.destroyed || !this.handle) {
      throw new Error("WasmClient already destroyed");
    }
  }
}

function readTestPreviewDelayMs(): number {
  return readTestDelayMs("__studioWebPreviewDelayMs");
}

function readTestFileReadDelayMs(): number {
  return readTestDelayMs("__studioWebFileReadDelayMs");
}

function readTestDelayMs(key: "__studioWebPreviewDelayMs" | "__studioWebFileReadDelayMs"): number {
  if (typeof window === "undefined") return 0;
  const value = (
    window as Window &
      Partial<
        Record<"__studioWebPreviewDelayMs" | "__studioWebFileReadDelayMs", unknown>
      >
  )[key];
  return typeof value === "number" && Number.isFinite(value)
    ? Math.max(0, value)
    : 0;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

function delayPromise<T>(promise: Promise<T>, ms: number): Promise<T> {
  return promise.then(
    (value) => delay(ms).then(() => value),
    (error: unknown) =>
      delay(ms).then(() => {
        throw error;
      }),
  );
}
