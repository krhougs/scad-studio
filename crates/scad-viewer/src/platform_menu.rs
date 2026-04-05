use muda::{
    Menu, MenuItem, PredefinedMenuItem, Submenu,
    accelerator::{Accelerator, CMD_OR_CTRL, Code},
};
use winit::{
    event_loop::{EventLoopBuilder, EventLoopProxy},
    window::Window,
};

use crate::UserEvent;
use scad_ui::platform_support;

pub const APP_NAME: &str = "scad-studio";

const NEW_WINDOW_MENU_ID: &str = "window.new";
const OPEN_MENU_ID: &str = "file.open";
const CLOSE_WINDOW_MENU_ID: &str = "window.close";
const SETTINGS_MENU_ID: &str = "file.settings";
const ABOUT_MENU_ID: &str = "app.about";
const QUIT_MENU_ID: &str = "app.quit";
#[cfg(target_os = "windows")]
const EXIT_MENU_ID: &str = "file.exit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    NewWindow,
    OpenFile,
    CloseWindow,
    OpenSettings,
    ShowAbout,
    QuitApp,
}

#[derive(Clone)]
pub struct PlatformMenu {
    menu: Menu,
    new_window_menu_id: Option<String>,
    open_menu_id: String,
    close_window_menu_id: Option<String>,
    settings_menu_id: Option<String>,
    about_menu_id: Option<String>,
    quit_menu_id: Option<String>,
}

impl PlatformMenu {
    pub fn new() -> Option<Self> {
        #[cfg(target_os = "linux")]
        {
            None
        }

        #[cfg(not(target_os = "linux"))]
        {
            Some(Self::build_native())
        }
    }

    pub fn configure_event_loop<T>(&self, event_loop_builder: &mut EventLoopBuilder<T>) {
        platform_support::configure_event_loop(event_loop_builder, &self.menu);
    }

    pub fn attach_event_handler(&self, proxy: EventLoopProxy<UserEvent>) {
        platform_support::attach_menu_handler(proxy, UserEvent::Menu);
    }

    pub fn install(&self, window: &Window) -> Result<(), String> {
        platform_support::install_native_menu(&self.menu, window)
    }

    pub fn command_for_event(&self, id: &str) -> Option<MenuCommand> {
        resolve_menu_command(
            id,
            self.new_window_menu_id.as_deref(),
            &self.open_menu_id,
            self.close_window_menu_id.as_deref(),
            self.settings_menu_id.as_deref(),
            self.about_menu_id.as_deref(),
            self.quit_menu_id.as_deref(),
        )
    }

    #[cfg(not(target_os = "linux"))]
    fn build_native() -> Self {
        #[cfg(target_os = "macos")]
        {
            build_macos_menu()
        }

        #[cfg(target_os = "windows")]
        {
            build_windows_menu()
        }
    }
}

pub(crate) fn resolve_menu_command(
    id: &str,
    new_window_menu_id: Option<&str>,
    open_menu_id: &str,
    close_window_menu_id: Option<&str>,
    settings_menu_id: Option<&str>,
    about_menu_id: Option<&str>,
    quit_menu_id: Option<&str>,
) -> Option<MenuCommand> {
    if new_window_menu_id.is_some_and(|menu_id| id == menu_id) {
        return Some(MenuCommand::NewWindow);
    }
    if id == open_menu_id {
        return Some(MenuCommand::OpenFile);
    }
    if close_window_menu_id.is_some_and(|menu_id| id == menu_id) {
        return Some(MenuCommand::CloseWindow);
    }
    if settings_menu_id.is_some_and(|menu_id| id == menu_id) {
        return Some(MenuCommand::OpenSettings);
    }
    if about_menu_id.is_some_and(|menu_id| id == menu_id) {
        return Some(MenuCommand::ShowAbout);
    }
    if quit_menu_id.is_some_and(|menu_id| id == menu_id) {
        return Some(MenuCommand::QuitApp);
    }
    None
}

#[cfg(target_os = "macos")]
fn build_macos_menu() -> PlatformMenu {
    let menu = Menu::new();
    let app_menu = Submenu::new(APP_NAME, true);
    let file_menu = Submenu::new("File", true);

    let about_item = MenuItem::with_id(
        ABOUT_MENU_ID,
        format!("关于 {APP_NAME}"),
        true,
        None::<Accelerator>,
    );
    let new_window_item = MenuItem::with_id(
        NEW_WINDOW_MENU_ID,
        "New Window",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyN)),
    );
    let open_item = MenuItem::with_id(
        OPEN_MENU_ID,
        "Open...",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyO)),
    );
    let close_window_item = MenuItem::with_id(
        CLOSE_WINDOW_MENU_ID,
        "Close Window",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyW)),
    );
    let settings_item = MenuItem::with_id(
        SETTINGS_MENU_ID,
        "设置...",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::Comma)),
    );
    let quit_item = MenuItem::with_id(
        QUIT_MENU_ID,
        format!("退出 {APP_NAME}"),
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyQ)),
    );
    let separator = PredefinedMenuItem::separator();

    app_menu
        .append_items(&[&about_item, &separator, &quit_item])
        .expect("构建 macOS App 菜单失败");
    file_menu
        .append_items(&[
            &new_window_item,
            &open_item,
            &close_window_item,
            &settings_item,
        ])
        .expect("构建 macOS File 菜单失败");
    menu.append_items(&[&app_menu, &file_menu])
        .expect("挂载 macOS 菜单栏失败");

    PlatformMenu {
        menu,
        new_window_menu_id: Some(new_window_item.id().as_ref().to_owned()),
        open_menu_id: open_item.id().as_ref().to_owned(),
        close_window_menu_id: Some(close_window_item.id().as_ref().to_owned()),
        settings_menu_id: Some(settings_item.id().as_ref().to_owned()),
        about_menu_id: Some(about_item.id().as_ref().to_owned()),
        quit_menu_id: Some(quit_item.id().as_ref().to_owned()),
    }
}

#[cfg(target_os = "windows")]
fn build_windows_menu() -> PlatformMenu {
    use muda::accelerator::Modifiers;

    let menu = Menu::new();
    let file_menu = Submenu::new("&File", true);
    let help_menu = Submenu::new("&Help", true);

    let new_window_item = MenuItem::with_id(
        NEW_WINDOW_MENU_ID,
        "&New Window",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyN)),
    );
    let open_item = MenuItem::with_id(
        OPEN_MENU_ID,
        "&Open...",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyO)),
    );
    let close_window_item = MenuItem::with_id(
        CLOSE_WINDOW_MENU_ID,
        "&Close Window",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyW)),
    );
    let settings_item = MenuItem::with_id(
        SETTINGS_MENU_ID,
        "&Settings...",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::Comma)),
    );
    let exit_item = MenuItem::with_id(
        EXIT_MENU_ID,
        "E&xit",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyQ)),
    );
    let about_item = MenuItem::with_id(
        ABOUT_MENU_ID,
        format!("&About {APP_NAME}"),
        true,
        None::<Accelerator>,
    );
    let separator = PredefinedMenuItem::separator();

    file_menu
        .append_items(&[
            &new_window_item,
            &open_item,
            &close_window_item,
            &settings_item,
            &separator,
            &exit_item,
        ])
        .expect("构建 Windows File 菜单失败");
    help_menu
        .append(&about_item)
        .expect("构建 Windows Help 菜单失败");
    menu.append_items(&[&file_menu, &help_menu])
        .expect("挂载 Windows 菜单栏失败");

    PlatformMenu {
        menu,
        new_window_menu_id: Some(new_window_item.id().as_ref().to_owned()),
        open_menu_id: open_item.id().as_ref().to_owned(),
        close_window_menu_id: Some(close_window_item.id().as_ref().to_owned()),
        settings_menu_id: Some(settings_item.id().as_ref().to_owned()),
        about_menu_id: Some(about_item.id().as_ref().to_owned()),
        quit_menu_id: Some(exit_item.id().as_ref().to_owned()),
    }
}
