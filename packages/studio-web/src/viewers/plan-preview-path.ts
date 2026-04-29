import { pathSegments } from "../workbench/path-utils";

export type PlanRunTarget = {
  planId: string;
  planRef: unknown;
};

const PLAN_DOCUMENTS = new Set(["plan.md", "request.md", "plan-result.md"]);
const PLAN_ID_PATTERN = /^\d{10}-[a-z0-9]+(?:-[a-z0-9]+)*$/;

export function planRunTargetForPath(path: unknown): PlanRunTarget | null {
  if (!path || typeof path !== "object") return null;
  const segments = pathSegments(path);
  if (
    segments.length !== 3 ||
    segments[0] !== "plans" ||
    !PLAN_ID_PATTERN.test(segments[1] ?? "") ||
    !PLAN_DOCUMENTS.has(segments[2] ?? "")
  ) {
    return null;
  }
  const workspaceId = (path as Record<string, unknown>)["workspace_id"];
  if (workspaceId === undefined) return null;
  return {
    planId: segments[1],
    planRef: {
      workspace_id: workspaceId,
      path_segments: ["plans", segments[1]],
    },
  };
}
