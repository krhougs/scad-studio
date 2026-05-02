/* tslint:disable */
/* eslint-disable */

export function protocol_decode_client_frame(bytes: Uint8Array): any;

export function protocol_decode_server_frame(bytes: Uint8Array): any;

export function protocol_encode_agent_cancel_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_agent_invoke_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_agent_model_params_update_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_agent_model_registry_request(request_id: bigint): Uint8Array;

export function protocol_encode_agent_model_select_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_agent_start_turn_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_cadquery_execute_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_cadquery_preview_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_cadquery_result_get_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_cancel_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_chat_archive_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_chat_create_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_chat_history_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_chat_list_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_chat_send_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_close_frame(): Uint8Array;

export function protocol_encode_config_load_request(request_id: bigint): Uint8Array;

export function protocol_encode_config_save_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_export_run_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_file_read_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_file_write_text_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_handshake_frame(request: any): Uint8Array;

export function protocol_encode_preview_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_reconnect_frame(request: any): Uint8Array;

export function protocol_encode_selection_update_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_session_reclaim_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_slicer_list_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_watch_subscribe_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_watch_unsubscribe_request(request_id: bigint, request: any): Uint8Array;

export function protocol_encode_workspace_current_request(request_id: bigint): Uint8Array;

export function protocol_encode_workspace_list_request(request_id: bigint, request: any): Uint8Array;

export function protocol_path_handle(workspace_id: string, segments: any): any;

export function protocol_resolve_relative_link(base: any, link: string): any;

export function protocol_validate_config(value: any): Uint8Array;

export function protocol_validate_finite_number(value: number, field: string): number;

export function protocol_validate_host_local_path(path: string): any;

export function protocol_wasm_start(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly protocol_decode_client_frame: (a: number, b: number) => [number, number, number];
    readonly protocol_decode_server_frame: (a: number, b: number) => [number, number, number];
    readonly protocol_encode_agent_cancel_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_agent_invoke_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_agent_model_params_update_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_agent_model_registry_request: (a: bigint) => [number, number, number, number];
    readonly protocol_encode_agent_model_select_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_agent_start_turn_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_cadquery_execute_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_cadquery_preview_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_cadquery_result_get_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_cancel_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_chat_archive_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_chat_create_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_chat_history_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_chat_list_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_chat_send_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_close_frame: () => [number, number, number, number];
    readonly protocol_encode_config_load_request: (a: bigint) => [number, number, number, number];
    readonly protocol_encode_config_save_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_export_run_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_file_read_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_file_write_text_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_handshake_frame: (a: any) => [number, number, number, number];
    readonly protocol_encode_preview_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_reconnect_frame: (a: any) => [number, number, number, number];
    readonly protocol_encode_selection_update_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_session_reclaim_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_slicer_list_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_watch_subscribe_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_watch_unsubscribe_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_encode_workspace_current_request: (a: bigint) => [number, number, number, number];
    readonly protocol_encode_workspace_list_request: (a: bigint, b: any) => [number, number, number, number];
    readonly protocol_path_handle: (a: number, b: number, c: any) => [number, number, number];
    readonly protocol_resolve_relative_link: (a: any, b: number, c: number) => [number, number, number];
    readonly protocol_validate_config: (a: any) => [number, number, number, number];
    readonly protocol_validate_finite_number: (a: number, b: number, c: number) => [number, number, number];
    readonly protocol_validate_host_local_path: (a: number, b: number) => [number, number, number];
    readonly protocol_wasm_start: () => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
