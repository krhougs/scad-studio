// Inspector —— Buddin inspector 结构：kicker + title + insp-sec 列。
// 包含：workspace 树（递归目录展开）、preview target / mesh summary、
// parameters / presets / slicer / export（只在 mesh 或 scad tab 显示相应子块）。

import { Box, Circle, Folder, Plus, type LucideIcon } from "lucide-react";
import type { WasmClient } from "../wasm-bridge";
import { ExportPanel } from "./export-panel";
import { SlicerPanel } from "./slicer-panel";

export type InspectorEntry = {
  label: string;
  path: unknown;
  kind: "file" | "directory";
};

export type InspectorDirectoryNode = {
  key: string;
  label: string;
  path: unknown;
  entries: InspectorEntry[] | null;
  loading: boolean;
  error: string | null;
};

export type InspectorMeshSummary = {
  label: string;
  vertices: number;
  indices: number;
};

type InspectorProps = {
  rootName: string;
  entries: InspectorEntry[];
  entriesLoaded: boolean;
  onRequestPreview: (entry: InspectorEntry) => void;
  onExpandDirectory: (entry: InspectorEntry) => void;
  onCollapseDirectory: (entry: InspectorEntry) => void;
  previewTargetLabel: string;
  meshSummary: InspectorMeshSummary | null;
  expandedDirectories: Map<string, InspectorDirectoryNode>;
  directoryKey: (path: unknown) => string;
  activeFilePath: unknown | null;
  bottomSlot?: React.ReactNode;
  client?: WasmClient | null;
  showMeshPanels?: boolean;
  meshSource?: unknown;
  defaultExportFilename?: string;
  onExportStatus?: (status: string) => void;
};

export function Inspector(props: InspectorProps) {
  const {
    rootName,
    entries,
    entriesLoaded,
    onRequestPreview,
    onExpandDirectory,
    onCollapseDirectory,
    previewTargetLabel,
    meshSummary,
    expandedDirectories,
    directoryKey,
    activeFilePath,
    bottomSlot,
    client,
    showMeshPanels,
    meshSource,
    defaultExportFilename,
    onExportStatus,
  } = props;

  return (
    <aside
      className="inspector"
      aria-label="inspector"
      data-testid="workbench-inspector"
    >
      <header className="inspector-head">
        <div className="kicker">§ inspector</div>
        <div className="title">{rootName}</div>
      </header>
      <div className="insp-body">
        <section className="insp-sec" data-testid="inspector-entries">
          <h5>
            <span>files</span>
            <button type="button" aria-label="add entry" title="add" disabled>
              <Plus size={12} strokeWidth={1.5} aria-hidden="true" />
            </button>
          </h5>
          {!entriesLoaded ? (
            <div className="tree">
              <div className="tree-loading">entries loading…</div>
            </div>
          ) : entries.length === 0 ? (
            <div className="tree">
              <div className="tree-empty">workspace is empty</div>
            </div>
          ) : (
            <div className="tree" data-testid="entries" role="tree">
              {entries.map((entry) => (
                <EntryRow
                  key={`${entry.label}-${directoryKey(entry.path)}`}
                  entry={entry}
                  depth={0}
                  activeFilePath={activeFilePath}
                  onRequestPreview={onRequestPreview}
                  onExpandDirectory={onExpandDirectory}
                  onCollapseDirectory={onCollapseDirectory}
                  expandedDirectories={expandedDirectories}
                  directoryKey={directoryKey}
                />
              ))}
            </div>
          )}
        </section>

        <section className="insp-sec">
          <h5>
            <span>preview</span>
          </h5>
          <div className="field">
            <div className="field-label">
              <span>active target</span>
            </div>
            <div
              className={`field-status${previewTargetLabel === "—" ? "" : " is-ok"}`}
              data-testid="preview-target"
            >
              {previewTargetLabel}
            </div>
          </div>
          {meshSummary ? (
            <div className="field">
              <div className="field-label">
                <span>mesh</span>
              </div>
              <div className="field-status is-ok" data-testid="preview-mesh-summary">
                {meshSummary.vertices} verts · {meshSummary.indices} idx
              </div>
            </div>
          ) : null}
        </section>

        {showMeshPanels && client && meshSource !== undefined ? (
          <>
            <section className="insp-sec">
              <h5>
                <span>export</span>
              </h5>
              <ExportPanel
                client={client}
                source={meshSource}
                defaultFilename={defaultExportFilename ?? "export.stl"}
                onStatus={onExportStatus ?? (() => {})}
              />
            </section>
            <section className="insp-sec">
              <h5>
                <span>slicers</span>
              </h5>
              <SlicerPanel client={client} />
            </section>
          </>
        ) : null}

        {bottomSlot ? (
          <section className="insp-sec" data-testid="inspector-bottom-slot">
            <h5>
              <span>log</span>
            </h5>
            {bottomSlot}
          </section>
        ) : null}
      </div>
    </aside>
  );
}

type EntryRowProps = {
  entry: InspectorEntry;
  depth: number;
  activeFilePath: unknown | null;
  onRequestPreview: (entry: InspectorEntry) => void;
  onExpandDirectory: (entry: InspectorEntry) => void;
  onCollapseDirectory: (entry: InspectorEntry) => void;
  expandedDirectories: Map<string, InspectorDirectoryNode>;
  directoryKey: (path: unknown) => string;
};

function EntryRow({
  entry,
  depth,
  activeFilePath,
  onRequestPreview,
  onExpandDirectory,
  onCollapseDirectory,
  expandedDirectories,
  directoryKey,
}: EntryRowProps) {
  const key = directoryKey(entry.path);
  const isDirectory = entry.kind === "directory";
  const expanded = isDirectory ? expandedDirectories.get(key) : undefined;
  const isExpanded = Boolean(expanded);
  const isActive =
    !isDirectory &&
    activeFilePath !== null &&
    directoryKey(activeFilePath) === key;

  const Icon = isDirectory ? Folder : iconForFile(entry.label);

  const handleClick = () => {
    if (isDirectory) {
      if (isExpanded) onCollapseDirectory(entry);
      else onExpandDirectory(entry);
    } else {
      onRequestPreview(entry);
    }
  };

  return (
    <>
      <button
        type="button"
        className={`tree-item${isActive ? " active" : ""}`}
        style={{ paddingLeft: 12 + depth * 12 }}
        onClick={handleClick}
        data-testid={`entry-${entry.label}`}
        aria-expanded={isDirectory ? isExpanded : undefined}
      >
        <Icon className="ic" size={14} strokeWidth={1.5} aria-hidden="true" />
        <span className="label-main">{entry.label}</span>
        <span className="dim">{isDirectory ? (isExpanded ? "open" : "dir") : "file"}</span>
      </button>
      {isDirectory && expanded ? (
        <div
          className="tree-group"
          role="group"
          data-testid={`entries-${key}`}
        >
          {expanded.loading ? (
            <div className="tree-loading" data-testid={`entries-${key}-loading`}>
              loading…
            </div>
          ) : expanded.error ? (
            <div className="tree-error" data-testid={`entries-${key}-error`}>
              {expanded.error}
            </div>
          ) : expanded.entries && expanded.entries.length === 0 ? (
            <div className="tree-empty" data-testid={`entries-${key}-empty`}>
              empty
            </div>
          ) : (
            (expanded.entries ?? []).map((child) => (
              <EntryRow
                key={`${child.label}-${directoryKey(child.path)}`}
                entry={child}
                depth={depth + 1}
                activeFilePath={activeFilePath}
                onRequestPreview={onRequestPreview}
                onExpandDirectory={onExpandDirectory}
                onCollapseDirectory={onCollapseDirectory}
                expandedDirectories={expandedDirectories}
                directoryKey={directoryKey}
              />
            ))
          )}
        </div>
      ) : null}
    </>
  );
}

function iconForFile(label: string): LucideIcon {
  const lower = label.toLowerCase();
  if (lower.endsWith(".stl") || lower.endsWith(".3mf")) return Box;
  return Circle;
}
