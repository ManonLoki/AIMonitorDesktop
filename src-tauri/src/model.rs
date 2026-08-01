// 桌面端运行期状态使用的数据模型：监控快照、窗口模式与几何。落盘的用户偏好
// （MainWindowPreferences/PetWindowPreferences/WindowPreferences/Preferences）在 preferences.rs。
// 这些类型通过 serde 在 Rust ↔ 前端 JSON 之间转换，字段命名需要和
// src/types/monitor.ts 中的 TypeScript 类型保持一致（camelCase 由 serde 自动转换而来）。
use crate::preferences::PetWindowPreferences;
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

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowState {
    pub(crate) active_mode: AppMode,
    pub(crate) pet_window: PetWindowPreferences,
    pub(crate) pet_size_min: u16,
    pub(crate) pet_size_max: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

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
