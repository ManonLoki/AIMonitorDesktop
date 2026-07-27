// HTTP 路由处理函数，按资源拆分到各自的子模块，由 http/mod.rs 统一挂载到 Axum Router。
pub(crate) mod device;
pub(crate) mod images;
pub(crate) mod slots;
