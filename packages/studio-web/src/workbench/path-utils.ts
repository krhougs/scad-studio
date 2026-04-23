// PathHandle 相关的只读辅助函数。
// PathHandle wire 形状：{ workspace_id: { 0: string }, path_segments: string[] }

export type PathHandleLike = {
  workspace_id?: unknown;
  path_segments?: unknown;
};

export function isPathHandleLike(value: unknown): value is PathHandleLike {
  return (
    typeof value === "object" &&
    value !== null &&
    "path_segments" in (value as Record<string, unknown>)
  );
}

export function pathSegments(path: unknown): string[] {
  if (!isPathHandleLike(path)) return [];
  const raw = (path as PathHandleLike).path_segments;
  if (!Array.isArray(raw)) return [];
  return raw.filter((s): s is string => typeof s === "string");
}

export function pathLabel(path: unknown): string {
  const segs = pathSegments(path);
  if (segs.length === 0) return "(root)";
  return segs[segs.length - 1] ?? "";
}

export function pathKey(path: unknown): string {
  const segs = pathSegments(path);
  if (segs.length === 0) return "__root__";
  return segs.join("/");
}

export function pathDisplay(path: unknown): string {
  const segs = pathSegments(path);
  if (segs.length === 0) return "/";
  return "/" + segs.join("/");
}
