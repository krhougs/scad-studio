#![allow(dead_code)]

#[derive(Debug, Clone)]
enum UserEvent {
    Menu(String),
}

#[path = "../src/platform_menu.rs"]
mod platform_menu;

use platform_menu::{MenuCommand, resolve_menu_command};

#[test]
fn resolves_open_about_and_quit_commands() {
    assert_eq!(
        resolve_menu_command(
            "file.open",
            "file.open",
            Some("file.settings"),
            Some("app.about"),
            Some("app.quit")
        ),
        Some(MenuCommand::OpenFile)
    );
    assert_eq!(
        resolve_menu_command(
            "file.settings",
            "file.open",
            Some("file.settings"),
            Some("app.about"),
            Some("app.quit")
        ),
        Some(MenuCommand::OpenSettings)
    );
    assert_eq!(
        resolve_menu_command(
            "app.about",
            "file.open",
            Some("file.settings"),
            Some("app.about"),
            Some("app.quit")
        ),
        Some(MenuCommand::ShowAbout)
    );
    assert_eq!(
        resolve_menu_command(
            "app.quit",
            "file.open",
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
            "file.open",
            Some("file.settings"),
            Some("app.about"),
            Some("app.quit")
        ),
        None
    );
}
