// AIMonitorDesktop 的桌面端运行时。
//
// 整个后端只有这一个文件：状态管理、HTTP API、UDP 广播发现、mDNS 注册、
// 窗口几何持久化都在这里。之所以不拆分成多个模块，是因为各部分都围绕同一份
// Runtime 状态展开，拆分反而会增加跨模块传递 Arc<Runtime> 的样板代码。
//
// 三个并发子系统（在 run() 的 setup 回调里一起启动，共享同一个 Arc<Runtime>）：
// 1. HTTP 服务器：手写的 TcpListener + 线程池，提供与 Android 版兼容的 REST API；
// 2. UDP 广播发现：监听 8080 端口，响应局域网内的探测广播；
// 3. mDNS：注册 `_aimonitor._tcp.` 服务，支持 Bonjour/Avahi 风格的服务发现。
use mdns_sd::{ServiceDaemon, ServiceInfo};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{IpAddr, Ipv4Addr, TcpListener, TcpStream, UdpSocket},
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex, RwLock},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow, WindowEvent,
};
use uuid::Uuid;

// HTTP 服务从这个端口开始向上寻找第一个可用端口（与 Android 版约定一致）。
const FIRST_HTTP_PORT: u16 = 10_241;
// 局域网设备发送 UDP 探测包的目标端口。
const UDP_DISCOVERY_PORT: u16 = 8_080;
// 单次 HTTP 请求体上限（8MB），超出直接拒绝，避免恶意/异常请求占满内存。
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
// 请求头总大小上限，防止畸形请求无限占用 BufReader 缓冲。
const MAX_HEADER_BYTES: usize = 16 * 1024;
// 对外暴露的 API 版本号，写入 /api/device 与 UDP 探测响应，供客户端做兼容判断。
const API_VERSION: u8 = 2;

// 单个监控宫格的数据。Default 用于清空宫格（DELETE /api/slots/{slot}）。
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct MonitorTile {
    username: String,
    ai_name: String,
    content: String,
    image_filename: Option<String>,
    updated_at_millis: Option<u64>,
}

// 图片显示模式：等比缩放（留黑边）或铺满裁剪，序列化为大写下划线风格与 Android 版对齐。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ImageDisplayMode {
    FitCenter,
    FillCrop,
}

impl Default for ImageDisplayMode {
    fn default() -> Self {
        Self::FitCenter
    }
}

// 桌面端运行时的完整状态快照，也是 Tauri 命令 get_monitor_state 的返回值。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MonitorState {
    rows: u8,
    columns: u8,
    image_display_mode: ImageDisplayMode,
    auto_start: bool,
    port: u16,
    app_version: String,
    device_id: String,
    device_name: String,
    is_server_running: bool,
    local_ip: String,
    tiles: Vec<MonitorTile>,
}

// 窗口位置与尺寸，用于跨会话恢复窗口摆放位置。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowGeometry {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

// 落盘到 preferences.json 的用户偏好，是 MonitorState 的一个持久化子集
// （端口、运行中标记、本机 IP、宫格内容都是运行期派生值，不需要持久化）。
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Preferences {
    rows: u8,
    columns: u8,
    image_display_mode: ImageDisplayMode,
    #[serde(default)]
    auto_start: bool,
    #[serde(default)]
    window: Option<WindowGeometry>,
    device_id: String,
    device_name: String,
}

// 贯穿三个子系统共享的运行时上下文，以 Arc 的形式在线程间传递。
struct Runtime {
    // 用 RwLock 而非 Mutex：HTTP 读请求（GET /api/device 等）远多于写请求，
    // 读写锁允许多个只读请求并发执行，只有修改宫格/设置时才需要独占锁。
    state: RwLock<MonitorState>,
    image_dir: PathBuf,
    preferences_path: PathBuf,
    window_geometry: Mutex<Option<WindowGeometry>>,
    app: AppHandle,
}

type SharedRuntime = Arc<Runtime>;

impl Runtime {
    // 克隆一份当前状态用于响应请求/广播，避免长时间持有锁阻塞其他线程。
    fn snapshot(&self) -> MonitorState {
        self.state.read().expect("state lock poisoned").clone()
    }

    // 把当前状态中可持久化的字段写入 preferences.json；调用方在每次状态变更后触发。
    fn save_preferences(&self) {
        let state = self.snapshot();
        let preferences = Preferences {
            rows: state.rows,
            columns: state.columns,
            image_display_mode: state.image_display_mode,
            auto_start: state.auto_start,
            window: self
                .window_geometry
                .lock()
                .expect("window geometry lock poisoned")
                .clone(),
            device_id: state.device_id,
            device_name: state.device_name,
        };
        if let Some(parent) = self.preferences_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(&preferences) {
            let _ = fs::write(&self.preferences_path, bytes);
        }
    }

    // 通知前端状态已变化；前端收到后会重新调用 get_monitor_state 拉取最新快照。
    fn changed(&self) {
        let _ = self.app.emit("monitor-state-changed", ());
    }
}

// —— 以下 5 个 #[tauri::command] 是前端唯一能触达 Rust 状态的入口 ——
// 每个命令的写法都遵循同一套约定：加写锁改状态 → 落盘 preferences → 广播 changed 事件。

#[tauri::command]
fn get_monitor_state(runtime: State<'_, SharedRuntime>) -> MonitorState {
    runtime.snapshot()
}

#[tauri::command]
fn set_grid(runtime: State<'_, SharedRuntime>, rows: u8, columns: u8) -> Result<(), String> {
    if !(1..=5).contains(&rows) || !(1..=5).contains(&columns) {
        return Err("行数和列数必须在 1–5 之间".into());
    }
    {
        let mut state = runtime.state.write().map_err(|_| "状态不可用")?;
        state.rows = rows;
        state.columns = columns;
    }
    runtime.save_preferences();
    runtime.changed();
    Ok(())
}

#[tauri::command]
fn set_image_display_mode(
    runtime: State<'_, SharedRuntime>,
    mode: ImageDisplayMode,
) -> Result<(), String> {
    runtime
        .state
        .write()
        .map_err(|_| "状态不可用")?
        .image_display_mode = mode;
    runtime.save_preferences();
    runtime.changed();
    Ok(())
}

#[tauri::command]
fn set_device_name(runtime: State<'_, SharedRuntime>, name: String) -> Result<(), String> {
    // 裁剪空白并限制长度，避免过长名称破坏 mDNS/局域网展示效果。
    let safe_name: String = name.trim().chars().take(40).collect();
    if safe_name.is_empty() {
        return Err("设备名称不能为空".into());
    }
    runtime.state.write().map_err(|_| "状态不可用")?.device_name = safe_name;
    runtime.save_preferences();
    runtime.changed();
    Ok(())
}

#[tauri::command]
fn set_auto_start(runtime: State<'_, SharedRuntime>, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;

    // 开机自启由操作系统层面的注册项/LaunchAgent 管理，这里只是转发给插件，
    // 状态字段 auto_start 仅用于前端展示当前值。
    let manager = runtime.app.autolaunch();
    if enabled {
        manager.enable().map_err(|error| error.to_string())?;
    } else {
        manager.disable().map_err(|error| error.to_string())?;
    }
    runtime.state.write().map_err(|_| "状态不可用")?.auto_start = enabled;
    runtime.save_preferences();
    runtime.changed();
    Ok(())
}

// 首次启动、没有历史偏好时，用主机名作为默认设备名称。
fn default_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "AIMonitorDesktop".into())
}

// 通过"连接"一个公网地址（不会真的发包，UDP 是无连接协议）来让操作系统
// 选出用于外发流量的本机网卡地址，从而拿到局域网 IP，不依赖任何外部服务可达。
fn local_ipv4() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .ok()
        .and_then(|addr| match addr.ip() {
            IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()),
            _ => None,
        })
        .unwrap_or_else(|| "未连接局域网".into())
}

// 通过文件头部的魔数判断图片格式，返回 (扩展名, MIME类型)；不依赖文件名/扩展名，
// 因为上传的文件名是随机生成的 UUID，必须靠内容本身判断类型。
fn detect_image(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some(("png", "image/png"))
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("jpg", "image/jpeg"))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(("gif", "image/gif"))
    } else {
        None
    }
}

// 浏览器会遵循 GIF 内嵌的 NETSCAPE/ANIMEXTS 循环次数；不少生成器默认写成 1，
// 导致监控宫格里的动图播完一轮后停住。这里不重新编码帧，只把已有循环次数改为 0
//（GIF 约定的无限循环），或在缺少循环扩展时补上一段标准 NETSCAPE2.0 扩展。
fn make_gif_loop_forever(bytes: &mut Vec<u8>) -> bool {
    if !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) || bytes.len() < 13 {
        return false;
    }

    let packed = bytes[10];
    let color_table_len = if packed & 0x80 != 0 {
        3usize << (usize::from(packed & 0x07) + 1)
    } else {
        0
    };
    let data_start = 13usize.saturating_add(color_table_len);
    if data_start > bytes.len() {
        return false;
    }

    const NETSCAPE: &[u8; 11] = b"NETSCAPE2.0";
    const ANIMEXTS: &[u8; 11] = b"ANIMEXTS1.0";
    let mut cursor = data_start;
    while cursor + 2 <= bytes.len() {
        match bytes[cursor] {
            0x21 if bytes[cursor + 1] == 0xff => {
                let block_size_index = cursor + 2;
                if block_size_index >= bytes.len() {
                    return false;
                }
                let block_size = usize::from(bytes[block_size_index]);
                let identifier_start = block_size_index + 1;
                let identifier_end = identifier_start.saturating_add(block_size);
                if identifier_end > bytes.len() {
                    return false;
                }
                let identifier = &bytes[identifier_start..identifier_end];
                if block_size == 11 && (identifier == NETSCAPE || identifier == ANIMEXTS) {
                    // 循环子块格式固定为 03 01 <count-le-u16> 00。
                    if identifier_end + 5 <= bytes.len()
                        && bytes[identifier_end] == 3
                        && bytes[identifier_end + 1] == 1
                        && bytes[identifier_end + 4] == 0
                    {
                        bytes[identifier_end + 2] = 0;
                        bytes[identifier_end + 3] = 0;
                        return true;
                    }
                    return false;
                }
                cursor = identifier_end;
                while cursor < bytes.len() {
                    let size = usize::from(bytes[cursor]);
                    cursor += 1;
                    if size == 0 {
                        break;
                    }
                    cursor = cursor.saturating_add(size);
                    if cursor > bytes.len() {
                        return false;
                    }
                }
            }
            // 注释、图形控制、纯文本等其他扩展也由长度前缀子块组成；跳过它们，
            // 避免把扩展正文中恰好出现的 0x2c 误判为第一帧图像标记。
            0x21 => {
                cursor += 2;
                while cursor < bytes.len() {
                    let size = usize::from(bytes[cursor]);
                    cursor += 1;
                    if size == 0 {
                        break;
                    }
                    cursor = cursor.saturating_add(size);
                    if cursor > bytes.len() {
                        return false;
                    }
                }
            }
            // 第一帧图像或文件结束标记之前没有循环扩展，补到全局色表之后。
            0x2c | 0x3b => break,
            // 容忍编码器写入的非标准填充字节，继续查找扩展/图像标记。
            _ => cursor += 1,
        }
    }

    const LOOP_FOREVER_EXTENSION: &[u8] = b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00";
    bytes.splice(
        data_start..data_start,
        LOOP_FOREVER_EXTENSION.iter().copied(),
    );
    true
}

// 只读取文件开头的几个字节来判断类型和 MIME，用 metadata 拿文件大小——
// 避免为了列出图片目录而把每个文件的全部内容读进内存（图片可能有数 MB）。
// 这是 GET /api/images 分页列表接口的关键优化点：无论请求哪一页，都需要遍历
// 全部文件以计算 total，如果每个文件都整体读入内存，目录越大、单张图片越大，
// 这个接口的内存占用和耗时就越不可控。
fn probe_image_file(path: &Path) -> Option<(&'static str, u64)> {
    let mut file = fs::File::open(path).ok()?;
    let mut header = [0u8; 8];
    let read = file.read(&mut header).ok()?;
    let (_, mime) = detect_image(&header[..read])?;
    let size = file.metadata().ok()?.len();
    Some((mime, size))
}

// 上传文件名的白名单校验：禁止路径分隔符（防目录穿越）、限制字符集与长度、
// 要求以已知图片扩展名结尾。用于 DELETE/GET /api/images/{filename} 之前的防御性检查。
fn safe_image_filename(filename: &str) -> bool {
    !filename.is_empty()
        && filename.len() <= 180
        && !filename.contains(['/', '\\'])
        && filename
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && matches!(
            filename
                .rsplit('.')
                .next()
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("jpg" | "jpeg" | "png" | "gif")
        )
}

// 解析后的最简 HTTP 请求：只保留后续路由逻辑需要的字段。
struct Request {
    method: String,
    target: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

// 手写的最小 HTTP/1.1 请求解析：读请求行 → 逐行读请求头（遇到空行结束）→
// 按 Content-Length 读取请求体。之所以不用 hyper/axum 等框架，是为了保持
// 桌面应用启动零额外依赖、便于跨平台交叉编译（尤其是 Windows xwin 交叉构建）。
fn read_request(stream: &TcpStream) -> Result<Request, String> {
    // 5 秒读超时，防止慢速/挂起连接占用工作线程。
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader
        .read_line(&mut request_line)
        .map_err(|error| error.to_string())?;
    let request_parts: Vec<_> = request_line.split_whitespace().collect();
    if request_parts.len() < 2 {
        return Err("invalid request line".into());
    }
    let mut headers: HashMap<String, String> = HashMap::new();
    let mut header_size = request_line.len();
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        header_size += line.len();
        if header_size > MAX_HEADER_BYTES {
            return Err("headers too large".into());
        }
        // 空行（CRLF 或 LF）标志请求头结束。
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().into());
        }
    }
    let content_length = headers
        .get("content-length")
        .map(|value| value.parse::<usize>().map_err(|_| "invalid content length"))
        .transpose()?
        .unwrap_or(0);
    if content_length > MAX_BODY_BYTES {
        return Err("request body too large".into());
    }
    let mut body = vec![0; content_length];
    reader
        .read_exact(&mut body)
        .map_err(|_| "incomplete request body".to_string())?;
    Ok(Request {
        method: request_parts[0].to_ascii_uppercase(),
        target: request_parts[1].into(),
        headers,
        body,
    })
}

fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

// 写出最简 HTTP 响应；固定带上宽松的 CORS 头，方便局域网内任意来源的网页/客户端调用。
// Connection: close 表示每个请求独立开关一次 TCP 连接，与下方线程池按连接分发的模型一致。
fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n",
        status_reason(status),
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn respond_json(stream: &mut TcpStream, status: u16, value: Value) {
    respond(
        stream,
        status,
        "application/json; charset=utf-8",
        value.to_string().as_bytes(),
    );
}

// 从 multipart/form-data 请求体中截取第一个字段的原始内容（图片上传只有一个文件字段，
// 不需要完整的 multipart 解析器）。非 multipart 请求直接原样返回 body。
fn extract_multipart(body: &[u8], content_type: &str) -> Option<Vec<u8>> {
    if !content_type
        .to_ascii_lowercase()
        .starts_with("multipart/form-data")
    {
        return Some(body.to_vec());
    }
    let boundary = content_type
        .split(';')
        .find_map(|part| part.trim().strip_prefix("boundary="))?
        .trim_matches('"');
    let marker = format!("--{boundary}").into_bytes();
    // 字段内容从第一个空行（分隔头部与正文）之后开始。
    let header_end = body.windows(4).position(|window| window == b"\r\n\r\n")? + 4;
    // 内容在下一个边界标记之前结束。
    let suffix = [b"\r\n--".as_slice(), boundary.as_bytes()].concat();
    let end = body[header_end..]
        .windows(suffix.len())
        .position(|window| window == suffix)?
        + header_end;
    if !body.starts_with(&marker) || end <= header_end {
        return None;
    }
    Some(body[header_end..end].to_vec())
}

// 解析 URL 查询字符串为简单的 key-value 映射（分页参数 offset/limit 用）。
fn query_params(target: &str) -> HashMap<String, String> {
    target
        .split_once('?')
        .map(|(_, query)| {
            query
                .split('&')
                .filter_map(|part| part.split_once('='))
                .map(|(key, value)| (key.into(), value.into()))
                .collect()
        })
        .unwrap_or_default()
}

// 单个 TCP 连接的完整请求处理：解析请求 → 按 method + path 路由 → 写响应。
// 这是与 Android 版 App 约定的 HTTP API 的核心实现，改动前需确认 Android 端兼容性。
fn handle_client(mut stream: TcpStream, runtime: &SharedRuntime) {
    let request = match read_request(&stream) {
        Ok(request) => request,
        Err(error) => {
            let status = if error == "request body too large" {
                413
            } else {
                400
            };
            respond_json(&mut stream, status, json!({ "error": error }));
            return;
        }
    };
    let path = request.target.split('?').next().unwrap_or("/");
    // 预检请求：所有跨源请求方法都直接放行，实际权限控制交给局域网可达性本身。
    if request.method == "OPTIONS" {
        respond_json(&mut stream, 200, json!({ "status": "ok" }));
        return;
    }
    if request.method == "GET" && path == "/health" {
        respond_json(&mut stream, 200, json!({ "status": "ok" }));
        return;
    }
    if request.method == "GET" && path == "/api/config" {
        let state = runtime.snapshot();
        respond_json(
            &mut stream,
            200,
            json!({ "rows": state.rows, "columns": state.columns, "port": state.port }),
        );
        return;
    }
    if request.method == "GET" && path == "/api/device" {
        let state = runtime.snapshot();
        respond_json(
            &mut stream,
            200,
            json!({
                "id": state.device_id,
                "name": state.device_name,
                "manufacturer": if cfg!(target_os = "macos") { "Apple" } else if cfg!(target_os = "windows") { "Microsoft" } else { "Desktop" },
                "model": default_device_name(),
                "androidVersion": std::env::consts::OS,
                "appVersion": state.app_version,
                "apiVersion": API_VERSION,
                "port": state.port,
                "rows": state.rows,
                "columns": state.columns,
                "capabilities": ["images", "slots"]
            }),
        );
        return;
    }
    if request.method == "POST" && path == "/api/images" {
        // 从 multipart 请求体里取出文件字节，再靠魔数校验确实是受支持的图片格式，
        // 通过后以随机 UUID 命名落盘，避免信任客户端提供的原始文件名。
        let Some(mut bytes) = extract_multipart(
            &request.body,
            request
                .headers
                .get("content-type")
                .map(String::as_str)
                .unwrap_or(""),
        ) else {
            respond_json(
                &mut stream,
                400,
                json!({ "error": "an image file is required" }),
            );
            return;
        };
        let Some((extension, _)) = detect_image(&bytes) else {
            respond_json(
                &mut stream,
                400,
                json!({ "error": "image must be a valid JPG, PNG, or GIF file" }),
            );
            return;
        };
        if extension == "gif" {
            make_gif_loop_forever(&mut bytes);
        }
        let filename = format!("{}.{}", Uuid::new_v4(), extension);
        if fs::write(runtime.image_dir.join(&filename), bytes).is_err() {
            respond_json(
                &mut stream,
                400,
                json!({ "error": "image must be a valid JPG, PNG, or GIF file" }),
            );
            return;
        }
        respond_json(&mut stream, 200, json!({ "filename": filename }));
        return;
    }
    if request.method == "GET" && path == "/api/images" {
        let query = query_params(&request.target);
        let offset = query.get("offset").map(|v| v.parse::<usize>()).transpose();
        let limit = query.get("limit").map(|v| v.parse::<usize>()).transpose();
        let (offset, limit) = match (offset, limit) {
            (Ok(offset), Ok(limit)) => (offset.unwrap_or(0), limit.unwrap_or(50)),
            _ => {
                respond_json(
                    &mut stream,
                    400,
                    json!({ "error": "offset must be >= 0 and limit must be between 1 and 100" }),
                );
                return;
            }
        };
        if !(1..=100).contains(&limit) {
            respond_json(
                &mut stream,
                400,
                json!({ "error": "offset must be >= 0 and limit must be between 1 and 100" }),
            );
            return;
        }
        // 用 probe_image_file 只读文件头 + 元数据，而不是整份读入内存再丢弃——
        // 见 probe_image_file 上方注释。offset/limit 只影响返回的分页切片，
        // 但排序和 total 计数仍需要遍历目录中的每一个文件。
        let mut images: Vec<Value> = fs::read_dir(&runtime.image_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let (mime, size) = probe_image_file(&entry.path())?;
                Some(json!({ "filename": name, "mimeType": mime, "size": size, "url": format!("/api/images/{name}") }))
            })
            .collect();
        images.sort_by_key(|value| value["filename"].as_str().unwrap_or("").to_owned());
        let total = images.len();
        let page: Vec<_> = images.into_iter().skip(offset).take(limit).collect();
        respond_json(
            &mut stream,
            200,
            json!({ "images": page, "offset": offset, "limit": limit, "total": total, "hasMore": offset + page.len() < total }),
        );
        return;
    }
    if let Some(filename) = path.strip_prefix("/api/images/") {
        if !safe_image_filename(filename) {
            respond_json(&mut stream, 404, json!({ "error": "image not found" }));
            return;
        }
        let image_path = runtime.image_dir.join(filename);
        if request.method == "GET" {
            if let Ok(mut bytes) = fs::read(&image_path) {
                if let Some((_, mime)) = detect_image(&bytes) {
                    // 兼容升级前已经落盘的 GIF：响应时同样修正为无限循环，
                    // 无需用户删除并重新上传旧图片。
                    if mime == "image/gif" {
                        make_gif_loop_forever(&mut bytes);
                    }
                    respond(&mut stream, 200, mime, &bytes);
                    return;
                }
            }
            respond_json(&mut stream, 404, json!({ "error": "image not found" }));
            return;
        }
        if request.method == "DELETE" {
            if fs::remove_file(&image_path).is_ok() {
                // 删除图片文件后，同步清空所有引用了该文件名的宫格，避免出现悬空引用。
                let mut state = runtime.state.write().expect("state lock poisoned");
                for tile in &mut state.tiles {
                    if tile.image_filename.as_deref() == Some(filename) {
                        tile.image_filename = None;
                    }
                }
                drop(state);
                runtime.changed();
                respond_json(
                    &mut stream,
                    200,
                    json!({ "status": "deleted", "filename": filename }),
                );
            } else {
                respond_json(&mut stream, 404, json!({ "error": "image not found" }));
            }
            return;
        }
    }
    if let Some(slot_text) = path.strip_prefix("/api/slots/") {
        // 宫格编号对外是 1-25（与 Android 版一致），内部数组下标是 0-24。
        let slot = slot_text.parse::<usize>().ok();
        if slot.is_none() || !(1..=25).contains(&slot.unwrap()) {
            respond_json(
                &mut stream,
                404,
                json!({ "error": "slot must be between 1 and 25" }),
            );
            return;
        }
        let index = slot.unwrap() - 1;
        if request.method == "DELETE" {
            runtime.state.write().expect("state lock poisoned").tiles[index] =
                MonitorTile::default();
            runtime.changed();
            respond_json(
                &mut stream,
                200,
                json!({ "status": "cleared", "slot": index + 1 }),
            );
            return;
        }
        if request.method == "POST" {
            let Ok(body) = serde_json::from_slice::<Value>(&request.body) else {
                respond_json(&mut stream, 400, json!({ "error": "invalid JSON" }));
                return;
            };
            let username = body
                .get("username")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let ai_name = body
                .get("aiName")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let image = body
                .get("image")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if username.is_empty() {
                respond_json(&mut stream, 400, json!({ "error": "username is required" }));
                return;
            }
            if ai_name.is_empty() {
                respond_json(&mut stream, 400, json!({ "error": "aiName is required" }));
                return;
            }
            if image.is_empty() {
                respond_json(&mut stream, 400, json!({ "error": "image is required" }));
                return;
            }
            // image 字段必须是此前 POST /api/images 上传接口返回的合法文件名，
            // 防止客户端传入任意路径引用到宫格上。
            if !safe_image_filename(image) {
                respond_json(
                    &mut stream,
                    400,
                    json!({ "error": "image must be a valid uploaded filename" }),
                );
                return;
            }
            runtime.state.write().expect("state lock poisoned").tiles[index] = MonitorTile {
                username: username.into(),
                ai_name: ai_name.into(),
                content: body
                    .get("content")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                image_filename: Some(image.into()),
                updated_at_millis: Some(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                ),
            };
            runtime.changed();
            respond_json(
                &mut stream,
                200,
                json!({ "status": "updated", "slot": index + 1 }),
            );
            return;
        }
    }
    respond_json(&mut stream, 404, json!({ "error": "not found" }));
}

// 启动 HTTP 服务器：从 FIRST_HTTP_PORT 起向上找第一个可绑定的端口，
// 用一个有界 mpsc 队列 + 4 个常驻工作线程处理连接，接收线程本身只管 accept，
// 不在 accept 循环里做任何耗时逻辑，避免拖慢新连接的接纳速度。
fn start_http_server(runtime: SharedRuntime) -> u16 {
    let (port, listener) = (FIRST_HTTP_PORT..=u16::MAX)
        .find_map(|port| {
            TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))
                .ok()
                .map(|listener| (port, listener))
        })
        .expect("没有可用的 HTTP 端口");
    {
        let mut state = runtime.state.write().expect("state lock poisoned");
        state.port = port;
        state.is_server_running = true;
    }
    runtime.changed();
    // 队列容量 16：突发连接超过这个数量时，多余的连接会被直接告知"服务器繁忙"，
    // 而不是无限堆积导致内存增长或让客户端无限期挂起。
    let (sender, receiver) = mpsc::sync_channel::<TcpStream>(16);
    let receiver = Arc::new(Mutex::new(receiver));
    for _ in 0..4 {
        let worker_runtime = runtime.clone();
        let worker_receiver = receiver.clone();
        thread::spawn(move || loop {
            let stream = worker_receiver.lock().expect("HTTP queue poisoned").recv();
            match stream {
                Ok(stream) => handle_client(stream, &worker_runtime),
                Err(_) => break,
            }
        });
    }
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            if let Err(error) = sender.try_send(stream) {
                // 队列已满或已断开：尽力返回 503 而不是直接丢弃连接。
                stream = match error {
                    mpsc::TrySendError::Full(stream) | mpsc::TrySendError::Disconnected(stream) => {
                        stream
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
    port
}

// UDP 主动发现：客户端向 8080 端口广播固定的探测帧，桌面端收到后单播回应设备信息。
// 相比 mDNS，这是给不支持/未启用 mDNS 的客户端准备的兜底发现方式。
fn start_udp_discovery(runtime: SharedRuntime) {
    thread::spawn(move || {
        let Ok(socket) = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, UDP_DISCOVERY_PORT)) else {
            return;
        };
        let mut buffer = [0u8; 256];
        loop {
            let Ok((length, source)) = socket.recv_from(&mut buffer) else {
                continue;
            };
            if &buffer[..length] != b"AIMONITOR_DISCOVER_V1" {
                continue;
            }
            let state = runtime.snapshot();
            let response = json!({
                "id": state.device_id,
                "name": state.device_name,
                "port": state.port,
                "apiVersion": API_VERSION.to_string()
            });
            let _ = socket.send_to(response.to_string().as_bytes(), source);
        }
    });
}

// 注册 mDNS 服务，使支持 Bonjour/Avahi 的客户端可以按 `_aimonitor._tcp.` 类型发现本机。
fn start_mdns(runtime: &SharedRuntime) {
    let state = runtime.snapshot();
    let Ok(daemon) = ServiceDaemon::new() else {
        return;
    };
    let ip: IpAddr = state
        .local_ip
        .parse()
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let properties = [
        ("id", state.device_id.as_str()),
        ("name", state.device_name.as_str()),
        ("apiVersion", "2"),
        ("path", "/api/device"),
    ];
    if let Ok(info) = ServiceInfo::new(
        "_aimonitor._tcp.local.",
        &state.device_name,
        &format!("{}.local.", state.device_id),
        ip,
        state.port,
        &properties[..],
    ) {
        let _ = daemon.register(info);
        // ServiceDaemon 需要在整个应用生命周期内存活才能持续应答 mDNS 查询，
        // 这里主动 forget 放弃其析构，是有意为之的常驻资源，不是遗漏的 drop。
        std::mem::forget(daemon);
    }
}

// 判断保存的窗口矩形与某个显示器工作区是否有"足够可见"的重叠——
// 用于窗口恢复时排除已经被移除/断开的显示器上的历史坐标。
fn rectangles_have_visible_overlap(
    window: &WindowGeometry,
    monitor_x: i32,
    monitor_y: i32,
    monitor_width: u32,
    monitor_height: u32,
) -> bool {
    let left = i64::from(window.x).max(i64::from(monitor_x));
    let top = i64::from(window.y).max(i64::from(monitor_y));
    let right = (i64::from(window.x) + i64::from(window.width))
        .min(i64::from(monitor_x) + i64::from(monitor_width));
    let bottom = (i64::from(window.y) + i64::from(window.height))
        .min(i64::from(monitor_y) + i64::from(monitor_height));
    // 至少要有 64x64 像素的重叠区域才算"可见"，避免窗口只露出一条边缘的边界情况。
    let required_width = i64::from(window.width.min(64));
    let required_height = i64::from(window.height.min(64));
    right - left >= required_width && bottom - top >= required_height
}

// 保存的窗口几何是否落在当前任意一块可用显示器范围内。
fn window_geometry_is_available(window: &WebviewWindow, geometry: &WindowGeometry) -> bool {
    if geometry.width == 0 || geometry.height == 0 {
        return false;
    }
    window
        .available_monitors()
        .map(|monitors| {
            monitors.iter().any(|monitor| {
                let area = monitor.work_area();
                rectangles_have_visible_overlap(
                    geometry,
                    area.position.x,
                    area.position.y,
                    area.size.width,
                    area.size.height,
                )
            })
        })
        .unwrap_or(false)
}

// 应用启动时恢复窗口：优先使用保存的几何信息（前提是仍在可用显示器范围内），
// 否则回退到"主显示器 + 最大化"，满足产品要求里"启动即最大化"的强制项。
fn restore_window(window: &WebviewWindow, geometry: Option<&WindowGeometry>) -> tauri::Result<()> {
    if let Some(geometry) =
        geometry.filter(|geometry| window_geometry_is_available(window, geometry))
    {
        window.unmaximize()?;
        window.set_size(PhysicalSize::new(geometry.width, geometry.height))?;
        window.set_position(PhysicalPosition::new(geometry.x, geometry.y))?;
        return Ok(());
    }

    if let Some(primary_monitor) = window.primary_monitor()? {
        let position = primary_monitor.work_area().position;
        window.set_position(PhysicalPosition::new(position.x, position.y))?;
    }
    window.maximize()
}

// 窗口移动/缩放/关闭时调用：只在非最小化、非最大化状态下记录几何信息，
// 因为最大化时的 outer_position/inner_size 并不代表用户期望的"正常窗口大小"。
fn save_window_geometry(window: &WebviewWindow, runtime: &SharedRuntime) {
    if window.is_minimized().unwrap_or(false) || window.is_maximized().unwrap_or(false) {
        return;
    }
    let (Ok(position), Ok(size)) = (window.outer_position(), window.inner_size()) else {
        return;
    };
    if size.width == 0 || size.height == 0 {
        return;
    }
    *runtime
        .window_geometry
        .lock()
        .expect("window geometry lock poisoned") = Some(WindowGeometry {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    });
    runtime.save_preferences();
}

// 读取 preferences.json；文件不存在或损坏时回退到一组合理默认值（含随机生成的新设备 ID）。
fn load_preferences(path: &Path) -> Preferences {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| Preferences {
            rows: 2,
            columns: 2,
            image_display_mode: ImageDisplayMode::FitCenter,
            auto_start: false,
            window: None,
            device_id: Uuid::new_v4().to_string(),
            device_name: default_device_name(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_gif(payload: &[u8]) -> Vec<u8> {
        let mut bytes = b"GIF89a\x01\x00\x01\x00\x00\x00\x00".to_vec();
        bytes.extend_from_slice(payload);
        bytes
    }

    #[test]
    fn changes_single_play_gif_to_infinite_loop() {
        let mut gif = minimal_gif(b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x01\x00\x00\x2c\x3b");

        assert!(make_gif_loop_forever(&mut gif));
        assert!(gif
            .windows(5)
            .any(|window| window == b"\x03\x01\x00\x00\x00"));
    }

    #[test]
    fn adds_infinite_loop_extension_when_gif_has_none() {
        let mut gif = minimal_gif(b"\x2c\x3b");

        assert!(make_gif_loop_forever(&mut gif));
        assert_eq!(&gif[13..32], b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00");
    }

    #[test]
    fn finds_loop_extension_after_other_extension_data() {
        let mut gif =
            minimal_gif(b"\x21\xfe\x01\x2c\x00\x21\xff\x0bNETSCAPE2.0\x03\x01\x01\x00\x00\x2c\x3b");
        let original_len = gif.len();

        assert!(make_gif_loop_forever(&mut gif));
        assert_eq!(gif.len(), original_len);
        assert!(gif
            .windows(5)
            .any(|window| window == b"\x03\x01\x00\x00\x00"));
    }

    #[test]
    fn leaves_non_gif_bytes_unchanged() {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        let original = png.clone();

        assert!(!make_gif_loop_forever(&mut png));
        assert_eq!(png, original);
    }

    #[test]
    fn accepts_geometry_with_a_reachable_window_area() {
        let geometry = WindowGeometry {
            x: 1_856,
            y: 100,
            width: 1_000,
            height: 700,
        };

        assert!(rectangles_have_visible_overlap(
            &geometry, 0, 0, 1_920, 1_080
        ));
    }

    #[test]
    fn rejects_geometry_left_on_a_removed_monitor() {
        let geometry = WindowGeometry {
            x: 1_900,
            y: 100,
            width: 1_000,
            height: 700,
        };

        assert!(!rectangles_have_visible_overlap(
            &geometry, 0, 0, 1_920, 1_080
        ));
    }
}

// Tauri 应用入口：注册插件、装配 Runtime、启动三个子系统、暴露 Tauri 命令。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            use tauri_plugin_autostart::ManagerExt;

            let app_handle = app.handle().clone();
            let config_dir = app.path().app_config_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            let preferences_path = config_dir.join("preferences.json");
            let image_dir = cache_dir.join("monitor_images");
            fs::create_dir_all(&image_dir)?;
            let preferences = load_preferences(&preferences_path);
            // 自启动的真实开关状态以操作系统为准，读取失败时才回退到上次保存的偏好值。
            let auto_start = app
                .autolaunch()
                .is_enabled()
                .unwrap_or(preferences.auto_start);
            if let Some(window) = app.get_webview_window("main") {
                restore_window(&window, preferences.window.as_ref())?;
            }
            let runtime = Arc::new(Runtime {
                state: RwLock::new(MonitorState {
                    rows: preferences.rows.clamp(1, 5),
                    columns: preferences.columns.clamp(1, 5),
                    image_display_mode: preferences.image_display_mode,
                    auto_start,
                    // 端口先占位，等 start_http_server 实际绑定成功后再回填真实值。
                    port: FIRST_HTTP_PORT,
                    app_version: app.package_info().version.to_string(),
                    device_id: preferences.device_id,
                    device_name: preferences.device_name,
                    is_server_running: false,
                    local_ip: local_ipv4(),
                    tiles: vec![MonitorTile::default(); 25],
                }),
                image_dir,
                preferences_path,
                window_geometry: Mutex::new(preferences.window),
                app: app_handle,
            });
            runtime.save_preferences();
            if let Some(window) = app.get_webview_window("main") {
                let runtime_for_events = runtime.clone();
                let window_for_events = window.clone();
                window.on_window_event(move |event| {
                    if matches!(
                        event,
                        WindowEvent::Moved(_)
                            | WindowEvent::Resized(_)
                            | WindowEvent::CloseRequested { .. }
                    ) {
                        save_window_geometry(&window_for_events, &runtime_for_events);
                    }
                });
            }
            let port = start_http_server(runtime.clone());
            start_udp_discovery(runtime.clone());
            start_mdns(&runtime);
            runtime.state.write().expect("state lock poisoned").port = port;
            app.manage(runtime);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_monitor_state,
            set_grid,
            set_image_display_mode,
            set_device_name,
            set_auto_start
        ])
        .run(tauri::generate_context!())
        .expect("AIMonitorDesktop 启动失败");
}
