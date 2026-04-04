use scad_data::watcher;
use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

#[test]
fn matches_path_accepts_canonicalized_equivalent_paths() {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("scad-studio-watch-{suffix}"));
    let file_path = root.join("sample.scad");
    let symlink_path = root.join("alias.scad");

    fs::create_dir_all(&root).expect("temp dir should be created");
    fs::write(&file_path, "cube();").expect("temp file should be created");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&file_path, &symlink_path).expect("symlink should be created");

    let matched = watcher::matches_path(std::slice::from_ref(&file_path), Some(&symlink_path));

    assert!(matched);

    let _ = fs::remove_file(&symlink_path);
    let _ = fs::remove_file(&file_path);
    let _ = fs::remove_dir(&root);
}

#[test]
fn matches_path_rejects_unrelated_paths() {
    let watched = PathBuf::from("/tmp/example.scad");
    let changed = vec![PathBuf::from("/tmp/other.scad")];

    assert!(!watcher::matches_path(&changed, Some(&watched)));
}

#[test]
fn matches_any_watched_path_when_preset_file_changes() {
    let watched = vec![
        PathBuf::from("/tmp/example.scad"),
        PathBuf::from("/tmp/example.scad.json"),
    ];
    let changed = vec![PathBuf::from("/tmp/example.scad.json")];

    assert!(watcher::matches_any_path(&changed, &watched));
}
