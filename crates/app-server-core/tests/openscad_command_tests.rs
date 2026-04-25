use app_server_core::{
    CliOutputFormat, OpenScadError, build_cli_args, build_preview_job_args, detect_openscad_path,
    finalize_job, preview_artifact, resolve_openscad_path,
};
use app_server_protocol::PreviewArtifact;
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::{ExitStatus, Output},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
#[cfg(windows)]
use std::os::windows::process::ExitStatusExt;
use zip::{ZipWriter, write::SimpleFileOptions};

#[test]
fn build_cli_args_includes_defines_before_source_path() {
    let args = build_cli_args(
        CliOutputFormat::BinaryStl,
        Path::new("/tmp/out.stl"),
        &["height=12".into(), "name=\"fine\"".into()],
        Path::new("/tmp/model.scad"),
    );

    assert_eq!(
        args,
        vec![
            "--export-format".to_string(),
            "binstl".to_string(),
            "-o".to_string(),
            "/tmp/out.stl".to_string(),
            "-D".to_string(),
            "height=12".to_string(),
            "-D".to_string(),
            "name=\"fine\"".to_string(),
            "/tmp/model.scad".to_string(),
        ]
    );
}

#[test]
fn preview_job_args_force_3mf_output() {
    let (output_path, args) = build_preview_job_args(
        Path::new("/tmp/model.scad"),
        &["height=12".into(), "name=\"fine\"".into()],
    );

    assert_eq!(
        output_path.extension().and_then(|value| value.to_str()),
        Some("3mf")
    );
    assert_eq!(
        args,
        vec![
            "--export-format".to_string(),
            "3mf".to_string(),
            "-o".to_string(),
            output_path.display().to_string(),
            "-D".to_string(),
            "height=12".to_string(),
            "-D".to_string(),
            "name=\"fine\"".to_string(),
            "/tmp/model.scad".to_string(),
        ]
    );
}

#[test]
fn preview_artifact_stl_reads_raw_bytes_without_server_decode() {
    let path = temp_file("preview-raw").with_extension("stl");
    let bytes = b"not a valid stl but still raw preview bytes".to_vec();
    fs::write(&path, &bytes).expect("write raw stl");

    let artifact = preview_artifact(None, &path, &[]).expect("stl raw preview should succeed");

    match artifact {
        PreviewArtifact::Stl(stl) => {
            assert_eq!(stl.bytes, bytes);
            assert_eq!(stl.media_type, "model/stl");
        }
        other => panic!("unexpected artifact: {other:?}"),
    }
    remove_file(&path);
}

#[test]
fn preview_artifact_3mf_reads_raw_bytes_without_server_decode() {
    let path = temp_file("preview-raw").with_extension("3mf");
    let bytes = b"not a valid 3mf but direct preview is client-decoded".to_vec();
    fs::write(&path, &bytes).expect("write raw 3mf");

    let artifact = preview_artifact(None, &path, &[]).expect("3mf raw preview should succeed");

    match artifact {
        PreviewArtifact::ThreeMf(three_mf) => {
            assert_eq!(three_mf.bytes, bytes);
            assert_eq!(three_mf.media_type, "model/3mf");
        }
        other => panic!("unexpected artifact: {other:?}"),
    }
    remove_file(&path);
}

#[test]
fn preview_job_uses_3mf_temp_filename() {
    let (output_path, _) = build_preview_job_args(Path::new("/tmp/widget.scad"), &[]);

    let file_name = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .expect("preview output should have a file name");

    assert!(file_name.starts_with("scad-studio-widget-"));
    assert!(file_name.ends_with(".3mf"));
}

#[test]
fn resolve_openscad_path_prefers_configured_path() {
    let configured_path = temp_file("configured-openscad");
    let env_path = temp_file("env-openscad");
    let auto_path = temp_file("auto-openscad");
    create_file(&configured_path);
    create_file(&env_path);
    create_file(&auto_path);

    let resolved = resolve_openscad_path(
        Some(configured_path.clone()),
        Some(env_path.clone()),
        Some(auto_path.clone()),
    )
    .expect("configured path should win");

    assert_eq!(resolved, configured_path);
    remove_file(&configured_path);
    remove_file(&env_path);
    remove_file(&auto_path);
}

#[test]
fn resolve_openscad_path_keeps_generic_missing_cli_message() {
    let error = resolve_openscad_path(None, None, None).expect_err("missing path should fail");
    let message = error.to_string();

    assert!(message.contains("未找到 OpenSCAD CLI"));
    assert!(message.contains("OPENSCAD_PATH"));
    assert!(message.contains("Settings"));
    assert!(!message.contains("os error 2"));
}

#[test]
fn resolve_openscad_path_falls_back_when_configured_path_is_missing() {
    let missing_configured = temp_path("missing-configured-openscad");
    let env_path = temp_file("env-openscad");
    create_file(&env_path);

    let resolved = resolve_openscad_path(Some(missing_configured), Some(env_path.clone()), None)
        .expect("env path should be used when configured path is missing");

    assert_eq!(resolved, env_path);
    remove_file(&env_path);
}

#[test]
fn resolve_openscad_path_falls_back_to_auto_path_after_missing_overrides() {
    let missing_configured = temp_path("missing-configured-openscad");
    let missing_env = temp_path("missing-env-openscad");
    let auto_path = temp_file("auto-openscad");
    create_file(&auto_path);

    let resolved = resolve_openscad_path(
        Some(missing_configured),
        Some(missing_env),
        Some(auto_path.clone()),
    )
    .expect("auto detected path should be used after missing overrides");

    assert_eq!(resolved, auto_path);
    remove_file(&auto_path);
}

#[test]
fn resolve_openscad_path_expands_macos_app_bundle_candidate() {
    let bundle_root = temp_dir("OpenSCAD-bundle").with_extension("app");
    let executable = bundle_root.join("Contents/MacOS/OpenSCAD");
    fs::create_dir_all(executable.parent().expect("bundle executable parent"))
        .expect("create app bundle executable directory");
    create_file(&executable);

    let resolved = resolve_openscad_path(Some(bundle_root.clone()), None, None)
        .expect("app bundle should resolve to executable");

    assert_eq!(resolved, executable);
    remove_dir(&bundle_root);
}

#[test]
fn detect_openscad_path_resolves_bare_command_from_path() {
    let _guard = env_lock().lock().expect("env lock");
    let path_dir = temp_dir("openscad-path-dir");
    let executable = path_dir.join(default_openscad_binary_name());
    create_file(&executable);

    with_path(&path_dir, || {
        let resolved = detect_openscad_path(Some(PathBuf::from("openscad")))
            .expect("bare openscad should resolve through PATH");

        assert_eq!(resolved, executable);
    });
    remove_dir(&path_dir);
}

#[test]
fn detect_openscad_path_falls_back_after_bare_command_misses_path() {
    let _guard = env_lock().lock().expect("env lock");
    let empty_path_dir = temp_dir("empty-openscad-path-dir");
    fs::create_dir_all(&empty_path_dir).expect("create empty PATH dir");
    let env_path = temp_file("env-openscad");
    create_file(&env_path);

    with_path(&empty_path_dir, || {
        with_env_path(&env_path, || {
            let resolved = detect_openscad_path(Some(PathBuf::from("openscad")))
                .expect("env path should be used when bare command is absent from PATH");

            assert_eq!(resolved, env_path);
        });
    });
    remove_dir(&empty_path_dir);
    remove_file(&env_path);
}

#[test]
fn finalize_job_cleans_preview_file_when_output_collection_fails() {
    let preview_path = std::env::temp_dir().join(format!(
        "scad-studio-preview-cleanup-{}.3mf",
        std::process::id()
    ));
    fs::write(&preview_path, b"fixture").expect("should create temp preview file");

    let result = finalize_job(
        PathBuf::from("/tmp/example.scad"),
        preview_path.clone(),
        true,
        Err(OpenScadError::new("collect output failed")),
    );

    assert!(result.is_err());
    assert!(
        !preview_path.exists(),
        "preview file should be removed on error"
    );
}

#[test]
fn finalize_job_reads_valid_3mf_bytes_and_cleans_preview_file() {
    let preview_path = temp_file("preview-finalize-valid").with_extension("3mf");
    let bytes = minimal_three_mf_bytes();
    fs::write(&preview_path, &bytes).expect("write valid 3mf");

    let artifact = finalize_job(
        PathBuf::from("/tmp/example.scad"),
        preview_path.clone(),
        true,
        Ok(successful_output()),
    )
    .expect("valid 3mf should finalize");

    assert_eq!(artifact.bytes, bytes);
    assert!(!preview_path.exists());
}

#[test]
fn finalize_job_rejects_invalid_3mf_and_cleans_preview_file() {
    let preview_path = temp_file("preview-finalize-invalid").with_extension("3mf");
    fs::write(&preview_path, b"not a 3mf").expect("write invalid 3mf");

    let result = finalize_job(
        PathBuf::from("/tmp/example.scad"),
        preview_path.clone(),
        true,
        Ok(successful_output()),
    );

    assert!(result.is_err());
    assert!(!preview_path.exists());
}

fn temp_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "scad-studio-{label}-{}-{nonce}",
        std::process::id()
    ))
}

fn temp_file(label: &str) -> PathBuf {
    temp_path(label)
}

fn temp_dir(label: &str) -> PathBuf {
    temp_path(label)
}

fn create_file(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create temp parent directory");
    }
    fs::write(path, "#!/bin/sh\n").expect("create temp executable file");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .expect("read temp executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("mark temp executable file");
    }
}

fn successful_output() -> Output {
    Output {
        status: successful_status(),
        stdout: Vec::new(),
        stderr: Vec::new(),
    }
}

#[cfg(unix)]
fn successful_status() -> ExitStatus {
    ExitStatus::from_raw(0)
}

#[cfg(windows)]
fn successful_status() -> ExitStatus {
    ExitStatus::from_raw(0)
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
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("3D/3dmodel.model", SimpleFileOptions::default())
        .expect("fixture should open archive entry");
    writer
        .write_all(xml.as_bytes())
        .expect("fixture should write xml");
    writer
        .finish()
        .expect("fixture should finish archive")
        .into_inner()
}

fn remove_file(path: &Path) {
    let _ = fs::remove_file(path);
}

fn remove_dir(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

fn default_openscad_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "openscad.exe"
    } else {
        "openscad"
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn with_path(path: &Path, run: impl FnOnce()) {
    let previous = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", path);
    }
    run();
    unsafe {
        restore_env("PATH", previous);
    }
}

fn with_env_path(path: &Path, run: impl FnOnce()) {
    let previous = std::env::var_os("OPENSCAD_PATH");
    unsafe {
        std::env::set_var("OPENSCAD_PATH", path);
    }
    run();
    unsafe {
        restore_env("OPENSCAD_PATH", previous);
    }
}

unsafe fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
    if let Some(value) = value {
        unsafe {
            std::env::set_var(key, value);
        }
    } else {
        unsafe {
            std::env::remove_var(key);
        }
    }
}
