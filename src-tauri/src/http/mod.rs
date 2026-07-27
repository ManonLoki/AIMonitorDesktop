// 手写的最小 HTTP/1.1 服务器：接收线程只负责 accept 并分发到工作线程池，
// 具体的请求解析/响应写出在 protocol 子模块，路由分发在 routes 子模块。
mod protocol;
mod routes;

use crate::constants::FIRST_HTTP_PORT;
use crate::runtime::SharedRuntime;
use protocol::respond_json;
use routes::handle_client;
use serde_json::json;
use std::{
    net::{Ipv4Addr, TcpListener, TcpStream},
    sync::{mpsc, Arc, Mutex},
    thread,
};

// 启动 HTTP 服务器：从 FIRST_HTTP_PORT 起向上找第一个可绑定的端口，
// 用一个有界 mpsc 队列 + 4 个常驻工作线程处理连接，接收线程本身只管 accept，
// 不在 accept 循环里做任何耗时逻辑，避免拖慢新连接的接纳速度。
pub(crate) fn start_http_server(runtime: SharedRuntime) -> u16 {
    let (port, listener) = (FIRST_HTTP_PORT..=u16::MAX)
        .find_map(|port| {
            TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)) // 依次尝试绑定端口，监听所有网卡
                .ok()
                .map(|listener| (port, listener))
        })
        .expect("没有可用的 HTTP 端口"); // 极端情况下所有端口都被占用，直接 panic 终止启动
    {
        let mut state = runtime.state.write().expect("state lock poisoned"); // 加写锁回填真实监听端口与运行状态
        state.port = port;
        state.is_server_running = true;
    }
    runtime.changed(); // 通知前端端口/运行状态已确定

    // 队列容量 16：突发连接超过这个数量时，多余的连接会被直接告知"服务器繁忙"，
    // 而不是无限堆积导致内存增长或让客户端无限期挂起。
    let (sender, receiver) = mpsc::sync_channel::<TcpStream>(16); // 有界同步队列，容量 16
    let receiver = Arc::new(Mutex::new(receiver)); // 多个工作线程共享同一个接收端，需要用 Mutex 互斥取任务
    for _ in 0..4 {
        let worker_runtime = runtime.clone(); // 每个工作线程持有自己的 Arc 克隆
        let worker_receiver = receiver.clone();
        thread::spawn(move || loop {
            let stream = worker_receiver.lock().expect("HTTP queue poisoned").recv(); // 阻塞等待下一个待处理连接
            match stream {
                Ok(stream) => handle_client(stream, &worker_runtime), // 取到连接，同步处理完整个请求
                Err(_) => break, // 发送端已全部丢弃（服务关闭），退出工作线程
            }
        });
    }
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            // accept 循环：只做分发，不做任何耗时逻辑
            if let Err(error) = sender.try_send(stream) {
                // 队列已满或已断开：尽力返回 503 而不是直接丢弃连接。
                stream = match error {
                    mpsc::TrySendError::Full(stream) | mpsc::TrySendError::Disconnected(stream) => {
                        stream // 两种错误都能拿回原始连接对象，用于返回错误响应
                    }
                };
                respond_json(
                    &mut stream,
                    503,
                    json!({ "error": "server busy; retry later" }),
                );
            }
        }
    });
    port // 返回实际绑定成功的端口号，供 setup 回调回填状态
}
