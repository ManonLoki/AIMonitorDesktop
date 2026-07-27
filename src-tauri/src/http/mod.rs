// 基于 Axum 的纯异步 HTTP 服务器：路由挂载、CORS、端口自动选择、后台任务派生。
// 不再有手写的 TcpListener 解析或线程池——每个连接由 Tokio 调度到独立的异步任务，
// 具体的资源处理函数在 routes 子模块，按 method + path 拆分到各自的文件。
mod routes;

use crate::constants::{FIRST_HTTP_PORT, MAX_BODY_BYTES};
use crate::runtime::SharedRuntime;
use axum::{
    extract::DefaultBodyLimit,
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::net::Ipv4Addr;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

// 组装所有路由；.with_state(runtime) 之后，每个 handler 都能通过 State 提取器
// 拿到同一份 Arc<Runtime>，等价于原来手写实现里到处传递的 &SharedRuntime。
fn build_router(runtime: SharedRuntime) -> Router {
    Router::new()
        .route("/health", get(routes::device::health))
        .route("/api/config", get(routes::device::get_config))
        .route("/api/device", get(routes::device::get_device))
        .route(
            "/api/images",
            get(routes::images::list_images).post(routes::images::upload_image),
        )
        .route(
            "/api/images/{filename}",
            get(routes::images::get_image).delete(routes::images::delete_image),
        )
        .route(
            "/api/slots/{slot}",
            post(routes::slots::update_slot).delete(routes::slots::clear_slot),
        )
        .fallback(not_found)
        .layer(
            // 局域网内任意来源、任意来源头都放行，与原实现里手写的宽松 CORS 头等价；
            // OPTIONS 预检请求由本层自动应答，不再需要手动特判。
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
                .allow_headers(Any),
        )
        // 单次请求体上限，超出直接拒绝，避免恶意/异常请求占满内存（图片上传走这里）。
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(runtime)
}

// 兜底 404：未匹配任何注册路由时返回，错误结构与手写实现时期保持一致。
async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Json(json!({ "error": "not found" })))
}

// 启动 HTTP 服务器：先同步阻塞查找从 FIRST_HTTP_PORT 起第一个可绑定端口
//（用 tauri::async_runtime::block_on 借用 Tauri 已经建好的 Tokio 运行时，
// 与旧版"循环 bind 直到成功"的观测行为一致，只是底层换成了异步 API），
// 随后把 accept 循环交给 axum::serve 在后台任务中运行，函数本身仍同步返回端口号，
// 供 Tauri 的 setup() 回调（本身是同步闭包）继续往下走。
pub(crate) fn start_http_server(runtime: SharedRuntime) -> u16 {
    let listener = tauri::async_runtime::block_on(bind_first_available_port());
    let port = listener.local_addr().expect("HTTP 监听地址不可用").port();
    {
        let mut state = runtime.state.write().expect("state lock poisoned"); // 加写锁回填真实监听端口与运行状态
        state.port = port;
        state.is_server_running = true;
    }
    runtime.changed(); // 通知前端端口/运行状态已确定

    let app = build_router(runtime);
    tauri::async_runtime::spawn(async move {
        // axum::serve 对每个连接内部派生独立的 Tokio 任务处理，单个连接出错
        // （例如客户端异常断开）不会影响其他连接；这里只在 serve 本身退出时记录，
        // 正常情况下它会随应用生命周期一直运行，不会主动返回。
        if let Err(error) = axum::serve(listener, app).await {
            eprintln!("HTTP 服务器异常退出: {error}");
        }
    });
    port // 返回实际绑定成功的端口号，供 setup 回调回填状态
}

// 从 FIRST_HTTP_PORT 开始向上异步尝试绑定，返回第一个绑定成功的监听器。
async fn bind_first_available_port() -> TcpListener {
    for port in FIRST_HTTP_PORT..=u16::MAX {
        if let Ok(listener) = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).await {
            return listener;
        }
    }
    panic!("没有可用的 HTTP 端口"); // 极端情况下所有端口都被占用，直接 panic 终止启动
}
