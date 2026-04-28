use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use app_server_core::{
    CadQueryRunConfig, CadQueryRunnerErrorKind, cadquery_result_ready, parse_cadquery_success_json,
    run_cadquery_runner, run_cadquery_runner_with_cancel,
};
use app_server_protocol::{
    CadQueryExportFormat, CadQueryObjectKind, PreviewUnit, ProtocolErrorCode,
};

fn success_json() -> String {
    r#"{
      "status":"success",
      "result_id":"cq_abc",
      "build_id":"sha256:7d7152e43de9e062366d794b6319a4d3a90e6972ad00f940179245833d410403",
      "unit":"millimeter",
      "root_ref_text":"@part[top_lid]",
      "root_object_kind":"part",
      "parts":[{
        "name":"top_lid",
        "object_kind":"part",
        "ref_text":"@part[top_lid]",
        "instance_path":null,
        "transform":null,
        "mesh":{
          "faces":[{
            "face_idx":0,
            "positions":[0,0,0,1,0,0,0,1,0],
            "normals":[0,0,1,0,0,1,0,0,1],
            "features":["top_surface"],
            "ambiguous":false
          }],
          "edges":[{"edge_idx":0,"polyline":[0,0,0,1,0,0],"adjacent_faces":[0]}],
          "vertices":[{"vertex_idx":0,"position":[0,0,0],"adjacent_edges":[0]}]
        },
        "feature_map":{"top_surface":{"face_indices":[0],"selector":"faces(\">Z\")"}}
      }],
      "exports":{"step":"outputs/top_lid.step"},
      "metadata":{"bounding_box":{"min":[0,0,0],"max":[1,1,1]}},
      "manifest":{
        "source_path":"parts/top_lid.py",
        "source_hash":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "params":{},
        "params_hash":"sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
        "dependencies":[{"path":"parts/top_lid.py","hash":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}],
        "deps_hash":"sha256:486f81788f9250ca562a11da138c690884aebda032157fe8fa66e2ad952ebfdc",
        "export_hashes":{"step":"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}
      }
    }"#
    .into()
}

#[test]
fn parses_cadquery_runner_json_into_protocol_payload() {
    let payload = parse_cadquery_success_json(&success_json()).expect("valid payload");

    assert_eq!(payload.result_id, "cq_abc");
    assert_eq!(payload.unit, PreviewUnit::Millimeter);
    assert_eq!(payload.root_ref_text, "@part[top_lid]");
    assert_eq!(payload.root_object_kind, CadQueryObjectKind::Part);
    assert_eq!(payload.parts[0].ref_text, "@part[top_lid]");
    assert_eq!(payload.parts[0].faces[0].positions.len(), 9);
    assert_eq!(payload.parts[0].feature_map[0].feature, "top_surface");

    let ready = cadquery_result_ready(&payload);
    assert_eq!(ready.part_count, 1);
    assert_eq!(ready.face_count, 1);
    assert_eq!(ready.edge_count, 1);
    assert_eq!(ready.vertex_count, 1);
}

#[test]
fn rejects_non_finite_cadquery_mesh_numbers() {
    let json = success_json().replace("[0,0,0,1,0,0,0,1,0]", "[0,0,0,1,0,0,0,1,null]");
    let err = parse_cadquery_success_json(&json).expect_err("null should fail");
    assert_eq!(err.code, ProtocolErrorCode::InvalidWireFrame);

    let json = success_json().replace("[0,0,0,1,0,0,0,1,0]", "[0,0,0,1,0,0,0,1,1e999]");
    let err = parse_cadquery_success_json(&json).expect_err("out-of-range number should fail");
    assert_eq!(err.code, ProtocolErrorCode::InvalidWireFrame);
}

#[test]
fn rejects_cadquery_arrays_with_invalid_lengths_or_indices() {
    let json = success_json().replace("[0,0,0,1,0,0,0,1,0]", "[0,0,0,1]");
    let err = parse_cadquery_success_json(&json).expect_err("positions length should fail");
    assert_eq!(err.code, ProtocolErrorCode::InvalidWireFrame);

    let json = success_json().replace("\"face_indices\":[0]", "\"face_indices\":[9]");
    let err = parse_cadquery_success_json(&json).expect_err("face index should fail");
    assert_eq!(err.code, ProtocolErrorCode::InvalidWireFrame);

    let json = success_json().replace("\"face_idx\":0", "\"face_idx\":9");
    let err = parse_cadquery_success_json(&json).expect_err("face_idx should fail");
    assert_eq!(err.code, ProtocolErrorCode::InvalidWireFrame);

    let json = success_json().replace("\"edge_idx\":0", "\"edge_idx\":9");
    let err = parse_cadquery_success_json(&json).expect_err("edge_idx should fail");
    assert_eq!(err.code, ProtocolErrorCode::InvalidWireFrame);

    let json = success_json().replace("\"vertex_idx\":0", "\"vertex_idx\":9");
    let err = parse_cadquery_success_json(&json).expect_err("vertex_idx should fail");
    assert_eq!(err.code, ProtocolErrorCode::InvalidWireFrame);
}

#[test]
fn cadquery_runner_invokes_subprocess_and_parses_mesh_payload() {
    let root = temp_dir("cadquery-runner");
    fs::create_dir_all(&root).expect("temp root");
    let runner = fake_runner(&root, &success_json());

    let result = run_cadquery_runner(&CadQueryRunConfig {
        python: runner,
        project_root: root.clone(),
        script: "parts/top_lid.py".into(),
        output_dir: root.join("outputs"),
        export_formats: vec![CadQueryExportFormat::Step, CadQueryExportFormat::Stl],
        params_json: "{}".into(),
        timeout: Duration::from_secs(5),
    })
    .expect("runner should parse");

    assert_eq!(result.ready.result_id, "cq_abc");
    assert_eq!(result.mesh.parts[0].ref_text, "@part[top_lid]");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cadquery_runner_maps_python_import_failure_to_error_kind() {
    let root = temp_dir("cadquery-python-import");
    fs::create_dir_all(&root).expect("temp root");
    let runner = fake_error_runner(
        &root,
        2,
        r#"{"status":"runner_error","error_type":"ModuleNotFoundError","error":"No module named 'cadquery'"}"#,
    );

    let error = run_cadquery_runner(&CadQueryRunConfig {
        python: runner,
        project_root: root.clone(),
        script: "parts/top_lid.py".into(),
        output_dir: root.join("outputs"),
        export_formats: Vec::new(),
        params_json: "{}".into(),
        timeout: Duration::from_secs(5),
    })
    .expect_err("python import failure should be classified");

    assert_eq!(error.kind, CadQueryRunnerErrorKind::PythonImport);
    assert!(error.message.contains("ModuleNotFoundError"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cadquery_runner_cancels_subprocess_before_timeout() {
    let root = temp_dir("cadquery-cancel");
    fs::create_dir_all(&root).expect("temp root");
    let runner = sleeping_runner(&root);
    let cancelled = Arc::new(AtomicBool::new(true));
    let error = run_cadquery_runner_with_cancel(
        &CadQueryRunConfig {
            python: runner,
            project_root: root.clone(),
            script: "parts/top_lid.py".into(),
            output_dir: root.join("outputs"),
            export_formats: Vec::new(),
            params_json: "{}".into(),
            timeout: Duration::from_secs(5),
        },
        &|| cancelled.load(Ordering::SeqCst),
    )
    .expect_err("cancelled runner should fail");

    assert_eq!(error.kind, CadQueryRunnerErrorKind::Cancelled);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_cadquery_manifest_build_id_mismatch() {
    let json = success_json().replace(
        "sha256:7d7152e43de9e062366d794b6319a4d3a90e6972ad00f940179245833d410403",
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    );
    let err = parse_cadquery_success_json(&json).expect_err("build_id mismatch should fail");
    assert_eq!(err.code, ProtocolErrorCode::InvalidWireFrame);
}

#[test]
fn rejects_cadquery_manifest_paths_that_are_not_project_relative() {
    let json = success_json().replace(
        "\"source_path\":\"parts/top_lid.py\"",
        "\"source_path\":\"../parts/top_lid.py\"",
    );
    let err = parse_cadquery_success_json(&json).expect_err("escaping source_path should fail");
    assert_eq!(err.code, ProtocolErrorCode::InvalidWireFrame);
    assert!(err.message.contains("路径"));
}

#[test]
fn rejects_cadquery_export_paths_and_hashes_that_are_not_valid() {
    let json = success_json().replace(
        "\"exports\":{\"step\":\"outputs/top_lid.step\"}",
        "\"exports\":{\"step\":\"../top_lid.step\"}",
    );
    let err = parse_cadquery_success_json(&json).expect_err("escaping export path should fail");
    assert_eq!(err.code, ProtocolErrorCode::InvalidWireFrame);

    let json = success_json().replace(
        "\"export_hashes\":{\"step\":\"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"}",
        "\"export_hashes\":{\"step\":\"sha256:short\"}",
    );
    let err = parse_cadquery_success_json(&json).expect_err("invalid export hash should fail");
    assert_eq!(err.code, ProtocolErrorCode::InvalidWireFrame);
}

#[test]
fn rejects_cadquery_export_hashes_that_do_not_match_exports() {
    let json = success_json().replace(
        "\"export_hashes\":{\"step\":\"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"}",
        "\"export_hashes\":{}",
    );
    let err = parse_cadquery_success_json(&json).expect_err("missing export hash should fail");
    assert_eq!(err.code, ProtocolErrorCode::InvalidWireFrame);
}

fn fake_runner(root: &Path, stdout_json: &str) -> std::path::PathBuf {
    let runner = root.join("fake-runner.sh");
    fs::write(
        &runner,
        format!(
            "#!/bin/sh\nout=''\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = '--output-dir' ]; then\n    shift\n    out=\"$1\"\n  fi\n  shift\ndone\nif [ -n \"$out\" ]; then\n  mkdir -p \"$out\"\n  printf 'artifact\\n' > \"$out/top_lid.step\"\nfi\ncat <<'JSON'\n{stdout_json}\nJSON\n"
        ),
    )
    .expect("write fake runner");
    make_executable(&runner);
    runner
}

fn fake_error_runner(root: &Path, exit_code: i32, stdout_json: &str) -> std::path::PathBuf {
    let runner = root.join("fake-error-runner.sh");
    fs::write(
        &runner,
        format!("#!/bin/sh\ncat <<'JSON'\n{stdout_json}\nJSON\nexit {exit_code}\n"),
    )
    .expect("write fake error runner");
    make_executable(&runner);
    runner
}

fn sleeping_runner(root: &Path) -> std::path::PathBuf {
    let runner = root.join("sleep-runner.sh");
    fs::write(&runner, "#!/bin/sh\nsleep 5\n").expect("write sleep runner");
    make_executable(&runner);
    runner
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("{label}-{}", unique_suffix()))
}

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

fn unique_suffix() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let seq = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
    format!("{}-{nanos}-{seq}", std::process::id())
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).expect("runner metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("runner permissions");
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
