// 跨模块共享的常量定义，避免多个模块各自重复声明同一个值。

// HTTP 服务从这个端口开始向上寻找第一个可用端口（与 Android 版约定一致）。
pub(crate) const FIRST_HTTP_PORT: u16 = 10_241;
// 局域网设备发送 UDP 探测包的目标端口。
pub(crate) const UDP_DISCOVERY_PORT: u16 = 8_080;
// 单次 HTTP 请求体上限（8MB），超出直接拒绝，避免恶意/异常请求占满内存；
// 通过 axum 的 DefaultBodyLimit 层生效（见 http/mod.rs）。请求头大小上限
// 交由 hyper 内部的默认限制处理，不再需要手动实现。
pub(crate) const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
// 对外暴露的 API 版本号，写入 /api/device 与 UDP 探测响应，供客户端做兼容判断。
pub(crate) const API_VERSION: u8 = 3;
