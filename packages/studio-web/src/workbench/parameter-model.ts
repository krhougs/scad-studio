import {
  parameters_format_defines,
  parameters_parse_source,
} from "@scad-studio/studio-web-wasm";

export type ParameterValue = number | boolean | string;

export type ParameterKind =
  | "Bool"
  | { Number: { min?: number | null; step?: number | null; max?: number | null } }
  | { Choice: { options: string[] } };

export type ParameterDefinition = {
  name: string;
  group?: string | null;
  hidden: boolean;
  kind: ParameterKind;
  default_value: ParameterValue;
};

export type ParameterEntry = {
  definition: ParameterDefinition;
  value: ParameterValue;
};

export type ParsedParameterDocument = {
  entries: ParameterEntry[];
  warnings: string[];
};

export type PresetValueMap = Record<string, ParameterValue>;

export function parseParameterSource(source: string): ParsedParameterDocument {
  const raw = parameters_parse_source(source) as unknown;
  if (!isRecord(raw)) return { entries: [], warnings: ["invalid parameter parse result"] };
  const entries = Array.isArray(raw["entries"])
    ? raw["entries"].filter(isParameterEntry)
    : [];
  const warnings = Array.isArray(raw["warnings"])
    ? raw["warnings"].filter((item): item is string => typeof item === "string")
    : [];
  return { entries, warnings };
}

export function mergeParameterEntries(
  previous: ParameterEntry[],
  next: ParameterEntry[],
): ParameterEntry[] {
  const previousValues = new Map(
    previous.map((entry) => [entry.definition.name, entry.value]),
  );
  return next.map((entry) => ({
    ...entry,
    value: previousValues.get(entry.definition.name) ?? entry.value,
  }));
}

export function updateParameterValue(
  entries: ParameterEntry[],
  name: string,
  value: ParameterValue,
): ParameterEntry[] {
  return entries.map((entry) =>
    entry.definition.name === name ? { ...entry, value } : entry,
  );
}

export function restoreParameterValue(
  entries: ParameterEntry[],
  name: string,
): ParameterEntry[] {
  return entries.map((entry) =>
    entry.definition.name === name
      ? { ...entry, value: entry.definition.default_value }
      : entry,
  );
}

export function restoreAllParameterValues(entries: ParameterEntry[]): ParameterEntry[] {
  return entries.map((entry) => ({
    ...entry,
    value: entry.definition.default_value,
  }));
}

export function applyPresetValues(
  entries: ParameterEntry[],
  values: PresetValueMap,
): ParameterEntry[] {
  return entries.map((entry) => {
    const value = values[entry.definition.name];
    return value === undefined ? entry : { ...entry, value };
  });
}

export function currentParameterValues(entries: ParameterEntry[]): PresetValueMap {
  return Object.fromEntries(
    entries.map((entry) => [entry.definition.name, entry.value]),
  );
}

export function formatCurrentDefines(entries: ParameterEntry[]): string[] {
  const raw = parameters_format_defines(entries) as unknown;
  return Array.isArray(raw)
    ? raw.filter((item): item is string => typeof item === "string")
    : [];
}

export function parameterKind(entry: ParameterEntry): "number" | "bool" | "choice" | "text" {
  const kind = entry.definition.kind;
  if (kind === "Bool") return "bool";
  if (hasNumberKind(kind)) return "number";
  if (hasChoiceKind(kind)) return "choice";
  return "text";
}

export function numberBounds(entry: ParameterEntry): {
  min?: number;
  step?: number;
  max?: number;
} {
  const kind = entry.definition.kind;
  if (!hasNumberKind(kind)) return {};
  const numberKind = kind.Number;
  return {
    min: optionalNumber(numberKind["min"]),
    step: optionalNumber(numberKind["step"]),
    max: optionalNumber(numberKind["max"]),
  };
}

export function sliderBounds(entry: ParameterEntry): {
  min: number;
  step: number;
  max: number;
} {
  const bounds = numberBounds(entry);
  const current = typeof entry.value === "number" ? entry.value : 0;
  const fallback =
    typeof entry.definition.default_value === "number"
      ? entry.definition.default_value
      : current;
  const base = Math.max(Math.abs(current), Math.abs(fallback), 1);
  const inferredMin = -2 * base;
  const inferredMax = 2 * base;
  return {
    min: bounds.min ?? inferredMin,
    max: bounds.max ?? inferredMax,
    step: bounds.step ?? inferStep(current, fallback),
  };
}

export function choiceOptions(entry: ParameterEntry): string[] {
  const kind = entry.definition.kind;
  if (!hasChoiceKind(kind)) return [];
  const options = kind.Choice.options;
  return Array.isArray(options)
    ? options.filter((item): item is string => typeof item === "string")
    : [];
}

function isParameterEntry(value: unknown): value is ParameterEntry {
  if (!isRecord(value)) return false;
  const definition = value["definition"];
  return isRecord(definition) && typeof definition["name"] === "string";
}

function hasNumberKind(
  value: ParameterKind,
): value is { Number: { min?: number | null; step?: number | null; max?: number | null } } {
  return typeof value === "object" && value !== null && "Number" in value;
}

function hasChoiceKind(value: ParameterKind): value is { Choice: { options: string[] } } {
  return typeof value === "object" && value !== null && "Choice" in value;
}

function optionalNumber(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object";
}

function inferStep(current: number, fallback: number): number {
  const value = Math.abs(current) > 0 ? current : fallback;
  if (!Number.isFinite(value) || Number.isInteger(value)) return 1;
  const text = String(value);
  const decimal = text.includes(".") ? text.split(".")[1]?.length ?? 0 : 0;
  return 1 / 10 ** Math.min(decimal, 4);
}
