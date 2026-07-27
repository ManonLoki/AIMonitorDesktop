// 注册 mDNS 服务，使支持 Bonjour/Avahi 的客户端可以按 `_aimonitor._tcp.` 类型发现本机。
use crate::runtime::SharedRuntime;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use std::net::{IpAddr, Ipv4Addr};

pub(crate) fn start_mdns(runtime: &SharedRuntime) {
    let state = runtime.snapshot(); // 注册时用到的设备信息快照（后续设备名变化不会更新已注册的 mDNS 记录）
    let Ok(daemon) = ServiceDaemon::new() else {
        return; // 守护进程创建失败（例如平台不支持），放弃 mDNS，不影响其余子系统
    };
    let ip: IpAddr = state
        .local_ip
        .parse() // 尝试把"未连接局域网"等非 IP 字符串解析失败
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)); // 解析失败时退回本机回环地址
    let properties = [
        ("id", state.device_id.as_str()),     // TXT 记录：设备 ID
        ("name", state.device_name.as_str()), // TXT 记录：设备名称
        ("apiVersion", "2"),                  // TXT 记录：API 版本
        ("path", "/api/device"),              // TXT 记录：设备信息接口路径
    ];
    if let Ok(info) = ServiceInfo::new(
        "_aimonitor._tcp.local.", // mDNS 服务类型
        &state.device_name,       // 实例名，使用设备名称
        &format!("{}.local.", state.device_id), // 主机名，用设备 ID 保证唯一
        ip,
        state.port,
        &properties[..],
    ) {
        let _ = daemon.register(info); // 注册服务；失败则静默忽略，不影响其他发现方式
        // ServiceDaemon 需要在整个应用生命周期内存活才能持续应答 mDNS 查询，
        // 这里主动 forget 放弃其析构，是有意为之的常驻资源，不是遗漏的 drop。
        std::mem::forget(daemon);
    }
}
