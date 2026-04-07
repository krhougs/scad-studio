use scad_data::LogLevel;
use scad_data::openscad::collect_process_logs;

#[test]
fn collect_process_logs_ignores_blank_lines_and_tags_stdout_as_info() {
    let logs = collect_process_logs(b"line one\n\nline two\n", b"", true);

    assert_eq!(logs.len(), 2);
    assert_eq!(logs[0].level, LogLevel::Info);
    assert_eq!(logs[0].message, "line one");
    assert_eq!(logs[1].level, LogLevel::Info);
    assert_eq!(logs[1].message, "line two");
}

#[test]
fn collect_process_logs_tags_stderr_as_error_when_process_fails() {
    let logs = collect_process_logs(b"", b"warning line\nfatal line\n", false);

    assert_eq!(logs.len(), 2);
    assert!(logs.iter().all(|entry| entry.level == LogLevel::Error));
}
