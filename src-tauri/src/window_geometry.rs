//! 主窗口与桌宠窗口的几何保存、跨 DPI 恢复和工作区收敛。

use crate::model::{MainWindowPreferences, PetLayout, WindowGeometry};
use crate::runtime::SharedRuntime;
use tauri::{LogicalSize, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

const MAIN_LABEL: &str = "main";
const PET_LABEL: &str = "pet";
pub(crate) const PET_PAGER_HEIGHT: u16 = 24;
pub(crate) fn rectangles_have_visible_overlap(
    window: &WindowGeometry,
    monitor_x: i32,
    monitor_y: i32,
    monitor_width: u32,
    monitor_height: u32,
) -> bool {
    let left = i64::from(window.x).max(i64::from(monitor_x));
    let top = i64::from(window.y).max(i64::from(monitor_y));
    let right = (i64::from(window.x) + i64::from(window.width))
        .min(i64::from(monitor_x) + i64::from(monitor_width));
    let bottom = (i64::from(window.y) + i64::from(window.height))
        .min(i64::from(monitor_y) + i64::from(monitor_height));
    let required_width = i64::from(window.width.min(64));
    let required_height = i64::from(window.height.min(64));
    right - left >= required_width && bottom - top >= required_height
}

fn size_for_scale(geometry: &WindowGeometry, target_scale: f64) -> PhysicalSize<u32> {
    if geometry.scale_factor > 0.0 {
        let ratio = target_scale / geometry.scale_factor;
        PhysicalSize::new(
            (f64::from(geometry.width) * ratio).round().max(1.0) as u32,
            (f64::from(geometry.height) * ratio).round().max(1.0) as u32,
        )
    } else {
        // 1.1 及更早版本没有 scaleFactor，宽高就是原始物理像素。
        PhysicalSize::new(geometry.width, geometry.height)
    }
}

fn monitor_index_for_geometry(window: &WebviewWindow, geometry: &WindowGeometry) -> Option<usize> {
    let monitors = window.available_monitors().ok()?;
    monitors.iter().position(|monitor| {
        let area = monitor.work_area();
        let scaled = size_for_scale(geometry, monitor.scale_factor());
        let candidate = WindowGeometry {
            x: geometry.x,
            y: geometry.y,
            width: scaled.width,
            height: scaled.height,
            scale_factor: monitor.scale_factor(),
        };
        rectangles_have_visible_overlap(
            &candidate,
            area.position.x,
            area.position.y,
            area.size.width,
            area.size.height,
        )
    })
}

fn clamped_position(
    x: i32,
    y: i32,
    size: PhysicalSize<u32>,
    area_position: PhysicalPosition<i32>,
    area_size: PhysicalSize<u32>,
) -> PhysicalPosition<i32> {
    let right = i64::from(area_position.x) + i64::from(area_size.width);
    let bottom = i64::from(area_position.y) + i64::from(area_size.height);
    let max_x = (right - i64::from(size.width)).max(i64::from(area_position.x));
    let max_y = (bottom - i64::from(size.height)).max(i64::from(area_position.y));
    PhysicalPosition::new(
        i64::from(x).clamp(i64::from(area_position.x), max_x) as i32,
        i64::from(y).clamp(i64::from(area_position.y), max_y) as i32,
    )
}

pub(crate) fn restore_main_window(
    window: &WebviewWindow,
    preferences: &MainWindowPreferences,
) -> tauri::Result<()> {
    if let Some(geometry) = preferences.normal_geometry.as_ref() {
        if let Some(index) = monitor_index_for_geometry(window, geometry) {
            let monitors = window.available_monitors()?;
            let monitor = &monitors[index];
            let size = size_for_scale(geometry, monitor.scale_factor());
            window.unmaximize()?;
            window.set_size(size)?;
            window.set_position(clamped_position(
                geometry.x,
                geometry.y,
                size,
                monitor.work_area().position,
                monitor.work_area().size,
            ))?;
            if preferences.maximized {
                window.maximize()?;
            }
            return Ok(());
        }
    }

    if let Some(primary) = window.primary_monitor()? {
        window.set_position(primary.work_area().position)?;
    }
    window.maximize()
}

pub(crate) fn pet_canvas_min(layout: PetLayout) -> u16 {
    match layout {
        PetLayout::Single => 64,
        PetLayout::Grid => 256,
    }
}

fn pet_size_range_for_monitor(monitor: &Monitor, layout: PetLayout) -> (u16, u16) {
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

pub(crate) fn pet_size_range(window: &WebviewWindow, layout: PetLayout) -> (u16, u16) {
    window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
        .map_or((pet_canvas_min(layout), 360), |monitor| {
            pet_size_range_for_monitor(&monitor, layout)
        })
}

fn logical_pet_window_size(canvas_size: u16) -> LogicalSize<f64> {
    LogicalSize::new(
        f64::from(canvas_size),
        f64::from(canvas_size + PET_PAGER_HEIGHT),
    )
}

fn physical_pet_window_size(canvas_size: u16, scale: f64) -> PhysicalSize<u32> {
    let logical = logical_pet_window_size(canvas_size);
    PhysicalSize::new(
        (logical.width * scale).round() as u32,
        (logical.height * scale).round() as u32,
    )
}

pub(crate) fn apply_pet_constraints(
    window: &WebviewWindow,
    layout: PetLayout,
) -> tauri::Result<()> {
    let (min, max) = pet_size_range(window, layout);
    window.set_min_size(Some(logical_pet_window_size(min)))?;
    window.set_max_size(Some(logical_pet_window_size(max)))
}

pub(crate) fn restore_pet_window(
    window: &WebviewWindow,
    geometry: Option<&WindowGeometry>,
    layout: PetLayout,
    pet_size: u16,
) -> tauri::Result<()> {
    apply_pet_constraints(window, layout)?;
    if let Some(geometry) = geometry {
        if let Some(index) = monitor_index_for_geometry(window, geometry) {
            let monitors = window.available_monitors()?;
            let monitor = &monitors[index];
            let saved_size = size_for_scale(geometry, monitor.scale_factor());
            let (min, max) = pet_size_range_for_monitor(monitor, layout);
            let scale = monitor.scale_factor();
            let footer = (f64::from(PET_PAGER_HEIGHT) * scale).round() as u32;
            let requested_canvas = saved_size
                .width
                .max(saved_size.height.saturating_sub(footer));
            let canvas_size = (f64::from(requested_canvas) / scale).round() as u16;
            let size = physical_pet_window_size(canvas_size.clamp(min, max), scale);
            window.set_size(size)?;
            window.set_position(clamped_position(
                geometry.x,
                geometry.y,
                size,
                monitor.work_area().position,
                monitor.work_area().size,
            ))?;
            return Ok(());
        }
    }

    let monitor = window
        .primary_monitor()?
        .or(window.current_monitor()?)
        .ok_or_else(|| tauri::Error::FailedToReceiveMessage)?;
    let (min, max) = pet_size_range_for_monitor(&monitor, layout);
    let scale = monitor.scale_factor();
    let size = physical_pet_window_size(pet_size.clamp(min, max), scale);
    let area = monitor.work_area();
    let margin = (16.0 * scale).round() as i32;
    window.set_size(size)?;
    window.set_position(PhysicalPosition::new(
        area.position.x + area.size.width as i32 - size.width as i32 - margin,
        area.position.y + area.size.height as i32 - size.height as i32 - margin,
    ))
}

pub(crate) fn capture_window_state(window: &WebviewWindow, runtime: &SharedRuntime) {
    if window.is_minimized().unwrap_or(false) {
        return;
    }
    let mut windows = runtime.windows.lock().expect("window state lock poisoned");
    if window.label() == MAIN_LABEL {
        windows.main_window.maximized = window.is_maximized().unwrap_or(false);
        if windows.main_window.maximized {
            return;
        }
    }
    let (Ok(position), Ok(size)) = (window.outer_position(), window.inner_size()) else {
        return;
    };
    if size.width == 0 || size.height == 0 {
        return;
    }
    let geometry = WindowGeometry {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        scale_factor: window.scale_factor().unwrap_or(1.0),
    };
    match window.label() {
        MAIN_LABEL => windows.main_window.normal_geometry = Some(geometry),
        PET_LABEL => match windows.pet_window.layout {
            PetLayout::Single => windows.pet_window.single_geometry = Some(geometry),
            PetLayout::Grid => windows.pet_window.grid_geometry = Some(geometry),
        },
        _ => {}
    }
}

pub(crate) fn save_window_state(window: &WebviewWindow, runtime: &SharedRuntime, immediate: bool) {
    capture_window_state(window, runtime);
    if immediate {
        runtime.save_preferences();
    } else {
        runtime.save_preferences_debounced();
    }
}

pub(crate) fn keep_pet_square(
    window: &WebviewWindow,
    size: PhysicalSize<u32>,
    runtime: &SharedRuntime,
) {
    let layout = runtime
        .windows
        .lock()
        .map(|windows| windows.pet_window.layout)
        .unwrap_or_default();
    let previous = runtime
        .windows
        .lock()
        .ok()
        .and_then(|windows| match windows.pet_window.layout {
            PetLayout::Single => windows.pet_window.single_geometry.clone(),
            PetLayout::Grid => windows.pet_window.grid_geometry.clone(),
        })
        .map(|geometry| {
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
    let (min, max) = pet_size_range(window, layout);
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
    let layout = runtime
        .windows
        .lock()
        .map(|windows| windows.pet_window.layout)
        .unwrap_or_default();
    let _ = apply_pet_constraints(window, layout);
    let Ok(size) = window.inner_size() else {
        return;
    };
    keep_pet_square(window, size, runtime);
}

pub(crate) fn clamp_window_to_work_area(window: &WebviewWindow) {
    let Ok(position) = window.outer_position() else {
        return;
    };
    let Ok(size) = window.inner_size() else {
        return;
    };
    let monitor = window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let clamped = clamped_position(position.x, position.y, size, area.position, area.size);
        if clamped != position {
            let _ = window.set_position(clamped);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(x: i32, width: u32) -> WindowGeometry {
        WindowGeometry {
            x,
            y: 100,
            width,
            height: 700,
            scale_factor: 1.0,
        }
    }
    #[test]
    fn accepts_geometry_with_a_reachable_window_area() {
        assert!(rectangles_have_visible_overlap(
            &geometry(1_856, 1_000),
            0,
            0,
            1_920,
            1_080
        ));
    }
    #[test]
    fn rejects_geometry_left_on_a_removed_monitor() {
        assert!(!rectangles_have_visible_overlap(
            &geometry(1_900, 1_000),
            0,
            0,
            1_920,
            1_080
        ));
    }
    #[test]
    fn rescales_saved_physical_size_for_new_dpi() {
        let saved = WindowGeometry {
            x: 0,
            y: 0,
            width: 400,
            height: 400,
            scale_factor: 2.0,
        };
        assert_eq!(size_for_scale(&saved, 1.0), PhysicalSize::new(200, 200));
    }
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
