use std::hash::{Hash, Hasher};

use scad_viewer::app::LogLevel;

use super::*;

impl ViewerTab {
    pub(super) fn load_initial_state(&mut self) -> Result<(), String> {
        self.viewer.set_current_file(self.path.clone());
        match self.kind {
            ViewerSourceKind::Scad => self.load_scad_document(),
            ViewerSourceKind::Stl | ViewerSourceKind::ThreeMf => self.load_direct_mesh(),
        }
    }

    fn load_scad_document(&mut self) -> Result<(), String> {
        let source_text = self.client.read_text_file(&self.path, "源文件")?;
        self.document.load_source(self.path.clone(), &source_text);
        self.refresh_presets();
        self.flush_document_warnings();
        self.sync_watch_subscriptions()?;
        self.start_render();
        Ok(())
    }

    pub(super) fn load_direct_mesh(&mut self) -> Result<(), String> {
        self.sync_watch_subscriptions()?;
        self.start_render();
        Ok(())
    }

    pub(super) fn reload_source_document(&mut self, path: &Path) -> Result<(), String> {
        let source_text = self.client.read_text_file(path, "源文件")?;
        self.document.reload_source(&source_text);
        self.flush_document_warnings();
        Ok(())
    }

    pub(super) fn refresh_presets(&mut self) {
        let Some(path) = self.document.preset_path() else {
            return;
        };
        match self.client.read_presets(&path) {
            Ok(presets) => self.document.set_presets(presets),
            Err(error) => self.viewer.push_log(LogLevel::Warning, error),
        }
    }

    fn flush_document_warnings(&mut self) {
        for warning in self.document.take_warnings() {
            self.viewer.push_log(LogLevel::Warning, warning);
        }
    }

    pub(super) fn start_render(&mut self) {
        self.preview_request_serial = self.preview_request_serial.wrapping_add(1);
        let serial = self.preview_request_serial;
        self.viewer.set_rendering(match self.kind {
            ViewerSourceKind::Scad => "正在通过 App Server 调用 OpenSCAD 生成 3MF",
            ViewerSourceKind::Stl | ViewerSourceKind::ThreeMf => "正在通过 App Server 加载模型预览",
        });
        self.viewer
            .push_log(LogLevel::Info, format!("开始渲染 {}", self.path.display()));
        let proxy = self.proxy.clone();
        let window_id = self.window_id;
        let tab_id = self.id;
        self.client.request_preview_async(
            self.path.clone(),
            self.cached_openscad_path.clone(),
            self.document.current_defines(),
            move |result| {
                let _ =
                    proxy.send_event(UserEvent::PreviewReady(window_id, tab_id, serial, result));
            },
        );
    }

    pub(super) fn apply_preview_ready(
        &mut self,
        serial: u64,
        result: Result<crate::protocol_client::PreviewSuccess, String>,
    ) {
        if serial != self.preview_request_serial {
            return;
        }
        match result {
            Ok(artifact) => {
                self.viewer.set_current_file(artifact.source_path);
                self.viewer.set_ready("预览已更新");
                self.viewer.push_log(LogLevel::Info, "预览生成完成");
                self.set_mesh(artifact.mesh);
            }
            Err(error) => {
                self.mesh = None;
                self.mesh_revision = self.mesh_revision.wrapping_add(1);
                self.viewer.set_error(error.clone());
                self.viewer.push_log(LogLevel::Error, error);
            }
        }
    }

    fn set_mesh(&mut self, mesh: MeshData) {
        self.clip_plane.visible_extent = mesh.bounds.radius().max(64.0);
        self.current_bounds = Some(mesh.bounds);
        self.camera.fit_bounds(mesh.bounds);
        self.mesh = Some(mesh);
        self.mesh_revision = self.mesh_revision.wrapping_add(1);
    }

    fn sync_watch_subscriptions(&mut self) -> Result<(), String> {
        self.watch_subscriptions.clear();
        let watch_paths = match self.kind {
            ViewerSourceKind::Scad => self.document.watch_paths(),
            ViewerSourceKind::Stl | ViewerSourceKind::ThreeMf => vec![self.path.clone()],
        };
        for watched_path in watch_paths {
            let changed_proxy = self.proxy.clone();
            let error_proxy = self.proxy.clone();
            let window_id = self.window_id;
            let tab_id = self.id;
            let subscription = self.client.subscribe_path(
                &watched_path,
                move |path| {
                    let _ =
                        changed_proxy.send_event(UserEvent::SourceChanged(window_id, tab_id, path));
                },
                move |message| {
                    let _ =
                        error_proxy.send_event(UserEvent::WatchError(window_id, tab_id, message));
                },
            )?;
            self.watch_subscriptions.push(subscription);
        }
        Ok(())
    }
}

pub(super) fn configured_slicers(config: &AppConfig) -> Vec<(String, PathBuf)> {
    config
        .slicers
        .iter()
        .map(|slicer| (slicer.name.clone(), slicer.path.clone()))
        .collect()
}

pub(super) fn tab_id_for_path(kind: &str, path: &Path) -> TabId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    kind.hash(&mut hasher);
    path.hash(&mut hasher);
    hasher.finish()
}

pub(super) fn detect_viewer_kind(path: &Path) -> Result<ViewerSourceKind, String> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("scad") => Ok(ViewerSourceKind::Scad),
        Some("stl") => Ok(ViewerSourceKind::Stl),
        Some("3mf") => Ok(ViewerSourceKind::ThreeMf),
        _ => Err(format!("不支持的模型类型: {}", path.display())),
    }
}

pub(super) fn file_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("模型")
        .to_owned()
}

pub(super) fn export_output_path(
    tab: &ViewerTab,
    source_path: &Path,
    slicer_name: Option<&str>,
) -> Option<PathBuf> {
    if slicer_name.is_some() {
        return Some(std::env::temp_dir().join(format!(
            "{}.{}",
            source_path
                .file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("export"),
            tab.document.export_format.extension()
        )));
    }
    let extension = tab.document.export_format.extension();
    let file_name = format!(
        "{}.{}",
        source_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("export"),
        extension
    );
    rfd::FileDialog::new()
        .set_file_name(file_name)
        .add_filter(extension.to_uppercase(), &[extension])
        .save_file()
}
