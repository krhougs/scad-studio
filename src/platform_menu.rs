use muda::{
    accelerator::{Accelerator, CMD_OR_CTRL, Code},
    Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu,
};
use winit::{
    event_loop::{EventLoopBuilder, EventLoopProxy},
    window::Window,
};

#[cfg(target_os = "macos")]
use winit::platform::macos::EventLoopBuilderExtMacOS;
#[cfg(target_os = "windows")]
use winit::platform::windows::EventLoopBuilderExtWindows;

use crate::UserEvent;

pub const APP_NAME: &str = "scad-studio";

const OPEN_MENU_ID: &str = "file.open";
const ABOUT_MENU_ID: &str = "app.about";
const QUIT_MENU_ID: &str = "app.quit";
#[cfg(target_os = "windows")]
const EXIT_MENU_ID: &str = "file.exit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuCommand {
    OpenFile,
    ShowAbout,
    QuitApp,
}

#[derive(Clone)]
pub struct PlatformMenu {
    menu: Menu,
    open_menu_id: String,
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
        #[cfg(target_os = "macos")]
        event_loop_builder.with_default_menu(false);

        #[cfg(target_os = "windows")]
        {
            let menu = self.menu.clone();
            event_loop_builder.with_msg_hook(move |msg| {
                use windows_sys::Win32::UI::WindowsAndMessaging::{MSG, TranslateAcceleratorW};

                let msg = msg as *const MSG;
                unsafe { TranslateAcceleratorW((*msg).hwnd, menu.haccel() as _, msg) == 1 }
            });
        }
    }

    pub fn attach_event_handler(&self, proxy: EventLoopProxy<UserEvent>) {
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let _ = proxy.send_event(UserEvent::Menu(event.id().as_ref().to_owned()));
        }));
    }

    pub fn install(&self, window: &Window) -> Result<(), String> {
        install_native_menu(&self.menu, window)
    }

    pub fn command_for_event(&self, id: &str) -> Option<MenuCommand> {
        resolve_menu_command(
            id,
            &self.open_menu_id,
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
    open_menu_id: &str,
    about_menu_id: Option<&str>,
    quit_menu_id: Option<&str>,
) -> Option<MenuCommand> {
    if id == open_menu_id {
        return Some(MenuCommand::OpenFile);
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
    let open_item = MenuItem::with_id(
        OPEN_MENU_ID,
        "Open...",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyO)),
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
        .append(&open_item)
        .expect("构建 macOS File 菜单失败");
    menu.append_items(&[&app_menu, &file_menu])
        .expect("挂载 macOS 菜单栏失败");

    PlatformMenu {
        menu,
        open_menu_id: open_item.id().as_ref().to_owned(),
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

    let open_item = MenuItem::with_id(
        OPEN_MENU_ID,
        "&Open...",
        true,
        Some(Accelerator::new(Some(CMD_OR_CTRL), Code::KeyO)),
    );
    let exit_item = MenuItem::with_id(
        EXIT_MENU_ID,
        "E&xit",
        true,
        Some(Accelerator::new(Some(Modifiers::ALT), Code::F4)),
    );
    let about_item = MenuItem::with_id(
        ABOUT_MENU_ID,
        format!("&About {APP_NAME}"),
        true,
        None::<Accelerator>,
    );
    let separator = PredefinedMenuItem::separator();

    file_menu
        .append_items(&[&open_item, &separator, &exit_item])
        .expect("构建 Windows File 菜单失败");
    help_menu
        .append(&about_item)
        .expect("构建 Windows Help 菜单失败");
    menu.append_items(&[&file_menu, &help_menu])
        .expect("挂载 Windows 菜单栏失败");

    PlatformMenu {
        menu,
        open_menu_id: open_item.id().as_ref().to_owned(),
        about_menu_id: Some(about_item.id().as_ref().to_owned()),
        quit_menu_id: Some(exit_item.id().as_ref().to_owned()),
    }
}

#[cfg(target_os = "macos")]
fn install_native_menu(menu: &Menu, _window: &Window) -> Result<(), String> {
    menu.init_for_nsapp();
    Ok(())
}

#[cfg(target_os = "windows")]
fn install_native_menu(menu: &Menu, window: &Window) -> Result<(), String> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = window
        .window_handle()
        .map_err(|error| format!("获取窗口句柄失败: {error}"))?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err("当前平台不是 Win32 窗口句柄".into());
    };
    let hwnd = handle.hwnd.get();
    unsafe {
        menu.init_for_hwnd(hwnd)
            .map_err(|error| format!("初始化 Windows 菜单失败: {error}"))?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_native_menu(_menu: &Menu, _window: &Window) -> Result<(), String> {
    Ok(())
}
