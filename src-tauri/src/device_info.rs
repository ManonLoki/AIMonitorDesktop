// 设备身份相关的辅助函数：默认设备名称、局域网 IP 探测。
use std::net::{IpAddr, UdpSocket};

// 首次启动、没有历史偏好时，用主机名作为默认设备名称。
pub(crate) fn default_device_name() -> String {
    std::env::var("COMPUTERNAME") // Windows 下的主机名环境变量
        .or_else(|_| std::env::var("HOSTNAME")) // macOS/Linux 下常见的主机名环境变量
        .ok() // 转成 Option，读取失败则丢弃错误
        .filter(|name| !name.trim().is_empty()) // 过滤掉空字符串主机名
        .unwrap_or_else(|| "AIMonitorDesktop".into()) // 两者都取不到时的兜底默认名
}

// 通过"连接"一个公网地址（不会真的发包，UDP 是无连接协议）来让操作系统
// 选出用于外发流量的本机网卡地址，从而拿到局域网 IP，不依赖任何外部服务可达。
pub(crate) fn local_ipv4() -> String {
    UdpSocket::bind("0.0.0.0:0") // 绑定一个临时的任意端口
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?; // "连接"公网地址，触发路由表选择出站网卡
            socket.local_addr() // 读取该 socket 被分配到的本地地址
        })
        .ok() // 绑定/连接失败则转为 None，走后面的兜底文案
        .and_then(|addr| match addr.ip() {
            IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()), // 排除回环地址，只接受真实局域网 IPv4
            _ => None, // IPv6 或回环地址都视为无效
        })
        .unwrap_or_else(|| "未连接局域网".into()) // 拿不到有效 IP 时展示的兜底文案
}
