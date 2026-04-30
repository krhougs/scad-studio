import { useMemo, useRef } from "react";
import type {
  ExternalStoreAdapter,
  ThreadMessageLike,
} from "@assistant-ui/react";
import { useExternalStoreRuntime } from "@assistant-ui/react";
import type {
  ChatMessageRecord,
  AgentEvent,
  AgentRun,
} from "./chat-zone";

export type RuntimeMessage = {
  readonly source: "history" | "stream" | "event" | "reasoning";
  readonly id: string;
  readonly record?: ChatMessageRecord;
  readonly streamText?: string;
  readonly reasoningText?: string;
  readonly event?: AgentEvent;
};

export function useChatRuntime(input: {
  messages: ChatMessageRecord[];
  agentEvents: AgentEvent[];
  agentRun: AgentRun | null;
  onNew: (text: string) => void;
  onCancel: () => void;
  disabled: boolean;
}): ReturnType<typeof useExternalStoreRuntime> {
  const converted = useMemo(
    () => convertAll(input.messages, input.agentEvents, input.agentRun),
    [input.messages, input.agentEvents, input.agentRun],
  );

  const onNewRef = useRef(input.onNew);
  onNewRef.current = input.onNew;
  const onCancelRef = useRef(input.onCancel);
  onCancelRef.current = input.onCancel;

  const store = useMemo(
    (): ExternalStoreAdapter<RuntimeMessage> => ({
      messages: converted,
      isRunning: Boolean(input.agentRun),
      isDisabled: input.disabled,
      convertMessage,
      onNew: async (msg) => {
        let text = "";
        for (const part of msg.content) {
          if (part.type === "text") text += part.text;
        }
        onNewRef.current(text);
      },
      onCancel: async () => {
        onCancelRef.current();
      },
    }),
    [converted, input.agentRun, input.disabled],
  );

  return useExternalStoreRuntime(store);
}

export function convertAll(
  history: ChatMessageRecord[],
  events: AgentEvent[],
  agentRun: AgentRun | null,
): RuntimeMessage[] {
  const result: RuntimeMessage[] = [];

  const visibleRunId = agentRun?.run_id ?? latestRunIdFromEvents(events);
  const hasEvents = events.length > 0;

  for (const record of history) {
    if (record.role === "meta") continue;
    if (hasEvents && visibleRunId && record.role === "assistant" && record.run_id === visibleRunId) continue;
    result.push({
      source: "history",
      id: `h-${record.message_id}`,
      record,
    });
  }

  if (!hasEvents) return result;

  let pendingText = "";
  let pendingReasoning = "";
  let streamIndex = 0;
  let reasoningIndex = 0;
  const flushStream = () => {
    if (!pendingText) return;
    result.push({
      source: "stream",
      id: `stream-${streamIndex}`,
      streamText: pendingText,
    });
    streamIndex += 1;
    pendingText = "";
  };
  const flushReasoning = () => {
    if (!pendingReasoning) return;
    result.push({
      source: "reasoning",
      id: `reasoning-${reasoningIndex}`,
      reasoningText: pendingReasoning,
    });
    reasoningIndex += 1;
    pendingReasoning = "";
  };

  for (let i = 0; i < events.length; i++) {
    const event = events[i]!;
    const reasoning = agentEventReasoning(event);
    if (reasoning) {
      flushStream();
      pendingReasoning += reasoning;
      continue;
    }
    const token = agentEventToken(event);
    if (token) {
      flushReasoning();
      pendingText += token;
      continue;
    }
    flushStream();
    flushReasoning();
    result.push({
      source: "event",
      id: `evt-${i}-${event.event}`,
      event,
    });
  }
  flushStream();
  flushReasoning();

  return keepLatestReasoning(result);
}

export function historyHasRun(
  history: ChatMessageRecord[],
  runId: string | null,
): boolean {
  if (!runId) return false;
  const assistantCount = history.filter((m) => m.role === "assistant").length;
  if (assistantCount === 0) return false;
  for (let i = history.length - 1; i >= 0; i--) {
    const msg = history[i]!;
    if (msg.role === "assistant" && msg.run_id === runId) return true;
    if (msg.role === "assistant" && msg.run_id) return false;
  }
  return false;
}

function latestRunIdFromEvents(events: AgentEvent[]): string | null {
  for (let i = events.length - 1; i >= 0; i--) {
    const runId = events[i]?.payload?.["run_id"];
    if (typeof runId === "string") return runId;
  }
  return null;
}

export function convertMessage(msg: RuntimeMessage): ThreadMessageLike {
  if (msg.source === "history" && msg.record) {
    const r = msg.record;
    const event = historyToolEvent(r);
    if (event) {
      return {
        role: "assistant",
        content: [
          {
            type: `data-${event.event}` as `data-${string}`,
            data: extractEventPayload(event),
          },
        ],
        id: r.message_id,
        createdAt: new Date(r.ts_ms),
        metadata: {
          custom: { run_id: r.run_id ?? undefined },
        },
      };
    }
    return {
      role: r.role === "user" ? "user" : "assistant",
      content: r.content,
      id: r.message_id,
      createdAt: new Date(r.ts_ms),
      metadata: {
        custom: { run_id: r.run_id ?? undefined },
      },
    };
  }

  if (msg.source === "stream") {
    return {
      role: "assistant",
      content: [{ type: "text" as const, text: msg.streamText ?? "" }],
      id: msg.id,
    };
  }

  if (msg.source === "event" && msg.event) {
    return {
      role: "assistant",
      content: [
        {
          type: `data-${msg.event.event}` as `data-${string}`,
          data: extractEventPayload(msg.event),
        },
      ],
      id: msg.id,
    };
  }

  if (msg.source === "reasoning") {
    return {
      role: "assistant",
      content: [
        {
          type: "data-agent.reasoning" as const,
          data: {
            event: "agent.reasoning",
            text: msg.reasoningText ?? "",
          },
        },
      ],
      id: msg.id,
    };
  }

  return { role: "assistant", content: "", id: msg.id };
}

function historyToolEvent(record: ChatMessageRecord): AgentEvent | null {
  const toolResult = record.tool_result;
  if (toolResult && typeof toolResult === "object") {
    return {
      event: "agent.tool_result",
      payload: objectPayload(toolResult),
    };
  }

  const firstToolCall = Array.isArray(record.tool_calls)
    ? record.tool_calls[0]
    : null;
  if (firstToolCall && typeof firstToolCall === "object") {
    return {
      event: "agent.tool_start",
      payload: objectPayload(firstToolCall),
    };
  }
  return null;
}

function objectPayload(value: object): Record<string, unknown> {
  return value as Record<string, unknown>;
}

function extractEventPayload(
  event: AgentEvent,
): Record<string, unknown> {
  const payload = event.payload ?? {};
  return {
    ...payload,
    event: event.event,
  };
}

export function agentEventToken(event: AgentEvent): string | null {
  if (event.event !== "agent.token") return null;
  const text = event.payload?.["text"];
  return typeof text === "string" ? text : null;
}

export function agentEventReasoning(event: AgentEvent): string | null {
  if (event.event !== "agent.reasoning") return null;
  const text = event.payload?.["text"];
  return typeof text === "string" ? text : null;
}

function keepLatestReasoning(messages: RuntimeMessage[]): RuntimeMessage[] {
  let latestReasoning = -1;
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    if (messages[index]?.source === "reasoning") {
      latestReasoning = index;
      break;
    }
  }
  if (latestReasoning < 0) return messages;
  return messages.filter(
    (message, index) => message.source !== "reasoning" || index === latestReasoning,
  );
}
