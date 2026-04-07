use crate::{
    app::StudioApp,
    document_session::DocumentKind,
    viewer_tab::ViewerUiOutcome,
    viewer_viewport,
    welcome::{self, WelcomeAction},
    macos_fused_titlebar,
    work_area_frame,
};
use scad_data::AppConfig;
use scad_ui::{
    document_tabs::{
        self, DocumentTabItem, DocumentTabKind, DocumentTabState, DocumentTabsResponse,
    },
    theme::palette,
};

pub fn show(
    ctx: &egui::Context,
    app: &mut StudioApp,
    config: &mut AppConfig,
    viewer_outcome: &mut Option<ViewerUiOutcome>,
) -> Option<WelcomeAction> {
    let mut action = None;

    if app.has_open_documents() {
        let m_top = palette::FLOATING_PANEL_MARGIN_TOP;
        let m_h = palette::FLOATING_PANEL_MARGIN_H;
        let gap_below = palette::TAB_STRIP_GAP_BELOW;
        let bar_total_h = f32::from(m_top) + document_tabs::rail_height() + gap_below;
        egui::TopBottomPanel::top("studio_tab_bar")
            .exact_height(bar_total_h)
            .show_separator_line(document_tabs::rail_show_separator_line())
            .frame(
                egui::Frame::NONE
                    .fill(document_tabs::rail_fill_color())
                    .inner_margin(egui::Margin {
                        left: m_h,
                        right: m_h,
                        top: m_top,
                        bottom: 0,
                    }),
            )
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let _ = viewer_viewport::allocate_filled_strip_ui(
                        ui,
                        egui::vec2(ui.available_width(), document_tabs::rail_height()),
                        document_tabs::rail_margin(),
                        document_tabs::rail_fill_color(),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            show_document_tab_bar(ui, app);
                        },
                    );
                    ui.allocate_space(egui::vec2(ui.available_width(), gap_below));
                });
            });
    }

    let viewer_active = app.active_viewer().is_some();
    egui::CentralPanel::default()
        .frame(work_area_frame::central_panel_frame(viewer_active))
        .show(ctx, |ui| {
            if app.show_welcome_state() {
                action = welcome::show_welcome(ui, app.recent_workspaces());
                return;
            }
            if let Some(viewer) = app.active_viewer_mut() {
                *viewer_outcome = Some(viewer.run_model_tab_frame(ctx, ui, config));
                return;
            }
            if let Some(markdown) = app.active_markdown_mut() {
                markdown.show_document(ui);
                return;
            }
            welcome::show_empty_workspace(ui, app.workspace_name().as_deref());
        });
    action
}

fn show_document_tab_bar(ui: &mut egui::Ui, app: &mut StudioApp) {
    let tabs = app.document_tabs();
    let items = tabs
        .iter()
        .map(|tab| DocumentTabItem {
            title: tab.title.as_str(),
            kind: match tab.kind {
                DocumentKind::Viewer => DocumentTabKind::Viewer,
                DocumentKind::Markdown => DocumentTabKind::Markdown,
            },
            active: tab.active,
            state: DocumentTabState::Normal,
            closable: true,
        })
        .collect::<Vec<_>>();

    let mut response = DocumentTabsResponse::default();
    ui.horizontal(|ui| {
        #[cfg(target_os = "macos")]
        {
            let total = ui.available_width();
            let drag_reserve = 32.0f32;
            let tabs_max = (total - drag_reserve).max(0.0);
            ui.scope(|ui| {
                ui.set_max_width(tabs_max);
                response = document_tabs::show_document_tabs(ui, &items);
            });
            macos_fused_titlebar::horizontal_drag_tail(ui, 8.0);
        }
        #[cfg(not(target_os = "macos"))]
        {
            response = document_tabs::show_document_tabs(ui, &items);
        }
    });

    if let Some(index) = response.activate {
        app.set_active_document(tabs[index].key.clone());
    }
    if let Some(index) = response.close {
        app.close_document(&tabs[index].key);
    }
}
