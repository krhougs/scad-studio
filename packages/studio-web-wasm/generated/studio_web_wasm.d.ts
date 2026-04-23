/* tslint:disable */
/* eslint-disable */

export class ClientHandle {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
}

export class MeshHandle {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
}

export class RendererHandle {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
}

export function client_begin_handshake(handle: ClientHandle, params: any): void;

export function client_cancel(handle: ClientHandle, request_id: bigint): bigint;

export function client_create(): ClientHandle;

export function client_create_with_timeouts(timeouts: any): ClientHandle;

export function client_destroy(handle: ClientHandle): void;

export function client_dispatch_config_load(handle: ClientHandle): bigint;

export function client_dispatch_config_save(handle: ClientHandle, params: any): bigint;

export function client_dispatch_export_run(handle: ClientHandle, params: any): bigint;

export function client_dispatch_file_read(handle: ClientHandle, params: any): bigint;

export function client_dispatch_file_write_text(handle: ClientHandle, params: any): bigint;

export function client_dispatch_preview_request(handle: ClientHandle, params: any): bigint;

export function client_dispatch_slicer_list(handle: ClientHandle, params: any): bigint;

export function client_dispatch_workspace_current(handle: ClientHandle): bigint;

export function client_dispatch_workspace_list(handle: ClientHandle, params: any): bigint;

export function client_drain_events(handle: ClientHandle): any;

export function client_mark_transport_closed(handle: ClientHandle, reason: any): void;

export function client_next_outbound(handle: ClientHandle): Uint8Array | undefined;

export function client_receive_inbound(handle: ClientHandle, bytes: Uint8Array): void;

export function client_snapshot(handle: ClientHandle): any;

export function client_subscribe_directory_watch(handle: ClientHandle, params: any): bigint;

export function client_tick(handle: ClientHandle, now_ms: bigint): void;

export function mesh_decode(bytes: Uint8Array): MeshHandle;

export function mesh_destroy(_handle: MeshHandle): void;

export function renderer_create(_canvas_id: string): RendererHandle;

export function renderer_destroy(_handle: RendererHandle): void;

export function renderer_render(_handle: RendererHandle, _mesh: MeshHandle, _camera: any): void;

export function renderer_resize(_handle: RendererHandle, _width: number, _height: number, _device_pixel_ratio: number): void;
