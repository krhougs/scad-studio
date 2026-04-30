import type { CadQueryResultReady, PathHandle } from "@budn/app-server-protocol";
import { isPathHandleLike, pathSegments } from "./path-utils";

export type CadQueryArtifactTabPath = PathHandle & {
  type: "cadquery_artifact";
  artifact_path: PathHandle;
  source_path: PathHandle;
  result_id: string;
};

export function cadQueryPreviewSourcePath(path: unknown, _label: string): unknown {
  const artifact = cadQueryArtifactTabPath(path);
  return artifact?.source_path ?? path;
}

export function cadQueryArtifactTabPathForFile(
  path: unknown,
  results: CadQueryResultReady[],
): CadQueryArtifactTabPath | null {
  const exportPath = workspaceRelativePath(path);
  if (!exportPath || !isPathHandleLike(path)) return null;
  for (let i = results.length - 1; i >= 0; i--) {
    const ready = results[i]!;
    const relation = ready.artifact_relation;
    if (!relation) continue;
    if (!relation.exports.some((artifact) => artifact.path === exportPath)) {
      continue;
    }
    const sourceSegments = relationPathSegments(relation.source_path);
    if (sourceSegments.length === 0) return null;
    const artifactPath = path as PathHandle;
    const sourcePath = {
      ...artifactPath,
      path_segments: sourceSegments,
    };
    return {
      ...artifactPath,
      type: "cadquery_artifact",
      artifact_path: artifactPath,
      source_path: sourcePath,
      result_id: ready.result_id,
    };
  }
  return null;
}

export function cadQueryTabMatchesReady(
  path: unknown,
  ready: CadQueryResultReady,
): boolean {
  const record = objectRecord(path);
  if (
    record?.["type"] === "cadquery_result" &&
    record["result_id"] === ready.result_id
  ) {
    return true;
  }
  const relation = ready.artifact_relation;
  if (!relation) return false;
  const candidatePath = workspaceRelativePath(path);
  if (!candidatePath) return false;
  return (
    candidatePath === relation.source_path ||
    relation.exports.some((artifact) => artifact.path === candidatePath)
  );
}

export function isCadQueryStepFile(label: string): boolean {
  const lower = label.toLowerCase();
  return lower.endsWith(".step") || lower.endsWith(".stp");
}

function cadQueryArtifactTabPath(path: unknown): CadQueryArtifactTabPath | null {
  const record = objectRecord(path);
  if (record?.["type"] !== "cadquery_artifact") return null;
  if (!isPathHandleLike(record["source_path"])) return null;
  if (!isPathHandleLike(record["artifact_path"])) return null;
  if (typeof record["result_id"] !== "string") return null;
  return path as CadQueryArtifactTabPath;
}

function workspaceRelativePath(path: unknown): string | null {
  const segments = pathSegments(path);
  return segments.length > 0 ? segments.join("/") : null;
}

function relationPathSegments(path: string): string[] {
  return path.split("/").filter((segment) => segment.length > 0);
}

function objectRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object"
    ? (value as Record<string, unknown>)
    : null;
}
