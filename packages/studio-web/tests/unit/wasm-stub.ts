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
