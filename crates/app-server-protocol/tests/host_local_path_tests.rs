use app_server_protocol::{HostLocalPath, ProtocolErrorCode};

#[test]
fn host_local_path_accepts_windows_macos_and_unix_examples() {
    for path in [
        r"C:\Program Files\OpenSCAD\openscad.exe",
        "/Applications/OpenSCAD.app/Contents/MacOS/OpenSCAD",
        "/usr/bin/openscad",
    ] {
        let value = HostLocalPath::new(path).expect("host-local path should be accepted");
        assert_eq!(value.as_str(), path);
    }
}

#[test]
fn host_local_path_rejects_empty_and_nul() {
    for path in ["", "bad\0path"] {
        let error = HostLocalPath::new(path).expect_err("host-local path should be rejected");
        assert_eq!(error.code(), ProtocolErrorCode::InvalidHostLocalPath);
    }
}
