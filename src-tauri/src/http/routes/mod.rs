// 请求路由：按 method + path 分发到具体的处理函数。
// 这是与 Android 版 App 约定的 HTTP API 的核心入口，改动前需确认 Android 端兼容性。
// 各资源的处理函数返回 bool：true 表示已经写完响应，false 表示未命中、继续尝试下一个路由。
mod device;
mod images;
mod slots;

use super::protocol::{read_request, respond_json};
use crate::runtime::SharedRuntime;
use serde_json::json;
use std::net::TcpStream;

// 单个 TCP 连接的完整请求处理：解析请求 → 按 method + path 路由 → 写响应。
pub(crate) fn handle_client(mut stream: TcpStream, runtime: &SharedRuntime) {
    let request = match read_request(&stream) {
        Ok(request) => request,
        Err(error) => {
            let status = if error == "request body too large" {
                413 // 请求体过大对应的专用状态码
            } else {
                400 // 其余解析错误统一按"错误请求"处理
            };
            respond_json(&mut stream, status, json!({ "error": error }));
            return; // 请求本身无法解析，直接结束本次连接处理
        }
    };
    let path = request.target.split('?').next().unwrap_or("/"); // 路由只关心去掉查询字符串后的纯路径

    // 预检请求：所有跨源请求方法都直接放行，实际权限控制交给局域网可达性本身。
    if request.method == "OPTIONS" {
        respond_json(&mut stream, 200, json!({ "status": "ok" }));
        return;
    }
    if request.method == "GET" && path == "/health" {
        respond_json(&mut stream, 200, json!({ "status": "ok" })); // 简单健康检查，供 Android 端探测服务是否存活
        return;
    }

    // 依次尝试各资源的路由处理器，命中即返回；全部未命中则落到最后的通用 404。
    if device::handle(&request, path, runtime, &mut stream) {
        return;
    }
    if images::handle(&request, path, runtime, &mut stream) {
        return;
    }
    if slots::handle(&request, path, runtime, &mut stream) {
        return;
    }
    respond_json(&mut stream, 404, json!({ "error": "not found" })); // 以上所有分支都未命中，说明是未知路由
}
