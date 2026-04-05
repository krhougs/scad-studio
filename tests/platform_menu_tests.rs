#![allow(dead_code)]

#[derive(Debug, Clone)]
enum UserEvent {
    Menu(String),
}

#[path = "../src/platform_menu.rs"]
mod platform_menu;

use platform_menu::{CommandIds, MenuCommand, resolve_menu_command};
use std::path::PathBuf;

#[test]
fn resolves_window_and_workspace_commands() {
    let recent = vec![("file.recent.0".to_string(), PathBuf::from("/tmp/workspace-a"))];

    assert!(matches!(
        resolve_menu_command(
            "file.new-window",
            CommandIds {
                new_window_id: "file.new-window",
                open_folder_id: "file.open-folder",
                close_window_id: "file.close-window",
                toggle_left_panel_id: "view.toggle-left-panel",
                toggle_log_panel_id: "view.toggle-log-panel",
                about_id: Some("app.about"),
                quit_id: Some("app.quit"),
            },
            &recent,
        ),
        Some(MenuCommand::NewWindow)
    ));
    assert!(matches!(
        resolve_menu_command(
            "file.open-folder",
            CommandIds {
                new_window_id: "file.new-window",
                open_folder_id: "file.open-folder",
                close_window_id: "file.close-window",
                toggle_left_panel_id: "view.toggle-left-panel",
                toggle_log_panel_id: "view.toggle-log-panel",
                about_id: Some("app.about"),
                quit_id: Some("app.quit"),
            },
            &recent,
        ),
        Some(MenuCommand::OpenFolder)
    ));
    assert!(matches!(
        resolve_menu_command(
            "file.close-window",
            CommandIds {
                new_window_id: "file.new-window",
                open_folder_id: "file.open-folder",
                close_window_id: "file.close-window",
                toggle_left_panel_id: "view.toggle-left-panel",
                toggle_log_panel_id: "view.toggle-log-panel",
                about_id: Some("app.about"),
                quit_id: Some("app.quit"),
            },
            &recent,
        ),
        Some(MenuCommand::CloseWindow)
    ));
}

#[test]
fn resolves_recent_and_view_commands() {
    let recent = vec![("file.recent.0".to_string(), PathBuf::from("/tmp/workspace-a"))];

    assert!(matches!(
        resolve_menu_command(
            "file.recent.0",
            CommandIds {
                new_window_id: "file.new-window",
                open_folder_id: "file.open-folder",
                close_window_id: "file.close-window",
                toggle_left_panel_id: "view.toggle-left-panel",
                toggle_log_panel_id: "view.toggle-log-panel",
                about_id: Some("app.about"),
                quit_id: Some("app.quit"),
            },
            &recent,
        ),
        Some(MenuCommand::OpenRecent(path)) if path == std::path::Path::new("/tmp/workspace-a")
    ));
    assert!(matches!(
        resolve_menu_command(
            "view.toggle-left-panel",
            CommandIds {
                new_window_id: "file.new-window",
                open_folder_id: "file.open-folder",
                close_window_id: "file.close-window",
                toggle_left_panel_id: "view.toggle-left-panel",
                toggle_log_panel_id: "view.toggle-log-panel",
                about_id: Some("app.about"),
                quit_id: Some("app.quit"),
            },
            &recent,
        ),
        Some(MenuCommand::ToggleLeftPanel)
    ));
    assert!(matches!(
        resolve_menu_command(
            "view.toggle-log-panel",
            CommandIds {
                new_window_id: "file.new-window",
                open_folder_id: "file.open-folder",
                close_window_id: "file.close-window",
                toggle_left_panel_id: "view.toggle-left-panel",
                toggle_log_panel_id: "view.toggle-log-panel",
                about_id: Some("app.about"),
                quit_id: Some("app.quit"),
            },
            &recent,
        ),
        Some(MenuCommand::ToggleLogPanel)
    ));
}
