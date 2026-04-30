import { describe, expect, it } from "vitest";
import type { ChatMessageRecord, AgentEvent, AgentRun } from "../../src/workbench/chat-zone";
import {
  convertMessage,
  convertAll,
  historyHasRun,
  agentEventToken,
  agentEventReasoning,
  type RuntimeMessage,
} from "../../src/workbench/chat-runtime";

function makeRecord(
  overrides: Partial<ChatMessageRecord> & { message_id: string; role: ChatMessageRecord["role"] },
): ChatMessageRecord {
  return {
    ts_ms: 1000,
    content: "",
    tool_calls: [],
    tool_result: null,
    mesh_result: null,
    ...overrides,
  };
}

function makeEvent(event: string, payload?: Record<string, unknown>): AgentEvent {
  return { event, payload };
}

describe("convertMessage", () => {
  it("converts history user message", () => {
    const msg: RuntimeMessage = {
      source: "history",
      id: "h-1",
      record: makeRecord({ message_id: "1", role: "user", content: "hello" }),
    };
    const result = convertMessage(msg);
    expect(result.role).toBe("user");
    expect(result.content).toBe("hello");
    expect(result.id).toBe("1");
  });

  it("converts history assistant message", () => {
    const msg: RuntimeMessage = {
      source: "history",
      id: "h-2",
      record: makeRecord({ message_id: "2", role: "assistant", content: "hi", run_id: "r1" }),
    };
    const result = convertMessage(msg);
    expect(result.role).toBe("assistant");
    expect(result.content).toBe("hi");
    expect((result.metadata as { custom: { run_id: string } }).custom.run_id).toBe("r1");
  });

  it("maps tool role to assistant", () => {
    const msg: RuntimeMessage = {
      source: "history",
      id: "h-3",
      record: makeRecord({ message_id: "3", role: "tool", content: "{}" }),
    };
    expect(convertMessage(msg).role).toBe("assistant");
  });

  it("converts history tool call placeholder to tool_start data", () => {
    const msg: RuntimeMessage = {
      source: "history",
      id: "h-tool-start",
      record: makeRecord({
        message_id: "tool-start",
        role: "assistant",
        content: "agent tool started",
        tool_calls: [
          {
            tool_call_id: "call-1",
            tool_name: "get_project_context",
            args_json: "{}",
          },
        ],
      }),
    };

    const result = convertMessage(msg);

    expect(result.role).toBe("assistant");
    expect(result.content).toEqual([
      {
        type: "data-agent.tool_start",
        data: {
          event: "agent.tool_start",
          tool_call_id: "call-1",
          tool_name: "get_project_context",
          args_json: "{}",
        },
      },
    ]);
  });

  it("converts history tool result placeholder to tool_result data", () => {
    const msg: RuntimeMessage = {
      source: "history",
      id: "h-tool-result",
      record: makeRecord({
        message_id: "tool-result",
        role: "tool",
        content: "agent tool completed",
        tool_call_id: "call-1",
        tool_result: {
          tool_call_id: "call-1",
          tool_name: "list_directory",
          result_json: "{\"status\":\"error\"}",
        },
      }),
    };

    const result = convertMessage(msg);

    expect(result.role).toBe("assistant");
    expect(result.content).toEqual([
      {
        type: "data-agent.tool_result",
        data: {
          event: "agent.tool_result",
          tool_call_id: "call-1",
          tool_name: "list_directory",
          result_json: "{\"status\":\"error\"}",
        },
      },
    ]);
  });

  it("includes createdAt from ts_ms", () => {
    const msg: RuntimeMessage = {
      source: "history",
      id: "h-4",
      record: makeRecord({ message_id: "4", role: "user", ts_ms: 1700000000000 }),
    };
    const result = convertMessage(msg);
    expect(result.createdAt).toEqual(new Date(1700000000000));
  });

  it("converts stream message to text content array", () => {
    const msg: RuntimeMessage = {
      source: "stream",
      id: "stream-0",
      streamText: "streaming...",
    };
    const result = convertMessage(msg);
    expect(result.role).toBe("assistant");
    expect(result.content).toEqual([{ type: "text", text: "streaming..." }]);
  });

  it("converts stream with empty text", () => {
    const msg: RuntimeMessage = { source: "stream", id: "stream-1" };
    const result = convertMessage(msg);
    expect(result.content).toEqual([{ type: "text", text: "" }]);
  });

  it("converts event message with data- prefix", () => {
    const msg: RuntimeMessage = {
      source: "event",
      id: "evt-0",
      event: makeEvent("agent.error", { message: "fail" }),
    };
    const result = convertMessage(msg);
    expect(result.role).toBe("assistant");
    expect(result.content).toEqual([
      {
        type: "data-agent.error",
        data: { event: "agent.error", message: "fail" },
      },
    ]);
  });

  it("converts reasoning message to Thinking data part", () => {
    const msg: RuntimeMessage = {
      source: "reasoning",
      id: "reasoning-0",
      reasoningText: "Checking AirPods case dimensions.",
    };
    const result = convertMessage(msg);
    expect(result.role).toBe("assistant");
    expect(result.content).toEqual([
      {
        type: "data-agent.reasoning",
        data: {
          event: "agent.reasoning",
          text: "Checking AirPods case dimensions.",
        },
      },
    ]);
  });

  it("event payload does not overwrite event name", () => {
    const msg: RuntimeMessage = {
      source: "event",
      id: "evt-1",
      event: makeEvent("agent.done", { event: "should_be_overwritten" }),
    };
    const result = convertMessage(msg);
    const data = (
      result.content as unknown as readonly { data: Record<string, unknown> }[]
    )[0]!.data;
    expect(data.event).toBe("agent.done");
  });

  it("returns fallback for unknown source combination", () => {
    const msg: RuntimeMessage = { source: "event", id: "unknown" };
    const result = convertMessage(msg);
    expect(result.role).toBe("assistant");
    expect(result.content).toBe("");
  });
});

describe("convertAll", () => {
  it("includes user and assistant history, excludes meta", () => {
    const history = [
      makeRecord({ message_id: "1", role: "user", content: "hi" }),
      makeRecord({ message_id: "2", role: "assistant", content: "hello" }),
      makeRecord({ message_id: "3", role: "meta", content: "system" }),
    ];
    const result = convertAll(history, [], null);
    expect(result).toHaveLength(2);
    expect(result[0]!.id).toBe("h-1");
    expect(result[1]!.id).toBe("h-2");
  });

  it("appends stream tokens from agent events", () => {
    const events: AgentEvent[] = [
      makeEvent("agent.token", { text: "hello " }),
      makeEvent("agent.token", { text: "world" }),
    ];
    const run: AgentRun = { session_id: "s", run_id: "r1" };
    const result = convertAll([], events, run);
    expect(result).toHaveLength(1);
    expect(result[0]!.source).toBe("stream");
    expect(result[0]!.streamText).toBe("hello world");
  });

  it("flushes stream on non-token event", () => {
    const events: AgentEvent[] = [
      makeEvent("agent.token", { text: "part1" }),
      makeEvent("agent.tool_start", { tool_name: "read_file" }),
      makeEvent("agent.token", { text: "part2" }),
    ];
    const run: AgentRun = { session_id: "s", run_id: "r1" };
    const result = convertAll([], events, run);
    expect(result).toHaveLength(3);
    expect(result[0]!.source).toBe("stream");
    expect(result[0]!.streamText).toBe("part1");
    expect(result[1]!.source).toBe("event");
    expect(result[1]!.event!.event).toBe("agent.tool_start");
    expect(result[2]!.source).toBe("stream");
    expect(result[2]!.streamText).toBe("part2");
  });

  it("shows only the latest reasoning block", () => {
    const events: AgentEvent[] = [
      makeEvent("agent.reasoning", { text: "older thought. " }),
      makeEvent("agent.tool_start", { tool_name: "get_project_context" }),
      makeEvent("agent.reasoning", { text: "newer " }),
      makeEvent("agent.reasoning", { text: "thought." }),
      makeEvent("agent.token", { text: "Final answer." }),
    ];
    const run: AgentRun = { session_id: "s", run_id: "r1" };
    const result = convertAll([], events, run);
    const reasoningMessages = result.filter((msg) => msg.source === "reasoning");

    expect(reasoningMessages).toHaveLength(1);
    expect(reasoningMessages[0]!.reasoningText).toBe("newer thought.");
    expect(result.some((msg) => msg.reasoningText === "older thought. ")).toBe(false);
  });

  it("prefers events over history assistant for the same run", () => {
    const history = [
      makeRecord({ message_id: "1", role: "user", content: "hi" }),
      makeRecord({ message_id: "2", role: "assistant", content: "done", run_id: "r1" }),
    ];
    const events: AgentEvent[] = [
      makeEvent("agent.token", { text: "done", run_id: "r1" }),
      makeEvent("agent.done", { run_id: "r1" }),
    ];
    const result = convertAll(history, events, null);
    expect(result).toHaveLength(3);
    expect(result[0]!.source).toBe("history");
    expect(result[0]!.record!.role).toBe("user");
    expect(result[1]!.source).toBe("stream");
    expect(result[2]!.source).toBe("event");
  });

  it("skips history assistant of current run while keeping older runs", () => {
    const history = [
      makeRecord({ message_id: "1", role: "assistant", content: "old", run_id: "r0" }),
      makeRecord({ message_id: "2", role: "user", content: "new question" }),
      makeRecord({ message_id: "3", role: "assistant", content: "new", run_id: "r1" }),
    ];
    const events: AgentEvent[] = [
      makeEvent("agent.token", { text: "new", run_id: "r1" }),
    ];
    const result = convertAll(history, events, null);
    expect(result[0]!.source).toBe("history");
    expect(result[0]!.record!.run_id).toBe("r0");
    expect(result[1]!.source).toBe("history");
    expect(result[1]!.record!.role).toBe("user");
    expect(result[2]!.source).toBe("stream");
  });

  it("keeps history assistant when run_id does not match events", () => {
    const history = [
      makeRecord({ message_id: "1", role: "assistant", content: "old" }),
    ];
    const events: AgentEvent[] = [
      makeEvent("agent.token", { text: "new", run_id: "r1" }),
    ];
    const run: AgentRun = { session_id: "s", run_id: "r1" };
    const result = convertAll(history, events, run);
    expect(result[0]!.source).toBe("history");
    expect(result[1]!.source).toBe("stream");
  });

  it("merges history and events when history does not cover run", () => {
    const history = [
      makeRecord({ message_id: "1", role: "user", content: "hi" }),
    ];
    const events: AgentEvent[] = [
      makeEvent("agent.token", { text: "reply", run_id: "r2" }),
    ];
    const result = convertAll(history, events, null);
    expect(result).toHaveLength(2);
    expect(result[0]!.source).toBe("history");
    expect(result[1]!.source).toBe("stream");
  });

  it("handles empty inputs", () => {
    expect(convertAll([], [], null)).toEqual([]);
  });

  it("handles events with no payload", () => {
    const events: AgentEvent[] = [makeEvent("agent.done")];
    const run: AgentRun = { session_id: "s", run_id: "r1" };
    const result = convertAll([], events, run);
    expect(result).toHaveLength(1);
    expect(result[0]!.source).toBe("event");
  });
});

describe("historyHasRun", () => {
  it("returns false for null runId", () => {
    const history = [makeRecord({ message_id: "1", role: "assistant", run_id: "r1" })];
    expect(historyHasRun(history, null)).toBe(false);
  });

  it("returns false for empty history", () => {
    expect(historyHasRun([], "r1")).toBe(false);
  });

  it("returns false when no assistant messages exist", () => {
    const history = [makeRecord({ message_id: "1", role: "user" })];
    expect(historyHasRun(history, "r1")).toBe(false);
  });

  it("returns true when last assistant has matching run_id", () => {
    const history = [
      makeRecord({ message_id: "1", role: "user" }),
      makeRecord({ message_id: "2", role: "assistant", run_id: "r1" }),
    ];
    expect(historyHasRun(history, "r1")).toBe(true);
  });

  it("returns false when last assistant has different run_id", () => {
    const history = [
      makeRecord({ message_id: "1", role: "assistant", run_id: "r1" }),
      makeRecord({ message_id: "2", role: "assistant", run_id: "r2" }),
    ];
    expect(historyHasRun(history, "r1")).toBe(false);
  });

  it("skips assistant messages without run_id", () => {
    const history = [
      makeRecord({ message_id: "1", role: "assistant", run_id: "r1" }),
      makeRecord({ message_id: "2", role: "assistant" }),
    ];
    expect(historyHasRun(history, "r1")).toBe(true);
  });

  it("scans backward past user messages", () => {
    const history = [
      makeRecord({ message_id: "1", role: "assistant", run_id: "r1" }),
      makeRecord({ message_id: "2", role: "user" }),
    ];
    expect(historyHasRun(history, "r1")).toBe(true);
  });
});

describe("agentEventToken", () => {
  it("extracts text from agent.token event", () => {
    expect(agentEventToken(makeEvent("agent.token", { text: "hello" }))).toBe("hello");
  });

  it("returns null for non-token events", () => {
    expect(agentEventToken(makeEvent("agent.done"))).toBeNull();
    expect(agentEventToken(makeEvent("agent.error", { message: "fail" }))).toBeNull();
  });

  it("returns null when text is not a string", () => {
    expect(agentEventToken(makeEvent("agent.token", { text: 42 }))).toBeNull();
  });

  it("returns null when payload is missing", () => {
    expect(agentEventToken(makeEvent("agent.token"))).toBeNull();
  });

  it("returns empty string as valid token", () => {
    expect(agentEventToken(makeEvent("agent.token", { text: "" }))).toBe("");
  });
});

describe("agentEventReasoning", () => {
  it("extracts text from agent.reasoning event", () => {
    expect(agentEventReasoning(makeEvent("agent.reasoning", { text: "thinking" }))).toBe("thinking");
  });

  it("returns null for non-reasoning events", () => {
    expect(agentEventReasoning(makeEvent("agent.token", { text: "answer" }))).toBeNull();
  });
});
