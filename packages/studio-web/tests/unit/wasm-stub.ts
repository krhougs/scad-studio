// Test-time stub standing in for `@scad-studio/studio-web-wasm`.
// The real module pulls in a .wasm file that vitest cannot load; the unit test
// only exercises the React hook wiring around renderer_create's error path.

export class ClientHandle {}
export class MeshHandle {}
export class RendererHandle {}

export function client_create(): ClientHandle {
  return new ClientHandle();
}
export function client_create_with_timeouts(): ClientHandle {
  return new ClientHandle();
}
export function client_begin_handshake(): void {}
export function client_cancel(_h: ClientHandle, id: bigint): bigint {
  return id;
}
export function client_destroy(): void {}
export function client_dispatch_config_load(): bigint {
  return 0n;
}
export function client_dispatch_config_save(): bigint {
  return 0n;
}
export function client_dispatch_export_run(): bigint {
  return 0n;
}
export function client_dispatch_file_read(): bigint {
  return 0n;
}
export function client_dispatch_file_write_text(): bigint {
  return 0n;
}
export function client_dispatch_preview_request(): bigint {
  return 0n;
}
export function client_dispatch_slicer_list(): bigint {
  return 0n;
}
export function client_dispatch_workspace_current(): bigint {
  return 0n;
}
export function client_dispatch_workspace_list(): bigint {
  return 0n;
}
export function client_drain_events(): unknown[] {
  return [];
}
export function client_mark_transport_closed(): void {}
export function client_next_outbound(): Uint8Array | undefined {
  return undefined;
}
export function client_receive_inbound(): void {}
export function client_snapshot(): unknown {
  return null;
}
export function client_subscribe_directory_watch(): bigint {
  return 0n;
}
export function client_tick(): void {}
export function mesh_decode(): MeshHandle {
  return new MeshHandle();
}
export function mesh_destroy(): void {}
export function renderer_create(_canvasId: string): RendererHandle {
  throw new Error("renderer not implemented on web yet (test stub)");
}
export function renderer_destroy(): void {}
export function renderer_render(): void {}
export function renderer_resize(): void {}

export function parameters_parse_source(source: string): unknown {
  const entries = source
    .split("\n")
    .map((line) => line.trim())
    .map(parseParameterLine)
    .filter((entry): entry is Record<string, unknown> => entry !== null);
  return { entries, warnings: [] };
}

export function parameters_format_defines(entries: unknown): unknown {
  if (!Array.isArray(entries)) return [];
  return entries
    .map((entry) => {
      const record = entry as Record<string, unknown>;
      const definition = record["definition"] as Record<string, unknown> | undefined;
      const name = definition?.["name"];
      if (typeof name !== "string") return null;
      return `${name}=${formatParameterValue(record["value"])}`;
    })
    .filter((item): item is string => typeof item === "string");
}

export function presets_parse_shared_file(text: string): unknown {
  return JSON.parse(text);
}

export function presets_stringify_shared_file(file: unknown): string {
  return `${JSON.stringify(file, null, 2)}\n`;
}

function parseParameterLine(line: string): Record<string, unknown> | null {
  const choice = line.match(/^(\w+)\s*=\s*"([^"]*)"\s*;\s*\/\/\s*\[([^\]]+)\]/);
  if (choice) {
    return parameterEntry(choice[1], choice[2], {
      Choice: { options: choice[3].split(",").map((item) => item.trim().replaceAll('"', "")) },
    });
  }
  const bool = line.match(/^(\w+)\s*=\s*(true|false)\s*;/);
  if (bool) return parameterEntry(bool[1], bool[2] === "true", "Bool");
  const number = line.match(/^(\w+)\s*=\s*(-?\d+(?:\.\d+)?)\s*;/);
  if (!number) return null;
  return parameterEntry(number[1], Number(number[2]), {
    Number: { min: null, step: null, max: null },
  });
}

function parameterEntry(
  name: string,
  defaultValue: unknown,
  kind: unknown,
): Record<string, unknown> {
  return {
    definition: {
      name,
      group: null,
      hidden: false,
      kind,
      default_value: defaultValue,
    },
    value: defaultValue,
  };
}

function formatParameterValue(value: unknown): string {
  if (typeof value === "number") return String(Number.isInteger(value) ? value : value);
  if (typeof value === "boolean") return String(value);
  return JSON.stringify(String(value ?? ""));
}
