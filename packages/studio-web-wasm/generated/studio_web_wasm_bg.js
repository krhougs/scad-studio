export class CadQueryMeshHandle {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(CadQueryMeshHandle.prototype);
        obj.__wbg_ptr = ptr;
        CadQueryMeshHandleFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        CadQueryMeshHandleFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_cadquerymeshhandle_free(ptr, 0);
    }
    /**
     * @returns {string}
     */
    get build_id() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.cadquerymeshhandle_build_id(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} part_index
     * @param {number} edge_index
     * @returns {Float32Array}
     */
    edge_polyline(part_index, edge_index) {
        const ret = wasm.cadquerymeshhandle_edge_polyline(this.__wbg_ptr, part_index, edge_index);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * @param {number} part_index
     * @param {number} face_index
     * @returns {Float32Array}
     */
    face_normals(part_index, face_index) {
        const ret = wasm.cadquerymeshhandle_face_normals(this.__wbg_ptr, part_index, face_index);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * @param {number} part_index
     * @param {number} face_index
     * @returns {Float32Array}
     */
    face_positions(part_index, face_index) {
        const ret = wasm.cadquerymeshhandle_face_positions(this.__wbg_ptr, part_index, face_index);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * @returns {any}
     */
    metadata() {
        const ret = wasm.cadquerymeshhandle_metadata(this.__wbg_ptr);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * @returns {number}
     */
    get part_count() {
        const ret = wasm.cadquerymeshhandle_part_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {string}
     */
    get result_id() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.cadquerymeshhandle_result_id(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    get root_object_kind() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.cadquerymeshhandle_root_object_kind(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    get root_ref_text() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.cadquerymeshhandle_root_ref_text(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} part_index
     * @param {number} vertex_index
     * @returns {Float32Array}
     */
    vertex_position(part_index, vertex_index) {
        const ret = wasm.cadquerymeshhandle_vertex_position(this.__wbg_ptr, part_index, vertex_index);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
}
if (Symbol.dispose) CadQueryMeshHandle.prototype[Symbol.dispose] = CadQueryMeshHandle.prototype.free;

export class ClientHandle {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(ClientHandle.prototype);
        obj.__wbg_ptr = ptr;
        ClientHandleFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        ClientHandleFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_clienthandle_free(ptr, 0);
    }
}
if (Symbol.dispose) ClientHandle.prototype[Symbol.dispose] = ClientHandle.prototype.free;

export class MeshHandle {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(MeshHandle.prototype);
        obj.__wbg_ptr = ptr;
        MeshHandleFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        MeshHandleFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_meshhandle_free(ptr, 0);
    }
    /**
     * 扁平 vertex colors: `[r0, g0, b0, a0, ...]`。若所有顶点都是纯白
     * 默认色（alpha = 1），返回空 `Vec<f32>` 让 TS 侧走无色路径。
     * @returns {Float32Array}
     */
    colors() {
        const ret = wasm.meshhandle_colors(this.__wbg_ptr);
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * 索引数量。
     * @returns {number}
     */
    get index_count() {
        const ret = wasm.meshhandle_index_count(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * 扁平 indices。
     * @returns {Uint32Array}
     */
    indices() {
        const ret = wasm.meshhandle_indices(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * 扁平 normals: `[nx0, ny0, nz0, ...]`。
     * @returns {Float32Array}
     */
    normals() {
        const ret = wasm.meshhandle_normals(this.__wbg_ptr);
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * 扁平 positions: `[x0, y0, z0, x1, y1, z1, ...]`。
     * @returns {Float32Array}
     */
    positions() {
        const ret = wasm.meshhandle_positions(this.__wbg_ptr);
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * 顶点数量（positions.len() / 3 与 vertex count 相等）。
     * @returns {number}
     */
    get vertex_count() {
        const ret = wasm.meshhandle_vertex_count(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) MeshHandle.prototype[Symbol.dispose] = MeshHandle.prototype.free;

export class RendererHandle {
    static __wrap(ptr) {
        ptr = ptr >>> 0;
        const obj = Object.create(RendererHandle.prototype);
        obj.__wbg_ptr = ptr;
        RendererHandleFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        RendererHandleFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_rendererhandle_free(ptr, 0);
    }
}
if (Symbol.dispose) RendererHandle.prototype[Symbol.dispose] = RendererHandle.prototype.free;

/**
 * @param {ClientHandle} handle
 * @param {any} params
 */
export function client_begin_handshake(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_begin_handshake(handle.__wbg_ptr, params);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

/**
 * @param {ClientHandle} handle
 * @param {bigint} request_id
 * @returns {bigint}
 */
export function client_cancel(handle, request_id) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_cancel(handle.__wbg_ptr, request_id);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @returns {ClientHandle}
 */
export function client_create() {
    const ret = wasm.client_create();
    return ClientHandle.__wrap(ret);
}

/**
 * @param {any} timeouts
 * @returns {ClientHandle}
 */
export function client_create_with_timeouts(timeouts) {
    const ret = wasm.client_create_with_timeouts(timeouts);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return ClientHandle.__wrap(ret[0]);
}

/**
 * @param {ClientHandle} handle
 */
export function client_destroy(handle) {
    _assertClass(handle, ClientHandle);
    wasm.client_destroy(handle.__wbg_ptr);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_agent_cancel(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_agent_cancel(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_agent_invoke(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_agent_invoke(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_agent_plan_confirm(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_agent_plan_confirm(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_agent_plan_reject(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_agent_plan_reject(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_cadquery_execute(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_cadquery_execute(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_cadquery_preview(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_cadquery_preview(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_cadquery_result_get(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_cadquery_result_get(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_chat_archive(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_chat_archive(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_chat_create(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_chat_create(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_chat_history(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_chat_history(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_chat_list(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_chat_list(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_chat_send(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_chat_send(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @returns {bigint}
 */
export function client_dispatch_config_load(handle) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_config_load(handle.__wbg_ptr);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_config_save(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_config_save(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_export_run(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_export_run(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_file_read(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_file_read(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_file_write_text(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_file_write_text(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_preview_request(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_preview_request(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_selection_update(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_selection_update(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_slicer_list(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_slicer_list(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @returns {bigint}
 */
export function client_dispatch_workspace_current(handle) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_workspace_current(handle.__wbg_ptr);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_dispatch_workspace_list(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_dispatch_workspace_list(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @returns {any}
 */
export function client_drain_events(handle) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_drain_events(handle.__wbg_ptr);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return takeFromExternrefTable0(ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} reason
 */
export function client_mark_transport_closed(handle, reason) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_mark_transport_closed(handle.__wbg_ptr, reason);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

/**
 * @param {ClientHandle} handle
 * @returns {Uint8Array | undefined}
 */
export function client_next_outbound(handle) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_next_outbound(handle.__wbg_ptr);
    if (ret[3]) {
        throw takeFromExternrefTable0(ret[2]);
    }
    let v1;
    if (ret[0] !== 0) {
        v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    }
    return v1;
}

/**
 * @param {ClientHandle} handle
 * @param {Uint8Array} bytes
 */
export function client_receive_inbound(handle, bytes) {
    _assertClass(handle, ClientHandle);
    const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.client_receive_inbound(handle.__wbg_ptr, ptr0, len0);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

/**
 * @param {ClientHandle} handle
 * @returns {any}
 */
export function client_snapshot(handle) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_snapshot(handle.__wbg_ptr);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return takeFromExternrefTable0(ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {any} params
 * @returns {bigint}
 */
export function client_subscribe_directory_watch(handle, params) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_subscribe_directory_watch(handle.__wbg_ptr, params);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return BigInt.asUintN(64, ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {string} result_id
 * @returns {CadQueryMeshHandle | undefined}
 */
export function client_take_cadquery_mesh(handle, result_id) {
    _assertClass(handle, ClientHandle);
    const ptr0 = passStringToWasm0(result_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.client_take_cadquery_mesh(handle.__wbg_ptr, ptr0, len0);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return ret[0] === 0 ? undefined : CadQueryMeshHandle.__wrap(ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {bigint} request_id
 * @returns {MeshHandle | undefined}
 */
export function client_take_preview_mesh(handle, request_id) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_take_preview_mesh(handle.__wbg_ptr, request_id);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return ret[0] === 0 ? undefined : MeshHandle.__wrap(ret[0]);
}

/**
 * @param {ClientHandle} handle
 * @param {bigint} now_ms
 */
export function client_tick(handle, now_ms) {
    _assertClass(handle, ClientHandle);
    const ret = wasm.client_tick(handle.__wbg_ptr, now_ms);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

/**
 * @param {Uint8Array} bytes
 * @returns {MeshHandle}
 */
export function mesh_decode(bytes) {
    const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.mesh_decode(ptr0, len0);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return MeshHandle.__wrap(ret[0]);
}

/**
 * @param {MeshHandle} _handle
 */
export function mesh_destroy(_handle) {
    _assertClass(_handle, MeshHandle);
    var ptr0 = _handle.__destroy_into_raw();
    wasm.mesh_destroy(ptr0);
}

/**
 * @param {any} entries
 * @returns {any}
 */
export function parameters_format_defines(entries) {
    const ret = wasm.parameters_format_defines(entries);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return takeFromExternrefTable0(ret[0]);
}

/**
 * @param {string} source
 * @returns {any}
 */
export function parameters_parse_source(source) {
    const ptr0 = passStringToWasm0(source, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.parameters_parse_source(ptr0, len0);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return takeFromExternrefTable0(ret[0]);
}

/**
 * @param {string} text
 * @returns {any}
 */
export function presets_parse_shared_file(text) {
    const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.presets_parse_shared_file(ptr0, len0);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return takeFromExternrefTable0(ret[0]);
}

/**
 * @param {any} file
 * @returns {string}
 */
export function presets_stringify_shared_file(file) {
    let deferred2_0;
    let deferred2_1;
    try {
        const ret = wasm.presets_stringify_shared_file(file);
        var ptr1 = ret[0];
        var len1 = ret[1];
        if (ret[3]) {
            ptr1 = 0; len1 = 0;
            throw takeFromExternrefTable0(ret[2]);
        }
        deferred2_0 = ptr1;
        deferred2_1 = len1;
        return getStringFromWasm0(ptr1, len1);
    } finally {
        wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
    }
}

/**
 * @param {string} _canvas_id
 * @returns {RendererHandle}
 */
export function renderer_create(_canvas_id) {
    const ptr0 = passStringToWasm0(_canvas_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.renderer_create(ptr0, len0);
    if (ret[2]) {
        throw takeFromExternrefTable0(ret[1]);
    }
    return RendererHandle.__wrap(ret[0]);
}

/**
 * @param {RendererHandle} _handle
 */
export function renderer_destroy(_handle) {
    _assertClass(_handle, RendererHandle);
    var ptr0 = _handle.__destroy_into_raw();
    wasm.renderer_destroy(ptr0);
}

/**
 * @param {RendererHandle} _handle
 * @param {MeshHandle} _mesh
 * @param {any} _camera
 */
export function renderer_render(_handle, _mesh, _camera) {
    _assertClass(_handle, RendererHandle);
    _assertClass(_mesh, MeshHandle);
    const ret = wasm.renderer_render(_handle.__wbg_ptr, _mesh.__wbg_ptr, _camera);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

/**
 * @param {RendererHandle} _handle
 * @param {number} _width
 * @param {number} _height
 * @param {number} _device_pixel_ratio
 */
export function renderer_resize(_handle, _width, _height, _device_pixel_ratio) {
    _assertClass(_handle, RendererHandle);
    wasm.renderer_resize(_handle.__wbg_ptr, _width, _height, _device_pixel_ratio);
}
export function __wbg_Error_2e59b1b37a9a34c3(arg0, arg1) {
    const ret = Error(getStringFromWasm0(arg0, arg1));
    return ret;
}
export function __wbg_Number_e6ffdb596c888833(arg0) {
    const ret = Number(arg0);
    return ret;
}
export function __wbg_String_8564e559799eccda(arg0, arg1) {
    const ret = String(arg1);
    const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
}
export function __wbg___wbindgen_bigint_get_as_i64_2c5082002e4826e2(arg0, arg1) {
    const v = arg1;
    const ret = typeof(v) === 'bigint' ? v : undefined;
    getDataViewMemory0().setBigInt64(arg0 + 8 * 1, isLikeNone(ret) ? BigInt(0) : ret, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
}
export function __wbg___wbindgen_boolean_get_a86c216575a75c30(arg0) {
    const v = arg0;
    const ret = typeof(v) === 'boolean' ? v : undefined;
    return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
}
export function __wbg___wbindgen_debug_string_dd5d2d07ce9e6c57(arg0, arg1) {
    const ret = debugString(arg1);
    const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
}
export function __wbg___wbindgen_in_4bd7a57e54337366(arg0, arg1) {
    const ret = arg0 in arg1;
    return ret;
}
export function __wbg___wbindgen_is_bigint_6c98f7e945dacdde(arg0) {
    const ret = typeof(arg0) === 'bigint';
    return ret;
}
export function __wbg___wbindgen_is_function_49868bde5eb1e745(arg0) {
    const ret = typeof(arg0) === 'function';
    return ret;
}
export function __wbg___wbindgen_is_object_40c5a80572e8f9d3(arg0) {
    const val = arg0;
    const ret = typeof(val) === 'object' && val !== null;
    return ret;
}
export function __wbg___wbindgen_is_string_b29b5c5a8065ba1a(arg0) {
    const ret = typeof(arg0) === 'string';
    return ret;
}
export function __wbg___wbindgen_is_undefined_c0cca72b82b86f4d(arg0) {
    const ret = arg0 === undefined;
    return ret;
}
export function __wbg___wbindgen_jsval_eq_7d430e744a913d26(arg0, arg1) {
    const ret = arg0 === arg1;
    return ret;
}
export function __wbg___wbindgen_jsval_loose_eq_3a72ae764d46d944(arg0, arg1) {
    const ret = arg0 == arg1;
    return ret;
}
export function __wbg___wbindgen_number_get_7579aab02a8a620c(arg0, arg1) {
    const obj = arg1;
    const ret = typeof(obj) === 'number' ? obj : undefined;
    getDataViewMemory0().setFloat64(arg0 + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, !isLikeNone(ret), true);
}
export function __wbg___wbindgen_string_get_914df97fcfa788f2(arg0, arg1) {
    const obj = arg1;
    const ret = typeof(obj) === 'string' ? obj : undefined;
    var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    var len1 = WASM_VECTOR_LEN;
    getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
    getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
}
export function __wbg___wbindgen_throw_81fc77679af83bc6(arg0, arg1) {
    throw new Error(getStringFromWasm0(arg0, arg1));
}
export function __wbg_call_7f2987183bb62793() { return handleError(function (arg0, arg1) {
    const ret = arg0.call(arg1);
    return ret;
}, arguments); }
export function __wbg_done_547d467e97529006(arg0) {
    const ret = arg0.done;
    return ret;
}
export function __wbg_entries_616b1a459b85be0b(arg0) {
    const ret = Object.entries(arg0);
    return ret;
}
export function __wbg_from_741da0f916ab74aa(arg0) {
    const ret = Array.from(arg0);
    return ret;
}
export function __wbg_get_4848e350b40afc16(arg0, arg1) {
    const ret = arg0[arg1 >>> 0];
    return ret;
}
export function __wbg_get_ed0642c4b9d31ddf() { return handleError(function (arg0, arg1) {
    const ret = Reflect.get(arg0, arg1);
    return ret;
}, arguments); }
export function __wbg_get_unchecked_7d7babe32e9e6a54(arg0, arg1) {
    const ret = arg0[arg1 >>> 0];
    return ret;
}
export function __wbg_get_with_ref_key_6412cf3094599694(arg0, arg1) {
    const ret = arg0[arg1];
    return ret;
}
export function __wbg_instanceof_ArrayBuffer_ff7c1337a5e3b33a(arg0) {
    let result;
    try {
        result = arg0 instanceof ArrayBuffer;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
}
export function __wbg_instanceof_Map_a10a2795ef4bfe97(arg0) {
    let result;
    try {
        result = arg0 instanceof Map;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
}
export function __wbg_instanceof_Uint8Array_4b8da683deb25d72(arg0) {
    let result;
    try {
        result = arg0 instanceof Uint8Array;
    } catch (_) {
        result = false;
    }
    const ret = result;
    return ret;
}
export function __wbg_isArray_db61795ad004c139(arg0) {
    const ret = Array.isArray(arg0);
    return ret;
}
export function __wbg_isSafeInteger_ea83862ba994770c(arg0) {
    const ret = Number.isSafeInteger(arg0);
    return ret;
}
export function __wbg_iterator_de403ef31815a3e6() {
    const ret = Symbol.iterator;
    return ret;
}
export function __wbg_length_0c32cb8543c8e4c8(arg0) {
    const ret = arg0.length;
    return ret;
}
export function __wbg_length_6e821edde497a532(arg0) {
    const ret = arg0.length;
    return ret;
}
export function __wbg_new_4f9fafbb3909af72() {
    const ret = new Object();
    return ret;
}
export function __wbg_new_99cabae501c0a8a0() {
    const ret = new Map();
    return ret;
}
export function __wbg_new_a560378ea1240b14(arg0) {
    const ret = new Uint8Array(arg0);
    return ret;
}
export function __wbg_new_f3c9df4f38f3f798() {
    const ret = new Array();
    return ret;
}
export function __wbg_next_01132ed6134b8ef5(arg0) {
    const ret = arg0.next;
    return ret;
}
export function __wbg_next_b3713ec761a9dbfd() { return handleError(function (arg0) {
    const ret = arg0.next();
    return ret;
}, arguments); }
export function __wbg_prototypesetcall_3e05eb9545565046(arg0, arg1, arg2) {
    Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
}
export function __wbg_set_08463b1df38a7e29(arg0, arg1, arg2) {
    const ret = arg0.set(arg1, arg2);
    return ret;
}
export function __wbg_set_6be42768c690e380(arg0, arg1, arg2) {
    arg0[arg1] = arg2;
}
export function __wbg_set_6c60b2e8ad0e9383(arg0, arg1, arg2) {
    arg0[arg1 >>> 0] = arg2;
}
export function __wbg_value_7f6052747ccf940f(arg0) {
    const ret = arg0.value;
    return ret;
}
export function __wbindgen_cast_0000000000000001(arg0) {
    // Cast intrinsic for `F64 -> Externref`.
    const ret = arg0;
    return ret;
}
export function __wbindgen_cast_0000000000000002(arg0) {
    // Cast intrinsic for `I64 -> Externref`.
    const ret = arg0;
    return ret;
}
export function __wbindgen_cast_0000000000000003(arg0, arg1) {
    // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
    const ret = getArrayU8FromWasm0(arg0, arg1);
    return ret;
}
export function __wbindgen_cast_0000000000000004(arg0, arg1) {
    // Cast intrinsic for `Ref(String) -> Externref`.
    const ret = getStringFromWasm0(arg0, arg1);
    return ret;
}
export function __wbindgen_cast_0000000000000005(arg0) {
    // Cast intrinsic for `U64 -> Externref`.
    const ret = BigInt.asUintN(64, arg0);
    return ret;
}
export function __wbindgen_init_externref_table() {
    const table = wasm.__wbindgen_externrefs;
    const offset = table.grow(4);
    table.set(0, undefined);
    table.set(offset + 0, undefined);
    table.set(offset + 1, null);
    table.set(offset + 2, true);
    table.set(offset + 3, false);
}
const CadQueryMeshHandleFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_cadquerymeshhandle_free(ptr >>> 0, 1));
const ClientHandleFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_clienthandle_free(ptr >>> 0, 1));
const MeshHandleFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_meshhandle_free(ptr >>> 0, 1));
const RendererHandleFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_rendererhandle_free(ptr >>> 0, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

function _assertClass(instance, klass) {
    if (!(instance instanceof klass)) {
        throw new Error(`expected instance of ${klass.name}`);
    }
}

function debugString(val) {
    // primitive types
    const type = typeof val;
    if (type == 'number' || type == 'boolean' || val == null) {
        return  `${val}`;
    }
    if (type == 'string') {
        return `"${val}"`;
    }
    if (type == 'symbol') {
        const description = val.description;
        if (description == null) {
            return 'Symbol';
        } else {
            return `Symbol(${description})`;
        }
    }
    if (type == 'function') {
        const name = val.name;
        if (typeof name == 'string' && name.length > 0) {
            return `Function(${name})`;
        } else {
            return 'Function';
        }
    }
    // objects
    if (Array.isArray(val)) {
        const length = val.length;
        let debug = '[';
        if (length > 0) {
            debug += debugString(val[0]);
        }
        for(let i = 1; i < length; i++) {
            debug += ', ' + debugString(val[i]);
        }
        debug += ']';
        return debug;
    }
    // Test for built-in
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    let className;
    if (builtInMatches && builtInMatches.length > 1) {
        className = builtInMatches[1];
    } else {
        // Failed to match the standard '[object ClassName]'
        return toString.call(val);
    }
    if (className == 'Object') {
        // we're a user defined class or Object
        // JSON.stringify avoids problems with cycles, and is generally much
        // easier than looping through ownProperties of `val`.
        try {
            return 'Object(' + JSON.stringify(val) + ')';
        } catch (_) {
            return 'Object';
        }
    }
    // errors
    if (val instanceof Error) {
        return `${val.name}: ${val.message}\n${val.stack}`;
    }
    // TODO we could test for more things here, like `Set`s and `Map`s.
    return className;
}

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;


let wasm;
export function __wbg_set_wasm(val) {
    wasm = val;
}
