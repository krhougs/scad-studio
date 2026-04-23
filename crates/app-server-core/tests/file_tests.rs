use app_server_core::{canonicalize_or_original, read_binary_file, read_text_file};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn read_text_file_returns_contents() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should move forward")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("scad-studio-file-{suffix}.md"));
    fs::write(&path, "# title\nbody").unwrap();

    let text = read_text_file(&path, "Markdown").unwrap();
    assert_eq!(text, "# title\nbody");

    let _ = fs::remove_file(path);
}

#[test]
fn read_binary_file_returns_bytes() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should move forward")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("scad-studio-file-{suffix}.bin"));
    fs::write(&path, [1_u8, 2, 3, 4]).unwrap();

    let bytes = read_binary_file(&path, "图片").unwrap();
    assert_eq!(bytes, vec![1, 2, 3, 4]);

    let _ = fs::remove_file(path);
}

#[test]
fn canonicalize_or_original_keeps_missing_path() {
    let path = PathBuf::from("/tmp/scad-studio-missing-file.txt");
    assert_eq!(canonicalize_or_original(path.clone()), path);
}
