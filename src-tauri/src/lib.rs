// AIMonitorDesktop 的桌面端运行时入口。
//
// 具体实现按职责拆分到各个子模块（数据模型、运行时状态、Tauri 命令、
// HTTP 服务器、UDP 发现、mDNS、窗口几何持久化），本文件只负责：
// 声明子模块、在 Tauri 的 setup 回调里完成装配与启动、注册命令列表。
// 单文件不超过 400 行是本项目记录在 CLAUDE.md "代码门禁" 一节的事实标准，
// 拆分到这些子模块正是为了满足该门禁，同时让每个文件只承担一个职责：
// - constants.rs        跨模块共享的常量（端口、上限、API 版本）
// - model.rs             MonitorTile / MonitorState / WindowGeometry / Preferences 数据模型
// - runtime.rs           Runtime 共享状态、快照/落盘/广播事件、偏好加载
// - tray.rs              系统托盘、“显示窗口”“开机自启”“退出”菜单、主窗口恢复
// - commands.rs          5 个 #[tauri::command]，前端唯一的写入入口
// - device_info.rs       默认设备名、局域网 IP 探测
// - image.rs              图片格式探测、GIF 循环修正、文件名校验、异步文件探测
// - window_geometry.rs    窗口几何的可用性判断、恢复与保存
// - discovery.rs          UDP 主动发现（纯异步）
// - mdns.rs                mDNS 服务注册
// - http/                 基于 Axum 的纯异步 HTTP 服务器：mod.rs 负责路由挂载与共享的
//                          error_json 错误响应助手，device.rs/images.rs/slots.rs 按资源
//                          拆分处理函数（无额外的子目录嵌套）
//
// 三个并发子系统（在 setup 回调里一起启动，共享同一个 Arc<Runtime>）：
// 1. HTTP 服务器：Axum + Tokio 纯异步实现，提供与 Android 版兼容的 REST API；
//    没有手写的线程池或阻塞 socket，每个连接由 Tokio 调度到独立的异步任务。
// 2. UDP 广播发现：tokio::net::UdpSocket 异步监听 8080 端口，响应局域网内的探测广播。
// 3. mDNS：注册 `_aimonitor._tcp.` 服务，支持 Bonjour/Avahi 风格的服务发现
//    （`mdns-sd` 内部自带一个常驻线程，这是该第三方库自身的实现方式，不在本项目
//    "网络层纯异步" 的范围内——我们只负责调用它注册一次）。
mod commands;
mod constants;
mod device_info;
mod discovery;
mod heartbeat;
mod http;
mod image;
mod mdns;
mod model;
mod runtime;
mod tray;
mod window_geometry;

use commands::{
    get_monitor_state, set_auto_start, set_device_name, set_grid, set_image_display_mode,
};
use runtime::{load_preferences, Runtime};
use std::fs;
use tauri::{Manager, WindowEvent};
use window_geometry::{restore_window, save_window_geometry};

// Tauri 应用入口：注册插件、装配 Runtime、启动三个子系统、暴露 Tauri 命令。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 单实例限制：必须是注册的第一个插件（Tauri 官方要求），否则无法保证在
        // 其他插件初始化之前拦截到"已有实例正在运行"这一事件。第二次启动时不再
        // 创建新窗口/新进程，而是把已运行实例的主窗口取消最小化、显示并置前。
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_autostart::Builder::new().build()) // 开机自启插件
        .plugin(tauri_plugin_opener::init()) // 系统级"用默认程序打开"插件（前端未直接用到，但保留通用能力）
        .setup(|app| {
            use tauri_plugin_autostart::ManagerExt; // 引入 autolaunch() 扩展方法

            let app_handle = app.handle().clone(); // 克隆一份句柄供 Runtime 长期持有
            let config_dir = app.path().app_config_dir()?; // 应用配置目录，存放 preferences.json
            let cache_dir = app.path().app_cache_dir()?; // 应用缓存目录，存放上传的图片
            let preferences_path = config_dir.join("preferences.json");
            let image_dir = cache_dir.join("monitor_images");
            fs::create_dir_all(&image_dir)?; // 确保图片目录存在，后续上传直接写入
            let preferences = load_preferences(&preferences_path); // 加载历史偏好（或生成默认值）
                                                                   // 自启动的真实开关状态以操作系统为准，读取失败时才回退到上次保存的偏好值。
            let auto_start = app
                .autolaunch()
                .is_enabled()
                .unwrap_or(preferences.auto_start);
            if let Some(window) = app.get_webview_window("main") {
                restore_window(&window, preferences.window.as_ref())?; // 按历史几何或默认策略摆放主窗口
            }
            let runtime = Runtime::new(
                app_handle,
                image_dir,
                preferences_path,
                preferences,
                auto_start,
                app.package_info().version.to_string(), // 从 Tauri 打包信息读取版本号
            );
            runtime.save_preferences(); // 首次落盘，规范化/补全可能缺失的字段
            let tray_menu = tray::setup(app, runtime.clone(), auto_start)?; // 创建托盘菜单并绑定共享状态
            if let Some(window) = app.get_webview_window("main") {
                let runtime_for_events = runtime.clone(); // 为窗口事件回调准备独立的 Arc 克隆
                let window_for_events = window.clone();
                window.on_window_event(move |event| {
                    if matches!(
                        event,
                        WindowEvent::Moved(_)
                            | WindowEvent::Resized(_)
                            | WindowEvent::CloseRequested { .. } // 移动、缩放、即将关闭时都需要记录最新几何
                    ) {
                        save_window_geometry(&window_for_events, &runtime_for_events);
                    }
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close(); // 关闭主窗口时保留后台服务和托盘
                        let _ = window_for_events.hide();
                    }
                });
            }
            let port = http::start_http_server(runtime.clone()); // 启动 HTTP 服务器并拿到实际绑定端口
            heartbeat::start_cleanup(runtime.clone()); // 定期清理心跳已过期控制端占用的槽位
            discovery::start_udp_discovery(runtime.clone()); // 启动 UDP 探测响应线程
            mdns::start_mdns(&runtime); // 注册 mDNS 服务
            runtime.state.write().expect("state lock poisoned").port = port; // 用真实端口覆盖占位值（start_http_server 内部其实已经写过一次，这里是双保险）
            app.manage(tray_menu); // 供设置页命令同步托盘中的“开机自启”勾选状态
            app.manage(runtime); // 交给 Tauri 管理，之后各 #[tauri::command] 才能通过 State 取到它
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_monitor_state,
            set_grid,
            set_image_display_mode,
            set_device_name,
            set_auto_start
        ]) // 注册所有可从前端 invoke 调用的命令
        .run(tauri::generate_context!())
        .expect("AIMonitorDesktop 启动失败");
}
