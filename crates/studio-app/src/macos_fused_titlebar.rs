//! macOS：融合标题栏（FullSizeContentView + 透明标题栏）、非全屏时首行红绿灯与标签条对齐、可拖区域。
//! 原生全屏下不在应用内容区对齐系统按钮（左侧 inset 为 0，与 Windows / Linux 一致），由系统边缘标题栏展示默认红绿灯。

#[cfg(target_os = "macos")]
use std::sync::Mutex;
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use winit::platform::macos::WindowAttributesExtMacOS;
use winit::window::WindowAttributes;

#[cfg(target_os = "macos")]
const TRAFFIC_LIGHTS_CLOSE_LEFT_IN_CONTENT_X: f64 = 20.0;

#[cfg(target_os = "macos")]
static TRAFFIC_LIGHT_CLUSTER_ELEVATED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "macos")]
static TRAFFIC_LIGHT_NUDGE_CUMULATIVE_SV: Mutex<(f64, f64)> = Mutex::new((0.0, 0.0));

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

pub fn traffic_lights_left_inset(content_syncs_with_tab_rail: bool) -> f32 {
    #[cfg(target_os = "macos")]
    {
        if content_syncs_with_tab_rail {
            88.0
        } else {
            0.0
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = content_syncs_with_tab_rail;
        0.0
    }
}

pub fn horizontal_drag_tail(ui: &mut egui::Ui, min_width: f32) {
    #[cfg(target_os = "macos")]
    {
        let w = ui.available_width();
        let h = ui.available_height();
        if w >= min_width && h > 1.0 {
            let response = ui.allocate_response(egui::vec2(w, h), egui::Sense::click_and_drag());
            if response.drag_started() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }
        }
    }
}

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

    if window.fullscreen().is_some() {
        reset_traffic_lights_for_native_fullscreen(ns_window, content_view);
        return;
    }

    elevate_traffic_light_cluster_above_content(ns_window, content_view);
    let strip_center = f64::from(strip_pills_center_y);
    let tab_h = f64::from(tab_height_pts);
    let Some(ds) = traffic_lights_superview_delta(ns_window, content_view, strip_center, tab_h)
    else {
        return;
    };
    nudge_traffic_lights_by(ns_window, ds);
}

#[cfg(target_os = "macos")]
fn reset_traffic_lights_for_native_fullscreen(
    ns_window: &objc2_app_kit::NSWindow,
    content_view: &objc2_app_kit::NSView,
) {
    undo_traffic_light_nudge_cumulative(ns_window);
    if TRAFFIC_LIGHT_CLUSTER_ELEVATED.load(Ordering::Relaxed) {
        demote_traffic_light_cluster_below_content(ns_window, content_view);
    }
}

#[cfg(target_os = "macos")]
const TRAFFIC_LIGHT_VISUAL_DIAMETER_PT: f64 = 12.0;
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
    let target_cy = strip_pills_center_y + (nominal_h - TRAFFIC_LIGHT_VISUAL_DIAMETER_PT) * 0.5
        - TRAFFIC_LIGHTS_TARGET_NUDGE_UP_PT;
    let delta_y = target_cy - cur_cy;
    let delta_x = TRAFFIC_LIGHTS_CLOSE_LEFT_IN_CONTENT_X - cur_left;
    if delta_x.abs() < 0.2 && delta_y.abs() < 0.2 {
        return None;
    }
    let sv_ret = unsafe { close.superview() }?;
    let sv: &NSView = &sv_ret;
    let p0 = content_view.convertPoint_toView(NSPoint::new(0.0, 0.0), Some(sv));
    let p1 = content_view.convertPoint_toView(NSPoint::new(delta_x, delta_y), Some(sv));
    Some(NSPoint::new(p1.x - p0.x, p1.y - p0.y))
}

#[cfg(target_os = "macos")]
fn traffic_light_cluster_host(
    ns_window: &objc2_app_kit::NSWindow,
    content: &objc2_app_kit::NSView,
) -> Option<(
    Retained<objc2_app_kit::NSView>,
    Retained<objc2_app_kit::NSView>,
)> {
    use objc2_app_kit::{NSButton, NSView, NSWindowButton};
    use std::ptr;

    let close_ret = ns_window.standardWindowButton(NSWindowButton::CloseButton)?;
    let close: &NSButton = &close_ret;
    let cp_ret = unsafe { content.superview() }?;
    let cp_ref: &NSView = cp_ret.as_ref();
    let mut v = unsafe { close.superview() }?;
    for _ in 0..48 {
        let p = unsafe { v.superview() }?;
        if ptr::eq(ptr::from_ref(p.as_ref()), ptr::from_ref(cp_ref)) {
            return Some((cp_ret, v));
        }
        v = p;
    }
    None
}

#[cfg(target_os = "macos")]
fn elevate_traffic_light_cluster_above_content(
    ns_window: &objc2_app_kit::NSWindow,
    content: &objc2_app_kit::NSView,
) {
    use objc2_app_kit::{NSView, NSWindowOrderingMode};

    let Some((cp_ret, cluster_ret)) = traffic_light_cluster_host(ns_window, content) else {
        TRAFFIC_LIGHT_CLUSTER_ELEVATED.store(false, Ordering::Relaxed);
        return;
    };
    let cp: &NSView = cp_ret.as_ref();
    let cluster: &NSView = cluster_ret.as_ref();
    cluster.setHidden(false);
    cluster.setAlphaValue(1.0);
    cp.addSubview_positioned_relativeTo(cluster, NSWindowOrderingMode::Above, Some(content));
    TRAFFIC_LIGHT_CLUSTER_ELEVATED.store(true, Ordering::Relaxed);
}

#[cfg(target_os = "macos")]
fn demote_traffic_light_cluster_below_content(
    ns_window: &objc2_app_kit::NSWindow,
    content: &objc2_app_kit::NSView,
) {
    use objc2_app_kit::{NSView, NSWindowOrderingMode};

    let Some((cp_ret, cluster_ret)) = traffic_light_cluster_host(ns_window, content) else {
        TRAFFIC_LIGHT_CLUSTER_ELEVATED.store(false, Ordering::Relaxed);
        return;
    };
    let cp: &NSView = cp_ret.as_ref();
    let cluster: &NSView = cluster_ret.as_ref();
    cp.addSubview_positioned_relativeTo(cluster, NSWindowOrderingMode::Below, Some(content));
    TRAFFIC_LIGHT_CLUSTER_ELEVATED.store(false, Ordering::Relaxed);
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
fn move_traffic_light_buttons_by(
    ns_window: &objc2_app_kit::NSWindow,
    ds: objc2_foundation::NSPoint,
) {
    use objc2_app_kit::{NSButton, NSWindowButton};
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
            let mut frame = btn.frame();
            frame.origin = NSPoint::new(frame.origin.x + ds.x, frame.origin.y + ds.y);
            btn.setFrame(frame);
        }
    }
    CATransaction::commit();
}

#[cfg(target_os = "macos")]
fn nudge_traffic_lights_by(ns_window: &objc2_app_kit::NSWindow, ds: objc2_foundation::NSPoint) {
    if ds.x.abs() < 0.1 && ds.y.abs() < 0.1 {
        return;
    }
    move_traffic_light_buttons_by(ns_window, ds);
    if let Ok(mut cumulative) = TRAFFIC_LIGHT_NUDGE_CUMULATIVE_SV.lock() {
        cumulative.0 += ds.x;
        cumulative.1 += ds.y;
    }
}

#[cfg(target_os = "macos")]
fn undo_traffic_light_nudge_cumulative(ns_window: &objc2_app_kit::NSWindow) {
    use objc2_foundation::NSPoint;

    let Ok(mut cumulative) = TRAFFIC_LIGHT_NUDGE_CUMULATIVE_SV.lock() else {
        return;
    };
    let (dx, dy) = *cumulative;
    if dx.abs() < 0.1 && dy.abs() < 0.1 {
        return;
    }
    move_traffic_light_buttons_by(ns_window, NSPoint::new(-dx, -dy));
    *cumulative = (0.0, 0.0);
}
