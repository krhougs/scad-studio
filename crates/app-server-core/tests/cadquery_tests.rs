use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

use app_server_core::{
    CadQueryExecuteConfig, CadQueryRunConfig, CadQueryRunnerErrorKind, cadquery_result_ready,
    execute_cadquery_with_staging, parse_cadquery_success_json, run_cadquery_runner,
    stage_cadquery_project,
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
fn cadquery_staging_commits_target_only_after_successful_runner() {
    let root = workspace_with_part("old = True\n");
    let runner = fake_runner(&root, &success_json());

    let result = execute_cadquery_with_staging(&CadQueryExecuteConfig {
        python: runner,
        workspace_root: root.clone(),
        target_relative_path: Path::new("parts/top_lid.py").into(),
        code: "new = True\n".into(),
        export_formats: Vec::new(),
        params_json: "{}".into(),
        timeout: Duration::from_secs(5),
    })
    .expect("staged execute");

    assert_eq!(
        result.ready.build_id,
        "sha256:7d7152e43de9e062366d794b6319a4d3a90e6972ad00f940179245833d410403"
    );
    assert_eq!(
        fs::read_to_string(root.join("parts/top_lid.py")).unwrap(),
        "new = True\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("outputs/top_lid.step")).unwrap(),
        "artifact\n"
    );
    assert!(!root.join(".budn_staging").exists());
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

#[test]
fn cadquery_staging_rejects_commit_when_original_file_changed() {
    let root = workspace_with_part("old = True\n");
    let staged = stage_cadquery_project(&root, Path::new("parts/top_lid.py"), "new = True\n")
        .expect("stage project");
    fs::write(root.join("parts/top_lid.py"), "external = True\n").unwrap();

    let error = staged
        .commit_target()
        .expect_err("commit should detect conflict");

    assert_eq!(error.kind, CadQueryRunnerErrorKind::FileConflict);
    assert_eq!(
        fs::read_to_string(root.join("parts/top_lid.py")).unwrap(),
        "external = True\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cadquery_staging_baseline_is_captured_before_workspace_copy() {
    let root = workspace_with_part("old = True\n");
    let bulk_dir = root.join("bulk");
    fs::create_dir_all(&bulk_dir).unwrap();
    let bytes = vec![b'x'; 128 * 1024];
    for index in 0..120 {
        fs::write(bulk_dir.join(format!("file_{index}.bin")), &bytes).unwrap();
    }

    let root_for_thread = root.clone();
    let handle = thread::spawn(move || {
        while !root_for_thread.join(".budn_staging").exists() {
            thread::yield_now();
        }
        fs::write(
            root_for_thread.join("parts/top_lid.py"),
            "external = True\n",
        )
        .unwrap();
    });

    let staged = stage_cadquery_project(&root, Path::new("parts/top_lid.py"), "new = True\n")
        .expect("stage project");
    handle.join().expect("external edit thread");

    let error = staged
        .commit_target()
        .expect_err("commit should detect edit during staging");

    assert_eq!(error.kind, CadQueryRunnerErrorKind::FileConflict);
    assert_eq!(
        fs::read_to_string(root.join("parts/top_lid.py")).unwrap(),
        "external = True\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cadquery_staging_rolls_back_outputs_when_output_commit_fails() {
    let root = workspace_with_part("old = True\n");
    let staged = stage_cadquery_project(&root, Path::new("parts/top_lid.py"), "new = True\n")
        .expect("stage project");
    let staged_outputs = staged.output_dir();
    fs::create_dir_all(staged_outputs.join("b_dir")).unwrap();
    fs::write(staged_outputs.join("a.step"), "new artifact\n").unwrap();
    fs::write(staged_outputs.join("b_dir/c.step"), "blocked artifact\n").unwrap();
    fs::create_dir_all(root.join("outputs")).unwrap();
    fs::write(root.join("outputs/a.step"), "old artifact\n").unwrap();
    fs::write(root.join("outputs/b_dir"), "not a directory\n").unwrap();

    let error = staged
        .commit_outputs()
        .expect_err("commit should fail on conflicting output path");

    assert_eq!(error.kind, CadQueryRunnerErrorKind::InvalidProjectPath);
    assert_eq!(
        fs::read_to_string(root.join("outputs/a.step")).unwrap(),
        "old artifact\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("outputs/b_dir")).unwrap(),
        "not a directory\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cadquery_staging_does_not_recommit_outputs_copied_from_workspace() {
    let root = workspace_with_part("old = True\n");
    fs::create_dir_all(root.join("outputs")).unwrap();
    fs::write(root.join("outputs/stale.step"), "old artifact\n").unwrap();
    let staged = stage_cadquery_project(&root, Path::new("parts/top_lid.py"), "new = True\n")
        .expect("stage project");
    fs::write(root.join("outputs/stale.step"), "external artifact\n").unwrap();

    staged.commit_outputs().expect("no generated outputs");

    assert_eq!(
        fs::read_to_string(root.join("outputs/stale.step")).unwrap(),
        "external artifact\n"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cadquery_staging_rejects_output_commit_when_original_file_changed() {
    let root = workspace_with_part("old = True\n");
    let staged = stage_cadquery_project(&root, Path::new("parts/top_lid.py"), "new = True\n")
        .expect("stage project");
    fs::create_dir_all(staged.output_dir()).unwrap();
    fs::write(staged.output_dir().join("top_lid.step"), "artifact\n").unwrap();
    fs::write(root.join("parts/top_lid.py"), "external = True\n").unwrap();

    let error = staged
        .commit_outputs()
        .expect_err("output commit should detect source conflict");

    assert_eq!(error.kind, CadQueryRunnerErrorKind::FileConflict);
    assert!(!root.join("outputs/top_lid.step").exists());
    let _ = fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn cadquery_staging_rejects_output_symlink_escape() {
    let root = workspace_with_part("old = True\n");
    let outside = temp_dir("cadquery-output-escape");
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("outputs")).unwrap();
    let staged = stage_cadquery_project(&root, Path::new("parts/top_lid.py"), "new = True\n")
        .expect("stage project");
    fs::create_dir_all(staged.output_dir()).unwrap();
    fs::write(staged.output_dir().join("top_lid.step"), "artifact\n").unwrap();

    let error = staged
        .commit_outputs()
        .expect_err("output commit should reject symlink escape");

    assert_eq!(error.kind, CadQueryRunnerErrorKind::InvalidProjectPath);
    assert!(!outside.join("top_lid.step").exists());
    let _ = fs::remove_file(root.join("outputs"));
    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
}

#[test]
fn cadquery_staging_rolls_back_outputs_when_target_commit_fails() {
    let root = workspace_with_part("old = True\n");
    let staged = stage_cadquery_project(&root, Path::new("parts/top_lid.py"), "new = True\n")
        .expect("stage project");
    fs::create_dir_all(staged.output_dir()).unwrap();
    fs::write(staged.output_dir().join("top_lid.step"), "new artifact\n").unwrap();
    fs::create_dir_all(root.join("outputs")).unwrap();
    fs::write(root.join("outputs/top_lid.step"), "old artifact\n").unwrap();
    fs::remove_file(staged.root().join(staged.script_arg())).unwrap();

    let error = staged
        .commit_success()
        .expect_err("commit should fail when staged target is missing");

    assert_eq!(error.kind, CadQueryRunnerErrorKind::Io);
    assert_eq!(
        fs::read_to_string(root.join("parts/top_lid.py")).unwrap(),
        "old = True\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("outputs/top_lid.step")).unwrap(),
        "old artifact\n"
    );
    let _ = fs::remove_dir_all(root);
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

fn workspace_with_part(contents: &str) -> std::path::PathBuf {
    let root = temp_dir("cadquery-stage");
    fs::create_dir_all(root.join("parts")).expect("workspace parts");
    fs::write(root.join("parts/top_lid.py"), contents).expect("part file");
    root
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
