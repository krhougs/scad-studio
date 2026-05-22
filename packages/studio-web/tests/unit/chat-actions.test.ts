import { describe, expect, it } from "vitest";
import {
  activeAgentModelSelection,
  parseSlashCommand,
} from "../../src/workbench/chat-actions";

describe("parseSlashCommand", () => {
  it("returns agent for plain text", () => {
    const result = parseSlashCommand("design a phone case");
    expect(result).toEqual({ mode: "agent", prompt: "design a phone case" });
  });

  it("parses /plan with prompt", () => {
    const result = parseSlashCommand("/plan design a sliding lid");
    expect(result).toEqual({ mode: "plan", prompt: "design a sliding lid" });
  });

  it("parses /agent with prompt", () => {
    const result = parseSlashCommand("/agent apply the fillet");
    expect(result).toEqual({ mode: "agent", prompt: "apply the fillet" });
  });

  it("handles /plan with no prompt", () => {
    const result = parseSlashCommand("/plan");
    expect(result).toEqual({ mode: "plan", prompt: "" });
  });

  it("ignores partial matches like /planning", () => {
    const result = parseSlashCommand("/planning something");
    expect(result).toEqual({ mode: "agent", prompt: "/planning something" });
  });

  it("handles leading whitespace before command", () => {
    const result = parseSlashCommand("  /agent do it");
    expect(result).toEqual({ mode: "agent", prompt: "do it" });
  });

  it("treats unknown slash as plain text", () => {
    const result = parseSlashCommand("/help me");
    expect(result).toEqual({ mode: "agent", prompt: "/help me" });
  });
});

describe("activeAgentModelSelection", () => {
  it("returns the active provider and model binding", () => {
    expect(activeAgentModelSelection({
      active_provider_id: "openai",
      active_model_id: "gpt-5.2",
      active_reasoning_effort: "high",
      active_reasoning_effort_applied: true,
      active_service_label: "fast",
      active_service_label_applied: true,
      reasoning_effort_options: ["high"],
      service_label_options: ["fast"],
      providers: [
        {
          id: "openai",
          kind: "openai_responses",
          label: "OpenAI",
          discovery: {
            enabled: false,
            status: "not_started",
            error: null,
          },
          models: [
            {
              id: "gpt-5.2",
              label: "GPT 5.2",
              source: "manual",
              reasoning_effort: "high",
              service_label: "fast",
              native_web_search_enabled: true,
              native_web_search_applied: true,
              web_search_supported: true,
              web_search_unsupported_reason: null,
              search_sources_supported: true,
            },
          ],
        },
      ],
    })).toEqual({
      provider_id: "openai",
      provider_type: "openai_responses",
      model_id: "gpt-5.2",
      reasoning_effort: "high",
      service_label: "fast",
    });
  });

  it("returns null when the active model is missing", () => {
    expect(activeAgentModelSelection({
      active_provider_id: "openai",
      active_model_id: "missing",
      active_reasoning_effort: null,
      active_reasoning_effort_applied: true,
      active_service_label: null,
      active_service_label_applied: true,
      reasoning_effort_options: [],
      service_label_options: [],
      providers: [
        {
          id: "openai",
          kind: "openai_responses",
          label: "OpenAI",
          discovery: {
            enabled: false,
            status: "not_started",
            error: null,
          },
          models: [],
        },
      ],
    })).toBeNull();
  });
});
