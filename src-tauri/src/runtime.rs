// 贯穿 HTTP/UDP/mDNS 三个子系统共享的运行时上下文，以及它的持久化逻辑。
// 三个子系统各自的实现分别在 http/discovery/mdns 模块，都通过 SharedRuntime
// （即 Arc<Runtime>）读写同一份状态。
use crate::constants::FIRST_HTTP_PORT;
use crate::device_info::{default_device_name, local_ipv4};
use crate::heartbeat::ClientLeases;
use crate::model::{MonitorState, MonitorTile, Preferences, WindowGeometry};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

// 贯穿三个子系统共享的运行时上下文，以 Arc 的形式在线程间传递。
pub(crate) struct Runtime {
    // 用 RwLock 而非 Mutex：HTTP 读请求（GET /api/device 等）远多于写请求，
    // 读写锁允许多个只读请求并发执行，只有修改宫格/设置时才需要独占锁。
    pub(crate) state: RwLock<MonitorState>,
    pub(crate) image_dir: PathBuf, // 图片文件落盘目录（应用缓存目录下）
    preferences_path: PathBuf,     // preferences.json 的完整路径，只在本文件内使用
    pub(crate) window_geometry: Mutex<Option<WindowGeometry>>, // 待落盘的最新窗口几何
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
        preferences: Preferences,
        auto_start: bool,
        app_version: String,
    ) -> SharedRuntime {
        Arc::new(Self {
            state: RwLock::new(MonitorState {
                rows: preferences.rows.clamp(1, 5), // 防御性 clamp：即便偏好文件被手工改坏也不会越界
                columns: preferences.columns.clamp(1, 5),
                image_display_mode: preferences.image_display_mode,
                auto_start,
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
            window_geometry: Mutex::new(preferences.window), // 初始值来自历史偏好，之后由窗口事件持续更新
            client_leases: Mutex::new(ClientLeases::default()),
            app,
        })
    }

    // 克隆一份当前状态用于响应请求/广播，避免长时间持有锁阻塞其他线程。
    pub(crate) fn snapshot(&self) -> MonitorState {
        self.state.read().expect("state lock poisoned").clone() // 读锁 + clone，锁在函数返回前就已释放
    }

    // 把当前状态中可持久化的字段写入 preferences.json；调用方在每次状态变更后触发。
    pub(crate) fn save_preferences(&self) {
        let state = self.snapshot(); // 先拿到状态快照，避免在构造 Preferences 时持锁
        let preferences = Preferences {
            rows: state.rows,
            columns: state.columns,
            image_display_mode: state.image_display_mode,
            auto_start: state.auto_start,
            window: self
                .window_geometry
                .lock()
                .expect("window geometry lock poisoned")
                .clone(), // 单独加锁读取窗口几何，与 state 锁互不干扰
            device_id: state.device_id,
            device_name: state.device_name,
        };
        if let Some(parent) = self.preferences_path.parent() {
            let _ = fs::create_dir_all(parent); // 确保配置目录存在（首次启动时可能还未创建）
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(&preferences) {
            let _ = fs::write(&self.preferences_path, bytes); // 序列化失败或写盘失败都静默忽略，不影响主流程
        }
    }

    // 通知前端状态已变化；前端收到后会重新调用 get_monitor_state 拉取最新快照。
    pub(crate) fn changed(&self) {
        let _ = self.app.emit("monitor-state-changed", ()); // 事件无载荷，前端收到后自行重新拉取全量状态
    }

    // 续租控制端心跳；HTTP 端两个入口（心跳接口、更新槽位）共用同一处加锁逻辑。
    pub(crate) fn heartbeat_client(&self, client_id: &str) {
        self.client_leases
            .lock()
            .expect("client lease lock poisoned")
            .heartbeat(client_id);
    }
}

// 读取 preferences.json；文件不存在或损坏时回退到一组合理默认值（含随机生成的新设备 ID）。
pub(crate) fn load_preferences(path: &Path) -> Preferences {
    fs::read(path)
        .ok() // 文件不存在/读取失败则转为 None
        .and_then(|bytes| serde_json::from_slice(&bytes).ok()) // 内容不是合法 JSON 或字段不匹配也转为 None
        .unwrap_or_else(|| Preferences {
            rows: 2,                                                       // 默认 2 行
            columns: 2,                                                    // 默认 2 列
            image_display_mode: crate::model::ImageDisplayMode::default(), // 默认等比缩放
            auto_start: false,                                             // 默认不开机自启
            window: None, // 默认无历史窗口几何，交给 restore_window 走最大化兜底
            device_id: Uuid::new_v4().to_string(), // 首次启动生成一个新的随机设备 ID
            device_name: default_device_name(), // 首次启动用主机名作为默认设备名
        })
}
