use std::{
    path::{Path, PathBuf},
    process::Command,
};

use crate::{
    config::{AppConfig, SlicerConfig},
    openscad::{CliOutputFormat, OpenScadError, build_cli_args, detect_openscad_path},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Stl,
    ThreeMf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlicerInstall {
    pub name: String,
    pub path: PathBuf,
}

pub fn build_export_filename(source_path: &Path, format: ExportFormat) -> String {
    let stem = source_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("export");
    format!("{stem}.{}", format.extension())
}

pub fn detect_slicer_paths(config: &AppConfig) -> Vec<SlicerInstall> {
    let mut detected = config
        .slicers
        .iter()
        .map(|slicer| SlicerInstall {
            name: slicer.name.clone(),
            path: slicer.path.clone(),
        })
        .collect::<Vec<_>>();
    detected.extend(
        default_slicer_candidates()
            .into_iter()
            .filter(|candidate| candidate.path.exists()),
    );
    detected
}

pub fn export_model(
    config: &AppConfig,
    source_path: &Path,
    defines: &[String],
    output_path: &Path,
    format: ExportFormat,
) -> Result<(), OpenScadError> {
    let executable = detect_openscad_path(config.openscad_path.clone())?;
    let status = Command::new(executable)
        .args(build_cli_args(
            format.into(),
            output_path,
            defines,
            source_path,
        ))
        .status()
        .map_err(|error| OpenScadError::new(format!("启动 OpenSCAD CLI 失败: {error}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(OpenScadError::new("导出模型失败"))
    }
}

pub fn send_to_slicer(slicer: &SlicerConfig, model_path: &Path) -> Result<(), String> {
    Command::new(&slicer.path)
        .arg(model_path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("启动切片软件失败: {error}"))
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Stl => "stl",
            Self::ThreeMf => "3mf",
        }
    }
}

impl From<ExportFormat> for CliOutputFormat {
    fn from(value: ExportFormat) -> Self {
        match value {
            ExportFormat::Stl => CliOutputFormat::BinaryStl,
            ExportFormat::ThreeMf => CliOutputFormat::ThreeMf,
        }
    }
}

fn default_slicer_candidates() -> Vec<SlicerInstall> {
    if cfg!(target_os = "macos") {
        vec![
            slicer(
                "PrusaSlicer",
                "/Applications/PrusaSlicer.app/Contents/MacOS/PrusaSlicer",
            ),
            slicer(
                "Bambu Studio",
                "/Applications/Bambu Studio.app/Contents/MacOS/BambuStudio",
            ),
            slicer(
                "Cura",
                "/Applications/Ultimaker Cura.app/Contents/MacOS/UltiMaker-Cura",
            ),
        ]
    } else if cfg!(target_os = "windows") {
        vec![
            slicer(
                "PrusaSlicer",
                "C:\\Program Files\\Prusa3D\\PrusaSlicer\\prusa-slicer.exe",
            ),
            slicer(
                "Bambu Studio",
                "C:\\Program Files\\Bambu Studio\\BambuStudio.exe",
            ),
            slicer(
                "Cura",
                "C:\\Program Files\\UltiMaker Cura 5.0\\UltiMaker-Cura.exe",
            ),
        ]
    } else {
        vec![
            slicer("PrusaSlicer", "/usr/bin/prusa-slicer"),
            slicer("Bambu Studio", "/usr/bin/BambuStudio"),
            slicer("Cura", "/usr/bin/cura"),
        ]
    }
}

fn slicer(name: &str, path: &str) -> SlicerInstall {
    SlicerInstall {
        name: name.to_string(),
        path: PathBuf::from(path),
    }
}
