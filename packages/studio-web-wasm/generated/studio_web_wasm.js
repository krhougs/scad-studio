/* @ts-self-types="./studio_web_wasm.d.ts" */

import * as wasm from "./studio_web_wasm_bg.wasm";
import { __wbg_set_wasm } from "./studio_web_wasm_bg.js";
__wbg_set_wasm(wasm);
wasm.__wbindgen_start();
export {
    ClientHandle, MeshHandle, RendererHandle, client_begin_handshake, client_cancel, client_create, client_create_with_timeouts, client_destroy, client_dispatch_config_load, client_dispatch_config_save, client_dispatch_export_run, client_dispatch_file_read, client_dispatch_file_write_text, client_dispatch_preview_request, client_dispatch_slicer_list, client_dispatch_workspace_current, client_dispatch_workspace_list, client_drain_events, client_mark_transport_closed, client_next_outbound, client_receive_inbound, client_snapshot, client_subscribe_directory_watch, client_tick, mesh_decode, mesh_destroy, parameters_format_defines, parameters_parse_source, presets_parse_shared_file, presets_stringify_shared_file, renderer_create, renderer_destroy, renderer_render, renderer_resize
} from "./studio_web_wasm_bg.js";
