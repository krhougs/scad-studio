import type { DocumentTab } from "../state/ui-store";
import { pathKey } from "./path-utils";

export function shouldRefreshDocumentForWatch(
  tab: Pick<DocumentTab, "kind" | "path">,
  changed: Set<string>,
  matchedSettings: boolean,
): boolean {
  const matchedSpecific = changed.has(pathKey(tab.path));
  if (matchedSpecific) return refreshableDocumentKind(tab.kind);
  if (matchedSettings) return false;
  if (tab.kind === "scad") return changed.size === 0;
  return directoryRefreshableDocumentKind(tab.kind);
}

export function shouldRefreshScadSettingsForWatch(
  tab: Pick<DocumentTab, "kind">,
  changed: Set<string>,
  matchedSettings: boolean,
): boolean {
  if (tab.kind !== "scad") return false;
  if (matchedSettings) return true;
  return changed.size === 0;
}

function refreshableDocumentKind(kind: DocumentTab["kind"]): boolean {
  return (
    kind === "scad" ||
    kind === "cadquery" ||
    kind === "mesh" ||
    kind === "markdown" ||
    kind === "image"
  );
}

function directoryRefreshableDocumentKind(kind: DocumentTab["kind"]): boolean {
  return kind === "mesh" || kind === "markdown" || kind === "image";
}
