import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MarkdownViewer } from "../../src/viewers/markdown-viewer";
import type { WasmClient } from "../../src/wasm-bridge";

describe("MarkdownViewer", () => {
  afterEach(cleanup);

  it("shows Run Plan for plan package markdown and emits plan_ref", async () => {
    const onRunPlan = vi.fn();
    render(
      <MarkdownViewer
        path={{
          workspace_id: "ws",
          path_segments: ["plans", "2026050100-add-lid-vents", "plan.md"],
        }}
        client={fakeClient() as unknown as WasmClient}
        onRunPlan={onRunPlan}
      />,
    );

    await waitFor(() => expect(screen.getByTestId("markdown-body")).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: "Run Plan" }));

    expect(onRunPlan).toHaveBeenCalledWith({
      planId: "2026050100-add-lid-vents",
      planRef: {
        workspace_id: "ws",
        path_segments: ["plans", "2026050100-add-lid-vents"],
      },
    });
  });

  it("does not show Run Plan for ordinary markdown files", async () => {
    render(
      <MarkdownViewer
        path={{ workspace_id: "ws", path_segments: ["docs", "README.md"] }}
        client={fakeClient() as unknown as WasmClient}
        onRunPlan={vi.fn()}
      />,
    );

    await waitFor(() => expect(screen.getByTestId("markdown-body")).toBeTruthy());
    expect(screen.queryByRole("button", { name: "Run Plan" })).toBeNull();
  });
});

function fakeClient(): Pick<WasmClient, "dispatchFileRead"> {
  return {
    dispatchFileRead: vi.fn().mockResolvedValue({
      contents: { kind: "utf8_text", payload: "# Plan\n\nDo the work." },
      media_type: "text/markdown",
    }),
  };
}
