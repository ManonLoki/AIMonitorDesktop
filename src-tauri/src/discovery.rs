// UDP 主动发现：客户端向 8080 端口广播固定的探测帧，桌面端收到后单播回应设备信息。
// 相比 mDNS，这是给不支持/未启用 mDNS 的客户端准备的兜底发现方式。
// 用 tokio::net::UdpSocket 异步收发，作为一个 Tokio 后台任务运行，不再占用独立的
// 阻塞 std::thread——与 HTTP 服务器一样，是"网络层纯异步实现"的一部分。
use crate::constants::{API_VERSION, UDP_DISCOVERY_PORT};
use crate::runtime::SharedRuntime;
use serde_json::json;
use std::net::Ipv4Addr;
use tokio::net::UdpSocket;

pub(crate) fn start_udp_discovery(runtime: SharedRuntime) {
    tauri::async_runtime::spawn(async move {
        let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, UDP_DISCOVERY_PORT)).await else {
            return; // 端口被占用等情况下直接放弃该子系统，不影响 HTTP/mDNS 正常工作
        };
        let mut buffer = [0u8; 256]; // 探测帧很短，256 字节足够容纳
        loop {
            let Ok((length, source)) = socket.recv_from(&mut buffer).await else {
                continue; // 单次接收出错不影响后续循环，继续等待下一个包
            };
            if &buffer[..length] != b"AIMONITOR_DISCOVER_V1" {
                continue; // 不是约定的探测帧内容，忽略（防止误响应任意 UDP 流量）
            }
            let state = runtime.snapshot(); // 读取当前状态用于组装响应
            let response = json!({
                "id": state.device_id,
                "name": state.device_name,
                "port": state.port,
                "apiVersion": API_VERSION.to_string()
            });
            let _ = socket
                .send_to(response.to_string().as_bytes(), source)
                .await; // 单播回应给探测方的源地址
        }
    });
}
