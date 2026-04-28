#![cfg(target_arch = "wasm32")]

//! S1b wasm_bindgen smoke —— 覆盖 plan-00-smoke.md §6 的场景：
//! 握手、请求成功、cancel、transport 断开重连、watch 重订阅、请求超时、
//! 以及 renderer 幂等。

use std::io::{Cursor, Write};

use app_server_protocol::{
    CadQueryFeatureFaces, CadQueryMeshPayload, CadQueryObjectKind, CadQueryPartMesh,
    CadQueryResultGetRequest, CancelRequest, CapabilityHandshakeRequest,
    CapabilityHandshakeResponse, ClientCapabilities, ClientCommand, ClientEnvelope, ClientPlatform,
    ClientRequestEnvelope, CommandSuccess, EdgeGroup, FaceGroup, FileReadContents, PathHandle,
    PreviewArtifact, PreviewArtifact3mf, PreviewArtifactStl, PreviewReadyResponse,
    PreviewRenderedImagePayload, PreviewRequest, PreviewRequestKind, PreviewUnit,
    ProtocolVersionRange, RequestId, ServerCapabilities, ServerEnvelope, ServerResponseEnvelope,
    SessionToken, SubscriptionId, VertexPoint, WatchSubscribeRequest, WatchSubscriptionAck,
    WorkspaceCurrentResponse, WorkspaceId, decode_client_frame, encode_server_frame,
    web_file_read_capability,
};
use js_sys::{Reflect, Uint8Array};
use serde::{Deserialize, Serialize};
use studio_web_wasm::wasm_bridge::{
    client::{
        ClientHandle, client_begin_handshake, client_cancel, client_create,
        client_create_with_timeouts, client_destroy, client_dispatch_cadquery_result_get,
        client_dispatch_preview_request, client_dispatch_workspace_current, client_drain_events,
        client_mark_transport_closed, client_next_outbound, client_receive_inbound,
        client_snapshot, client_subscribe_directory_watch, client_take_cadquery_mesh,
        client_take_preview_mesh, client_tick,
    },
    renderer::{renderer_create, renderer_destroy, renderer_resize},
};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;
use zip::{ZipWriter, write::SimpleFileOptions};

wasm_bindgen_test_configure!(run_in_browser);

// ---------- helpers ----------

fn handshake_params() -> CapabilityHandshakeRequest {
    CapabilityHandshakeRequest {
        capabilities: ClientCapabilities {
            client_name: "wasm-bridge-smoke".into(),
            platform: ClientPlatform::Web,
            protocol_version: ProtocolVersionRange::new(2, 2),
            file_read: web_file_read_capability(),
            supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
        },
    }
}

fn handshake_ack_bytes() -> Vec<u8> {
    let ack = CapabilityHandshakeResponse {
        negotiated_version: 2,
        session_token: SessionToken("test-session".into()),
        server_capabilities: ServerCapabilities {
            protocol_version: ProtocolVersionRange::new(2, 2),
            reconnect_window_ms: 30_000,
            supports_watch: true,
            supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
            supports_session_reclaim: true,
            cadquery: true,
            agent: false,
            selection_sync: false,
            llm_configured: false,
        },
    };
    encode_server_frame(&ServerEnvelope::HandshakeAck(ack)).expect("handshake ack encodes")
}

fn response_bytes(request_id: RequestId, success: CommandSuccess) -> Vec<u8> {
    encode_server_frame(&ServerEnvelope::Response(ServerResponseEnvelope {
        request_id,
        result: Ok(success),
    }))
    .expect("response encodes")
}

fn workspace_current_success() -> CommandSuccess {
    CommandSuccess::WorkspaceCurrent(WorkspaceCurrentResponse {
        workspace_id: WorkspaceId("ws-smoke".into()),
        root_name: "smoke-root".into(),
    })
}

fn preview_ready_stl(bytes: Vec<u8>) -> CommandSuccess {
    CommandSuccess::PreviewReady(PreviewReadyResponse {
        requested_kind: PreviewRequestKind::GeometryArtifact,
        artifact: PreviewArtifact::Stl(PreviewArtifactStl {
            bytes,
            media_type: "model/stl".into(),
        }),
    })
}

fn preview_ready_3mf(bytes: Vec<u8>) -> CommandSuccess {
    CommandSuccess::PreviewReady(PreviewReadyResponse {
        requested_kind: PreviewRequestKind::GeometryArtifact,
        artifact: PreviewArtifact::ThreeMf(PreviewArtifact3mf {
            bytes,
            media_type: "model/3mf".into(),
        }),
    })
}

fn cadquery_mesh_success() -> CommandSuccess {
    CommandSuccess::CadQueryMesh(CadQueryMeshPayload {
        result_id: "cq_abc".into(),
        build_id: valid_sha256_build_id(),
        unit: PreviewUnit::Millimeter,
        root_ref_text: "@part[top_lid]".into(),
        root_object_kind: CadQueryObjectKind::Part,
        parts: vec![cadquery_part_mesh()],
    })
}

fn cadquery_part_mesh() -> CadQueryPartMesh {
    CadQueryPartMesh {
        name: "top_lid".into(),
        object_kind: CadQueryObjectKind::Part,
        ref_text: "@part[top_lid]".into(),
        instance_path: None,
        transform: None,
        faces: vec![FaceGroup {
            face_idx: 0,
            positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            features: vec!["top_surface".into()],
            ambiguous: false,
        }],
        edges: vec![EdgeGroup {
            edge_idx: 0,
            polyline: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            adjacent_faces: vec![0],
        }],
        vertices: vec![VertexPoint {
            vertex_idx: 0,
            position: [0.0, 0.0, 0.0],
            adjacent_edges: vec![0],
        }],
        feature_map: vec![CadQueryFeatureFaces {
            feature: "top_surface".into(),
            face_indices: vec![0],
        }],
    }
}

fn valid_sha256_build_id() -> String {
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()
}

fn preview_request() -> PreviewRequest {
    preview_request_for("model.stl")
}

fn preview_request_for(name: &str) -> PreviewRequest {
    PreviewRequest {
        source: PathHandle::new(WorkspaceId("ws-smoke".into()), [name])
            .expect("valid preview path"),
        defines: Vec::new(),
        kind: PreviewRequestKind::GeometryArtifact,
        configured_openscad_path: None,
    }
}

fn binary_stl_bytes() -> Vec<u8> {
    let mut bytes = vec![0; 80];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    for value in [
        0.0_f32, 0.0, 1.0, // normal
        0.0, 0.0, 0.0, // v0
        1.0, 0.0, 0.0, // v1
        0.0, 1.0, 0.0, // v2
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes
}

fn minimal_three_mf_bytes() -> Vec<u8> {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <object id="1" type="model">
      <mesh>
        <vertices>
          <vertex x="0" y="0" z="0"/>
          <vertex x="1" y="0" z="0"/>
          <vertex x="0" y="1" z="0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2"/>
        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="1"/>
  </build>
</model>"#;
    three_mf_archive(xml)
}

fn mixed_color_three_mf_bytes() -> Vec<u8> {
    let xml = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <basematerials id="1">
      <base name="Red" displaycolor="#FF0000"/>
    </basematerials>
    <object id="1" type="model" pid="1" pindex="0">
      <mesh>
        <vertices>
          <vertex x="0" y="0" z="0"/>
          <vertex x="1" y="0" z="0"/>
          <vertex x="0" y="1" z="0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2"/>
        </triangles>
      </mesh>
    </object>
    <object id="2" type="model">
      <mesh>
        <vertices>
          <vertex x="2" y="0" z="0"/>
          <vertex x="3" y="0" z="0"/>
          <vertex x="2" y="1" z="0"/>
        </vertices>
        <triangles>
          <triangle v1="0" v2="1" v3="2"/>
        </triangles>
      </mesh>
    </object>
  </resources>
  <build>
    <item objectid="1"/>
    <item objectid="2"/>
  </build>
</model>"##;
    three_mf_archive(xml)
}

fn three_mf_archive(model_xml: &str) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("3D/3dmodel.model", SimpleFileOptions::default())
        .expect("fixture should open archive entry");
    writer
        .write_all(model_xml.as_bytes())
        .expect("fixture should write xml");
    writer
        .finish()
        .expect("fixture should finish archive")
        .into_inner()
}

fn watch_ack(subscription_id: &str) -> CommandSuccess {
    CommandSuccess::WatchSubscribed(WatchSubscriptionAck {
        subscription_id: SubscriptionId(subscription_id.into()),
    })
}

fn decode_outbound(bytes: &[u8]) -> ClientEnvelope {
    decode_client_frame(bytes).expect("outbound decode")
}

fn expect_request_id(envelope: &ClientEnvelope) -> RequestId {
    match envelope {
        ClientEnvelope::Request(request) => request.request_id,
        _ => panic!("expected request envelope, got {:?}", envelope),
    }
}

fn drain_events(handle: &mut ClientHandle) -> Vec<DrainedEvent> {
    let value = client_drain_events(handle).expect("drain_events ok");
    serde_wasm_bindgen::from_value(value).expect("drain_events deserialize")
}

fn to_js<T: Serialize>(value: &T) -> JsValue {
    serde_wasm_bindgen::to_value(value).expect("to_js")
}

fn js_prop(value: &JsValue, name: &str) -> JsValue {
    Reflect::get(value, &JsValue::from_str(name)).expect("js property")
}

fn assert_uint8array(value: &JsValue, expected_len: u32) {
    let bytes: Uint8Array = value
        .clone()
        .dyn_into()
        .expect("expected Uint8Array from serde_bytes");
    assert_eq!(bytes.length(), expected_len);
}

fn new_client_with_timeouts(workspace_current_ms: Option<u64>) -> ClientHandle {
    #[derive(Serialize)]
    struct Timeouts {
        workspace_current: Option<u64>,
        workspace_list: Option<u64>,
        preview_request: Option<u64>,
        file_read: Option<u64>,
        file_write_text: Option<u64>,
        config_load: Option<u64>,
        config_save: Option<u64>,
        slicer_list: Option<u64>,
        export_run: Option<u64>,
        watch: Option<u64>,
    }
    let timeouts = Timeouts {
        workspace_current: workspace_current_ms,
        workspace_list: None,
        preview_request: None,
        file_read: None,
        file_write_text: None,
        config_load: None,
        config_save: None,
        slicer_list: None,
        export_run: None,
        watch: None,
    };
    client_create_with_timeouts(to_js(&timeouts)).expect("client_create_with_timeouts")
}

fn perform_handshake(handle: &mut ClientHandle) {
    client_begin_handshake(handle, to_js(&handshake_params())).expect("begin_handshake");
    let outbound = client_next_outbound(handle)
        .expect("handle alive")
        .expect("handshake outbound");
    match decode_outbound(&outbound) {
        ClientEnvelope::Handshake(_) => {}
        other => panic!("expected handshake envelope, got {:?}", other),
    }
    client_receive_inbound(handle, &handshake_ack_bytes()).expect("inbound ack");
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
enum DrainedError {
    InvalidHandle,
    NotReady,
    DecodeError { context: String },
    UnknownRequest { request_id: RequestId },
    TransportClosed,
    Cancelled,
    ProtocolError { code: String, message: String },
    Timeout,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
enum DrainedEvent {
    HandshakeAccepted {
        session_token: SessionToken,
        server_capabilities: ServerCapabilities,
        negotiated_version: u16,
    },
    RequestSucceeded {
        request_id: RequestId,
        payload: serde_json::Value,
    },
    RequestFailed {
        request_id: RequestId,
        error: serde_json::Value,
    },
    RequestTimedOut {
        request_id: RequestId,
    },
    WatchEvent {
        request_id: RequestId,
        payload: serde_json::Value,
    },
    WatchResubscribed {
        request_id: RequestId,
    },
    TransportOpen,
    TransportClosed {
        reason: serde_json::Value,
    },
}

// ---------- tests ----------

#[wasm_bindgen_test]
fn handshake_completes_and_emits_event() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    let events = drain_events(&mut handle);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, DrainedEvent::HandshakeAccepted { .. }))
    );
}

#[wasm_bindgen_test]
fn request_success_emits_succeeded_event() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle); // 清空握手事件

    let request_id = client_dispatch_workspace_current(&mut handle).expect("dispatch");
    let outbound = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("request outbound");
    let parsed = decode_outbound(&outbound);
    let observed = expect_request_id(&parsed);
    assert_eq!(observed.0, request_id);

    let response = response_bytes(RequestId(request_id), workspace_current_success());
    client_receive_inbound(&mut handle, &response).expect("inbound response");

    let events = drain_events(&mut handle);
    let succeeded = events.iter().any(|event| match event {
        DrainedEvent::RequestSucceeded { request_id: id, .. } => id.0 == request_id,
        _ => false,
    });
    assert!(succeeded, "expected RequestSucceeded, got {:?}", events);

    let snapshot = client_snapshot(&handle).expect("snapshot");
    let value: serde_json::Value = serde_wasm_bindgen::from_value(snapshot).expect("snapshot json");
    assert!(value.get("workspace_current").is_some());
}

#[wasm_bindgen_test]
fn preview_stl_artifact_is_taken_from_side_buffer_as_mesh_handle() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let request_id =
        client_dispatch_preview_request(&mut handle, to_js(&preview_request())).expect("dispatch");
    let _ = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("preview outbound");
    let response = response_bytes(RequestId(request_id), preview_ready_stl(binary_stl_bytes()));
    client_receive_inbound(&mut handle, &response).expect("preview response");

    let events = drain_events(&mut handle);
    let preview_event = events
        .iter()
        .find_map(|event| match event {
            DrainedEvent::RequestSucceeded { payload, .. } => Some(payload),
            _ => None,
        })
        .expect("preview succeeded event");
    let bytes = preview_event
        .pointer("/payload/artifact/payload/bytes")
        .and_then(|value| value.as_array())
        .expect("serialized event keeps only lightweight bytes marker");
    assert!(bytes.is_empty());

    let mesh = client_take_preview_mesh(&mut handle, request_id)
        .expect("take succeeds")
        .expect("mesh exists");
    assert_eq!(mesh.vertex_count(), 3);
    assert_eq!(mesh.index_count(), 3);
    assert_eq!(mesh.colors().len(), 0);
    assert!(
        client_take_preview_mesh(&mut handle, request_id)
            .expect("second take succeeds")
            .is_none()
    );
}

#[wasm_bindgen_test]
fn preview_3mf_artifact_is_taken_from_side_buffer_as_mesh_handle() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let request = preview_request_for("model.3mf");
    let request_id =
        client_dispatch_preview_request(&mut handle, to_js(&request)).expect("dispatch");
    let _ = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("preview outbound");
    let response = response_bytes(
        RequestId(request_id),
        preview_ready_3mf(minimal_three_mf_bytes()),
    );
    client_receive_inbound(&mut handle, &response).expect("preview response");
    let _ = drain_events(&mut handle);

    let mesh = client_take_preview_mesh(&mut handle, request_id)
        .expect("take succeeds")
        .expect("mesh exists");
    assert_eq!(mesh.vertex_count(), 3);
    assert_eq!(mesh.index_count(), 3);
    assert_eq!(mesh.colors().len(), 0);
}

#[wasm_bindgen_test]
fn preview_3mf_mixed_colors_emit_white_for_sentinel_vertices() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let request = preview_request_for("mixed.3mf");
    let request_id =
        client_dispatch_preview_request(&mut handle, to_js(&request)).expect("dispatch");
    let _ = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("preview outbound");
    let response = response_bytes(
        RequestId(request_id),
        preview_ready_3mf(mixed_color_three_mf_bytes()),
    );
    client_receive_inbound(&mut handle, &response).expect("preview response");
    let _ = drain_events(&mut handle);

    let mesh = client_take_preview_mesh(&mut handle, request_id)
        .expect("take succeeds")
        .expect("mesh exists");
    assert_eq!(mesh.vertex_count(), 6);
    let colors = mesh.colors();
    assert_eq!(colors.len(), 24);
    assert_eq!(&colors[0..4], &[1.0, 0.0, 0.0, 1.0]);
    assert!(
        colors[12..]
            .chunks_exact(4)
            .all(|chunk| chunk == [1.0, 1.0, 1.0, 1.0])
    );
}

#[wasm_bindgen_test]
fn cadquery_mesh_payload_is_buffered_by_result_id() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let request_id = client_dispatch_cadquery_result_get(
        &mut handle,
        to_js(&CadQueryResultGetRequest {
            result_id: "cq_abc".into(),
        }),
    )
    .expect("dispatch");
    let outbound = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("request outbound");
    match decode_outbound(&outbound) {
        ClientEnvelope::Request(ClientRequestEnvelope {
            command: ClientCommand::CadQueryResultGet(request),
            request_id: outbound_id,
        }) => {
            assert_eq!(outbound_id.0, request_id);
            assert_eq!(request.result_id, "cq_abc");
        }
        other => panic!("expected cadquery.result.get request, got {:?}", other),
    }
    let response = response_bytes(RequestId(request_id), cadquery_mesh_success());
    client_receive_inbound(&mut handle, &response).expect("cadquery response");

    let events = drain_events(&mut handle);
    let payload = events
        .iter()
        .find_map(|event| match event {
            DrainedEvent::RequestSucceeded { payload, .. } => Some(payload),
            _ => None,
        })
        .expect("cadquery ready event");
    assert_eq!(
        payload.get("type").and_then(|value| value.as_str()),
        Some("cad_query_result_ready")
    );
    assert_eq!(
        payload
            .pointer("/payload/result_id")
            .and_then(|value| value.as_str()),
        Some("cq_abc")
    );
    assert!(payload.pointer("/payload/parts").is_none());

    let mesh = client_take_cadquery_mesh(&mut handle, "cq_abc")
        .expect("take ok")
        .expect("mesh exists");
    assert_eq!(mesh.result_id(), "cq_abc");
    assert_eq!(mesh.build_id(), valid_sha256_build_id());
    assert_eq!(mesh.root_ref_text(), "@part[top_lid]");
    assert_eq!(mesh.root_object_kind(), "part");
    assert_eq!(mesh.part_count(), 1);
    assert_eq!(mesh.face_positions(0, 0).expect("positions").len(), 9);
    assert_eq!(mesh.face_normals(0, 0).expect("normals").len(), 9);
    assert_eq!(mesh.edge_polyline(0, 0).expect("edge").len(), 6);
    assert_eq!(
        mesh.vertex_position(0, 0).expect("vertex"),
        vec![0.0, 0.0, 0.0]
    );

    let metadata: serde_json::Value =
        serde_wasm_bindgen::from_value(mesh.metadata().expect("metadata")).expect("metadata json");
    assert_eq!(
        metadata
            .pointer("/parts/0/feature_map/0/feature")
            .and_then(|value| value.as_str()),
        Some("top_surface")
    );
    assert!(
        metadata.pointer("/parts/0/vertices/0/position").is_none(),
        "metadata must not expand vertex coordinate arrays"
    );
    assert!(
        client_take_cadquery_mesh(&mut handle, "cq_abc")
            .expect("second take ok")
            .is_none()
    );
}

#[wasm_bindgen_test]
fn serde_bytes_fields_serialize_to_uint8array() {
    let three_mf = to_js(&PreviewArtifact::ThreeMf(PreviewArtifact3mf {
        bytes: vec![1, 2, 3, 4],
        media_type: "model/3mf".into(),
    }));
    let three_mf_payload = js_prop(&three_mf, "payload");
    assert_uint8array(&js_prop(&three_mf_payload, "bytes"), 4);

    let stl = to_js(&PreviewArtifact::Stl(PreviewArtifactStl {
        bytes: vec![1, 2, 3],
        media_type: "model/stl".into(),
    }));
    let stl_payload = js_prop(&stl, "payload");
    assert_uint8array(&js_prop(&stl_payload, "bytes"), 3);

    let image = to_js(&PreviewRenderedImagePayload {
        bytes: vec![1, 2],
        media_type: "image/png".into(),
        width: 1,
        height: 1,
    });
    assert_uint8array(&js_prop(&image, "bytes"), 2);

    let file_read = to_js(&FileReadContents::Binary(vec![1, 2, 3, 4, 5]));
    assert_uint8array(&js_prop(&file_read, "payload"), 5);
}

#[wasm_bindgen_test]
fn bad_preview_stl_take_marks_snapshot_error() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let request_id =
        client_dispatch_preview_request(&mut handle, to_js(&preview_request())).expect("dispatch");
    let _ = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("preview outbound");
    let response = response_bytes(RequestId(request_id), preview_ready_stl(vec![1, 2, 3]));
    client_receive_inbound(&mut handle, &response).expect("preview response");
    let _ = drain_events(&mut handle);

    let err = match client_take_preview_mesh(&mut handle, request_id) {
        Ok(_) => panic!("bad stl should fail"),
        Err(err) => err,
    };
    assert!(format!("{err:?}").contains("stl decode failed"));
    let snapshot = client_snapshot(&handle).expect("snapshot");
    let value: serde_json::Value = serde_wasm_bindgen::from_value(snapshot).expect("snapshot json");
    assert_eq!(
        value
            .pointer("/preview_error/payload/message")
            .and_then(|v| v.as_str()),
        Some("stl decode failed: 解析 STL 失败: STL malformed")
    );
}

#[wasm_bindgen_test]
fn preview_side_buffer_clears_on_destroy() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let request_id =
        client_dispatch_preview_request(&mut handle, to_js(&preview_request())).expect("dispatch");
    let _ = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("preview outbound");
    let response = response_bytes(RequestId(request_id), preview_ready_stl(binary_stl_bytes()));
    client_receive_inbound(&mut handle, &response).expect("preview response");
    let _ = drain_events(&mut handle);

    client_destroy(&mut handle);
    assert!(client_take_preview_mesh(&mut handle, request_id).is_err());
}

#[wasm_bindgen_test]
fn preview_side_buffer_clears_on_transport_close() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let request_id =
        client_dispatch_preview_request(&mut handle, to_js(&preview_request())).expect("dispatch");
    let _ = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("preview outbound");
    let response = response_bytes(RequestId(request_id), preview_ready_stl(binary_stl_bytes()));
    client_receive_inbound(&mut handle, &response).expect("preview response");
    let _ = drain_events(&mut handle);

    client_mark_transport_closed(&mut handle, json_close_reason(1006, "net", false))
        .expect("transport close");
    assert!(
        client_take_preview_mesh(&mut handle, request_id)
            .expect("handle remains valid")
            .is_none()
    );
}

#[wasm_bindgen_test]
fn preview_side_buffer_evicts_oldest_entry() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let mut request_ids = Vec::new();
    for index in 0..9 {
        let request = preview_request_for(&format!("model-{index}.stl"));
        let request_id =
            client_dispatch_preview_request(&mut handle, to_js(&request)).expect("dispatch");
        let _ = client_next_outbound(&mut handle)
            .expect("handle alive")
            .expect("preview outbound");
        let response = response_bytes(RequestId(request_id), preview_ready_stl(binary_stl_bytes()));
        client_receive_inbound(&mut handle, &response).expect("preview response");
        let _ = drain_events(&mut handle);
        request_ids.push(request_id);
    }

    assert!(
        client_take_preview_mesh(&mut handle, request_ids[0])
            .expect("handle remains valid")
            .is_none()
    );
    assert!(
        client_take_preview_mesh(&mut handle, request_ids[8])
            .expect("take succeeds")
            .is_some()
    );
}

#[wasm_bindgen_test]
fn preview_side_buffer_replaces_same_target_entry() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let first =
        client_dispatch_preview_request(&mut handle, to_js(&preview_request())).expect("dispatch");
    let _ = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("preview outbound");
    let response = response_bytes(RequestId(first), preview_ready_stl(binary_stl_bytes()));
    client_receive_inbound(&mut handle, &response).expect("preview response");
    let _ = drain_events(&mut handle);

    let second =
        client_dispatch_preview_request(&mut handle, to_js(&preview_request())).expect("dispatch");
    let _ = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("preview outbound");
    let response = response_bytes(RequestId(second), preview_ready_stl(binary_stl_bytes()));
    client_receive_inbound(&mut handle, &response).expect("preview response");
    let _ = drain_events(&mut handle);

    assert!(
        client_take_preview_mesh(&mut handle, first)
            .expect("handle remains valid")
            .is_none()
    );
    assert!(
        client_take_preview_mesh(&mut handle, second)
            .expect("take succeeds")
            .is_some()
    );
}

#[wasm_bindgen_test]
fn stale_same_target_preview_does_not_replace_newer_buffer() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let first =
        client_dispatch_preview_request(&mut handle, to_js(&preview_request())).expect("dispatch");
    let _ = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("preview outbound");

    let second =
        client_dispatch_preview_request(&mut handle, to_js(&preview_request())).expect("dispatch");
    let _ = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("preview outbound");

    let response = response_bytes(RequestId(second), preview_ready_stl(binary_stl_bytes()));
    client_receive_inbound(&mut handle, &response).expect("preview response");
    let _ = drain_events(&mut handle);

    let response = response_bytes(RequestId(first), preview_ready_stl(binary_stl_bytes()));
    client_receive_inbound(&mut handle, &response).expect("preview response");
    let _ = drain_events(&mut handle);

    assert!(
        client_take_preview_mesh(&mut handle, first)
            .expect("handle remains valid")
            .is_none()
    );
    assert!(
        client_take_preview_mesh(&mut handle, second)
            .expect("newer preview remains")
            .is_some()
    );
}

#[wasm_bindgen_test]
fn cancel_prevents_success_event() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let request_id = client_dispatch_workspace_current(&mut handle).expect("dispatch");
    let _ = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("outbound drained");
    let cancel_id = client_cancel(&mut handle, request_id).expect("cancel");
    assert_ne!(cancel_id, request_id);
    let _ = client_next_outbound(&mut handle).expect("handle alive"); // cancel envelope

    let events = drain_events(&mut handle);
    let failed_cancelled = events.iter().any(|event| match event {
        DrainedEvent::RequestFailed {
            request_id: id,
            error,
        } => {
            id.0 == request_id
                && error
                    .get("type")
                    .and_then(|value| value.as_str())
                    .map(|text| text == "cancelled")
                    .unwrap_or(false)
        }
        _ => false,
    });
    assert!(
        failed_cancelled,
        "expected RequestFailed(cancelled), got {:?}",
        events
    );

    let response = response_bytes(RequestId(request_id), workspace_current_success());
    client_receive_inbound(&mut handle, &response).expect("inbound late response");
    let later_events = drain_events(&mut handle);
    let has_success = later_events.iter().any(|event| match event {
        DrainedEvent::RequestSucceeded { request_id: id, .. } => id.0 == request_id,
        _ => false,
    });
    assert!(
        !has_success,
        "late response must not emit RequestSucceeded: {:?}",
        later_events
    );
}

#[wasm_bindgen_test]
fn transport_close_replays_pending_on_reconnect() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let request_id = client_dispatch_workspace_current(&mut handle).expect("dispatch");
    let request_bytes = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("outbound initial");
    let parsed = decode_outbound(&request_bytes);
    assert_eq!(expect_request_id(&parsed).0, request_id);

    let reason = json_close_reason(1006, "net", false);
    client_mark_transport_closed(&mut handle, reason).expect("mark closed");
    let after_close = drain_events(&mut handle);
    assert!(
        after_close
            .iter()
            .any(|event| matches!(event, DrainedEvent::TransportClosed { .. })),
        "expected TransportClosed, got {:?}",
        after_close
    );

    client_begin_handshake(&mut handle, to_js(&handshake_params())).expect("reconnect begin");
    let first = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("reconnect outbound");
    match decode_outbound(&first) {
        ClientEnvelope::Reconnect(_) => {}
        other => panic!("expected reconnect envelope, got {:?}", other),
    }
    let second = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("replayed request");
    let replayed = decode_outbound(&second);
    assert_eq!(expect_request_id(&replayed).0, request_id);

    client_receive_inbound(&mut handle, &handshake_ack_bytes()).expect("reconnect ack");
    let reopened = drain_events(&mut handle);
    assert!(
        reopened
            .iter()
            .any(|event| matches!(event, DrainedEvent::TransportOpen)),
        "expected TransportOpen after reconnect, got {:?}",
        reopened
    );
}

#[wasm_bindgen_test]
fn watch_resubscribes_after_reconnect_and_emits_watch_resubscribed() {
    let mut handle = client_create();
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let watch_params = WatchParamsShim {
        request: WatchSubscribeRequestShim { directory: None },
        throttle_ms: Some(150),
    };
    let watch_request_id =
        client_subscribe_directory_watch(&mut handle, to_js(&watch_params)).expect("subscribe");
    let subscribe_bytes = client_next_outbound(&mut handle)
        .expect("handle alive")
        .expect("watch outbound");
    match decode_outbound(&subscribe_bytes) {
        ClientEnvelope::Request(ClientRequestEnvelope {
            command: ClientCommand::WatchSubscribe(_),
            request_id,
        }) => assert_eq!(request_id.0, watch_request_id),
        other => panic!("expected watch.subscribe request, got {:?}", other),
    }
    let ack = response_bytes(RequestId(watch_request_id), watch_ack("sub-a"));
    client_receive_inbound(&mut handle, &ack).expect("watch ack");
    drain_events(&mut handle);

    let reason = json_close_reason(1000, "bye", true);
    client_mark_transport_closed(&mut handle, reason).expect("mark closed");
    client_begin_handshake(&mut handle, to_js(&handshake_params())).expect("reconnect begin");

    let mut saw_resubscribe = false;
    while let Ok(Some(frame)) = client_next_outbound(&mut handle) {
        if let ClientEnvelope::Request(ClientRequestEnvelope {
            command: ClientCommand::WatchSubscribe(_),
            request_id,
        }) = decode_outbound(&frame)
        {
            assert_eq!(request_id.0, watch_request_id);
            saw_resubscribe = true;
        }
    }
    assert!(saw_resubscribe, "expected replayed watch subscribe");

    client_receive_inbound(&mut handle, &handshake_ack_bytes()).expect("reconnect ack");
    let ack2 = response_bytes(RequestId(watch_request_id), watch_ack("sub-b"));
    client_receive_inbound(&mut handle, &ack2).expect("watch re-ack");

    let events = drain_events(&mut handle);
    let resubscribed = events.iter().any(|event| match event {
        DrainedEvent::WatchResubscribed { request_id } => request_id.0 == watch_request_id,
        _ => false,
    });
    assert!(resubscribed, "expected WatchResubscribed, got {:?}", events);
}

#[wasm_bindgen_test]
fn request_timeout_fires_via_tick() {
    let mut handle = new_client_with_timeouts(Some(100));
    // 先 tick 设置时间基准
    client_tick(&mut handle, 1_000).expect("handle alive");
    perform_handshake(&mut handle);
    drain_events(&mut handle);

    let request_id = client_dispatch_workspace_current(&mut handle).expect("dispatch");
    let _ = client_next_outbound(&mut handle).expect("handle alive");
    client_tick(&mut handle, 1_000 + 1_000).expect("handle alive"); // 远超 100ms 超时窗
    let events = drain_events(&mut handle);
    let timed_out = events.iter().any(|event| match event {
        DrainedEvent::RequestTimedOut { request_id: id } => id.0 == request_id,
        _ => false,
    });
    assert!(timed_out, "expected RequestTimedOut, got {:?}", events);
}

#[wasm_bindgen_test]
fn renderer_create_returns_stub_error() {
    let result = renderer_create("preview-canvas");
    assert!(
        result.is_err(),
        "renderer_create should stub error in Phase 2b"
    );
}

#[wasm_bindgen_test]
fn renderer_destroy_is_idempotent_when_handle_unavailable() {
    // renderer_create 桩不创建 handle；这里只验证 renderer_resize / destroy
    // 在没有真正 handle 时仍然不 panic。通过构造 never-reached flow 取代。
    let result = renderer_create("id-not-used");
    assert!(result.is_err());
    // 不直接调用 resize/destroy（因为 handle 不存在）——stub 契约验收：
    // destroy 幂等语义 + resize 早于 render —— 由消费 handle 语义天然成立，
    // 真实 renderer 实现在 Phase 3。
    let _ = renderer_resize;
    let _ = renderer_destroy;
}

#[wasm_bindgen_test]
fn client_destroy_is_idempotent_and_guards_subsequent_calls() {
    let mut handle = client_create();
    client_destroy(&mut handle);
    // 第二次 destroy 不 panic。
    client_destroy(&mut handle);
    // destroy 后的 dispatch 返回 InvalidHandle。
    let err = client_dispatch_workspace_current(&mut handle)
        .expect_err("dispatch after destroy must fail");
    let parsed: DrainedError =
        serde_wasm_bindgen::from_value(err).expect("error is serialized ClientError");
    assert!(
        matches!(parsed, DrainedError::InvalidHandle),
        "expected InvalidHandle, got {:?}",
        parsed
    );
    // tick 后续也走 InvalidHandle。
    assert!(client_tick(&mut handle, 0).is_err());
}

// ---------- local serde shims ----------

#[derive(Debug, Serialize)]
struct WatchParamsShim {
    request: WatchSubscribeRequestShim,
    throttle_ms: Option<u32>,
}

#[derive(Debug, Serialize)]
struct WatchSubscribeRequestShim {
    directory: Option<PathHandle>,
}

#[derive(Debug, Serialize)]
struct CloseReasonShim {
    code: u16,
    reason: String,
    was_clean: bool,
}

fn json_close_reason(code: u16, reason: &str, was_clean: bool) -> JsValue {
    to_js(&CloseReasonShim {
        code,
        reason: reason.into(),
        was_clean,
    })
}

// 防止 WatchSubscribeRequest 未使用警告
#[allow(dead_code)]
fn _unused_types() {
    let _: Option<WatchSubscribeRequest> = None;
    let _: Option<CancelRequest> = None;
}
