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
    /**
     * 扁平 vertex colors: `[r0, g0, b0, a0, ...]`。若所有顶点都是纯白
     * 默认色（alpha = 1），返回空 `Vec<f32>` 让 TS 侧走无色路径。
     */
    colors(): Float32Array;
    /**
     * 扁平 indices。
     */
    indices(): Uint32Array;
    /**
     * 扁平 normals: `[nx0, ny0, nz0, ...]`。
     */
    normals(): Float32Array;
    /**
     * 扁平 positions: `[x0, y0, z0, x1, y1, z1, ...]`。
     */
    positions(): Float32Array;
    /**
     * 索引数量。
     */
    readonly index_count: number;
    /**
     * 顶点数量（positions.len() / 3 与 vertex count 相等）。
     */
    readonly vertex_count: number;
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

export function parameters_format_defines(entries: any): any;

export function parameters_parse_source(source: string): any;

export function presets_parse_shared_file(text: string): any;

export function presets_stringify_shared_file(file: any): string;

export function renderer_create(_canvas_id: string): RendererHandle;

export function renderer_destroy(_handle: RendererHandle): void;

export function renderer_render(_handle: RendererHandle, _mesh: MeshHandle, _camera: any): void;

export function renderer_resize(_handle: RendererHandle, _width: number, _height: number, _device_pixel_ratio: number): void;
