import { describe, expect, it } from "vitest";
import { parseSlashCommand } from "../../src/workbench/chat-actions";

describe("parseSlashCommand", () => {
  it("returns auto for plain text", () => {
    const result = parseSlashCommand("design a phone case");
    expect(result).toEqual({ operation: "auto", prompt: "design a phone case" });
  });

  it("parses /plan with prompt", () => {
    const result = parseSlashCommand("/plan design a sliding lid");
    expect(result).toEqual({ operation: "plan", prompt: "design a sliding lid" });
  });

  it("parses /execute with prompt", () => {
    const result = parseSlashCommand("/execute apply the fillet");
    expect(result).toEqual({ operation: "execute", prompt: "apply the fillet" });
  });

  it("parses /inform with prompt", () => {
    const result = parseSlashCommand("/inform explain CadQuery loft");
    expect(result).toEqual({ operation: "inform", prompt: "explain CadQuery loft" });
  });

  it("handles /plan with no prompt", () => {
    const result = parseSlashCommand("/plan");
    expect(result).toEqual({ operation: "plan", prompt: "" });
  });

  it("ignores partial matches like /planning", () => {
    const result = parseSlashCommand("/planning something");
    expect(result).toEqual({ operation: "auto", prompt: "/planning something" });
  });

  it("handles leading whitespace before command", () => {
    const result = parseSlashCommand("  /execute do it");
    expect(result).toEqual({ operation: "execute", prompt: "do it" });
  });

  it("treats unknown slash as plain text", () => {
    const result = parseSlashCommand("/help me");
    expect(result).toEqual({ operation: "auto", prompt: "/help me" });
  });
});
