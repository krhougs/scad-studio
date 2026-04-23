// Maps a file extension to the DocumentTab kind. Unsupported extensions
// return null and are surfaced by WorkbenchLayout as a status message rather
// than a new tab.

import type { DocumentTabKind } from "../state/ui-store";

export function resolveTabKind(label: string): DocumentTabKind | null {
  const lower = label.toLowerCase();
  if (lower.endsWith(".md") || lower.endsWith(".markdown")) return "markdown";
  if (
    lower.endsWith(".png") ||
    lower.endsWith(".jpg") ||
    lower.endsWith(".jpeg") ||
    lower.endsWith(".webp")
  )
    return "image";
  if (lower.endsWith(".scad")) return "scad";
  if (lower.endsWith(".stl") || lower.endsWith(".3mf")) return "mesh";
  return null;
}

export function extensionOf(label: string): string {
  const idx = label.lastIndexOf(".");
  if (idx < 0) return "";
  return label.slice(idx);
}
