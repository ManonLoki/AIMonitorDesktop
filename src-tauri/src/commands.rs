// —— 以下 #[tauri::command] 是前端读写 Rust 状态的统一入口 ——
// 写命令遵循“修改 Rust 状态 → 按需落盘 preferences → 广播 changed 事件”；读命令只返回快照。
use crate::model::{
    AppMode, ImageDisplayMode, LanguagePreference, MonitorState, PetLayout, ResolvedLocale,
    WindowState,
};
use crate::pet_paging::{
    build_pet_view_state, clamp_focused_slot, first_populated_focus, focused_slot_after_turn,
    PageDirection, PetViewState,
};
use crate::runtime::SharedRuntime;
use crate::tray::TrayMenu;
use crate::window_manager;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub(crate) fn get_monitor_state(runtime: State<'_, SharedRuntime>) -> MonitorState {
    runtime.snapshot() // 只读，直接返回当前状态快照
}

fn validate_grid_axis(value: Option<u8>) -> Result<(), String> {
    if value.is_some_and(|value| !(1..=5).contains(&value)) {
        return Err("行数和列数必须在 1–5 之间".into());
    }
    Ok(())
}

/// 原子更新宫格维度与依赖它的桌宠焦点；加锁顺序始终为 state -> windows。
fn patch_grid(
    runtime: &SharedRuntime,
    requested_rows: Option<u8>,
    requested_columns: Option<u8>,
) -> Result<(), String> {
    validate_grid_axis(requested_rows)?;
    validate_grid_axis(requested_columns)?;
    {
        let mut state = runtime.state.write().map_err(|_| "状态不可用")?;
        let rows = requested_rows.unwrap_or(state.rows);
        let columns = requested_columns.unwrap_or(state.columns);
        let mut windows = runtime.windows.lock().map_err(|_| "窗口状态不可用")?;
        state.rows = rows;
        state.columns = columns;
        windows.pet_window.focused_slot =
            clamp_focused_slot(windows.pet_window.focused_slot, state.rows, state.columns);
    }
    // 锁已全部释放，再落盘和发事件，避免事件回调重入时死锁。
    runtime.save_preferences();
    runtime.changed();
    runtime.window_changed();
    Ok(())
}

#[tauri::command]
pub(crate) fn set_grid(
    runtime: State<'_, SharedRuntime>,
    rows: u8,
    columns: u8,
) -> Result<(), String> {
    patch_grid(&runtime, Some(rows), Some(columns))
}

#[tauri::command]
pub(crate) fn set_grid_rows(runtime: State<'_, SharedRuntime>, rows: u8) -> Result<(), String> {
    patch_grid(&runtime, Some(rows), None)
}

#[tauri::command]
pub(crate) fn set_grid_columns(
    runtime: State<'_, SharedRuntime>,
    columns: u8,
) -> Result<(), String> {
    patch_grid(&runtime, None, Some(columns))
}

#[tauri::command]
pub(crate) fn get_pet_view_state(
    runtime: State<'_, SharedRuntime>,
) -> Result<PetViewState, String> {
    // 同时持有两份读取所需的锁，避免组装出不同时刻的混合快照。
    let state = runtime.state.read().map_err(|_| "状态不可用")?;
    let windows = runtime.windows.lock().map_err(|_| "窗口状态不可用")?;
    Ok(build_pet_view_state(&state, &windows.pet_window))
}

fn patch_pet_focus(
    runtime: &SharedRuntime,
    select: impl FnOnce(&MonitorState, &crate::model::PetWindowPreferences) -> Option<u8>,
) -> Result<bool, String> {
    let changed = {
        let state = runtime.state.read().map_err(|_| "状态不可用")?;
        let mut windows = runtime.windows.lock().map_err(|_| "窗口状态不可用")?;
        let Some(selected) = select(&state, &windows.pet_window) else {
            return Ok(false);
        };
        let selected = clamp_focused_slot(selected, state.rows, state.columns);
        if windows.pet_window.focused_slot == selected {
            false
        } else {
            windows.pet_window.focused_slot = selected;
            true
        }
    };
    if changed {
        runtime.save_preferences();
        runtime.window_changed();
    }
    Ok(changed)
}

#[tauri::command]
pub(crate) fn turn_pet_page(
    runtime: State<'_, SharedRuntime>,
    direction: PageDirection,
) -> Result<(), String> {
    patch_pet_focus(&runtime, |state, preferences| {
        Some(focused_slot_after_turn(
            preferences.layout,
            state.rows,
            state.columns,
            preferences.focused_slot,
            direction,
        ))
    })?;
    Ok(())
}

#[tauri::command]
pub(crate) fn focus_first_populated_pet_page(
    runtime: State<'_, SharedRuntime>,
) -> Result<(), String> {
    patch_pet_focus(&runtime, |state, preferences| {
        first_populated_focus(
            &state.tiles,
            preferences.layout,
            state.rows,
            state.columns,
            preferences.focused_slot,
        )
    })?;
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
    patch_pet_focus(&runtime, |_state, _preferences| Some(slot))?;
    Ok(())
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
pub(crate) fn resize_pet_step(
    app: AppHandle,
    runtime: State<'_, SharedRuntime>,
    direction: window_manager::PetResizeDirection,
) -> Result<(), String> {
    window_manager::resize_pet_step(&app, &runtime, direction)
}

#[tauri::command]
pub(crate) fn start_pet_drag(
    app: AppHandle,
    runtime: State<'_, SharedRuntime>,
) -> Result<(), String> {
    window_manager::start_pet_drag(&app, &runtime)
}
