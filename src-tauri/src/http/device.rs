// /health、/api/config、/api/device 只读接口：返回设备与宫格配置信息。
use crate::constants::API_VERSION;
use crate::device_info::default_device_name;
use crate::runtime::SharedRuntime;
use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;

// 健康检查：Android 端探测服务是否存活，不依赖任何状态。
pub(crate) async fn health() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

// 精简的宫格配置信息。
pub(crate) async fn get_config(State(runtime): State<SharedRuntime>) -> impl IntoResponse {
    let state = runtime.snapshot(); // 读取当前状态快照
    Json(json!({ "rows": state.rows, "columns": state.columns, "port": state.port }))
}

// 设备身份、能力与版本信息，供 Android 端在 UDP/mDNS 发现后确认目标设备。
pub(crate) async fn get_device(State(runtime): State<SharedRuntime>) -> impl IntoResponse {
    let state = runtime.snapshot(); // 读取当前状态快照
    Json(json!({
        "id": state.device_id, // 设备唯一标识
        "name": state.device_name, // 设备展示名称
        "manufacturer": if cfg!(target_os = "macos") { "Apple" } else if cfg!(target_os = "windows") { "Microsoft" } else { "Desktop" }, // 按编译目标平台给出厂商信息，供 Android 端展示
        "model": default_device_name(), // 用主机名代替具体机型
        "androidVersion": std::env::consts::OS, // 字段名沿用 Android 协议，实际填入本机操作系统标识
        "appVersion": state.app_version,
        "apiVersion": API_VERSION,
        "port": state.port,
        "rows": state.rows,
        "columns": state.columns,
        "capabilities": ["images", "slots", "heartbeats"] // 声明本服务支持的能力集合，供客户端做特性探测
    }))
}
