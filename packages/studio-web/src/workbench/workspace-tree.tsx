import {
  Circle,
  File,
  FileCode,
  FileImage,
  FileText,
  Folder,
  type Icon,
} from "@phosphor-icons/react";
import { fileKindLabel } from "./file-kind";

export type WorkspaceEntry = {
  label: string;
  path: unknown;
  kind: "file" | "directory";
};

export type WorkspaceDirectoryNode = {
  key: string;
  label: string;
  path: unknown;
  entries: WorkspaceEntry[] | null;
  loading: boolean;
  error: string | null;
};

type WorkspaceTreeProps = {
  entries: WorkspaceEntry[];
  activeFilePath: unknown | null;
  expandedDirectories: Map<string, WorkspaceDirectoryNode>;
  directoryKey: (path: unknown) => string;
  onRequestPreview: (entry: WorkspaceEntry) => void;
  onExpandDirectory: (entry: WorkspaceEntry) => void;
  onCollapseDirectory: (entry: WorkspaceEntry) => void;
};

export function WorkspaceTree(props: WorkspaceTreeProps) {
  const {
    entries,
    activeFilePath,
    expandedDirectories,
    directoryKey,
    onRequestPreview,
    onExpandDirectory,
    onCollapseDirectory,
  } = props;
  return (
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
  );
}

type EntryRowProps = Omit<WorkspaceTreeProps, "entries"> & {
  entry: WorkspaceEntry;
  depth: number;
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
  const IconComponent = isDirectory ? Folder : iconForEntry(entry);
  const kindLabel = fileKindLabel(entry);

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
        <IconComponent className="ic" size={14} weight="regular" aria-hidden="true" />
        <span className="label-main">{entry.label}</span>
        <span className="dim" data-testid={`entry-kind-${entry.label}`}>
          {kindLabel}
        </span>
      </button>
      {isDirectory && expanded ? (
        <div className="tree-group" role="group" data-testid={`entries-${key}`}>
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

function iconForEntry(entry: WorkspaceEntry): Icon {
  const label = fileKindLabel(entry);
  if (label === "SCAD") return FileCode;
  if (label === "MD" || label === "TXT" || label === "JSON") return FileText;
  if (["PNG", "JPG", "JPEG", "GIF", "WEBP", "BMP", "TIF", "TIFF", "SVG"].includes(label)) {
    return FileImage;
  }
  if (label === "FILE") return File;
  return Circle;
}
