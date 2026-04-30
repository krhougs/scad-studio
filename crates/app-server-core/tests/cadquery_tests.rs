use std::{
    fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use app_server_core::{
    CadQueryContractConfig, CadQueryRunConfig, CadQueryRunnerErrorKind, cadquery_result_ready,
    parse_cadquery_success_json, run_cadquery_contract, run_cadquery_runner,
    run_cadquery_runner_with_cancel,
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
            "features":["lid_alignment_surface"],
            "ambiguous":false
          }],
          "edges":[{"edge_idx":0,"polyline":[0,0,0,1,0,0],"adjacent_faces":[0]}],
          "vertices":[{"vertex_idx":0,"position":[0,0,0],"adjacent_edges":[0]}]
        },
        "feature_map":{"lid_alignment_surface":{"face_indices":[0],"selector":"faces(\">Z\")"}}
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
    assert_eq!(
        payload.parts[0].feature_map[0].feature,
        "lid_alignment_surface"
    );
    let relation = payload
        .artifact_relation
        .as_ref()
        .expect("artifact relation");
    assert_eq!(relation.source_path, "parts/top_lid.py");
    assert_eq!(relation.exports[0].name, "step");
    assert_eq!(relation.exports[0].path, "outputs/top_lid.step");
    assert_eq!(
        relation.exports[0].hash,
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );

    let ready = cadquery_result_ready(&payload);
    assert_eq!(ready.part_count, 1);
    assert_eq!(ready.face_count, 1);
    assert_eq!(ready.edge_count, 1);
    assert_eq!(ready.vertex_count, 1);
    assert_eq!(ready.artifact_relation, payload.artifact_relation);
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

#[tokio::test]
async fn cadquery_runner_invokes_subprocess_and_parses_mesh_payload() {
    let root = temp_dir("cadquery-runner");
    fs::create_dir_all(&root).expect("temp root");
    let runner = fake_runner(&root, &success_json());

    let result = run_cadquery_runner(CadQueryRunConfig {
        python: runner,
        project_root: root.clone(),
        script: "parts/top_lid.py".into(),
        output_dir: root.join("outputs"),
        export_formats: vec![CadQueryExportFormat::Step, CadQueryExportFormat::Stl],
        params_json: "{}".into(),
        timeout: Duration::from_secs(5),
    })
    .await
    .expect("runner should parse");

    assert_eq!(result.ready.result_id, "cq_abc");
    assert_eq!(result.mesh.parts[0].ref_text, "@part[top_lid]");
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn cadquery_runner_drains_large_stdout_before_process_exit() {
    let root = temp_dir("cadquery-runner-large-stdout");
    fs::create_dir_all(&root).expect("temp root");
    let runner = fake_runner(&root, &large_success_json());

    let result = run_cadquery_runner(CadQueryRunConfig {
        python: runner,
        project_root: root.clone(),
        script: "parts/top_lid.py".into(),
        output_dir: root.join("outputs"),
        export_formats: Vec::new(),
        params_json: "{}".into(),
        timeout: Duration::from_secs(2),
    })
    .await
    .expect("runner should drain stdout while child is running");

    assert!(result.mesh.parts[0].faces[0].positions.len() > 64 * 1024);
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn cadquery_runner_maps_python_import_failure_to_error_kind() {
    let root = temp_dir("cadquery-python-import");
    fs::create_dir_all(&root).expect("temp root");
    let runner = fake_error_runner(
        &root,
        2,
        r#"{"status":"runner_error","error_type":"ModuleNotFoundError","error":"No module named 'cadquery'"}"#,
    );

    let error = run_cadquery_runner(CadQueryRunConfig {
        python: runner,
        project_root: root.clone(),
        script: "parts/top_lid.py".into(),
        output_dir: root.join("outputs"),
        export_formats: Vec::new(),
        params_json: "{}".into(),
        timeout: Duration::from_secs(5),
    })
    .await
    .expect_err("python import failure should be classified");

    assert_eq!(error.kind, CadQueryRunnerErrorKind::PythonImport);
    assert!(error.message.contains("ModuleNotFoundError"));
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn cadquery_runner_cancels_subprocess_before_timeout() {
    let root = temp_dir("cadquery-cancel");
    fs::create_dir_all(&root).expect("temp root");
    let marker = root.join("runner-finished");
    let runner = sleeping_runner_with_marker(&root, &marker);
    let cancelled = Arc::new(AtomicBool::new(true));
    let started = Instant::now();
    let error = run_cadquery_runner_with_cancel(
        CadQueryRunConfig {
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
    .await
    .expect_err("cancelled runner should fail");

    assert_eq!(error.kind, CadQueryRunnerErrorKind::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(2));
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!marker.exists());
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn cadquery_runner_kills_subprocess_on_timeout() {
    let root = temp_dir("cadquery-timeout");
    fs::create_dir_all(&root).expect("temp root");
    let marker = root.join("runner-finished");
    let runner = sleeping_runner_with_marker(&root, &marker);

    let error = run_cadquery_runner(CadQueryRunConfig {
        python: runner,
        project_root: root.clone(),
        script: "parts/top_lid.py".into(),
        output_dir: root.join("outputs"),
        export_formats: Vec::new(),
        params_json: "{}".into(),
        timeout: Duration::from_millis(50),
    })
    .await
    .expect_err("timeout should fail");

    assert_eq!(error.kind, CadQueryRunnerErrorKind::Timeout);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(!marker.exists());
    let _ = fs::remove_dir_all(root);
}

#[tokio::test]
async fn cadquery_contract_removes_temp_file_on_timeout() {
    let root = temp_dir("cadquery-contract-timeout");
    fs::create_dir_all(&root).expect("temp root");
    let runner = sleeping_runner(&root);
    let marker = format!("contract-marker-{}", unique_suffix());

    let error = run_cadquery_contract(CadQueryContractConfig {
        python: runner,
        code: format!("# {marker}\n"),
        timeout: Duration::from_millis(50),
    })
    .await
    .expect_err("contract timeout should fail");

    assert_eq!(error.kind, CadQueryRunnerErrorKind::Timeout);
    assert!(contract_temp_files_containing(&marker).is_empty());
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

fn large_success_json() -> String {
    let coordinates = std::iter::repeat("0")
        .take(90_000)
        .collect::<Vec<_>>()
        .join(",");
    success_json()
        .replace("[0,0,0,1,0,0,0,1,0]", &format!("[{coordinates}]"))
        .replace("[0,0,1,0,0,1,0,0,1]", &format!("[{coordinates}]"))
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

fn sleeping_runner_with_marker(root: &Path, marker: &Path) -> std::path::PathBuf {
    let runner = root.join("sleep-runner-marker.sh");
    fs::write(
        &runner,
        format!(
            "#!/bin/sh\nsleep 0.3\nprintf done > '{}'\n",
            marker.to_string_lossy().replace('\'', "'\\''")
        ),
    )
    .expect("write sleep runner");
    make_executable(&runner);
    runner
}

fn contract_temp_files_containing(marker: &str) -> Vec<std::path::PathBuf> {
    let prefix = format!("budn-cq-contract-{}-", std::process::id());
    fs::read_dir(std::env::temp_dir())
        .expect("read temp dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&prefix))
        })
        .filter(|path| {
            fs::read_to_string(path)
                .map(|contents| contents.contains(marker))
                .unwrap_or(false)
        })
        .collect()
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
