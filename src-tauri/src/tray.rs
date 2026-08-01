//! 系统托盘及其菜单行为。

use crate::runtime::SharedRuntime;
use crate::{
    model::{AppMode, LanguagePreference, ResolvedLocale},
    window_geometry::capture_window_state,
    window_manager,
};
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    App, AppHandle, Manager,
};

const SHOW_WINDOW_MENU_ID: &str = "show-window";
const SWITCH_MODE_MENU_ID: &str = "switch-mode";
const PET_LOCKED_MENU_ID: &str = "pet-locked";
const AUTO_START_MENU_ID: &str = "auto-start";
const QUIT_MENU_ID: &str = "quit";

/// 托盘中需要由其他入口同步的菜单状态。
pub(crate) struct TrayMenu {
    menu: Menu<tauri::Wry>,
    auto_start: CheckMenuItem<tauri::Wry>,
    show_window: MenuItem<tauri::Wry>,
    switch_mode: MenuItem<tauri::Wry>,
    pet_locked: CheckMenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
    locale: Arc<Mutex<ResolvedLocale>>,
}

impl TrayMenu {
    pub(crate) fn set_auto_start_checked(&self, enabled: bool) {
        let _ = self.auto_start.set_checked(enabled);
    }

    pub(crate) fn set_mode(&self, mode: AppMode) {
        let locale = *self.locale.lock().expect("tray locale lock poisoned");
        sync_mode_items(
            &self.menu,
            &self.show_window,
            &self.switch_mode,
            &self.pet_locked,
            mode,
            locale,
        );
    }

    pub(crate) fn set_pet_locked_checked(&self, locked: bool) {
        let _ = self.pet_locked.set_checked(locked);
    }

    pub(crate) fn set_language(&self, locale: ResolvedLocale, mode: AppMode) {
        *self.locale.lock().expect("tray locale lock poisoned") = locale;
        let english = locale == ResolvedLocale::English;
        let _ = self.auto_start.set_text(if english {
            "Launch at startup"
        } else {
            "开机自启"
        });
        let _ = self.pet_locked.set_text(if english {
            "Lock desktop pet"
        } else {
            "锁定桌宠"
        });
        let _ = self.quit.set_text(if english { "Quit" } else { "退出" });
        sync_mode_items(
            &self.menu,
            &self.show_window,
            &self.switch_mode,
            &self.pet_locked,
            mode,
            locale,
        );
    }
}

fn sync_mode_items(
    menu: &Menu<tauri::Wry>,
    show_window: &MenuItem<tauri::Wry>,
    switch_mode: &MenuItem<tauri::Wry>,
    pet_locked: &CheckMenuItem<tauri::Wry>,
    mode: AppMode,
    locale: ResolvedLocale,
) {
    let is_main = mode == AppMode::Main;
    let english = locale == ResolvedLocale::English;
    let _ = show_window.set_text(if english {
        "Show dashboard"
    } else {
        "显示看板"
    });
    let _ = switch_mode.set_text(if is_main && english {
        "Desktop pet mode"
    } else if is_main {
        "桌宠模式"
    } else if english {
        "Dashboard mode"
    } else {
        "看板模式"
    });
    let _ = menu.remove(show_window);
    let _ = menu.remove(pet_locked);
    if is_main {
        let _ = menu.insert(show_window, 1);
    } else {
        let _ = menu.insert(pet_locked, 1);
    }
    let _ = pet_locked.set_enabled(!is_main);
}

/// 第二实例启动时显示当前模式窗口。
pub fn show_active_window(app: &AppHandle<tauri::Wry>) {
    if let Some(runtime) = app.try_state::<SharedRuntime>() {
        let _ = window_manager::show_active_window(app, &runtime);
    }
}

/// 创建常驻系统托盘，以及“显示窗口”“开机自启”和“退出”菜单项。
pub fn setup(
    app: &mut App,
    runtime: SharedRuntime,
    auto_start_enabled: bool,
) -> tauri::Result<TrayMenu> {
    let show_window = MenuItem::with_id(app, SHOW_WINDOW_MENU_ID, "显示看板", true, None::<&str>)?;
    let switch_mode = MenuItem::with_id(app, SWITCH_MODE_MENU_ID, "桌宠模式", true, None::<&str>)?;
    let auto_start = CheckMenuItem::with_id(
        app,
        AUTO_START_MENU_ID,
        "开机自启",
        true,
        auto_start_enabled,
        None::<&str>,
    )?;
    let pet_locked = CheckMenuItem::with_id(
        app,
        PET_LOCKED_MENU_ID,
        "锁定桌宠",
        true,
        runtime.window_snapshot().pet_window.locked,
        None::<&str>,
    )?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, QUIT_MENU_ID, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show_window,
            &switch_mode,
            &pet_locked,
            &auto_start,
            &separator,
            &quit,
        ],
    )?;

    let auto_start_for_events = auto_start.clone();
    let runtime_for_events = runtime.clone();
    let show_for_events = show_window.clone();
    let switch_for_events = switch_mode.clone();
    let pet_locked_for_events = pet_locked.clone();
    let menu_for_events = menu.clone();
    let initial_locale = match runtime.snapshot().language {
        LanguagePreference::English => ResolvedLocale::English,
        LanguagePreference::System | LanguagePreference::Chinese => ResolvedLocale::Chinese,
    };
    let locale = Arc::new(Mutex::new(initial_locale));
    let locale_for_events = locale.clone();

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("AIMonitorDesktop")
        .on_menu_event(move |app, event| match event.id().as_ref() {
            SHOW_WINDOW_MENU_ID => {
                let _ = window_manager::show_active_window(app, &runtime_for_events);
            }
            SWITCH_MODE_MENU_ID => {
                let current = runtime_for_events.window_snapshot().active_mode;
                let target = match current {
                    AppMode::Main => AppMode::Pet,
                    AppMode::Pet => AppMode::Main,
                };
                if window_manager::switch_mode(app, &runtime_for_events, target).is_ok() {
                    sync_mode_items(
                        &menu_for_events,
                        &show_for_events,
                        &switch_for_events,
                        &pet_locked_for_events,
                        target,
                        *locale_for_events.lock().expect("tray locale lock poisoned"),
                    );
                }
            }
            AUTO_START_MENU_ID => {
                let previous = runtime_for_events.snapshot().auto_start;
                let enabled = auto_start_for_events.is_checked().unwrap_or(previous);
                if runtime_for_events.set_auto_start(enabled).is_err() {
                    let _ = auto_start_for_events.set_checked(previous);
                }
            }
            PET_LOCKED_MENU_ID => {
                let previous = runtime_for_events.window_snapshot().pet_window.locked;
                let locked = pet_locked_for_events.is_checked().unwrap_or(previous);
                if window_manager::set_pet_locked(&runtime_for_events, locked).is_err() {
                    let _ = pet_locked_for_events.set_checked(previous);
                }
            }
            QUIT_MENU_ID => {
                for label in ["main", "pet"] {
                    if let Some(window) = app.get_webview_window(label) {
                        capture_window_state(&window, &runtime_for_events);
                    }
                }
                runtime_for_events.save_preferences();
                app.exit(0);
            }
            _ => {}
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    let tray = TrayMenu {
        menu,
        auto_start,
        show_window,
        switch_mode,
        pet_locked,
        quit,
        locale,
    };
    tray.set_language(initial_locale, runtime.window_snapshot().active_mode);
    Ok(tray)
}
