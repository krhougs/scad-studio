import { describe, expect, it } from "vitest";
import { createSearchParams } from "react-router-dom";
import {
  LEFT_PANEL_PARAM,
  normalizeLeftPanelId,
} from "../../src/workbench/left-panel-routing";

describe("left panel routing", () => {
  it("normalizes supported panel ids from React Router search params", () => {
    expect(normalizeLeftPanelId("chat")).toBe("chat");
    expect(normalizeLeftPanelId("files")).toBe("files");
    expect(normalizeLeftPanelId("settings")).toBe("settings");
    expect(normalizeLeftPanelId("log")).toBe("log");
  });

  it("falls back to chat for missing or unsupported panel ids", () => {
    expect(normalizeLeftPanelId(null)).toBe("chat");
    expect(normalizeLeftPanelId("workspace")).toBe("chat");
    expect(normalizeLeftPanelId("<script>")).toBe("chat");
  });

  it("uses React Router search params for preserving workspace query params", () => {
    const params = createSearchParams("ws=/tmp/workspace");
    params.set(LEFT_PANEL_PARAM, "settings");

    expect(params.get("ws")).toBe("/tmp/workspace");
    expect(params.get(LEFT_PANEL_PARAM)).toBe("settings");
  });
});
