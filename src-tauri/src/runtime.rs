// 贯穿 HTTP、心跳清理、UDP 与 mDNS 后台服务的共享运行时上下文及其持久化逻辑。
// 各服务都通过 SharedRuntime（即 Arc<Runtime>）读写同一份状态。
use crate::constants::FIRST_HTTP_PORT;
use crate::device_info::{default_device_name, local_ipv4};
use crate::heartbeat::ClientLeases;
use crate::model::{MonitorState, MonitorTile, Preferences, WindowPreferences, WindowState};
use parking_lot::{Mutex, RwLock};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;
use uuid::Uuid;

// 贯穿后台服务共享的运行时上下文，以 Arc 的形式跨任务/线程传递。
pub(crate) struct Runtime {
    // 用 RwLock 而非 Mutex：HTTP 读请求（GET /api/device 等）远多于写请求，
    // 读写锁允许多个只读请求并发执行，只有修改宫格/设置时才需要独占锁。
    pub(crate) state: RwLock<MonitorState>,
    pub(crate) image_dir: PathBuf, // 图片文件落盘目录（应用缓存目录下）
    preferences_path: PathBuf,     // preferences.json 的完整路径，只在本文件内使用
    pub(crate) windows: Mutex<WindowPreferences>, // 主窗口与桌宠窗口的独立持久化状态
    preferences_save_lock: Mutex<()>, // 串行化“快照 + 写盘”，防止旧快照后写覆盖新值
    save_generation: AtomicU64,    // 移动/缩放写盘防抖代次
    pub(crate) client_leases: Mutex<ClientLeases>, // 控制端心跳租约，跨 HTTP 请求共享
    pub(crate) app: AppHandle,     // 用于向前端发送事件、访问自启动插件
}

// 各子系统之间共享同一份运行时的类型别名。
pub(crate) type SharedRuntime = Arc<Runtime>;

impl Runtime {
    // 组装初始运行时状态：合并落盘的偏好设置、当前自启动开关的真实值、
    // 应用版本号与刚探测到的局域网 IP，返回可直接 clone 的共享句柄。
    pub(crate) fn new(
        app: AppHandle,
        image_dir: PathBuf,
        preferences_path: PathBuf,
        mut preferences: Preferences,
        auto_start: bool,
        app_version: String,
    ) -> SharedRuntime {
        // 旧版只有顶层 window 字段，首次加载时无损迁移到 mainWindow.normalGeometry。
        if preferences.windows.main_window.normal_geometry.is_none() {
            preferences.windows.main_window.normal_geometry = preferences.window.take();
        }
        Arc::new(Self {
            state: RwLock::new(MonitorState {
                rows: preferences.rows.clamp(1, 5), // 防御性 clamp：即便偏好文件被手工改坏也不会越界
                columns: preferences.columns.clamp(1, 5),
                image_display_mode: preferences.image_display_mode,
                auto_start,
                language: preferences.language,
                // 端口先占位，等 start_http_server 实际绑定成功后再回填真实值。
                port: FIRST_HTTP_PORT,
                app_version,
                device_id: preferences.device_id,
                device_name: preferences.device_name,
                is_server_running: false, // HTTP 服务器尚未启动
                local_ip: local_ipv4(),   // 启动时探测一次局域网 IP
                tiles: vec![MonitorTile::default(); 25], // 固定 25 个空宫格
            }),
            image_dir,
            preferences_path,
            windows: Mutex::new(preferences.windows),
            preferences_save_lock: Mutex::new(()),
            save_generation: AtomicU64::new(0),
            client_leases: Mutex::new(ClientLeases::default()),
            app,
        })
    }

    // 克隆一份当前状态用于响应请求/广播，避免长时间持有锁阻塞其他线程。
    pub(crate) fn snapshot(&self) -> MonitorState {
        self.state.read().clone() // 读锁 + clone，锁在函数返回前就已释放
    }

    // 把当前状态中可持久化的字段写入 preferences.json；调用方在每次状态变更后触发。
    pub(crate) fn save_preferences(&self) {
        // 所有调用方都在进入此函数前释放 state/windows；此处统一按
        // save -> state -> windows 取锁，同时把快照和 persist 纳入一个串行临界区。
        let _save_guard = self.preferences_save_lock.lock();
        let preferences = {
            let state = self.state.read();
            let windows = self.windows.lock();
            Preferences {
                rows: state.rows,
                columns: state.columns,
                image_display_mode: state.image_display_mode.clone(),
                auto_start: state.auto_start,
                language: state.language,
                windows: windows.clone(),
                window: None,
                device_id: state.device_id.clone(),
                device_name: state.device_name.clone(),
            }
        };
        if let Some(parent) = self.preferences_path.parent() {
            let _ = fs::create_dir_all(parent); // 确保配置目录存在（首次启动时可能还未创建）
        }
        if let (Ok(bytes), Some(parent)) = (
            serde_json::to_vec_pretty(&preferences),
            self.preferences_path.parent(),
        ) {
            // 同目录临时文件 + persist：Unix 使用 rename，Windows 使用 ReplaceFile，
            // 避免异常退出留下半截 preferences.json。
            if let Ok(mut temp) = tempfile::NamedTempFile::new_in(parent) {
                if temp.write_all(&bytes).is_ok() && temp.as_file().sync_all().is_ok() {
                    let _ = temp.persist(&self.preferences_path);
                }
            }
        }
    }

    /// 窗口移动/缩放会产生高频事件；只让最后一次事件在 400ms 后真正写盘。
    pub(crate) fn save_preferences_debounced(self: &Arc<Self>) {
        let generation = self.save_generation.fetch_add(1, Ordering::Relaxed) + 1;
        let runtime = self.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            if runtime.save_generation.load(Ordering::Relaxed) == generation {
                runtime.save_preferences();
            }
        });
    }

    // pet_size_min/max 依赖当前显示器，只有拿到桌宠窗口句柄才能算准；
    // 算不到时回退到 pet_geometry 里唯一的兜底值，而不是在这里再写一份。
    pub(crate) fn window_snapshot(&self) -> WindowState {
        let (active_mode, pet_window) = {
            let windows = self.windows.lock();
            (windows.active_mode, windows.pet_window.clone()) // clone 完立刻释放锁，再去查窗口句柄
        };
        // 拿不到桌宠窗口句柄（理论上不会发生，三个窗口在 setup 里就创建好了）时用兜底区间；
        // 拿得到就按它当前所在的显示器精确计算允许的尺寸范围。
        let (pet_size_min, pet_size_max) = self.app.get_webview_window("pet").map_or_else(
            || crate::pet_geometry::pet_size_range_fallback(pet_window.layout),
            |window| crate::pet_geometry::pet_size_range(&window, pet_window.layout),
        );
        WindowState {
            active_mode,
            pet_window,
            pet_size_min,
            pet_size_max,
        }
    }

    pub(crate) fn window_changed(&self) {
        let _ = self.app.emit("window-state-changed", ());
    }

    // 通知前端状态已变化；前端收到后会重新调用 get_monitor_state 拉取最新快照。
    pub(crate) fn changed(&self) {
        let _ = self.app.emit("monitor-state-changed", ()); // 事件无载荷，前端收到后自行重新拉取全量状态
    }

    // 修改操作系统的开机自启注册项，并同步内存状态、偏好文件和前端展示。
    pub(crate) fn set_auto_start(&self, enabled: bool) -> Result<(), String> {
        let manager = self.app.autolaunch();
        if enabled {
            manager.enable().map_err(|error| error.to_string())?;
        } else {
            manager.disable().map_err(|error| error.to_string())?;
        }
        self.state.write().auto_start = enabled;
        self.save_preferences();
        self.changed();
        Ok(())
    }

    // 续租控制端心跳；HTTP 端两个入口（心跳接口、更新槽位）共用同一处加锁逻辑。
    pub(crate) fn heartbeat_client(&self, client_id: &str) {
        self.client_leases.lock().heartbeat(client_id);
    }
}

fn default_preferences() -> Preferences {
    Preferences {
        rows: 2,                                                       // 默认 2 行
        columns: 2,                                                    // 默认 2 列
        image_display_mode: crate::model::ImageDisplayMode::default(), // 默认等比缩放
        auto_start: false,                                             // 默认不开机自启
        language: crate::model::LanguagePreference::default(),         // 默认跟随系统语言
        windows: WindowPreferences::default(),
        window: None,
        device_id: Uuid::new_v4().to_string(), // 首次启动生成一个新的随机设备 ID
        device_name: default_device_name(),    // 首次启动用主机名作为默认设备名
    }
}

// serde_with 会把类型损坏的标量恢复为该类型的 Default；这里再把那些“类型默认值”
// 不等于产品默认值的字段归一化，避免 0 行宫格或空设备身份进入运行时。
fn normalize_preferences(mut preferences: Preferences) -> Preferences {
    if !(1..=5).contains(&preferences.rows) {
        preferences.rows = 2;
    }
    if !(1..=5).contains(&preferences.columns) {
        preferences.columns = 2;
    }
    if preferences.device_id.trim().is_empty() {
        preferences.device_id = Uuid::new_v4().to_string();
    }
    if preferences.device_name.trim().is_empty() {
        preferences.device_name = default_device_name();
    }
    preferences
}

// 读取 preferences.json；完整文件损坏时回退全部默认值，单个字段类型损坏则由
// serde_with 在字段/子对象边界恢复，只重置受影响部分而保留设备身份和其他偏好。
pub(crate) fn load_preferences(path: &Path) -> Preferences {
    let preferences = fs::read(path)
        .ok() // 文件不存在/读取失败则转为 None
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(default_preferences);
    normalize_preferences(preferences)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_field_is_normalized_without_losing_device_identity() {
        let directory = tempfile::tempdir().expect("temporary directory is available");
        let path = directory.path().join("preferences.json");
        fs::write(
            &path,
            r#"{
                "rows": { "invalid": true },
                "columns": 3,
                "deviceId": "stable-device-id",
                "deviceName": "studio-monitor"
            }"#,
        )
        .expect("test preferences are writable");

        let preferences = load_preferences(&path);

        assert_eq!(preferences.rows, 2);
        assert_eq!(preferences.columns, 3);
        assert_eq!(preferences.device_id, "stable-device-id");
        assert_eq!(preferences.device_name, "studio-monitor");
    }

    #[test]
    fn empty_recovered_identity_receives_safe_runtime_defaults() {
        let preferences: Preferences = serde_json::from_value(serde_json::json!({
            "rows": 2,
            "columns": 2,
            "deviceId": null,
            "deviceName": "   "
        }))
        .expect("field-level recovery keeps the document readable");

        let preferences = normalize_preferences(preferences);

        assert!(Uuid::parse_str(&preferences.device_id).is_ok());
        assert!(!preferences.device_name.trim().is_empty());
    }
}
