#![allow(dead_code)]

#[derive(Debug, Clone)]
enum UserEvent {
    Menu(String),
}

#[path = "../src/platform_menu.rs"]
mod platform_menu;

use platform_menu::{MenuCommand, resolve_menu_command};

#[test]
fn resolves_window_file_about_and_quit_commands() {
    assert_eq!(
        resolve_menu_command(
            "window.new",
            Some("window.new"),
            "file.open",
            Some("window.close"),
            Some("file.settings"),
            Some("app.about"),
            Some("app.quit")
        ),
        Some(MenuCommand::NewWindow)
    );
    assert_eq!(
        resolve_menu_command(
            "file.open",
            Some("window.new"),
            "file.open",
            Some("window.close"),
            Some("file.settings"),
            Some("app.about"),
            Some("app.quit")
        ),
        Some(MenuCommand::OpenFile)
    );
    assert_eq!(
        resolve_menu_command(
            "window.close",
            Some("window.new"),
            "file.open",
            Some("window.close"),
            Some("file.settings"),
            Some("app.about"),
            Some("app.quit")
        ),
        Some(MenuCommand::CloseWindow)
    );
    assert_eq!(
        resolve_menu_command(
            "file.settings",
            Some("window.new"),
            "file.open",
            Some("window.close"),
            Some("file.settings"),
            Some("app.about"),
            Some("app.quit")
        ),
        Some(MenuCommand::OpenSettings)
    );
    assert_eq!(
        resolve_menu_command(
            "app.about",
            Some("window.new"),
            "file.open",
            Some("window.close"),
            Some("file.settings"),
            Some("app.about"),
            Some("app.quit")
        ),
        Some(MenuCommand::ShowAbout)
    );
    assert_eq!(
        resolve_menu_command(
            "app.quit",
            Some("window.new"),
            "file.open",
            Some("window.close"),
            Some("file.settings"),
            Some("app.about"),
            Some("app.quit")
        ),
        Some(MenuCommand::QuitApp)
    );
}

#[test]
fn ignores_unknown_menu_ids() {
    assert_eq!(
        resolve_menu_command(
            "window.minimize",
            Some("window.new"),
            "file.open",
            Some("window.close"),
            Some("file.settings"),
            Some("app.about"),
            Some("app.quit")
        ),
        None
    );
}
