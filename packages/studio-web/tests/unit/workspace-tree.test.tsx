import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FilesPanel } from "../../src/workbench/files-panel";
import { WorkspaceTree } from "../../src/workbench/workspace-tree";

describe("WorkspaceTree", () => {
  it("shows invalid entries but does not open or expand them", () => {
    const onRequestPreview = vi.fn();
    const onExpandDirectory = vi.fn();

    render(
      <WorkspaceTree
        entries={[
          {
            label: "bad#dir",
            path: null,
            kind: "directory",
            pathError: "path segment contains a disallowed character",
            isOperable: false,
          },
          {
            label: "bad#file.scad",
            path: null,
            kind: "file",
            pathError: "path segment contains a disallowed character",
            isOperable: false,
          },
        ]}
        activeFilePath={null}
        expandedDirectories={new Map()}
        directoryKey={() => "__root__"}
        onRequestPreview={onRequestPreview}
        onExpandDirectory={onExpandDirectory}
        onCollapseDirectory={vi.fn()}
      />,
    );

    const directory = screen.getByTestId("entry-bad#dir");
    const file = screen.getByTestId("entry-bad#file.scad");
    expect((directory as HTMLButtonElement).disabled).toBe(true);
    expect((file as HTMLButtonElement).disabled).toBe(true);

    fireEvent.click(directory);
    fireEvent.click(file);

    expect(onExpandDirectory).not.toHaveBeenCalled();
    expect(onRequestPreview).not.toHaveBeenCalled();
    expect(screen.getByTestId("entry-kind-bad#dir").textContent).toBe("invalid");
    expect(screen.getByTestId("entry-kind-bad#file.scad").textContent).toBe(
      "invalid",
    );
  });
});

describe("FilesPanel", () => {
  it("shows a refresh button and calls the refresh handler", () => {
    const onRefreshFiles = vi.fn();
    render(
      <FilesPanel
        rootName="demo"
        entries={[]}
        entriesLoaded={true}
        activeFilePath={null}
        expandedDirectories={new Map()}
        directoryKey={() => "__root__"}
        onRequestPreview={vi.fn()}
        onExpandDirectory={vi.fn()}
        onCollapseDirectory={vi.fn()}
        onRefreshFiles={onRefreshFiles}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "refresh files" }));

    expect(onRefreshFiles).toHaveBeenCalledTimes(1);
  });
});
