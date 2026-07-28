//! 两个互斥窗口的显示、模式切换与桌宠交互。

use crate::model::{AppMode, PetLayout, PetWindowPreferences};
use crate::runtime::SharedRuntime;
use crate::window_geometry::{
    apply_pet_constraints, capture_window_state, restore_main_window, restore_pet_window,
    PET_PAGER_HEIGHT,
};
use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition};

const MAIN_LABEL: &str = "main";
const PET_LABEL: &str = "pet";
const PET_SETTINGS_LABEL: &str = "pet-settings";

fn label_for_mode(mode: AppMode) -> &'static str {
    match mode {
        AppMode::Main => MAIN_LABEL,
        AppMode::Pet => PET_LABEL,
    }
}

fn pet_geometry(preferences: &PetWindowPreferences) -> Option<&crate::model::WindowGeometry> {
    match preferences.layout {
        PetLayout::Single => preferences.single_geometry.as_ref(),
        PetLayout::Grid => preferences.grid_geometry.as_ref(),
    }
}

fn prepare_window(
    app: &AppHandle,
    runtime: &SharedRuntime,
    mode: AppMode,
) -> Result<tauri::WebviewWindow, String> {
    let window = app
        .get_webview_window(label_for_mode(mode))
        .ok_or_else(|| "目标窗口不存在".to_string())?;
    let windows = runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用".to_string())?
        .clone();
    match mode {
        AppMode::Main => restore_main_window(&window, &windows.main_window),
        AppMode::Pet => {
            window
                .set_always_on_top(windows.pet_window.always_on_top)
                .map_err(|error| error.to_string())?;
            restore_pet_window(
                &window,
                pet_geometry(&windows.pet_window),
                windows.pet_window.layout,
                windows.pet_window.pet_size,
            )
        }
    }
    .map_err(|error| error.to_string())?;
    Ok(window)
}

pub(crate) fn show_active_window(app: &AppHandle, runtime: &SharedRuntime) -> Result<(), String> {
    let mode = runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用".to_string())?
        .active_mode;
    let window = prepare_window(app, runtime, mode)?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

pub(crate) fn switch_mode(
    app: &AppHandle,
    runtime: &SharedRuntime,
    target: AppMode,
) -> Result<(), String> {
    let source_mode = runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用".to_string())?
        .active_mode;
    if source_mode == target {
        return show_active_window(app, runtime);
    }
    let source = app.get_webview_window(label_for_mode(source_mode));
    if let Some(source) = source.as_ref() {
        capture_window_state(source, runtime);
    }
    let target_window = prepare_window(app, runtime, target)?;
    if let Some(settings) = app.get_webview_window(PET_SETTINGS_LABEL) {
        let _ = settings.hide();
    }
    if let Some(source) = source.as_ref() {
        source.hide().map_err(|error| error.to_string())?;
    }
    let show_result = target_window
        .show()
        .and_then(|_| target_window.set_focus())
        .map_err(|error| error.to_string());
    if let Err(error) = show_result {
        if let Some(source) = source {
            let _ = source.show();
            let _ = source.set_focus();
        }
        return Err(error);
    }
    runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用".to_string())?
        .active_mode = target;
    runtime.save_preferences();
    runtime.window_changed();
    Ok(())
}

pub(crate) fn hide_active_window(app: &AppHandle, runtime: &SharedRuntime) -> Result<(), String> {
    let mode = runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用".to_string())?
        .active_mode;
    let window = app
        .get_webview_window(label_for_mode(mode))
        .ok_or_else(|| "当前窗口不存在".to_string())?;
    capture_window_state(&window, runtime);
    runtime.save_preferences();
    window.hide().map_err(|error| error.to_string())
}

pub(crate) fn show_pet_settings(app: &AppHandle) -> Result<(), String> {
    let pet = app
        .get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?;
    let monitor = pet
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or_else(|| pet.primary_monitor().ok().flatten())
        .ok_or_else(|| "无法确定桌宠所在显示器".to_string())?;
    let window = app
        .get_webview_window(PET_SETTINGS_LABEL)
        .ok_or_else(|| "桌宠设置窗口不存在".to_string())?;
    let area = monitor.work_area();
    let size = window.outer_size().map_err(|error| error.to_string())?;
    let x = i64::from(area.position.x)
        + (i64::from(area.size.width).saturating_sub(i64::from(size.width)) / 2);
    let y = i64::from(area.position.y)
        + (i64::from(area.size.height).saturating_sub(i64::from(size.height)) / 2);
    window
        .set_position(PhysicalPosition::new(x as i32, y as i32))
        .map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

pub(crate) fn hide_pet_settings(app: &AppHandle) -> Result<(), String> {
    app.get_webview_window(PET_SETTINGS_LABEL)
        .ok_or_else(|| "桌宠设置窗口不存在".to_string())?
        .hide()
        .map_err(|error| error.to_string())
}

pub(crate) fn set_pet_layout(
    app: &AppHandle,
    runtime: &SharedRuntime,
    layout: PetLayout,
) -> Result<(), String> {
    let window = app
        .get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?;
    capture_window_state(&window, runtime);
    let mut preferences = runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用".to_string())?
        .pet_window
        .clone();
    let previous_layout = preferences.layout;
    let previous_size = preferences.pet_size;
    // 切换布局时以当前窗口左上角为锚点；不要跳回该布局上一次保存的位置。
    // restore_pet_window 只会在新尺寸越过工作区右侧/下侧时把位置向内收敛。
    let current_geometry = pet_geometry(&preferences).cloned();
    preferences.layout = layout;
    let (min, max) = crate::window_geometry::pet_size_range(&window, layout);
    preferences.pet_size = preferences.pet_size.clamp(min, max);
    {
        let mut windows = runtime.windows.lock().map_err(|_| "窗口状态不可用")?;
        windows.pet_window.layout = layout;
        windows.pet_window.pet_size = preferences.pet_size;
    }
    if let Err(error) = restore_pet_window(
        &window,
        current_geometry.as_ref(),
        layout,
        preferences.pet_size,
    ) {
        let mut windows = runtime.windows.lock().map_err(|_| "窗口状态不可用")?;
        windows.pet_window.layout = previous_layout;
        windows.pet_window.pet_size = previous_size;
        return Err(error.to_string());
    }
    runtime.save_preferences();
    runtime.window_changed();
    Ok(())
}

pub(crate) fn set_pet_focused_slot(runtime: &SharedRuntime, slot: u8) -> Result<(), String> {
    let visible_slots = {
        let state = runtime.state.read().map_err(|_| "状态不可用")?;
        state.rows.saturating_mul(state.columns).max(1)
    };
    runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用")?
        .pet_window
        .focused_slot = slot.min(visible_slots - 1);
    runtime.save_preferences();
    runtime.window_changed();
    Ok(())
}

pub(crate) fn set_pet_always_on_top(
    app: &AppHandle,
    runtime: &SharedRuntime,
    enabled: bool,
) -> Result<(), String> {
    app.get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?
        .set_always_on_top(enabled)
        .map_err(|error| error.to_string())?;
    runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用")?
        .pet_window
        .always_on_top = enabled;
    runtime.save_preferences();
    runtime.window_changed();
    Ok(())
}

pub(crate) fn set_pet_locked(runtime: &SharedRuntime, locked: bool) -> Result<(), String> {
    runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用")?
        .pet_window
        .locked = locked;
    runtime.save_preferences();
    runtime.window_changed();
    Ok(())
}

pub(crate) fn set_pet_size(
    app: &AppHandle,
    runtime: &SharedRuntime,
    size: u16,
) -> Result<(), String> {
    let layout = runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用")?
        .pet_window
        .layout;
    let window = app
        .get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?;
    let (min, max) = crate::window_geometry::pet_size_range(&window, layout);
    if !(min..=max).contains(&size) {
        return Err(format!("桌宠大小必须在 {min}–{max} 像素之间"));
    }
    apply_pet_constraints(&window, layout).map_err(|error| error.to_string())?;
    window
        .set_size(LogicalSize::new(
            f64::from(size),
            f64::from(size + PET_PAGER_HEIGHT),
        ))
        .map_err(|error| error.to_string())?;
    runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用")?
        .pet_window
        .pet_size = size;
    capture_window_state(&window, runtime);
    runtime.save_preferences();
    runtime.window_changed();
    Ok(())
}

pub(crate) fn resize_pet_by(
    app: &AppHandle,
    runtime: &SharedRuntime,
    delta: i16,
) -> Result<(), String> {
    let (layout, locked) = {
        let windows = runtime.windows.lock().map_err(|_| "窗口状态不可用")?;
        (windows.pet_window.layout, windows.pet_window.locked)
    };
    if locked {
        return Ok(());
    }
    let window = app
        .get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let current = f64::from(
        window
            .inner_size()
            .map_err(|error| error.to_string())?
            .width,
    ) / scale;
    let (min, max) = crate::window_geometry::pet_size_range(&window, layout);
    let pet_size = (current + f64::from(delta)).clamp(f64::from(min), f64::from(max));
    window
        .set_size(LogicalSize::new(
            pet_size,
            pet_size + f64::from(PET_PAGER_HEIGHT),
        ))
        .map_err(|error| error.to_string())?;
    runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用")?
        .pet_window
        .pet_size = pet_size.round() as u16;
    capture_window_state(&window, runtime);
    runtime.save_preferences();
    runtime.window_changed();
    Ok(())
}

pub(crate) fn start_pet_drag(app: &AppHandle, runtime: &SharedRuntime) -> Result<(), String> {
    if runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用")?
        .pet_window
        .locked
    {
        return Ok(());
    }
    app.get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?
        .start_dragging()
        .map_err(|error| error.to_string())
}
