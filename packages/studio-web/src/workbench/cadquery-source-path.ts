import { isPathHandleLike, pathSegments } from "./path-utils";

export function cadQueryPreviewSourcePath(path: unknown, label: string): unknown {
  const lower = label.toLowerCase();
  if (lower.endsWith(".py")) return path;
  if (!lower.endsWith(".step") && !lower.endsWith(".stp")) return path;
  const segments = pathSegments(path);
  if (segments.length !== 2 || segments[0] !== "outputs") return path;
  const stem = sourceStem(segments[1]);
  if (!stem) return path;
  if (!isPathHandleLike(path)) return path;
  return { ...path, path_segments: ["parts", `${stem}.py`] };
}

function sourceStem(filename: string): string | null {
  const lower = filename.toLowerCase();
  const suffix = lower.endsWith(".step") ? ".step" : ".stp";
  const stem = filename.slice(0, filename.length - suffix.length);
  return stem.length > 0 ? stem : null;
}
