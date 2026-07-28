//! 桌宠窗口的尺寸约束与跨显示器收敛：按当前显示器算出允许的尺寸范围、
//! 保持宽高相等、把窗口收敛回工作区。resize/move 是高频事件，这里刻意把
//! “查显示器”和“按显示器算范围/收敛”拆开，一次事件只查一次显示器。

use crate::model::PetLayout;
use crate::runtime::SharedRuntime;
use crate::window_geometry::{clamped_position, size_for_scale, PET_PAGER_HEIGHT};
use tauri::{LogicalSize, Monitor, PhysicalSize, WebviewWindow};

pub(crate) fn pet_canvas_min(layout: PetLayout) -> u16 {
    match layout {
        PetLayout::Single => 64,
        PetLayout::Grid => 256,
    }
}

pub(crate) fn pet_size_range_for_monitor(monitor: &Monitor, layout: PetLayout) -> (u16, u16) {
    let area = monitor.work_area();
    let min = pet_canvas_min(layout);
    let max = maximum_pet_size(
        area.size.width.min(area.size.height),
        monitor.scale_factor(),
        layout,
    );
    (min, max.max(min))
}

fn maximum_pet_size(shortest_physical: u32, scale_factor: f64, layout: PetLayout) -> u16 {
    let divisor = if layout == PetLayout::Grid { 2.0 } else { 4.0 };
    (f64::from(shortest_physical) / scale_factor / divisor)
        .floor()
        .max(f64::from(pet_canvas_min(layout))) as u16
}

// 找不到任何显示器时的保守兜底（理论上不会发生，仅在 current_monitor/primary_monitor 都失败时使用）。
const PET_SIZE_MAX_FALLBACK: u16 = 360;

pub(crate) fn pet_size_range_fallback(layout: PetLayout) -> (u16, u16) {
    (pet_canvas_min(layout), PET_SIZE_MAX_FALLBACK)
}

fn resolve_pet_monitor(window: &WebviewWindow) -> Option<Monitor> {
    window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
}

fn pet_size_range_for_resolved_monitor(monitor: Option<&Monitor>, layout: PetLayout) -> (u16, u16) {
    monitor.map_or_else(
        || pet_size_range_fallback(layout),
        |monitor| pet_size_range_for_monitor(monitor, layout),
    )
}

pub(crate) fn pet_size_range(window: &WebviewWindow, layout: PetLayout) -> (u16, u16) {
    pet_size_range_for_resolved_monitor(resolve_pet_monitor(window).as_ref(), layout)
}

fn logical_pet_window_size(canvas_size: u16) -> LogicalSize<f64> {
    LogicalSize::new(
        f64::from(canvas_size),
        f64::from(canvas_size + PET_PAGER_HEIGHT),
    )
}

pub(crate) fn physical_pet_window_size(canvas_size: u16, scale: f64) -> PhysicalSize<u32> {
    let logical = logical_pet_window_size(canvas_size);
    PhysicalSize::new(
        (logical.width * scale).round() as u32,
        (logical.height * scale).round() as u32,
    )
}

fn apply_pet_constraints_for_resolved_monitor(
    window: &WebviewWindow,
    monitor: Option<&Monitor>,
    layout: PetLayout,
) -> tauri::Result<()> {
    let (min, max) = pet_size_range_for_resolved_monitor(monitor, layout);
    window.set_min_size(Some(logical_pet_window_size(min)))?;
    window.set_max_size(Some(logical_pet_window_size(max)))
}

pub(crate) fn apply_pet_constraints(window: &WebviewWindow, layout: PetLayout) -> tauri::Result<()> {
    apply_pet_constraints_for_resolved_monitor(window, resolve_pet_monitor(window).as_ref(), layout)
}

fn keep_pet_square_for_resolved_monitor(
    window: &WebviewWindow,
    size: PhysicalSize<u32>,
    runtime: &SharedRuntime,
    monitor: Option<&Monitor>,
) {
    let (layout, previous) = runtime
        .windows
        .lock()
        .map(|windows| {
            let layout = windows.pet_window.layout;
            let previous = match layout {
                PetLayout::Single => windows.pet_window.single_geometry.clone(),
                PetLayout::Grid => windows.pet_window.grid_geometry.clone(),
            };
            (layout, previous)
        })
        .unwrap_or_default();
    let previous = previous.map(|geometry| {
        size_for_scale(
            &geometry,
            window
                .scale_factor()
                .unwrap_or(geometry.scale_factor.max(1.0)),
        )
    });
    let scale = window.scale_factor().unwrap_or(1.0);
    let footer = (f64::from(PET_PAGER_HEIGHT) * scale).round() as u32;
    let requested_canvas = previous.map_or(size.width, |previous| {
        if size.width.abs_diff(previous.width) >= size.height.abs_diff(previous.height) {
            size.width
        } else {
            size.height.saturating_sub(footer)
        }
    });
    let (min, max) = pet_size_range_for_resolved_monitor(monitor, layout);
    let pet_size = ((f64::from(requested_canvas) / scale).round() as u16).clamp(min, max);
    let expected = physical_pet_window_size(pet_size, scale);
    if size.width.abs_diff(expected.width) > 1 || size.height.abs_diff(expected.height) > 1 {
        let _ = window.set_size(expected);
    }
    if let Ok(mut windows) = runtime.windows.lock() {
        if windows.pet_window.pet_size != pet_size {
            windows.pet_window.pet_size = pet_size;
            runtime.window_changed();
        }
    }
}

pub(crate) fn constrain_pet_to_current_monitor(window: &WebviewWindow, runtime: &SharedRuntime) {
    let monitor = resolve_pet_monitor(window);
    let layout = runtime
        .windows
        .lock()
        .map(|windows| windows.pet_window.layout)
        .unwrap_or_default();
    let _ = apply_pet_constraints_for_resolved_monitor(window, monitor.as_ref(), layout);
    let Ok(size) = window.inner_size() else {
        return;
    };
    keep_pet_square_for_resolved_monitor(window, size, runtime, monitor.as_ref());
}

fn clamp_window_to_work_area_for_resolved_monitor(window: &WebviewWindow, monitor: Option<&Monitor>) {
    let Ok(position) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.inner_size() else {
        return;
    };
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let clamped = clamped_position(position.x, position.y, size, area.position, area.size);
        if clamped != position {
            let _ = window.set_position(clamped);
        }
    }
}

// resize 事件里 keep_pet_square 和 clamp_window_to_work_area 都要用到当前显示器；
// 在这里只解析一次再分别传入，避免同一次事件里重复查询显示器。
pub(crate) fn handle_pet_resize(window: &WebviewWindow, size: PhysicalSize<u32>, runtime: &SharedRuntime) {
    let monitor = resolve_pet_monitor(window);
    keep_pet_square_for_resolved_monitor(window, size, runtime, monitor.as_ref());
    clamp_window_to_work_area_for_resolved_monitor(window, monitor.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_pet_to_one_quarter_of_the_logical_shortest_edge() {
        assert_eq!(maximum_pet_size(1_080, 1.0, PetLayout::Single), 270);
        assert_eq!(maximum_pet_size(1_080, 2.0, PetLayout::Single), 135);
        assert_eq!(maximum_pet_size(1_080, 1.0, PetLayout::Grid), 540);
        assert_eq!(logical_pet_window_size(64), LogicalSize::new(64.0, 88.0));
        assert_eq!(logical_pet_window_size(256), LogicalSize::new(256.0, 280.0));
        assert_eq!(pet_canvas_min(PetLayout::Single), 64);
        assert_eq!(pet_canvas_min(PetLayout::Grid), 256);
    }
}
