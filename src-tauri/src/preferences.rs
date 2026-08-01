// 落盘到 preferences.json 的用户偏好：窗口/桌宠摆放、宫格与设备身份的持久化子集。
// 与 model.rs 中运行期派生的状态类型（MonitorState/WindowState 等）分开维护。
use crate::model::{AppMode, ImageDisplayMode, LanguagePreference, PetLayout, WindowGeometry};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DefaultOnError};

#[serde_as]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainWindowPreferences {
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) normal_geometry: Option<WindowGeometry>,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) maximized: bool,
}

fn default_pet_size() -> u16 {
    64
}

fn default_true() -> bool {
    true
}

// pet_size/always_on_top 的产品默认值不是各自类型的 Default（u16::default()=0,
// bool::default()=false 都不对），所以不能用 serde_with 的 DefaultOnError；
// 这里手写等价逻辑：类型不匹配时退回到调用方传入的产品默认值，而不是让整个
// PetWindowPreferences 反序列化失败、被外层 DefaultOnError 连坐重置。
fn deserialize_or_default<'de, D, T>(
    deserializer: D,
    fallback: impl FnOnce() -> T,
) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(T::deserialize(value).unwrap_or_else(|_| fallback()))
}

fn deserialize_pet_size<'de, D>(deserializer: D) -> Result<u16, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_or_default(deserializer, default_pet_size)
}

fn deserialize_always_on_top<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_or_default(deserializer, default_true)
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PetWindowPreferences {
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) layout: PetLayout,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) focused_slot: u8,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) single_geometry: Option<WindowGeometry>,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) row_geometry: Option<WindowGeometry>,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) column_geometry: Option<WindowGeometry>,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) row3_geometry: Option<WindowGeometry>,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) column3_geometry: Option<WindowGeometry>,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) grid_geometry: Option<WindowGeometry>,
    #[serde(
        default = "default_pet_size",
        deserialize_with = "deserialize_pet_size"
    )]
    pub(crate) pet_size: u16,
    #[serde(
        default = "default_true",
        deserialize_with = "deserialize_always_on_top"
    )]
    pub(crate) always_on_top: bool,
    #[serde_as(deserialize_as = "DefaultOnError")]
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

#[serde_as]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WindowPreferences {
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) active_mode: AppMode,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) main_window: MainWindowPreferences,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) pet_window: PetWindowPreferences,
}

// 落盘到 preferences.json 的用户偏好，是 MonitorState 的一个持久化子集
// （端口、运行中标记、本机 IP、宫格内容都是运行期派生值，不需要持久化）。
#[serde_as]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Preferences {
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) rows: u8, // 持久化的宫格行数
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) columns: u8, // 持久化的宫格列数
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) image_display_mode: ImageDisplayMode, // 持久化的图片显示模式
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)] // 旧版本偏好文件里可能没有这个字段，缺省为 false
    pub(crate) auto_start: bool,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) language: LanguagePreference,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) windows: WindowPreferences,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) window: Option<WindowGeometry>, // 仅用于迁移 1.1 及更早版本
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) device_id: String, // 持久化的设备 ID，跨重启保持稳定
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub(crate) device_name: String, // 持久化的设备名称
}

impl Default for Preferences {
    // 偏好文件不存在（首次启动）或整体损坏时的兜底值；device_id/device_name
    // 需要现算（随机 UUID / 主机名），不能用 #[derive(Default)]。
    fn default() -> Self {
        Self {
            rows: 2,                                         // 默认 2 行
            columns: 2,                                      // 默认 2 列
            image_display_mode: ImageDisplayMode::default(), // 默认等比缩放
            auto_start: false,                               // 默认不开机自启
            language: LanguagePreference::default(),         // 默认跟随系统语言
            windows: WindowPreferences::default(),
            window: None,
            device_id: uuid::Uuid::new_v4().to_string(), // 首次启动生成一个新的随机设备 ID
            device_name: crate::device_info::default_device_name(), // 首次启动用主机名作为默认设备名
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete_preferences() -> serde_json::Value {
        serde_json::json!({
            "rows": 2,
            "columns": 3,
            "imageDisplayMode": "FILL_CROP",
            "autoStart": true,
            "language": "en",
            "windows": {
                "activeMode": "pet",
                "mainWindow": { "maximized": true },
                "petWindow": {
                    "layout": "row",
                    "focusedSlot": 1,
                    "petSize": 80,
                    "alwaysOnTop": false,
                    "locked": true
                }
            },
            "window": { "x": 20, "y": 30, "width": 800, "height": 600 },
            "deviceId": "stable-device-id",
            "deviceName": "studio-monitor"
        })
    }

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
    fn malformed_field_does_not_discard_device_identity() {
        let mut value = complete_preferences();
        value["rows"] = serde_json::json!({ "invalid": true });

        let preferences: Preferences =
            serde_json::from_value(value).expect("one malformed field should use its default");

        assert_eq!(preferences.rows, 0);
        assert_eq!(preferences.columns, 3);
        assert_eq!(preferences.device_id, "stable-device-id");
        assert_eq!(preferences.device_name, "studio-monitor");
    }

    #[test]
    fn malformed_legacy_window_only_resets_that_object() {
        let mut value = complete_preferences();
        value["window"]["width"] = serde_json::json!("invalid");

        let preferences: Preferences = serde_json::from_value(value)
            .expect("malformed legacy window geometry should use its boundary default");

        assert_eq!(preferences.windows.active_mode, AppMode::Pet);
        assert!(preferences.windows.main_window.maximized);
        assert_eq!(preferences.windows.pet_window.layout, PetLayout::Row);
        assert_eq!(preferences.windows.pet_window.pet_size, 80);
        assert!(!preferences.windows.pet_window.always_on_top);
        assert!(preferences.windows.pet_window.locked);
        assert!(preferences.window.is_none());
        assert_eq!(preferences.device_id, "stable-device-id");
    }

    #[test]
    fn malformed_pet_size_and_always_on_top_only_reset_those_two_fields() {
        // pet_size/always_on_top 有非类型默认值的产品默认值，走自定义 deserialize_with
        // 而不是 serde_with 的 DefaultOnError；这里确认它们能像其余字段一样单独恢复，
        // 不会像早期实现那样连坐重置整个 petWindow（layout/locked/geometry 应保持不变）。
        let mut value = complete_preferences();
        value["windows"]["petWindow"]["petSize"] = serde_json::json!("invalid");
        value["windows"]["petWindow"]["alwaysOnTop"] = serde_json::json!("invalid");

        let preferences: Preferences = serde_json::from_value(value)
            .expect("malformed scalar fields should use their own field-level default");

        assert_eq!(preferences.windows.pet_window.layout, PetLayout::Row);
        assert_eq!(preferences.windows.pet_window.focused_slot, 1);
        assert_eq!(preferences.windows.pet_window.pet_size, default_pet_size());
        assert!(preferences.windows.pet_window.always_on_top);
        assert!(preferences.windows.pet_window.locked);
        assert_eq!(preferences.device_id, "stable-device-id");
    }
}
