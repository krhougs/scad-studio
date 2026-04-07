//! macOS：融合标题栏（FullSizeContentView + 透明标题栏）、首行红绿灯与标签条对齐、可拖区域。
//! Windows / Linux 下除 `traffic_lights_left_inset` 为 0 外均为空操作。

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;
use winit::window::WindowAttributes;

/// 关闭键左缘在内容视图（flipped）中的目标 X，略大于系统默认，使整组按钮右移。
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHTS_CLOSE_LEFT_IN_CONTENT_X: f64 = 20.0;

pub fn apply_macos_fused_titlebar_attributes(mut attrs: WindowAttributes) -> WindowAttributes {
    #[cfg(target_os = "macos")]
    {
        attrs = attrs
            .with_fullsize_content_view(true)
            .with_titlebar_transparent(true)
            .with_title_hidden(true);
    }
    attrs
}

/// 为系统窗口按钮预留的左侧间距（逻辑点，与 `TRAFFIC_LIGHTS_CLOSE_LEFT_IN_CONTENT_X` 右移量匹配）。
pub fn traffic_lights_left_inset() -> f32 {
    #[cfg(target_os = "macos")]
    {
        88.0
    }
    #[cfg(not(target_os = "macos"))]
    {
        0.0
    }
}

/// 在水平布局末尾填充可拖动区域（标签条、工具条右侧空白）。
pub fn horizontal_drag_tail(ui: &mut egui::Ui, min_width: f32) {
    #[cfg(target_os = "macos")]
    {
        let w = ui.available_width();
        let h = ui.available_height();
        if w >= min_width && h > 1.0 {
            let response =
                ui.allocate_response(egui::vec2(w, h), egui::Sense::click_and_drag());
            if response.drag_started() {
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        }
    }
}

/// 将系统关闭 / 最小化 / 缩放按钮与首行药丸标签条对齐：水平略右移、垂直与药丸 **视觉** 中线一致。
/// `strip_pills_center_y`：与 `document_tabs::tab_rail_pills_center_y_from_strip_top` 一致（内容视图 flipped）。
/// `tab_height_pts`：与 `document_tabs::tab_height` 一致，**不读取** AppKit 按钮外框高度，避免随系统/机型变化。
#[cfg(target_os = "macos")]
pub fn sync_traffic_lights_with_tab_rail(
    window: &winit::window::Window,
    strip_pills_center_y: f32,
    tab_height_pts: f32,
) {
    use objc2_app_kit::{NSView, NSWindow};
    use objc2_foundation::MainThreadMarker;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    if MainThreadMarker::new().is_none() {
        return;
    }
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    let view_ptr = appkit.ns_view.as_ptr().cast::<NSView>();
    if view_ptr.is_null() {
        return;
    }
    let winit_view = unsafe { &*view_ptr };
    let Some(window_ret) = winit_view.window() else {
        return;
    };
    let ns_window: &NSWindow = &window_ret;
    ensure_standard_titlebar_controls_visible(ns_window);
    let Some(cv_ret) = ns_window.contentView() else {
        return;
    };
    let content_view: &NSView = &cv_ret;
    let strip_center = f64::from(strip_pills_center_y);
    let tab_h = f64::from(tab_height_pts);
    let Some(ds) =
        traffic_lights_superview_delta(ns_window, content_view, strip_center, tab_h)
    else {
        return;
    };
    nudge_traffic_lights_by(ns_window, ds);
}

/// 写死的红绿灯 **圆点**近似直径（pt），仅用于与 `tab_height_pts` 组合换算目标外框中心，不读系统几何。
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHT_VISUAL_DIAMETER_PT: f64 = 12.0;

/// 在 flipped 坐标下略减小目标中心 Y，使红绿灯视觉上略靠上（相对药丸中线）。
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHTS_TARGET_NUDGE_UP_PT: f64 = 3.0;

#[cfg(target_os = "macos")]
fn traffic_lights_superview_delta(
    ns_window: &objc2_app_kit::NSWindow,
    content_view: &objc2_app_kit::NSView,
    strip_pills_center_y: f64,
    tab_height_pts: f64,
) -> Option<objc2_foundation::NSPoint> {
    use objc2_app_kit::{NSButton, NSView, NSWindowButton};
    use objc2_foundation::NSPoint;

    let close_ret = ns_window.standardWindowButton(NSWindowButton::CloseButton)?;
    let close: &NSButton = &close_ret;
    let r = close.convertRect_toView(close.bounds(), Some(content_view));
    let cur_cy = f64::from(r.origin.y) + f64::from(r.size.height) * 0.5;
    let cur_left = f64::from(r.origin.x);
    let nominal_h = tab_height_pts.max(1.0);
    // 假定外框顶缘与条带顶缘对齐：目标外框中心 = 药丸中线 + (Tab 药丸高度 − 圆点直径)/2；nominal_h 一律用 UI Tab 高度。
    let target_cy = strip_pills_center_y
        + (nominal_h - TRAFFIC_LIGHT_VISUAL_DIAMETER_PT) * 0.5
        - TRAFFIC_LIGHTS_TARGET_NUDGE_UP_PT;
    let delta_y = target_cy - cur_cy;
    let delta_x = TRAFFIC_LIGHTS_CLOSE_LEFT_IN_CONTENT_X - cur_left;
    if delta_x.abs() < 0.2 && delta_y.abs() < 0.2 {
        return None;
    }
    let sv_ret = unsafe { close.superview() }?;
    let sv: &NSView = &sv_ret;
    let p0 = content_view.convertPoint_toView(NSPoint::new(0.0, 0.0), Some(sv));
    let p1 = content_view.convertPoint_toView(
        NSPoint::new(delta_x, delta_y),
        Some(sv),
    );
    Some(NSPoint::new(p1.x - p0.x, p1.y - p0.y))
}

#[cfg(target_os = "macos")]
fn ensure_standard_titlebar_controls_visible(ns_window: &objc2_app_kit::NSWindow) {
    use objc2_app_kit::{NSButton, NSWindowButton};

    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(btn_ret) = ns_window.standardWindowButton(kind) {
            let btn: &NSButton = &btn_ret;
            btn.setHidden(false);
            btn.setAlphaValue(1.0);
        }
    }
}

#[cfg(target_os = "macos")]
fn nudge_traffic_lights_by(ns_window: &objc2_app_kit::NSWindow, ds: objc2_foundation::NSPoint) {
    use objc2_app_kit::{NSButton, NSView, NSWindowButton};
    use objc2_foundation::NSPoint;
    use objc2_quartz_core::CATransaction;

    CATransaction::begin();
    CATransaction::setDisableActions(true);
    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(btn_ret) = ns_window.standardWindowButton(kind) {
            let btn: &NSButton = &btn_ret;
            let f = btn.frame();
            btn.setFrameOrigin(NSPoint::new(
                f.origin.x + ds.x,
                f.origin.y + ds.y,
            ));
            NSView::setNeedsDisplay(btn, true);
        }
    }
    CATransaction::commit();
}

#[cfg(not(target_os = "macos"))]
pub fn sync_traffic_lights_with_tab_rail(
    _window: &winit::window::Window,
    _strip_pills_center_y: f32,
    _tab_height_pts: f32,
) {
}
