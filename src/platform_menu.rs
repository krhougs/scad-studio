use std::path::PathBuf;

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

pub const APP_NAME: &str = "SCAD Studio";

const NEW_WINDOW_MENU_ID: &str = "file.new-window";
const OPEN_FOLDER_MENU_ID: &str = "file.open-folder";
const CLOSE_WINDOW_MENU_ID: &str = "file.close-window";
const TOGGLE_LEFT_PANEL_ID: &str = "view.toggle-left-panel";
const TOGGLE_LOG_PANEL_ID: &str = "view.toggle-log-panel";
const ABOUT_MENU_ID: &str = "app.about";
const QUIT_MENU_ID: &str = "app.quit";

#[derive(Debug, Clone)]
pub enum MenuCommand {
    NewWindow,
    OpenFolder,
    OpenRecent(PathBuf),
    CloseWindow,
    ToggleLeftPanel,
    ToggleLogPanel,
    ShowAbout,
    QuitApp,
}

#[derive(Clone)]
pub struct PlatformMenu {
    menu: Menu,
    new_window_id: String,
    open_folder_id: String,
    close_window_id: String,
    toggle_left_panel_id: String,
    toggle_log_panel_id: String,
    about_id: Option<String>,
    quit_id: Option<String>,
    recent_items: Vec<(String, PathBuf)>,
}

pub struct CommandIds<'a> {
    pub new_window_id: &'a str,
    pub open_folder_id: &'a str,
    pub close_window_id: &'a str,
    pub toggle_left_panel_id: &'a str,
    pub toggle_log_panel_id: &'a str,
    pub about_id: Option<&'a str>,
    pub quit_id: Option<&'a str>,
}

impl PlatformMenu {
    pub fn new(recent: &[PathBuf]) -> Option<Self> {
        #[cfg(target_os = "linux")]
        {
            None
        }
        #[cfg(not(target_os = "linux"))]
        {
            Some(build_menu(recent))
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
            CommandIds {
                new_window_id: &self.new_window_id,
                open_folder_id: &self.open_folder_id,
                close_window_id: &self.close_window_id,
                toggle_left_panel_id: &self.toggle_left_panel_id,
                toggle_log_panel_id: &self.toggle_log_panel_id,
                about_id: self.about_id.as_deref(),
                quit_id: self.quit_id.as_deref(),
            },
            &self.recent_items,
        )
    }
}

pub(crate) fn resolve_menu_command(
    id: &str,
    ids: CommandIds<'_>,
    recent_items: &[(String, PathBuf)],
) -> Option<MenuCommand> {
    if id == ids.new_window_id {
        return Some(MenuCommand::NewWindow);
    }
    if id == ids.open_folder_id {
        return Some(MenuCommand::OpenFolder);
    }
    if id == ids.close_window_id {
        return Some(MenuCommand::CloseWindow);
    }
    if id == ids.toggle_left_panel_id {
        return Some(MenuCommand::ToggleLeftPanel);
    }
    if id == ids.toggle_log_panel_id {
        return Some(MenuCommand::ToggleLogPanel);
    }
    if let Some((_, path)) = recent_items.iter().find(|(item_id, _)| item_id == id) {
        return Some(MenuCommand::OpenRecent(path.clone()));
    }
    if ids.about_id == Some(id) {
        return Some(MenuCommand::ShowAbout);
    }
    if ids.quit_id == Some(id) {
        return Some(MenuCommand::QuitApp);
    }
    None
}

fn build_menu(recent: &[PathBuf]) -> PlatformMenu {
    let menu = Menu::new();
    let file_menu = Submenu::new("File", true);
    let view_menu = Submenu::new("View", true);
    let help_menu = Submenu::new("Help", true);

    let new_window_item = MenuItem::with_id(
        NEW_WINDOW_MENU_ID,
        "New Window",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyN)),
    );
    let open_folder_item = MenuItem::with_id(
        OPEN_FOLDER_MENU_ID,
        "Open Folder...",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyO)),
    );
    let close_window_item = MenuItem::with_id(
        CLOSE_WINDOW_MENU_ID,
        "Close Window",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyW)),
    );
    let toggle_left_panel_item = MenuItem::with_id(
        TOGGLE_LEFT_PANEL_ID,
        "Toggle Left Panel",
        true,
        None::<Accelerator>,
    );
    let toggle_log_panel_item = MenuItem::with_id(
        TOGGLE_LOG_PANEL_ID,
        "Toggle Log Panel",
        true,
        None::<Accelerator>,
    );
    let recent_menu = build_recent_menu(recent);
    let about_item =
        MenuItem::with_id(ABOUT_MENU_ID, format!("About {APP_NAME}"), true, None::<Accelerator>);
    let quit_item = MenuItem::with_id(
        QUIT_MENU_ID,
        format!("Quit {APP_NAME}"),
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyQ)),
    );
    let separator = PredefinedMenuItem::separator();

    let mut file_items: Vec<&dyn muda::IsMenuItem> = vec![&new_window_item, &open_folder_item];
    if let Some(submenu) = recent_menu.0.as_ref() {
        file_items.push(submenu);
    }
    file_items.push(&separator);
    file_items.push(&close_window_item);
    file_items.push(&separator);
    file_items.push(&quit_item);
    file_menu
        .append_items(&file_items)
        .expect("构建 Studio File 菜单失败");
    view_menu
        .append_items(&[&toggle_left_panel_item, &toggle_log_panel_item])
        .expect("构建 Studio View 菜单失败");
    help_menu
        .append(&about_item)
        .expect("构建 Studio Help 菜单失败");
    menu.append_items(&[&file_menu, &view_menu, &help_menu])
        .expect("挂载 Studio 菜单栏失败");

    PlatformMenu {
        menu,
        new_window_id: new_window_item.id().as_ref().to_owned(),
        open_folder_id: open_folder_item.id().as_ref().to_owned(),
        close_window_id: close_window_item.id().as_ref().to_owned(),
        toggle_left_panel_id: toggle_left_panel_item.id().as_ref().to_owned(),
        toggle_log_panel_id: toggle_log_panel_item.id().as_ref().to_owned(),
        about_id: Some(about_item.id().as_ref().to_owned()),
        quit_id: Some(quit_item.id().as_ref().to_owned()),
        recent_items: recent_menu.1,
    }
}

fn build_recent_menu(recent: &[PathBuf]) -> (Option<Submenu>, Vec<(String, PathBuf)>) {
    if recent.is_empty() {
        return (None, Vec::new());
    }
    let submenu = Submenu::new("Recent Workspaces", true);
    let mut items = Vec::new();
    for (index, path) in recent.iter().enumerate() {
        let id = format!("file.recent.{index}");
        let label = path.display().to_string();
        let item = MenuItem::with_id(&id, label, true, None::<Accelerator>);
        submenu.append(&item).expect("添加最近工作区菜单失败");
        items.push((item.id().as_ref().to_owned(), path.clone()));
    }
    (Some(submenu), items)
}
