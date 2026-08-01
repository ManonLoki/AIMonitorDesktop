// 设备身份相关的辅助函数：默认设备名称、局域网 IP 探测。
use std::{
    ffi::OsStr,
    net::{IpAddr, UdpSocket},
};

const DEFAULT_DEVICE_NAME: &str = "AIMonitorDesktop";

// 主机名由操作系统提供，可能不是 UTF-8，也可能只包含空白。
// 只接受可安全展示的非空 UTF-8 文本，并去掉两端空白。
fn clean_hostname(hostname: Option<&OsStr>) -> String {
    hostname
        .and_then(OsStr::to_str)
        .map(str::trim)
        .filter(|hostname| !hostname.is_empty())
        .unwrap_or(DEFAULT_DEVICE_NAME)
        .to_owned()
}

// 首次启动、没有历史偏好时，用主机名作为默认设备名称。
pub(crate) fn default_device_name() -> String {
    let hostname = hostname::get().ok();
    clean_hostname(hostname.as_deref())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostname_cleaning_trims_displayable_names_and_rejects_empty_ones() {
        assert_eq!(
            clean_hostname(Some(OsStr::new("  studio-monitor  "))),
            "studio-monitor"
        );
        assert_eq!(
            clean_hostname(Some(OsStr::new(" \t\n "))),
            DEFAULT_DEVICE_NAME
        );
        assert_eq!(clean_hostname(None), DEFAULT_DEVICE_NAME);
    }

    #[cfg(unix)]
    #[test]
    fn hostname_cleaning_rejects_non_utf8_names() {
        use std::os::unix::ffi::OsStrExt;

        assert_eq!(
            clean_hostname(Some(OsStr::from_bytes(b"monitor-\xff"))),
            DEFAULT_DEVICE_NAME
        );
    }

    #[cfg(windows)]
    #[test]
    fn hostname_cleaning_rejects_non_utf8_names() {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};

        let hostname = OsString::from_wide(&[0xD800]);
        assert_eq!(clean_hostname(Some(&hostname)), DEFAULT_DEVICE_NAME);
    }
}
