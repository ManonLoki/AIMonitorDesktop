// 桌面端运行时状态使用的数据模型：MonitorTile / MonitorState / WindowGeometry / Preferences。
// 这些类型通过 serde 在 Rust ↔ 前端 JSON 之间转换，字段命名需要和
// src/types/monitor.ts 中的 TypeScript 类型保持一致（camelCase 由 serde 自动转换而来）。
use serde::{Deserialize, Serialize};

// 单个监控宫格的数据。Default 用于清空宫格（DELETE /api/slots/{slot}）。
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitorTile {
    #[serde(skip)]
    pub(crate) client_id: String, // 控制端所有者，仅用于心跳租约清理
    pub(crate) username: String,               // 推送方设置的用户名
    pub(crate) ai_name: String,                // 推送方设置的 AI 名称
    pub(crate) content: String,                // 展示的正文内容
    pub(crate) image_filename: Option<String>, // 已上传图片的文件名，未设置则不展示图片
    pub(crate) updated_at_millis: Option<u64>, // 最近一次更新的毫秒时间戳，未设置表示宫格从未被写入
}

// 图片显示模式：等比缩放（留黑边）或铺满裁剪，序列化为大写下划线风格与 Android 版对齐。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")] // 序列化为 "FIT_CENTER" / "FILL_CROP"
pub(crate) enum ImageDisplayMode {
    FitCenter, // 保持比例完整显示，多余区域留黑边
    FillCrop,  // 保持比例铺满宫格，超出部分裁剪
}

impl Default for ImageDisplayMode {
    fn default() -> Self {
        Self::FitCenter // 默认使用留黑边的等比缩放模式
    }
}

// 桌面端运行时的完整状态快照，也是 Tauri 命令 get_monitor_state 的返回值。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitorState {
    pub(crate) rows: u8,                             // 当前宫格行数（1-5）
    pub(crate) columns: u8,                          // 当前宫格列数（1-5）
    pub(crate) image_display_mode: ImageDisplayMode, // 全局图片显示模式
    pub(crate) auto_start: bool,                     // 是否已启用开机自启
    pub(crate) port: u16,                            // 当前 HTTP 服务实际监听的端口
    pub(crate) app_version: String,                  // 应用版本号（来自 Cargo/Tauri 打包信息）
    pub(crate) device_id: String,                    // 持久化的设备唯一标识
    pub(crate) device_name: String,                  // 展示给局域网其他设备的名称
    pub(crate) is_server_running: bool,              // HTTP 服务器是否已完成绑定并开始监听
    pub(crate) local_ip: String,                     // 当前探测到的局域网 IPv4 地址
    pub(crate) tiles: Vec<MonitorTile>,              // 固定长度 25 的宫格数组
}

// 窗口位置与尺寸，用于跨会话恢复窗口摆放位置。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowGeometry {
    pub(crate) x: i32,      // 窗口左上角横坐标（物理像素）
    pub(crate) y: i32,      // 窗口左上角纵坐标（物理像素）
    pub(crate) width: u32,  // 窗口宽度（物理像素）
    pub(crate) height: u32, // 窗口高度（物理像素）
}

// 落盘到 preferences.json 的用户偏好，是 MonitorState 的一个持久化子集
// （端口、运行中标记、本机 IP、宫格内容都是运行期派生值，不需要持久化）。
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Preferences {
    pub(crate) rows: u8,                             // 持久化的宫格行数
    pub(crate) columns: u8,                          // 持久化的宫格列数
    pub(crate) image_display_mode: ImageDisplayMode, // 持久化的图片显示模式
    #[serde(default)] // 旧版本偏好文件里可能没有这个字段，缺省为 false
    pub(crate) auto_start: bool,
    #[serde(default)] // 旧版本偏好文件里可能没有窗口几何，缺省为 None（走默认摆放逻辑）
    pub(crate) window: Option<WindowGeometry>,
    pub(crate) device_id: String,   // 持久化的设备 ID，跨重启保持稳定
    pub(crate) device_name: String, // 持久化的设备名称
}
