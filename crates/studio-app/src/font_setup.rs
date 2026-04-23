pub fn configure_egui_fonts(ctx: &egui::Context) {
    match scad_scene::system_fonts::configure_egui_fonts(ctx) {
        Ok(paths) if !paths.is_empty() => {
            log::info!("已加载 {} 个系统字体回退项", paths.len());
        }
        Ok(_) => {
            log::warn!("未获取到系统字体回退项，继续使用 egui 默认字体");
        }
        Err(error) => {
            log::warn!("加载系统字体回退链失败: {error}");
        }
    }
}
