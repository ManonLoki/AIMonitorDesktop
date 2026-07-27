// 窗口位置/尺寸的跨会话恢复与持久化：判断历史坐标是否仍落在可用显示器内，
// 启动时按此决定恢复位置还是走"主显示器 + 最大化"的默认策略。
use crate::model::WindowGeometry;
use crate::runtime::SharedRuntime;
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

// 判断保存的窗口矩形与某个显示器工作区是否有"足够可见"的重叠——
// 用于窗口恢复时排除已经被移除/断开的显示器上的历史坐标。
pub(crate) fn rectangles_have_visible_overlap(
    window: &WindowGeometry,
    monitor_x: i32,
    monitor_y: i32,
    monitor_width: u32,
    monitor_height: u32,
) -> bool {
    let left = i64::from(window.x).max(i64::from(monitor_x)); // 重叠区域左边界：取两矩形左边界的较大值
    let top = i64::from(window.y).max(i64::from(monitor_y)); // 重叠区域上边界：取两矩形上边界的较大值
    let right = (i64::from(window.x) + i64::from(window.width))
        .min(i64::from(monitor_x) + i64::from(monitor_width)); // 重叠区域右边界：取两矩形右边界的较小值
    let bottom = (i64::from(window.y) + i64::from(window.height))
        .min(i64::from(monitor_y) + i64::from(monitor_height)); // 重叠区域下边界：取两矩形下边界的较小值
    // 至少要有 64x64 像素的重叠区域才算"可见"，避免窗口只露出一条边缘的边界情况。
    let required_width = i64::from(window.width.min(64)); // 窗口本身若小于 64px 则要求完全重叠
    let required_height = i64::from(window.height.min(64));
    right - left >= required_width && bottom - top >= required_height // 重叠区域的宽高都需达到阈值
}

// 保存的窗口几何是否落在当前任意一块可用显示器范围内。
fn window_geometry_is_available(window: &WebviewWindow, geometry: &WindowGeometry) -> bool {
    if geometry.width == 0 || geometry.height == 0 {
        return false; // 尺寸为 0 的历史记录视为无效
    }
    window
        .available_monitors() // 查询当前所有已连接显示器
        .map(|monitors| {
            monitors.iter().any(|monitor| {
                let area = monitor.work_area(); // 排除任务栏/Dock 之后的可用工作区
                rectangles_have_visible_overlap(
                    geometry,
                    area.position.x,
                    area.position.y,
                    area.size.width,
                    area.size.height,
                )
            })
        })
        .unwrap_or(false) // 查询显示器失败时保守地认为不可用，走默认摆放逻辑
}

// 应用启动时恢复窗口：优先使用保存的几何信息（前提是仍在可用显示器范围内），
// 否则回退到"主显示器 + 最大化"，满足产品要求里"启动即最大化"的强制项。
pub(crate) fn restore_window(
    window: &WebviewWindow,
    geometry: Option<&WindowGeometry>,
) -> tauri::Result<()> {
    if let Some(geometry) =
        geometry.filter(|geometry| window_geometry_is_available(window, geometry)) // 历史几何存在且仍落在可用显示器内
    {
        window.unmaximize()?; // 先取消可能残留的最大化状态，才能设置自定义大小/位置
        window.set_size(PhysicalSize::new(geometry.width, geometry.height))?;
        window.set_position(PhysicalPosition::new(geometry.x, geometry.y))?;
        return Ok(()); // 已按历史几何恢复，不再走最大化兜底
    }

    if let Some(primary_monitor) = window.primary_monitor()? {
        let position = primary_monitor.work_area().position; // 把窗口先移动到主显示器工作区起点
        window.set_position(PhysicalPosition::new(position.x, position.y))?;
    }
    window.maximize() // 产品要求：无有效历史几何时必须以最大化状态启动
}

// 窗口移动/缩放/关闭时调用：只在非最小化、非最大化状态下记录几何信息，
// 因为最大化时的 outer_position/inner_size 并不代表用户期望的"正常窗口大小"。
pub(crate) fn save_window_geometry(window: &WebviewWindow, runtime: &SharedRuntime) {
    if window.is_minimized().unwrap_or(false) || window.is_maximized().unwrap_or(false) {
        return; // 最小化/最大化状态下的几何数据没有参考价值，跳过保存
    }
    let (Ok(position), Ok(size)) = (window.outer_position(), window.inner_size()) else {
        return; // 查询失败（例如窗口正在关闭过程中）则放弃本次保存
    };
    if size.width == 0 || size.height == 0 {
        return; // 尺寸异常为 0，不值得保存
    }
    *runtime
        .window_geometry
        .lock()
        .expect("window geometry lock poisoned") = Some(WindowGeometry {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    }); // 更新内存中待落盘的窗口几何
    runtime.save_preferences(); // 立即落盘，避免应用异常退出导致这次几何变化丢失
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_geometry_with_a_reachable_window_area() {
        let geometry = WindowGeometry {
            x: 1_856, // 窗口大部分落在 1920x1080 显示器范围内
            y: 100,
            width: 1_000,
            height: 700,
        };

        assert!(rectangles_have_visible_overlap(
            &geometry, 0, 0, 1_920, 1_080
        )); // 应判定为可见
    }

    #[test]
    fn rejects_geometry_left_on_a_removed_monitor() {
        let geometry = WindowGeometry {
            x: 1_900, // 窗口几乎完全落在显示器边界之外
            y: 100,
            width: 1_000,
            height: 700,
        };

        assert!(!rectangles_have_visible_overlap(
            &geometry, 0, 0, 1_920, 1_080
        )); // 应判定为不可见（视为已断开的旧显示器坐标）
    }
}
