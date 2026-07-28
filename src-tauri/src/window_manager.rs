//! 两个互斥窗口的显示、模式切换与桌宠交互。

use crate::model::{AppMode, PetLayout, PetWindowPreferences};
use crate::runtime::SharedRuntime;
use crate::window_geometry::{
    apply_pet_constraints, capture_window_state, restore_main_window, restore_pet_window,
};
use tauri::{AppHandle, LogicalSize, Manager};
use tauri_runtime::ResizeDirection;

const MAIN_LABEL: &str = "main";
const PET_LABEL: &str = "pet";

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
    preferences.layout = layout;
    restore_pet_window(&window, pet_geometry(&preferences), layout)
        .map_err(|error| error.to_string())?;
    runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用".to_string())?
        .pet_window
        .layout = layout;
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

fn pet_side_for_preset(layout: PetLayout, preset: u16) -> f64 {
    let base = match layout {
        PetLayout::Single => 240.0,
        PetLayout::Grid => 480.0,
    };
    base * f64::from(preset) / 100.0
}

pub(crate) fn set_pet_scale(
    app: &AppHandle,
    runtime: &SharedRuntime,
    preset: u16,
) -> Result<(), String> {
    if !matches!(preset, 75 | 100 | 125 | 150) {
        return Err("桌宠比例必须为 75、100、125 或 150".into());
    }
    let layout = runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用")?
        .pet_window
        .layout;
    let window = app
        .get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?;
    apply_pet_constraints(&window, layout).map_err(|error| error.to_string())?;
    let side = pet_side_for_preset(layout, preset);
    window
        .set_size(LogicalSize::new(side, side))
        .map_err(|error| error.to_string())?;
    runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用")?
        .pet_window
        .scale_preset = preset;
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
    let (min, max) = match layout {
        PetLayout::Single => (160.0, 360.0),
        PetLayout::Grid => (320.0, 720.0),
    };
    let side = (current + f64::from(delta)).clamp(min, max);
    window
        .set_size(LogicalSize::new(side, side))
        .map_err(|error| error.to_string())?;
    runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用")?
        .pet_window
        .scale_preset = 0;
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

pub(crate) fn start_pet_resize(app: &AppHandle, runtime: &SharedRuntime) -> Result<(), String> {
    {
        let mut windows = runtime.windows.lock().map_err(|_| "窗口状态不可用")?;
        if windows.pet_window.locked {
            return Ok(());
        }
        windows.pet_window.scale_preset = 0;
    }
    runtime.window_changed();
    app.get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?
        .as_ref()
        .window()
        .start_resize_dragging(ResizeDirection::SouthEast)
        .map_err(|error| error.to_string())
}
