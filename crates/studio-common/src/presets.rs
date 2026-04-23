use std::path::{Path, PathBuf};

pub fn preset_path_for_source(source_path: &Path) -> PathBuf {
    source_path.with_extension("scad.json")
}
