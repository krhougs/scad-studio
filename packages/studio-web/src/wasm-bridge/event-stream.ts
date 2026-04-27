// 把 wasm `client_drain_events` 返回的 JS 对象标准化并派发到 resolver / snapshot
// 刷新回调。所有 ClientEvent 语义的唯一入口；外部模块禁止直接消费 drain_events。

import type { RequestResolverMap } from "./request-resolvers";

export type ClientErrorShape = {
  type: string;
  payload?: unknown;
};

export type ClientEventShape = {
  type: string;
  payload?: Record<string, unknown>;
};

export type SnapshotDirtyCallback = () => void;

export type DispatchContext = {
  resolvers: RequestResolverMap;
  onSnapshotDirty: SnapshotDirtyCallback;
  onHandshakeAccepted?: (payload: Record<string, unknown>) => void;
  onTransportOpen?: () => void;
  onTransportClosed?: (reason: unknown) => void;
  onWatchEvent?: (requestId: bigint, payload: unknown) => void;
  onWatchResubscribed?: (requestId: bigint) => void;
  onAgentEvent?: (payload: unknown) => void;
};

export function dispatchClientEvents(
  events: ClientEventShape[],
  ctx: DispatchContext,
): void {
  for (const event of events) {
    dispatchOne(event, ctx);
  }
}

function dispatchOne(event: ClientEventShape, ctx: DispatchContext): void {
  const payload = (event.payload ?? {}) as Record<string, unknown>;
  switch (event.type) {
    case "handshake_accepted":
      ctx.onHandshakeAccepted?.(payload);
      ctx.onSnapshotDirty();
      return;
    case "request_succeeded": {
      const requestId = extractRequestId(payload);
      if (requestId !== null) {
        ctx.resolvers.resolve(requestId, payload["payload"]);
      }
      ctx.onSnapshotDirty();
      return;
    }
    case "request_failed": {
      const requestId = extractRequestId(payload);
      if (requestId !== null) {
        ctx.resolvers.reject(requestId, payload["error"]);
      }
      ctx.onSnapshotDirty();
      return;
    }
    case "request_timed_out": {
      const requestId = extractRequestId(payload);
      if (requestId !== null) {
        ctx.resolvers.reject(requestId, { type: "timeout" } satisfies ClientErrorShape);
      }
      ctx.onSnapshotDirty();
      return;
    }
    case "watch_event": {
      const requestId = extractRequestId(payload);
      if (requestId !== null) {
        ctx.onWatchEvent?.(requestId, payload["payload"]);
      }
      ctx.onSnapshotDirty();
      return;
    }
    case "watch_resubscribed": {
      const requestId = extractRequestId(payload);
      if (requestId !== null) {
        ctx.onWatchResubscribed?.(requestId);
      }
      return;
    }
    case "agent_event":
      ctx.onAgentEvent?.(payload["payload"]);
      ctx.onSnapshotDirty();
      return;
    case "transport_open":
      ctx.onTransportOpen?.();
      ctx.onSnapshotDirty();
      return;
    case "transport_closed":
      ctx.onTransportClosed?.(payload["reason"]);
      ctx.onSnapshotDirty();
      return;
    default:
      return;
  }
}

function extractRequestId(payload: Record<string, unknown>): bigint | null {
  const raw = payload["request_id"];
  if (typeof raw === "bigint") return raw;
  if (typeof raw === "number") return BigInt(raw);
  if (raw && typeof raw === "object") {
    const inner = (raw as Record<string, unknown>)[0 as unknown as string];
    if (typeof inner === "bigint") return inner;
    if (typeof inner === "number") return BigInt(inner);
  }
  return null;
}
