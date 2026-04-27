import type {
  CadQueryObjectKind,
  SelectionKind,
  SelectionRef,
} from "@budn/app-server-protocol";

export const DEFAULT_CADQUERY_TARGET_PATH = "parts/agent_model.py";

type SelectionSnapshotLike = {
  selections?: SelectionRef[];
  active_index?: number | null;
};

type ChatSnapshotLike = {
  workspace_current?: { workspace_id?: unknown } | null;
  current_selection?: SelectionSnapshotLike | null;
};

type TargetScope = {
  path: string;
  targetType: "part" | "component" | "assembly";
  affectedPaths?: string[];
};

type PathHandleLike = {
  workspace_id: string;
  path_segments: string[];
};

export function buildCadQueryConfirmation(
  snapshot: ChatSnapshotLike | null,
  targetPath: string,
  prompt: string,
) {
  const explicitPath = normalizeTargetPath(targetPath);
  const scope = scopeFromSelection(activeSelection(snapshot), prompt);
  const derived = explicitPath === DEFAULT_CADQUERY_TARGET_PATH ? scope : null;
  const effectivePath = derived?.path ?? explicitPath;
  const target = pathHandleFromWorkspace(snapshot, effectivePath);
  const affected_files = affectedPathHandles(snapshot, derived, target);
  return {
    request: {
      target_path: target,
      target_type: derived?.targetType ?? targetTypeFromPath(effectivePath),
      code: "",
      export_formats: ["step"],
      params_json: "{}",
    },
    plan_ref: null,
    affected_files,
    new_files: [],
    export_targets: [exportTargetFor(target)],
  };
}

export function cadQuerySelectionSummary(
  snapshot: ChatSnapshotLike | null,
): string | null {
  const selection = activeSelection(snapshot);
  return selection ? preferredSelectionRef(selection) : null;
}

function activeSelection(snapshot: ChatSnapshotLike | null): SelectionRef | null {
  const current = snapshot?.current_selection;
  const selections = current?.selections ?? [];
  const index = current?.active_index;
  if (typeof index === "number" && index >= 0 && index < selections.length) {
    return selections[index] ?? null;
  }
  return selections[selections.length - 1] ?? null;
}

function scopeFromSelection(
  selection: SelectionRef | null,
  prompt: string,
): TargetScope | null {
  if (!selection) return null;
  const movesPlacement = movementIntent(prompt);
  const replacesModel = replacementIntent(prompt);
  if (replacesModel && selection.instance_path) {
    return replacementScopeFromSelection(selection);
  }
  if (movesPlacement && selection.instance_path) {
    return assemblyScopeFromSelection(selection);
  }
  if (movesPlacement && selection.kind === "component") {
    throw new Error("需要选择 assembly instance 才能移动 component");
  }
  const kind = selection.owner_object_kind ?? objectKindFromSelection(selection.kind);
  return pathScopeFromRef(selection.owner_ref_text ?? selection.ref_text, kind);
}

function preferredSelectionRef(selection: SelectionRef): string {
  if (selection.ambiguous) return selection.ref_text;
  const candidate = selection.candidate_feature_ref?.trim();
  return candidate || selection.ref_text;
}

function pathScopeFromRef(
  refText: string,
  kind: CadQueryObjectKind | null,
): TargetScope | null {
  const name = objectNameFromRef(refText);
  if (!name || !kind) return null;
  if (kind === "assembly") return { path: `assemblies/${name}.py`, targetType: "assembly" };
  if (kind === "component") return { path: `components/${name}.py`, targetType: "component" };
  return { path: `parts/${name}.py`, targetType: "part" };
}

function objectKindFromSelection(kind: SelectionKind): CadQueryObjectKind | null {
  if (kind === "part" || kind === "component" || kind === "assembly") {
    return kind;
  }
  if (kind === "instance") return "assembly";
  return null;
}

function assemblyScopeFromSelection(selection: SelectionRef): TargetScope | null {
  const name = selection.instance_path?.split("/")[0]?.trim();
  if (!name) return null;
  const path = `assemblies/${name}.py`;
  return {
    path,
    targetType: "assembly",
    affectedPaths: [path],
  };
}

function movementIntent(prompt: string): boolean {
  const lower = prompt.toLowerCase();
  return (
    containsAsciiWord(lower, [
      "move",
      "shift",
      "place",
      "position",
      "align",
      "rotate",
    ]) || ["移动", "对齐", "旋转", "摆放"].some((word) => prompt.includes(word))
  );
}

function containsAsciiWord(input: string, words: string[]): boolean {
  return input
    .split(/[^a-z]+/u)
    .filter(Boolean)
    .some((token) => words.includes(token));
}

function replacementIntent(prompt: string): boolean {
  const lower = prompt.toLowerCase();
  return (
    ["replace", "swap", "change model", "different model"].some((word) =>
      lower.includes(word),
    ) || ["替换", "换型号", "更换"].some((word) => prompt.includes(word))
  );
}

function replacementScopeFromSelection(selection: SelectionRef): TargetScope | null {
  const assemblyName = selection.instance_path?.split("/")[0]?.trim();
  const assemblyPath = assemblyName ? `assemblies/${assemblyName}.py` : null;
  const owner = ownerPath(selection);
  if (!owner && !assemblyPath) return null;
  const path = assemblyPath ?? owner;
  if (!path) return null;
  return {
    path,
    targetType: targetTypeFromPath(path),
    affectedPaths: uniqueStrings([path, owner].filter(Boolean) as string[]),
  };
}

function ownerPath(selection: SelectionRef): string | null {
  return pathScopeFromRef(
    selection.owner_ref_text ?? selection.ref_text,
    selection.owner_object_kind ?? objectKindFromSelection(selection.kind),
  )?.path ?? null;
}

function objectNameFromRef(refText: string): string | null {
  const match = /^@[a-z_]+\[([^\]]+)\]$/.exec(refText);
  return match?.[1]?.trim() || null;
}

function normalizeTargetPath(targetPath: string): string {
  const trimmed = targetPath.trim();
  return trimmed || DEFAULT_CADQUERY_TARGET_PATH;
}

function pathHandleFromWorkspace(
  snapshot: ChatSnapshotLike | null,
  path: string,
): PathHandleLike {
  const workspaceId = workspaceIdString(snapshot?.workspace_current?.workspace_id);
  if (!workspaceId) throw new Error("workspace id missing for Execute");
  const path_segments = path
    .split("/")
    .map((segment) => segment.trim())
    .filter(Boolean);
  if (path_segments.length === 0 || path_segments.some(isInvalidSegment)) {
    throw new Error("CadQuery target path 无效");
  }
  return { workspace_id: workspaceId, path_segments };
}

function affectedPathHandles(
  snapshot: ChatSnapshotLike | null,
  scope: TargetScope | null,
  target: PathHandleLike,
): PathHandleLike[] {
  const paths = scope?.affectedPaths ?? [target.path_segments.join("/")];
  return paths.map((path) => pathHandleFromWorkspace(snapshot, path));
}

function workspaceIdString(value: unknown): string | null {
  if (typeof value === "string" && value.length > 0) return value;
  if (value && typeof value === "object") {
    const inner = (value as Record<string, unknown>)["0"];
    if (typeof inner === "string" && inner.length > 0) return inner;
  }
  return null;
}

function isInvalidSegment(value: string): boolean {
  return value === "." || value === ".." || value.includes("\\");
}

function targetTypeFromPath(path: string): "part" | "component" | "assembly" {
  if (path.startsWith("assemblies/")) return "assembly";
  if (path.startsWith("components/")) return "component";
  return "part";
}

function uniqueStrings(values: string[]): string[] {
  return values.filter((value, index) => values.indexOf(value) === index);
}

function exportTargetFor(target: PathHandleLike): PathHandleLike {
  const last =
    target.path_segments[target.path_segments.length - 1] ?? "agent_model.py";
  const stem = last.replace(/\.[^.]*$/, "") || "agent_model";
  return {
    workspace_id: target.workspace_id,
    path_segments: ["outputs", `${stem}.step`],
  };
}
