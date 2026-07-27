// /api/config、/api/device 只读接口：返回设备与宫格配置信息。
use crate::constants::API_VERSION;
use crate::device_info::default_device_name;
use crate::http::protocol::{respond_json, Request};
use crate::runtime::SharedRuntime;
use serde_json::json;
use std::net::TcpStream;

// 命中则处理并返回 true；未命中（路径/方法不匹配）返回 false，交给上一级继续尝试其他路由。
pub(crate) fn handle(
    request: &Request,
    path: &str,
    runtime: &SharedRuntime,
    stream: &mut TcpStream,
) -> bool {
    if request.method == "GET" && path == "/api/config" {
        let state = runtime.snapshot(); // 读取当前状态快照
        respond_json(
            stream,
            200,
            json!({ "rows": state.rows, "columns": state.columns, "port": state.port }), // 精简的宫格配置信息
        );
        return true;
    }
    if request.method == "GET" && path == "/api/device" {
        let state = runtime.snapshot(); // 读取当前状态快照
        respond_json(
            stream,
            200,
            json!({
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
                "capabilities": ["images", "slots"] // 声明本服务支持的能力集合，供客户端做特性探测
            }),
        );
        return true;
    }
    false
}
