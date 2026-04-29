import { describe, expect, it } from "vitest";
import { parseSlashCommand } from "../../src/workbench/chat-actions";

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
