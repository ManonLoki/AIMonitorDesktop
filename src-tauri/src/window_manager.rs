//! 两个互斥窗口的显示、模式切换与桌宠交互。

use crate::model::{AppMode, PetLayout, PetWindowPreferences, WindowPreferences};
use crate::pet_geometry::{
    apply_pet_constraints, constrain_pet_to_current_monitor, handle_pet_resize,
    logical_pet_window_size, physical_pet_window_size,
};
use crate::runtime::SharedRuntime;
use crate::window_geometry::{capture_window_state, restore_main_window, restore_pet_window};
use serde::Deserialize;
use std::sync::MutexGuard;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

const MAIN_LABEL: &str = "main";
const PET_LABEL: &str = "pet";
const PET_SETTINGS_LABEL: &str = "pet-settings";
const PET_RESIZE_STEP: i16 = 24;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum PetResizeDirection {
    Grow,
    Shrink,
}

// 本文件几乎每个函数都要加这把锁；统一成一个助手，出错时的提示文案也只维护一处。
fn windows_lock(runtime: &SharedRuntime) -> Result<MutexGuard<'_, WindowPreferences>, String> {
    runtime
        .windows
        .lock()
        .map_err(|_| "窗口状态不可用".to_string())
}

// 桌宠相关的写操作几乎都以“落盘 + 广播 window-state-changed”收尾，抽成一个助手。
fn commit(runtime: &SharedRuntime) {
    runtime.save_preferences();
    runtime.window_changed();
}

// 模式 → 对应的固定窗口 label，Tauri 侧按 label 查找/操作窗口都要经过这一层映射。
fn label_for_mode(mode: AppMode) -> &'static str {
    match mode {
        AppMode::Main => MAIN_LABEL,
        AppMode::Pet => PET_LABEL,
    }
}

// 取当前布局对应的已保存几何，恢复窗口位置时用。
fn pet_geometry(preferences: &PetWindowPreferences) -> Option<&crate::model::WindowGeometry> {
    match preferences.layout {
        PetLayout::Single => preferences.single_geometry.as_ref(),
        PetLayout::Row => preferences.row_geometry.as_ref(),
        PetLayout::Column => preferences.column_geometry.as_ref(),
        PetLayout::Row3 => preferences.row3_geometry.as_ref(),
        PetLayout::Column3 => preferences.column3_geometry.as_ref(),
        PetLayout::Grid => preferences.grid_geometry.as_ref(),
    }
}

// 把目标模式的窗口按保存的偏好恢复到位（位置/尺寸/置顶等），但不负责显示/隐藏——
// show/hide 时机由调用方（show_active_window / switch_mode）决定，方便切换失败时回滚。
fn prepare_window(
    app: &AppHandle,
    runtime: &SharedRuntime,
    mode: AppMode,
) -> Result<tauri::WebviewWindow, String> {
    let window = app
        .get_webview_window(label_for_mode(mode))
        .ok_or_else(|| "目标窗口不存在".to_string())?;
    let windows = windows_lock(runtime)?.clone(); // clone 出来后立刻释放锁，恢复窗口的过程不需要一直持锁
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

// 单实例二次启动 / 托盘“显示看板”时调用：不切模式，只是把当前活动模式对应的
// 窗口恢复位置、取消最小化、显示并聚焦。
pub(crate) fn show_active_window(app: &AppHandle, runtime: &SharedRuntime) -> Result<(), String> {
    let mode = windows_lock(runtime)?.active_mode;
    let window = prepare_window(app, runtime, mode)?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

// 主窗口 ⇄ 桌宠模式切换，两个窗口互斥（同一时刻只显示一个）。
// 步骤：保存旧窗口当前状态 → 恢复新窗口到位 → 隐藏旧窗口/设置窗口 → 显示新窗口；
// 显示新窗口失败时把旧窗口重新显示出来，避免切换失败后界面上什么都看不见。
pub(crate) fn switch_mode(
    app: &AppHandle,
    runtime: &SharedRuntime,
    target: AppMode,
) -> Result<(), String> {
    let source_mode = windows_lock(runtime)?.active_mode;
    if source_mode == target {
        return show_active_window(app, runtime); // 目标就是当前模式，退化成普通的显示
    }
    let source = app.get_webview_window(label_for_mode(source_mode));
    if let Some(source) = source.as_ref() {
        capture_window_state(source, runtime); // 切走前先记住旧窗口的位置，下次切回来能恢复
    }
    let target_window = prepare_window(app, runtime, target)?;
    if let Some(settings) = app.get_webview_window(PET_SETTINGS_LABEL) {
        let _ = settings.hide(); // 桌宠设置窗口只在桌宠模式下有意义，切走时一并隐藏
    }
    if let Some(source) = source.as_ref() {
        source.hide().map_err(|error| error.to_string())?;
    }
    let show_result = target_window
        .show()
        .and_then(|_| target_window.set_focus())
        .map_err(|error| error.to_string());
    if let Err(error) = show_result {
        // 新窗口显示失败：把旧窗口显示回来，不留下“两个窗口都不可见”的状态。
        if let Some(source) = source {
            let _ = source.show();
            let _ = source.set_focus();
        }
        return Err(error);
    }
    windows_lock(runtime)?.active_mode = target;
    commit(runtime);
    Ok(())
}

// 托盘“退出”/关闭快捷键触发：只隐藏当前活动窗口（不退出进程，行为等同点关闭按钮）。
pub(crate) fn hide_active_window(app: &AppHandle, runtime: &SharedRuntime) -> Result<(), String> {
    let mode = windows_lock(runtime)?.active_mode;
    let window = app
        .get_webview_window(label_for_mode(mode))
        .ok_or_else(|| "当前窗口不存在".to_string())?;
    capture_window_state(&window, runtime);
    runtime.save_preferences();
    if mode == AppMode::Pet {
        if let Some(settings) = app.get_webview_window(PET_SETTINGS_LABEL) {
            settings.hide().map_err(|error| error.to_string())?;
        }
    }
    window.hide().map_err(|error| error.to_string())
}

// 打开桌宠右键菜单对应的独立设置窗口：每次都重新居中到“桌宠当前所在显示器”的工作区，
// 而不是设置窗口自己上次的位置或全局主显示器——桌宠可能被拖到了副屏，设置窗口要跟过去。
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
    // 居中公式：工作区起点 + (工作区尺寸 - 窗口尺寸) / 2；用 i64 避免中间结果溢出 i32。
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

// 关闭桌宠设置窗口（右上角 × 按钮 / 点击桌宠外部区域触发），只隐藏不销毁。
pub(crate) fn hide_pet_settings(app: &AppHandle) -> Result<(), String> {
    app.get_webview_window(PET_SETTINGS_LABEL)
        .ok_or_else(|| "桌宠设置窗口不存在".to_string())?
        .hide()
        .map_err(|error| error.to_string())
}

// 六种宫格布局切换。用当前窗口位置作为锚点（见下面注释），并把该布局上次
// 保存的尺寸夹到新布局允许的区间；恢复窗口几何失败时把内存状态回滚，
// 避免“落盘的偏好”和“窗口实际状态”不一致。
pub(crate) fn set_pet_layout(
    app: &AppHandle,
    runtime: &SharedRuntime,
    layout: PetLayout,
) -> Result<(), String> {
    let window = app
        .get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?;
    capture_window_state(&window, runtime); // 记住切换前的位置，作为下面的锚点用
    let mut preferences = windows_lock(runtime)?.pet_window.clone();
    let previous_layout = preferences.layout; // 恢复失败时回滚用
    let previous_size = preferences.pet_size;
    // 切换布局时以当前窗口左上角为锚点；不要跳回该布局上一次保存的位置。
    // restore_pet_window 只会在新尺寸越过工作区右侧/下侧时把位置向内收敛。
    let mut current_geometry = pet_geometry(&preferences).cloned();
    preferences.layout = layout;
    let (min, max) = crate::pet_geometry::pet_size_range(&window, layout);
    preferences.pet_size = preferences.pet_size.clamp(min, max); // 旧尺寸可能超出新布局允许的区间
    if let Some(geometry) = current_geometry.as_mut() {
        // 只复用旧布局的位置作为锚点；宽高必须按目标布局重算，否则横排切竖排会错误缩小一半。
        let scale = geometry.scale_factor.max(1.0);
        let size = physical_pet_window_size(layout, preferences.pet_size, scale);
        geometry.width = size.width;
        geometry.height = size.height;
    }
    {
        let mut windows = windows_lock(runtime)?;
        windows.pet_window.layout = layout;
        windows.pet_window.pet_size = preferences.pet_size;
    }
    if let Err(error) = restore_pet_window(
        &window,
        current_geometry.as_ref(),
        layout,
        preferences.pet_size,
    ) {
        // 窗口没能恢复成功：把内存状态改回切换前的值，不留下半切换状态。
        let mut windows = windows_lock(runtime)?;
        windows.pet_window.layout = previous_layout;
        windows.pet_window.pet_size = previous_size;
        return Err(error.to_string());
    }
    commit(runtime);
    Ok(())
}

// 先调用 OS API 生效，成功了才更新内存状态并落盘——避免“偏好里记着开，实际窗口没置顶”。
pub(crate) fn set_pet_always_on_top(
    app: &AppHandle,
    runtime: &SharedRuntime,
    enabled: bool,
) -> Result<(), String> {
    app.get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?
        .set_always_on_top(enabled)
        .map_err(|error| error.to_string())?;
    windows_lock(runtime)?.pet_window.always_on_top = enabled;
    commit(runtime);
    Ok(())
}

// 锁定桌宠：锁定后拖拽/滚轮缩放会被 start_pet_drag / resize_pet_by 直接短路。
pub(crate) fn set_pet_locked(runtime: &SharedRuntime, locked: bool) -> Result<(), String> {
    windows_lock(runtime)?.pet_window.locked = locked;
    commit(runtime);
    Ok(())
}

// 桌宠设置面板里的尺寸滑杆调用：显式校验请求值落在当前显示器允许的区间内，
// 越界直接拒绝（而不是静默 clamp），让前端能感知到无效输入。
pub(crate) fn set_pet_size(
    app: &AppHandle,
    runtime: &SharedRuntime,
    size: u16,
) -> Result<(), String> {
    let layout = windows_lock(runtime)?.pet_window.layout;
    let window = app
        .get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?;
    let (min, max) = crate::pet_geometry::pet_size_range(&window, layout);
    if !(min..=max).contains(&size) {
        return Err(format!("桌宠大小必须在 {min}–{max} 像素之间"));
    }
    apply_pet_constraints(&window, layout).map_err(|error| error.to_string())?; // 同步 OS 级别的 min/max，避免下面 set_size 被系统钳制成别的值
    window
        .set_size(logical_pet_window_size(layout, size))
        .map_err(|error| error.to_string())?;
    windows_lock(runtime)?.pet_window.pet_size = size;
    capture_window_state(&window, runtime); // set_size 后窗口位置可能因为工作区收敛而变化，一并记下来
    commit(runtime);
    Ok(())
}

// Ctrl/Cmd + 滚轮缩放：在当前尺寸基础上加一个增量，而不是直接设定绝对值，
// 所以要先读窗口的实际当前尺寸（而非偏好里存的值，两者理论一致但以窗口为准更保险）。
pub(crate) fn resize_pet_by(
    app: &AppHandle,
    runtime: &SharedRuntime,
    delta: i16,
) -> Result<(), String> {
    let (layout, locked) = {
        let windows = windows_lock(runtime)?;
        (windows.pet_window.layout, windows.pet_window.locked)
    };
    if locked {
        return Ok(()); // 锁定状态下静默忽略，不报错——用户滚轮误触不应该弹错误提示
    }
    let window = app
        .get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?;
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let current_size = window.inner_size().map_err(|error| error.to_string())?;
    let (_, columns) = layout.dimensions();
    let current = f64::from(current_size.width) / f64::from(columns) / scale;
    let (min, max) = crate::pet_geometry::pet_size_range(&window, layout);
    let pet_size = (current + f64::from(delta)).clamp(f64::from(min), f64::from(max));
    window
        .set_size(logical_pet_window_size(layout, pet_size.round() as u16))
        .map_err(|error| error.to_string())?;
    windows_lock(runtime)?.pet_window.pet_size = pet_size.round() as u16;
    capture_window_state(&window, runtime);
    commit(runtime);
    Ok(())
}

// Ctrl/Cmd + 滚轮只传递用户意图，具体步长由 Rust 统一管理。
pub(crate) fn resize_pet_step(
    app: &AppHandle,
    runtime: &SharedRuntime,
    direction: PetResizeDirection,
) -> Result<(), String> {
    let delta = match direction {
        PetResizeDirection::Grow => PET_RESIZE_STEP,
        PetResizeDirection::Shrink => -PET_RESIZE_STEP,
    };
    resize_pet_by(app, runtime, delta)
}

// 桌宠区域按下左键拖拽：调用 Tauri 的原生拖拽，由 OS 接管后续的鼠标移动。
pub(crate) fn start_pet_drag(app: &AppHandle, runtime: &SharedRuntime) -> Result<(), String> {
    if windows_lock(runtime)?.pet_window.locked {
        return Ok(()); // 锁定状态下不允许拖拽，静默忽略
    }
    app.get_webview_window(PET_LABEL)
        .ok_or_else(|| "桌宠窗口不存在".to_string())?
        .start_dragging()
        .map_err(|error| error.to_string())
}

// lib.rs 用同一个事件回调处理 main/pet/pet-settings 三个窗口；
// 桌宠专属的 resize/move 规则在这里按 label 判断，回调本身保持窗口无关。
pub(crate) fn handle_window_resized(
    window: &tauri::WebviewWindow,
    size: PhysicalSize<u32>,
    runtime: &SharedRuntime,
) {
    if window.label() == PET_LABEL {
        handle_pet_resize(window, size, runtime);
    }
}

// 同上，Moved 事件版本：跨显示器拖动后重新应用尺寸约束并广播一次状态变化
// （constrain_pet_to_current_monitor 内部只在尺寸真的变化时才广播，这里额外广播
// 一次是因为位置本身也变了，前端翻页条等 UI 需要感知）。
pub(crate) fn handle_window_moved(window: &tauri::WebviewWindow, runtime: &SharedRuntime) {
    if window.label() == PET_LABEL {
        constrain_pet_to_current_monitor(window, runtime);
        runtime.window_changed();
    }
}
