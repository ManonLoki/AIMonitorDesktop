//! 桌宠窗口的尺寸约束与跨显示器收敛：按当前显示器算出允许的尺寸范围、
//! 保持布局宽高比、把窗口收敛回工作区。resize/move 是高频事件，这里刻意把
//! “查显示器”和“按显示器算范围/收敛”拆开，一次事件只查一次显示器。

use crate::model::PetLayout;
use crate::runtime::SharedRuntime;
use crate::window_geometry::{clamped_position, size_for_scale};
use tauri::{LogicalSize, Monitor, PhysicalSize, WebviewWindow};

const PET_CELL_MIN: u16 = 32;

// 按“当前显示器工作区的最短边”换算出这块屏幕上桌宠允许的尺寸区间。
pub(crate) fn pet_size_range_for_monitor(monitor: &Monitor, layout: PetLayout) -> (u16, u16) {
    let area = monitor.work_area();
    let min = PET_CELL_MIN;
    let max = maximum_pet_size(
        area.size.width.min(area.size.height), // 用工作区最短边而不是宽/高，横竖屏都不会超出可视范围
        monitor.scale_factor(),
        layout,
    );
    (min, max) // maximum_pet_size 已保证极小显示器上也不会低于统一最小值
}

// 单格最大边长 = 显示器逻辑最短边 / 4 / 布局最长轴格子数。
// 因此无论横排、竖排还是 2×2，整个窗口的长边上限都等于屏幕最短边的 1/4。
fn maximum_pet_size(shortest_physical: u32, scale_factor: f64, layout: PetLayout) -> u16 {
    let (rows, columns) = layout.dimensions();
    let axis_count = rows.max(columns);
    (f64::from(shortest_physical) / scale_factor / 4.0 / f64::from(axis_count))
        .floor()
        .max(f64::from(PET_CELL_MIN)) as u16
}

// 找不到任何显示器时的保守兜底（理论上不会发生，仅在 current_monitor/primary_monitor 都失败时使用）。
const PET_LOGICAL_SHORTEST_FALLBACK: u32 = 1_440;

// 无法解析出显示器时对外暴露的兜底区间；runtime::window_snapshot 在拿不到桌宠窗口句柄时
// 也使用同一套公式和统一兜底屏幕尺寸，避免另写一套范围逻辑。
pub(crate) fn pet_size_range_fallback(layout: PetLayout) -> (u16, u16) {
    (
        PET_CELL_MIN,
        maximum_pet_size(PET_LOGICAL_SHORTEST_FALLBACK, 1.0, layout),
    )
}

// 优先用窗口当前所在的显示器，取不到（例如窗口刚创建还未 map）再退回主显示器。
fn resolve_pet_monitor(window: &WebviewWindow) -> Option<Monitor> {
    window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
}

// 内部统一入口：显示器已经解析好就直接按显示器算，解析不到就走兜底值。
fn pet_size_range_for_resolved_monitor(monitor: Option<&Monitor>, layout: PetLayout) -> (u16, u16) {
    monitor.map_or_else(
        || pet_size_range_fallback(layout),
        |monitor| pet_size_range_for_monitor(monitor, layout),
    )
}

// 对外的单次查询入口：非高频调用点（set_pet_size / set_pet_layout / resize_pet_by 等一次性
// 用户操作）直接用这个即可，内部会自己查一次显示器。resize/move 高频事件请走 handle_pet_resize，
// 避免同一次事件里重复查询显示器。
pub(crate) fn pet_size_range(window: &WebviewWindow, layout: PetLayout) -> (u16, u16) {
    pet_size_range_for_resolved_monitor(resolve_pet_monitor(window).as_ref(), layout)
}

// 窗口尺寸只做乘法：单格边长 × 列数、单格边长 × 行数。
pub(crate) fn logical_pet_window_size(layout: PetLayout, pet_size: u16) -> LogicalSize<f64> {
    let (rows, columns) = layout.dimensions();
    let cell = f64::from(pet_size);
    LogicalSize::new(cell * f64::from(columns), cell * f64::from(rows))
}

// 物理像素尺寸 = 逻辑尺寸 × 当前显示器缩放比例，供 set_size/尺寸比较等需要物理像素的场景使用。
pub(crate) fn physical_pet_window_size(
    layout: PetLayout,
    pet_size: u16,
    scale: f64,
) -> PhysicalSize<u32> {
    let logical = logical_pet_window_size(layout, pet_size);
    PhysicalSize::new(
        (logical.width * scale).round() as u32,
        (logical.height * scale).round() as u32,
    )
}

// 把 OS 级别的窗口最小/最大尺寸设成当前显示器允许的区间，用户手动拖拽缩放时
// 就不可能拖出这个范围，不需要额外校验。
fn apply_pet_constraints_for_resolved_monitor(
    window: &WebviewWindow,
    monitor: Option<&Monitor>,
    layout: PetLayout,
) -> tauri::Result<()> {
    let (min, max) = pet_size_range_for_resolved_monitor(monitor, layout);
    window.set_min_size(Some(logical_pet_window_size(layout, min)))?;
    window.set_max_size(Some(logical_pet_window_size(layout, max)))
}

// 对外单次入口，内部自己查一次显示器；跟 pet_size_range 一样，高频路径请走 handle_pet_resize。
pub(crate) fn apply_pet_constraints(
    window: &WebviewWindow,
    layout: PetLayout,
) -> tauri::Result<()> {
    apply_pet_constraints_for_resolved_monitor(window, resolve_pet_monitor(window).as_ref(), layout)
}

// 原生 resize 会给任意矩形；按变化更大的边反推 pet_size，再纠正为当前布局的宽高比。
fn keep_pet_aspect_for_resolved_monitor(
    window: &WebviewWindow,
    size: PhysicalSize<u32>,
    runtime: &SharedRuntime,
    monitor: Option<&Monitor>,
) {
    // 只加一次锁就把本次要用的 layout 和上一次保存的几何都读出来，避免连续两次 lock。
    let (layout, previous) = runtime
        .windows
        .lock()
        .map(|windows| {
            let layout = windows.pet_window.layout;
            let previous = match layout {
                PetLayout::Single => windows.pet_window.single_geometry.clone(),
                PetLayout::Row => windows.pet_window.row_geometry.clone(),
                PetLayout::Column => windows.pet_window.column_geometry.clone(),
                PetLayout::Row3 => windows.pet_window.row3_geometry.clone(),
                PetLayout::Column3 => windows.pet_window.column3_geometry.clone(),
                PetLayout::Grid => windows.pet_window.grid_geometry.clone(),
            };
            (layout, previous)
        })
        .unwrap_or_default();
    // 把上一次保存的几何换算到当前显示器的缩放比例下，才能跟本次 resize 事件的物理像素比较。
    let previous = previous.map(|geometry| {
        size_for_scale(
            &geometry,
            window
                .scale_factor()
                .unwrap_or(geometry.scale_factor.max(1.0)),
        )
    });
    let scale = window.scale_factor().unwrap_or(1.0);
    let (rows, columns) = layout.dimensions();
    let width_candidate = f64::from(size.width) / f64::from(columns);
    let height_candidate = f64::from(size.height) / f64::from(rows);
    let requested_cell_size = previous.map_or(width_candidate, |previous| {
        if size.width.abs_diff(previous.width) >= size.height.abs_diff(previous.height) {
            width_candidate
        } else {
            height_candidate
        }
    });
    let (min, max) = pet_size_range_for_resolved_monitor(monitor, layout);
    let pet_size = ((requested_cell_size / scale).round() as u16).clamp(min, max);
    let expected = physical_pet_window_size(layout, pet_size, scale);
    // 差值在 1px 以内视为已符合比例（浮点换算误差），避免死循环触发 resize 事件。
    if size.width.abs_diff(expected.width) > 1 || size.height.abs_diff(expected.height) > 1 {
        let _ = window.set_size(expected);
    }
    // 尺寸真的变化了才落盘 + 广播，避免每次 resize tick 都写 preferences.json。
    if let Ok(mut windows) = runtime.windows.lock() {
        if windows.pet_window.pet_size != pet_size {
            windows.pet_window.pet_size = pet_size;
            runtime.window_changed();
        }
    }
}

// 跨显示器拖动后调用：先按新显示器重新设置 OS 级别的 min/max 尺寸约束，
// 再用当前窗口尺寸跑一遍“保持布局宽高比”的收敛逻辑（新显示器允许的区间可能比旧的更小）。
pub(crate) fn constrain_pet_to_current_monitor(window: &WebviewWindow, runtime: &SharedRuntime) {
    let monitor = resolve_pet_monitor(window); // 只查一次，下面两步复用同一个显示器
    let layout = runtime
        .windows
        .lock()
        .map(|windows| windows.pet_window.layout)
        .unwrap_or_default();
    let _ = apply_pet_constraints_for_resolved_monitor(window, monitor.as_ref(), layout);
    let Ok(size) = window.inner_size() else {
        return;
    };
    keep_pet_aspect_for_resolved_monitor(window, size, runtime, monitor.as_ref());
}

// 把窗口位置收敛回当前显示器工作区内（不改变尺寸），拖拽/缩放到屏幕边缘外时兜底拉回来。
fn clamp_window_to_work_area_for_resolved_monitor(
    window: &WebviewWindow,
    monitor: Option<&Monitor>,
) {
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

// resize 事件里宽高比纠正和工作区收敛都要用到当前显示器；
// 在这里只解析一次再分别传入，避免同一次事件里重复查询显示器。
pub(crate) fn handle_pet_resize(
    window: &WebviewWindow,
    size: PhysicalSize<u32>,
    runtime: &SharedRuntime,
) {
    let monitor = resolve_pet_monitor(window);
    keep_pet_aspect_for_resolved_monitor(window, size, runtime, monitor.as_ref());
    clamp_window_to_work_area_for_resolved_monitor(window, monitor.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_size_uses_uniform_minimum_and_layout_aware_maximum() {
        assert_eq!(maximum_pet_size(1_080, 1.0, PetLayout::Single), 270);
        assert_eq!(maximum_pet_size(1_080, 2.0, PetLayout::Single), 135);
        assert_eq!(maximum_pet_size(1_080, 1.0, PetLayout::Grid), 135);
        assert_eq!(maximum_pet_size(1_080, 1.0, PetLayout::Row3), 90);
        assert_eq!(
            logical_pet_window_size(PetLayout::Single, 32),
            LogicalSize::new(32.0, 32.0)
        );
        assert_eq!(
            logical_pet_window_size(PetLayout::Row, 32),
            LogicalSize::new(64.0, 32.0)
        );
        assert_eq!(
            logical_pet_window_size(PetLayout::Column, 32),
            LogicalSize::new(32.0, 64.0)
        );
        assert_eq!(
            logical_pet_window_size(PetLayout::Row3, 32),
            LogicalSize::new(96.0, 32.0)
        );
        assert_eq!(
            logical_pet_window_size(PetLayout::Column3, 32),
            LogicalSize::new(32.0, 96.0)
        );
        assert_eq!(
            logical_pet_window_size(PetLayout::Grid, 32),
            LogicalSize::new(64.0, 64.0)
        );
        assert_eq!(pet_size_range_fallback(PetLayout::Grid), (32, 180));
    }
}
