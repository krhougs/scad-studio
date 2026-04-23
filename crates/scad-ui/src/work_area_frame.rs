pub fn central_panel_frame(viewer_active: bool) -> egui::Frame {
    if viewer_active {
        egui::Frame::NONE
    } else {
        egui::Frame::default().fill(crate::theme::palette::BG_WINDOW)
    }
}
