// —— 以下 5 个 #[tauri::command] 是前端唯一能触达 Rust 状态的入口 ——
// 每个命令的写法都遵循同一套约定：加写锁改状态 → 落盘 preferences → 广播 changed 事件。
use crate::model::{ImageDisplayMode, MonitorState};
use crate::runtime::SharedRuntime;
use tauri::State;

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
    runtime.save_preferences(); // 持久化到 preferences.json
    runtime.changed(); // 通知前端刷新
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
    enabled: bool,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt; // 引入 autolaunch() 扩展方法

    // 开机自启由操作系统层面的注册项/LaunchAgent 管理，这里只是转发给插件，
    // 状态字段 auto_start 仅用于前端展示当前值。
    let manager = runtime.app.autolaunch(); // 拿到自启动插件的管理器
    if enabled {
        manager.enable().map_err(|error| error.to_string())?; // 向操作系统注册开机自启项
    } else {
        manager.disable().map_err(|error| error.to_string())?; // 从操作系统移除开机自启项
    }
    runtime.state.write().map_err(|_| "状态不可用")?.auto_start = enabled; // 同步内存中的展示状态
    runtime.save_preferences(); // 持久化
    runtime.changed(); // 通知前端刷新
    Ok(())
}
