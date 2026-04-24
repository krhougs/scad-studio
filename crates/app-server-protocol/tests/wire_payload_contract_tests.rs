use std::{fs, path::Path};

#[test]
fn protocol_wire_payload_does_not_expose_pathbuf_or_json_config_payload() {
    let source = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/protocol.rs"))
        .expect("protocol source should be readable");

    assert!(
        !source.contains("use std::path::PathBuf"),
        "protocol wire payload must not import PathBuf"
    );
    assert!(
        !source.contains("pub json: String"),
        "config wire payload must not carry structured config as json string"
    );
    assert!(
        !source.contains("output_path: PathBuf"),
        "export output target must be workspace portable path"
    );
}
