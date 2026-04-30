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
  return directoryRefreshableDocumentKind(tab.kind);
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
