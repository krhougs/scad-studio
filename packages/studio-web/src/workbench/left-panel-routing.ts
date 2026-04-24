export type LeftPanelId =
  | "chat"
  | "files"
  | "settings"
  | "log"
  | "parts"
  | "materials"
  | "queue"
  | "history";

export const LEFT_PANEL_PARAM = "left-panel";

const SUPPORTED_PANELS = new Set<LeftPanelId>([
  "chat",
  "files",
  "settings",
  "log",
  "parts",
  "materials",
  "queue",
  "history",
]);

export function normalizeLeftPanelId(value: unknown): LeftPanelId {
  return isLeftPanelId(value) ? value : "chat";
}

export function isLeftPanelId(value: unknown): value is LeftPanelId {
  return typeof value === "string" && SUPPORTED_PANELS.has(value as LeftPanelId);
}
