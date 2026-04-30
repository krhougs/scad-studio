use app_server_host::verify_cadquery_runner_environment;

#[test]
fn cadquery_environment_verify_accepts_python_that_can_import_runner_dependencies() {
    let dir = tempfile::tempdir().unwrap();
    let python = fake_python(dir.path(), 0, "");

    verify_cadquery_runner_environment(&python).expect("fake python should verify");
}

#[test]
fn cadquery_environment_verify_reports_python_import_failure_with_env_hint() {
    let dir = tempfile::tempdir().unwrap();
    let python = fake_python(
        dir.path(),
        1,
        "ModuleNotFoundError: No module named 'cadquery'",
    );

    let error = verify_cadquery_runner_environment(&python).expect_err("verify should fail");

    assert!(error.contains("CadQuery Python 环境验证失败"));
    assert!(error.contains("CADQUERY_RUNNER_PYTHON"));
    assert!(error.contains(&python.display().to_string()));
    assert!(error.contains("ModuleNotFoundError"));
}

fn fake_python(root: &std::path::Path, exit_code: i32, stderr: &str) -> std::path::PathBuf {
    let path = root.join("fake-python.sh");
    std::fs::write(
        &path,
        format!("#!/bin/sh\nprintf '%s\\n' \"{stderr}\" >&2\nexit {exit_code}\n"),
    )
    .expect("write fake python");
    make_executable(&path);
    path
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("permissions");
}

#[cfg(not(unix))]
fn make_executable(_path: &std::path::Path) {}
