use muda::{Menu, MenuEvent};
use winit::{
    event_loop::{EventLoopBuilder, EventLoopProxy},
    window::Window,
};

#[cfg(target_os = "macos")]
use winit::platform::macos::EventLoopBuilderExtMacOS;
#[cfg(target_os = "windows")]
use winit::platform::windows::EventLoopBuilderExtWindows;

pub fn configure_event_loop<T>(event_loop_builder: &mut EventLoopBuilder<T>, menu: &Menu) {
    let _ = menu;
    #[cfg(target_os = "macos")]
    event_loop_builder.with_default_menu(false);

    #[cfg(target_os = "windows")]
    {
        let menu = menu.clone();
        event_loop_builder.with_msg_hook(move |msg| {
            use windows_sys::Win32::UI::WindowsAndMessaging::{MSG, TranslateAcceleratorW};

            let msg = msg as *const MSG;
            unsafe { TranslateAcceleratorW((*msg).hwnd, menu.haccel() as _, msg) == 1 }
        });
    }
}

pub fn attach_menu_handler<Event: Send + Sync + 'static>(
    proxy: EventLoopProxy<Event>,
    map_event: impl Fn(String) -> Event + Send + Sync + 'static,
) {
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = proxy.send_event(map_event(event.id().as_ref().to_owned()));
    }));
}

pub fn install_native_menu(menu: &Menu, window: &Window) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        menu.init_for_nsapp();
        let _ = window;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        use winit::raw_window_handle::HasWindowHandle;

        let raw = window
            .window_handle()
            .map_err(|error| format!("获取窗口句柄失败: {error}"))?
            .as_raw();
        match raw {
            winit::raw_window_handle::RawWindowHandle::Win32(handle) => unsafe {
                menu
                    .init_for_hwnd(handle.hwnd.get() as _)
                    .map_err(|error| format!("安装 Windows 菜单失败: {error}"))
            },
            _ => Err("当前平台不支持的窗口句柄".into()),
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = (menu, window);
        Ok(())
    }
}
