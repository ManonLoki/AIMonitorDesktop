//! 主窗口与桌宠窗口的几何持久化：保存、跨 DPI 恢复和工作区收敛。
//! 桌宠尺寸约束/跨显示器收敛的数学在 pet_geometry.rs（职责分开，避免单文件超过 400 行）。

use crate::model::{PetLayout, WindowGeometry};
use crate::pet_geometry::{
    apply_pet_constraints, pet_size_range_for_monitor, physical_pet_window_size,
};
use crate::preferences::MainWindowPreferences;
use crate::runtime::SharedRuntime;
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

const MAIN_LABEL: &str = "main";
const PET_LABEL: &str = "pet";
// 判断上次保存的窗口矩形是否还落在这块显示器上：显示器被拔掉/换分辨率后，
// 保存的坐标可能落在虚空里，这时应该改用默认位置而不是把窗口摆到看不见的地方。
// 用 i64 计算是因为 x+width 这类求和在极端坐标下可能超出 i32 范围。
pub(crate) fn rectangles_have_visible_overlap(
    window: &WindowGeometry,
    monitor_x: i32,
    monitor_y: i32,
    monitor_width: u32,
    monitor_height: u32,
) -> bool {
    let left = i64::from(window.x).max(i64::from(monitor_x)); // 两个矩形重叠区域的左边界
    let top = i64::from(window.y).max(i64::from(monitor_y));
    let right = (i64::from(window.x) + i64::from(window.width))
        .min(i64::from(monitor_x) + i64::from(monitor_width));
    let bottom = (i64::from(window.y) + i64::from(window.height))
        .min(i64::from(monitor_y) + i64::from(monitor_height));
    // 不要求整个窗口都在屏幕内，只要至少 64 物理像素可见（或窗口本身更小）就算“够得着”，
    // 用户还能用鼠标拖回来；完全看不见才判定为需要重置到默认位置。
    let required_width = i64::from(window.width.min(64));
    let required_height = i64::from(window.height.min(64));
    right - left >= required_width && bottom - top >= required_height
}

// 把上次保存的物理像素尺寸换算到目标显示器的缩放比例下：同一份逻辑尺寸在
// 不同 DPI 显示器上的物理像素不同，直接套用旧的物理像素会导致窗口忽大忽小。
pub(crate) fn size_for_scale(geometry: &WindowGeometry, target_scale: f64) -> PhysicalSize<u32> {
    if geometry.scale_factor > 0.0 {
        let ratio = target_scale / geometry.scale_factor; // 新旧缩放比例的换算系数
        PhysicalSize::new(
            (f64::from(geometry.width) * ratio).round().max(1.0) as u32,
            (f64::from(geometry.height) * ratio).round().max(1.0) as u32,
        )
    } else {
        // 1.1 及更早版本没有 scaleFactor，宽高就是原始物理像素。
        PhysicalSize::new(geometry.width, geometry.height)
    }
}

// 在当前所有显示器里找出“上次保存的窗口矩形”还够得着的那一块，用于恢复时
// 判断该用保存的坐标，还是显示器已经变了、要退回默认位置。
fn monitor_index_for_geometry(window: &WebviewWindow, geometry: &WindowGeometry) -> Option<usize> {
    let monitors = window.available_monitors().ok()?;
    monitors.iter().position(|monitor| {
        let area = monitor.work_area();
        // 先把保存的尺寸换算到这块显示器的缩放比例下，再判断重叠，
        // 避免用错误的缩放比例得出错误的可见性结论。
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

// 把窗口左上角坐标收敛到工作区内，使整个窗口（按传入的 size）都落在可见范围。
// 窗口比工作区还大时，max_x/max_y 会被夹到 area_position，退化成贴左上角对齐。
pub(crate) fn clamped_position(
    x: i32,
    y: i32,
    size: PhysicalSize<u32>,
    area_position: PhysicalPosition<i32>,
    area_size: PhysicalSize<u32>,
) -> PhysicalPosition<i32> {
    let right = i64::from(area_position.x) + i64::from(area_size.width);
    let bottom = i64::from(area_position.y) + i64::from(area_size.height);
    let max_x = (right - i64::from(size.width)).max(i64::from(area_position.x)); // 允许的最大左上角 x，保证右边不越界
    let max_y = (bottom - i64::from(size.height)).max(i64::from(area_position.y));
    PhysicalPosition::new(
        i64::from(x).clamp(i64::from(area_position.x), max_x) as i32,
        i64::from(y).clamp(i64::from(area_position.y), max_y) as i32,
    )
}

// 应用启动/切回主窗口时调用：优先恢复上次保存的位置和尺寸（若所在显示器还在），
// 恢复后再按 maximized 标记决定是否最大化；找不到可用的保存位置就退回默认的
// “贴到主显示器工作区左上角并最大化”。
pub(crate) fn restore_main_window(
    window: &WebviewWindow,
    preferences: &MainWindowPreferences,
) -> tauri::Result<()> {
    if let Some(geometry) = preferences.normal_geometry.as_ref() {
        if let Some(index) = monitor_index_for_geometry(window, geometry) {
            let monitors = window.available_monitors()?;
            let monitor = &monitors[index];
            let size = size_for_scale(geometry, monitor.scale_factor());
            window.unmaximize()?; // 先取消最大化才能自由 set_size/set_position
            window.set_size(size)?;
            window.set_position(clamped_position(
                geometry.x,
                geometry.y,
                size,
                monitor.work_area().position,
                monitor.work_area().size,
            ))?;
            if preferences.maximized {
                window.maximize()?; // 恢复到正确位置后再最大化，避免在错误的显示器上最大化
            }
            return Ok(());
        }
    }

    // 没有可用的保存几何（首次启动或显示器已变化）：贴主显示器工作区左上角并直接最大化。
    if let Some(primary) = window.primary_monitor()? {
        window.set_position(primary.work_area().position)?;
    }
    window.maximize()
}

// 切到桌宠模式（或该布局第一次显示）时调用：优先恢复该布局上次保存的位置/尺寸；
// 找不到可用的保存位置（首次使用该布局，或显示器已变化）就退回默认位置——
// 贴当前/主显示器工作区右下角，留出 16 逻辑像素的边距，方便用户第一眼就能找到桌宠。
pub(crate) fn restore_pet_window(
    window: &WebviewWindow,
    geometry: Option<&WindowGeometry>,
    layout: PetLayout,
    pet_size: u16,
) -> tauri::Result<()> {
    apply_pet_constraints(window, layout)?; // 先设好这块显示器允许的 min/max，再 set_size 才不会被系统钳制
    if let Some(geometry) = geometry {
        if let Some(index) = monitor_index_for_geometry(window, geometry) {
            let monitors = window.available_monitors()?;
            let monitor = &monitors[index];
            let saved_size = size_for_scale(geometry, monitor.scale_factor());
            let (min, max) = pet_size_range_for_monitor(monitor, layout);
            let scale = monitor.scale_factor();
            // 从窗口实际宽度反推单格边长，可自然迁移旧版“整体画布尺寸”的持久化几何。
            let (_, columns) = layout.dimensions();
            let cell_size = f64::from(saved_size.width) / f64::from(columns);
            let pet_size = (cell_size / scale).round() as u16;
            let size = physical_pet_window_size(layout, pet_size.clamp(min, max), scale);
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

    // 默认位置分支：优先用主显示器，取不到再退回窗口当前所在的显示器。
    let monitor = window
        .primary_monitor()?
        .or(window.current_monitor()?)
        .ok_or_else(|| tauri::Error::FailedToReceiveMessage)?;
    let (min, max) = pet_size_range_for_monitor(&monitor, layout);
    let scale = monitor.scale_factor();
    let size = physical_pet_window_size(layout, pet_size.clamp(min, max), scale);
    let area = monitor.work_area();
    let margin = (16.0 * scale).round() as i32; // 16 逻辑像素的贴边距，避免正好贴住屏幕边缘
    window.set_size(size)?;
    window.set_position(PhysicalPosition::new(
        area.position.x + area.size.width as i32 - size.width as i32 - margin,
        area.position.y + area.size.height as i32 - size.height as i32 - margin,
    ))
}

// 把窗口当前的位置/尺寸（或主窗口的最大化标记）写回内存中的 windows 状态，
// 不负责落盘——落盘由调用方通过 save_window_state 决定是否需要防抖。
pub(crate) fn capture_window_state(window: &WebviewWindow, runtime: &SharedRuntime) {
    if window.is_minimized().unwrap_or(false) {
        return; // 最小化时的坐标/尺寸没有意义，不覆盖已保存的正常状态
    }
    let mut windows = runtime.windows.lock();
    if window.label() == MAIN_LABEL {
        windows.main_window.maximized = window.is_maximized().unwrap_or(false);
        if windows.main_window.maximized {
            return; // 最大化时不记录矩形，保留上一次非最大化时的位置，下次取消最大化能恢复回去
        }
    }
    let (Ok(position), Ok(size)) = (window.outer_position(), window.inner_size()) else {
        return;
    };
    if size.width == 0 || size.height == 0 {
        return; // 窗口刚创建还未真正 map 时可能读到 0，不能当作有效状态保存
    }
    let geometry = WindowGeometry {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        scale_factor: window.scale_factor().unwrap_or(1.0),
    };
    // 主窗口只有一份几何；桌宠窗口按当前布局分别保存到六个槽位，
    // 这样切换布局后各自都能恢复到上次的位置，不会互相覆盖。
    match window.label() {
        MAIN_LABEL => windows.main_window.normal_geometry = Some(geometry),
        PET_LABEL => match windows.pet_window.layout {
            PetLayout::Single => windows.pet_window.single_geometry = Some(geometry),
            PetLayout::Row => windows.pet_window.row_geometry = Some(geometry),
            PetLayout::Column => windows.pet_window.column_geometry = Some(geometry),
            PetLayout::Row3 => windows.pet_window.row3_geometry = Some(geometry),
            PetLayout::Column3 => windows.pet_window.column3_geometry = Some(geometry),
            PetLayout::Grid => windows.pet_window.grid_geometry = Some(geometry),
        },
        _ => {} // pet-settings 窗口不持久化几何，每次都重新居中显示
    }
}

// immediate=true 用于关闭窗口等必须立刻落盘的场景；false 用于 resize/move 这类高频事件，
// 走 save_preferences_debounced 合并连续事件，避免每个像素的移动都写一次磁盘。
pub(crate) fn save_window_state(window: &WebviewWindow, runtime: &SharedRuntime, immediate: bool) {
    capture_window_state(window, runtime);
    if immediate {
        runtime.save_preferences();
    } else {
        runtime.save_preferences_debounced();
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
}
