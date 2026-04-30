use std::{
    path::{Path, PathBuf},
};
use tokio::process::Command;

const VERIFY_SCRIPT: &str = r#"
import cadquery
import budn_cad_runner
"#;

pub fn cadquery_python_path() -> PathBuf {
    std::env::var_os("CADQUERY_RUNNER_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("python3"))
}

pub async fn verify_cadquery_runner_environment(python: &Path) -> Result<(), String> {
    let output = Command::new(python)
        .arg("-c")
        .arg(VERIFY_SCRIPT)
        .output()
        .await
        .map_err(|error| verification_error(python, &format!("启动 Python 失败: {error}")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail = if stderr.trim().is_empty() {
        stdout.trim()
    } else {
        stderr.trim()
    };
    Err(verification_error(python, detail))
}

fn verification_error(python: &Path, detail: &str) -> String {
    format!(
        "CadQuery Python 环境验证失败。\n\
         Python: {}\n\
         环境变量: CADQUERY_RUNNER_PYTHON\n\
         详情: {}\n\
         修复建议: 将 CADQUERY_RUNNER_PYTHON 指向能 import cadquery 和 budn_cad_runner 的 Python，例如在 .env 中设置 CADQUERY_RUNNER_PYTHON=/path/to/python3.11。",
        python.display(),
        if detail.is_empty() {
            "(no output)"
        } else {
            detail
        }
    )
}
