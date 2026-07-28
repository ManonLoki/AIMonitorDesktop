//! 系统托盘及其菜单行为。

use crate::runtime::SharedRuntime;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    App, AppHandle, Manager, Runtime,
};

const SHOW_WINDOW_MENU_ID: &str = "show-window";
const AUTO_START_MENU_ID: &str = "auto-start";
const QUIT_MENU_ID: &str = "quit";

/// 托盘中需要由其他入口同步的菜单状态。
pub(crate) struct TrayMenu {
    auto_start: CheckMenuItem<tauri::Wry>,
}

impl TrayMenu {
    pub(crate) fn set_auto_start_checked(&self, enabled: bool) {
        let _ = self.auto_start.set_checked(enabled);
    }
}

/// 显示并聚焦主窗口；窗口被最小化或关闭后隐藏时均可恢复。
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 创建常驻系统托盘，以及“显示窗口”“开机自启”和“退出”菜单项。
pub fn setup(
    app: &mut App,
    runtime: SharedRuntime,
    auto_start_enabled: bool,
) -> tauri::Result<TrayMenu> {
    let show_window = MenuItem::with_id(app, SHOW_WINDOW_MENU_ID, "显示窗口", true, None::<&str>)?;
    let auto_start = CheckMenuItem::with_id(
        app,
        AUTO_START_MENU_ID,
        "开机自启",
        true,
        auto_start_enabled,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, QUIT_MENU_ID, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_window, &auto_start, &separator, &quit])?;

    let auto_start_for_events = auto_start.clone();
    let runtime_for_events = runtime.clone();

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("AIMonitorDesktop")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            SHOW_WINDOW_MENU_ID => show_main_window(app),
            AUTO_START_MENU_ID => {
                let previous = runtime_for_events.snapshot().auto_start;
                let enabled = auto_start_for_events.is_checked().unwrap_or(previous);
                if runtime_for_events.set_auto_start(enabled).is_err() {
                    let _ = auto_start_for_events.set_checked(previous);
                }
            }
            QUIT_MENU_ID => app.exit(0),
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(TrayMenu { auto_start })
}
