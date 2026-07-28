// —— 以下 #[tauri::command] 是前端读写 Rust 状态的统一入口 ——
// 写命令遵循“修改 Rust 状态 → 按需落盘 preferences → 广播 changed 事件”；读命令只返回快照。
use crate::model::{
    AppMode, ImageDisplayMode, LanguagePreference, MonitorState, PetLayout, ResolvedLocale,
    WindowState,
};
use crate::runtime::SharedRuntime;
use crate::tray::TrayMenu;
use crate::window_manager;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub(crate) fn get_monitor_state(runtime: State<'_, SharedRuntime>) -> MonitorState {
    runtime.snapshot() // 只读，直接返回当前状态快照
}

#[tauri::command]
pub(crate) fn set_grid(
    runtime: State<'_, SharedRuntime>,
    rows: u8,
    columns: u8,
) -> Result<(), String> {
    if !(1..=5).contains(&rows) || !(1..=5).contains(&columns) {
        return Err("行数和列数必须在 1–5 之间".into()); // 参数校验，行列数超界直接拒绝
    }
    {
        let mut state = runtime.state.write().map_err(|_| "状态不可用")?; // 获取写锁
        state.rows = rows; // 更新行数
        state.columns = columns; // 更新列数
    } // 写锁在此处离开作用域被释放，之后再落盘/广播，缩短持锁时间
    {
        // 行列数变小可能让桌宠原本聚焦的槽位越界（例如从 3×3 改成 2×2），收敛到新范围内。
        let mut windows = runtime.windows.lock().map_err(|_| "窗口状态不可用")?;
        windows.pet_window.focused_slot =
            window_manager::clamp_focused_slot(windows.pet_window.focused_slot, rows, columns);
    }
    runtime.save_preferences(); // 持久化到 preferences.json
    runtime.changed(); // 通知前端刷新
    runtime.window_changed();
    Ok(())
}

#[tauri::command]
pub(crate) fn set_image_display_mode(
    runtime: State<'_, SharedRuntime>,
    mode: ImageDisplayMode,
) -> Result<(), String> {
    runtime
        .state
        .write()
        .map_err(|_| "状态不可用")?
        .image_display_mode = mode; // 加写锁后直接赋值新的显示模式
    runtime.save_preferences(); // 持久化
    runtime.changed(); // 通知前端刷新
    Ok(())
}

#[tauri::command]
pub(crate) fn set_device_name(
    runtime: State<'_, SharedRuntime>,
    name: String,
) -> Result<(), String> {
    // 裁剪空白并限制长度，避免过长名称破坏 mDNS/局域网展示效果。
    let safe_name: String = name.trim().chars().take(40).collect(); // 去除首尾空白，最多保留 40 个字符
    if safe_name.is_empty() {
        return Err("设备名称不能为空".into()); // 清洗后为空说明原始输入全是空白，拒绝保存
    }
    runtime.state.write().map_err(|_| "状态不可用")?.device_name = safe_name; // 写入清洗后的名称
    runtime.save_preferences(); // 持久化
    runtime.changed(); // 通知前端刷新
    Ok(())
}

#[tauri::command]
pub(crate) fn set_auto_start(
    runtime: State<'_, SharedRuntime>,
    tray_menu: State<'_, TrayMenu>,
    enabled: bool,
) -> Result<(), String> {
    runtime.set_auto_start(enabled)?;
    tray_menu.set_auto_start_checked(enabled);
    Ok(())
}

#[tauri::command]
pub(crate) fn set_language(
    runtime: State<'_, SharedRuntime>,
    language: LanguagePreference,
) -> Result<(), String> {
    runtime.state.write().map_err(|_| "状态不可用")?.language = language;
    runtime.save_preferences();
    runtime.changed();
    Ok(())
}

#[tauri::command]
pub(crate) fn sync_language(
    app: AppHandle,
    runtime: State<'_, SharedRuntime>,
    tray_menu: State<'_, TrayMenu>,
    locale: ResolvedLocale,
) {
    tray_menu.set_language(locale, runtime.window_snapshot().active_mode);
    let english = locale == ResolvedLocale::English;
    for (label, title) in [
        ("main", "AIMonitorDesktop"),
        (
            "pet",
            if english {
                "AIMonitorDesktop Desktop Pet"
            } else {
                "AIMonitorDesktop 桌宠"
            },
        ),
        (
            "pet-settings",
            if english {
                "AIMonitorDesktop Desktop Pet Settings"
            } else {
                "AIMonitorDesktop 桌宠设置"
            },
        ),
    ] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.set_title(title);
        }
    }
}

#[tauri::command]
pub(crate) fn get_window_state(runtime: State<'_, SharedRuntime>) -> WindowState {
    runtime.window_snapshot() // 只读，pet_size_min/max 的显示器查询也在 window_snapshot 内部完成
}

#[tauri::command]
pub(crate) fn switch_app_mode(
    app: AppHandle,
    runtime: State<'_, SharedRuntime>,
    tray_menu: State<'_, TrayMenu>,
    mode: AppMode,
) -> Result<(), String> {
    window_manager::switch_mode(&app, &runtime, mode)?;
    tray_menu.set_mode(mode);
    Ok(())
}

#[tauri::command]
pub(crate) fn hide_current_window(
    app: AppHandle,
    runtime: State<'_, SharedRuntime>,
) -> Result<(), String> {
    window_manager::hide_active_window(&app, &runtime)
}

#[tauri::command]
pub(crate) fn show_pet_settings(app: AppHandle) -> Result<(), String> {
    window_manager::show_pet_settings(&app)
}

#[tauri::command]
pub(crate) fn hide_pet_settings(app: AppHandle) -> Result<(), String> {
    window_manager::hide_pet_settings(&app)
}

#[tauri::command]
pub(crate) fn set_pet_layout(
    app: AppHandle,
    runtime: State<'_, SharedRuntime>,
    layout: PetLayout,
) -> Result<(), String> {
    window_manager::set_pet_layout(&app, &runtime, layout)
}

#[tauri::command]
pub(crate) fn set_pet_focused_slot(
    runtime: State<'_, SharedRuntime>,
    slot: u8,
) -> Result<(), String> {
    window_manager::set_pet_focused_slot(&runtime, slot)
}

#[tauri::command]
pub(crate) fn set_pet_always_on_top(
    app: AppHandle,
    runtime: State<'_, SharedRuntime>,
    enabled: bool,
) -> Result<(), String> {
    window_manager::set_pet_always_on_top(&app, &runtime, enabled)
}

#[tauri::command]
pub(crate) fn set_pet_locked(
    runtime: State<'_, SharedRuntime>,
    tray_menu: State<'_, TrayMenu>,
    locked: bool,
) -> Result<(), String> {
    window_manager::set_pet_locked(&runtime, locked)?;
    tray_menu.set_pet_locked_checked(locked);
    Ok(())
}

#[tauri::command]
pub(crate) fn set_pet_size(
    app: AppHandle,
    runtime: State<'_, SharedRuntime>,
    size: u16,
) -> Result<(), String> {
    window_manager::set_pet_size(&app, &runtime, size)
}

#[tauri::command]
pub(crate) fn resize_pet_by(
    app: AppHandle,
    runtime: State<'_, SharedRuntime>,
    delta: i16,
) -> Result<(), String> {
    window_manager::resize_pet_by(&app, &runtime, delta)
}

#[tauri::command]
pub(crate) fn start_pet_drag(
    app: AppHandle,
    runtime: State<'_, SharedRuntime>,
) -> Result<(), String> {
    window_manager::start_pet_drag(&app, &runtime)
}
