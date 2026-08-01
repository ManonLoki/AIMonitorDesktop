// 桌面端运行时状态使用的数据模型：监控数据、窗口模式与持久化偏好。
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
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")] // 序列化为 "FIT_CENTER" / "FILL_CROP"
pub(crate) enum ImageDisplayMode {
    #[default]
    FitCenter, // 保持比例完整显示，多余区域留黑边
    FillCrop, // 保持比例铺满宫格，超出部分裁剪
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum LanguagePreference {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "zh-CN")]
    Chinese,
    #[serde(rename = "en")]
    English,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub(crate) enum ResolvedLocale {
    #[default]
    #[serde(rename = "zh-CN")]
    Chinese,
    #[serde(rename = "en")]
    English,
}

// 桌面端运行时的完整状态快照，也是 Tauri 命令 get_monitor_state 的返回值。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MonitorState {
    pub(crate) rows: u8,                             // 当前宫格行数（1-5）
    pub(crate) columns: u8,                          // 当前宫格列数（1-5）
    pub(crate) image_display_mode: ImageDisplayMode, // 全局图片显示模式
    pub(crate) auto_start: bool,                     // 是否已启用开机自启
    pub(crate) language: LanguagePreference,         // 界面语言偏好，默认跟随系统
    pub(crate) port: u16,                            // 当前 HTTP 服务实际监听的端口
    pub(crate) app_version: String,                  // 应用版本号（来自 Cargo/Tauri 打包信息）
    pub(crate) device_id: String,                    // 持久化的设备唯一标识
    pub(crate) device_name: String,                  // 展示给局域网其他设备的名称
    pub(crate) is_server_running: bool,              // HTTP 服务器是否已完成绑定并开始监听
    pub(crate) local_ip: String,                     // 当前探测到的局域网 IPv4 地址
    pub(crate) tiles: Vec<MonitorTile>,              // 固定长度 25 的宫格数组
}

// 窗口位置与尺寸，用于跨会话恢复窗口摆放位置。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowGeometry {
    pub(crate) x: i32,      // 窗口左上角横坐标（物理像素）
    pub(crate) y: i32,      // 窗口左上角纵坐标（物理像素）
    pub(crate) width: u32,  // 窗口宽度（物理像素）
    pub(crate) height: u32, // 窗口高度（物理像素）
    #[serde(default)]
    pub(crate) scale_factor: f64, // 保存时的 DPI 缩放；0 表示来自旧版偏好的物理尺寸
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AppMode {
    #[default]
    Main,
    Pet,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PetLayout {
    Single,
    Row,
    Column,
    Row3,
    Column3,
    #[default]
    Grid,
}

impl PetLayout {
    pub(crate) const fn dimensions(self) -> (u16, u16) {
        match self {
            Self::Single => (1, 1),
            Self::Row => (1, 2),
            Self::Column => (2, 1),
            Self::Row3 => (1, 3),
            Self::Column3 => (3, 1),
            Self::Grid => (2, 2),
        }
    }

    pub(crate) const fn capacity(self) -> usize {
        let (rows, columns) = self.dimensions();
        rows as usize * columns as usize
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainWindowPreferences {
    #[serde(default)]
    pub(crate) normal_geometry: Option<WindowGeometry>,
    #[serde(default)]
    pub(crate) maximized: bool,
}

fn default_pet_size() -> u16 {
    64
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PetWindowPreferences {
    #[serde(default)]
    pub(crate) layout: PetLayout,
    #[serde(default)]
    pub(crate) focused_slot: u8,
    #[serde(default)]
    pub(crate) single_geometry: Option<WindowGeometry>,
    #[serde(default)]
    pub(crate) row_geometry: Option<WindowGeometry>,
    #[serde(default)]
    pub(crate) column_geometry: Option<WindowGeometry>,
    #[serde(default)]
    pub(crate) row3_geometry: Option<WindowGeometry>,
    #[serde(default)]
    pub(crate) column3_geometry: Option<WindowGeometry>,
    #[serde(default)]
    pub(crate) grid_geometry: Option<WindowGeometry>,
    #[serde(default = "default_pet_size")]
    pub(crate) pet_size: u16,
    #[serde(default = "default_true")]
    pub(crate) always_on_top: bool,
    #[serde(default)]
    pub(crate) locked: bool,
}

impl Default for PetWindowPreferences {
    fn default() -> Self {
        Self {
            layout: PetLayout::Grid,
            focused_slot: 0,
            single_geometry: None,
            row_geometry: None,
            column_geometry: None,
            row3_geometry: None,
            column3_geometry: None,
            grid_geometry: None,
            pet_size: default_pet_size(),
            always_on_top: true,
            locked: false,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowPreferences {
    #[serde(default)]
    pub(crate) active_mode: AppMode,
    #[serde(default)]
    pub(crate) main_window: MainWindowPreferences,
    #[serde(default)]
    pub(crate) pet_window: PetWindowPreferences,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowState {
    pub(crate) active_mode: AppMode,
    pub(crate) pet_window: PetWindowPreferences,
    pub(crate) pet_size_min: u16,
    pub(crate) pet_size_max: u16,
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
    #[serde(default)]
    pub(crate) language: LanguagePreference,
    #[serde(default)]
    pub(crate) windows: WindowPreferences,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) window: Option<WindowGeometry>, // 仅用于迁移 1.1 及更早版本
    pub(crate) device_id: String,   // 持久化的设备 ID，跨重启保持稳定
    pub(crate) device_name: String, // 持久化的设备名称
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_preferences_receive_safe_window_defaults() {
        let preferences: Preferences = serde_json::from_value(serde_json::json!({
            "rows": 2,
            "columns": 2,
            "imageDisplayMode": "FIT_CENTER",
            "autoStart": false,
            "window": { "x": 20, "y": 30, "width": 800, "height": 600 },
            "deviceId": "device",
            "deviceName": "monitor"
        }))
        .expect("legacy preferences should deserialize");

        assert_eq!(preferences.windows.active_mode, AppMode::Main);
        assert_eq!(preferences.language, LanguagePreference::System);
        assert_eq!(preferences.windows.pet_window.layout, PetLayout::Grid);
        assert!(preferences.windows.pet_window.always_on_top);
        assert_eq!(
            preferences.window.expect("legacy geometry").scale_factor,
            0.0
        );
    }

    #[test]
    fn window_state_uses_frontend_enum_names() {
        let state = WindowState {
            active_mode: AppMode::Pet,
            pet_window: PetWindowPreferences::default(),
            pet_size_min: 256,
            pet_size_max: 270,
        };
        let value = serde_json::to_value(state).expect("window state serializes");
        assert_eq!(value["activeMode"], "pet");
        assert_eq!(value["petWindow"]["layout"], "grid");
        assert_eq!(serde_json::to_value(PetLayout::Row3).unwrap(), "row3");
        assert_eq!(serde_json::to_value(PetLayout::Column3).unwrap(), "column3");
        assert_eq!(
            serde_json::to_value(LanguagePreference::System).unwrap(),
            "system"
        );
        assert_eq!(
            serde_json::to_value(LanguagePreference::Chinese).unwrap(),
            "zh-CN"
        );
        assert_eq!(
            serde_json::to_value(LanguagePreference::English).unwrap(),
            "en"
        );
    }
}
