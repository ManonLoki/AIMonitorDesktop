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

const FIRST_HTTP_PORT: u16 = 10_241;
const UDP_DISCOVERY_PORT: u16 = 8_080;
const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
const MAX_HEADER_BYTES: usize = 16 * 1024;
const API_VERSION: u8 = 2;

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct MonitorTile {
    username: String,
    ai_name: String,
    content: String,
    image_filename: Option<String>,
    updated_at_millis: Option<u64>,
}

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

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WindowGeometry {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

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

struct Runtime {
    state: RwLock<MonitorState>,
    image_dir: PathBuf,
    preferences_path: PathBuf,
    window_geometry: Mutex<Option<WindowGeometry>>,
    app: AppHandle,
}

type SharedRuntime = Arc<Runtime>;

impl Runtime {
    fn snapshot(&self) -> MonitorState {
        self.state.read().expect("state lock poisoned").clone()
    }

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

    fn changed(&self) {
        let _ = self.app.emit("monitor-state-changed", ());
    }
}

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

fn default_device_name() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "AIMonitorDesktop".into())
}

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

struct Request {
    method: String,
    target: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn read_request(stream: &TcpStream) -> Result<Request, String> {
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
    let header_end = body.windows(4).position(|window| window == b"\r\n\r\n")? + 4;
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
        let Some(bytes) = extract_multipart(
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
        let mut images: Vec<Value> = fs::read_dir(&runtime.image_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let bytes = fs::read(entry.path()).ok()?;
                let (_, mime) = detect_image(&bytes)?;
                Some(json!({ "filename": name, "mimeType": mime, "size": bytes.len(), "url": format!("/api/images/{name}") }))
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
            if let Ok(bytes) = fs::read(&image_path) {
                if let Some((_, mime)) = detect_image(&bytes) {
                    respond(&mut stream, 200, mime, &bytes);
                    return;
                }
            }
            respond_json(&mut stream, 404, json!({ "error": "image not found" }));
            return;
        }
        if request.method == "DELETE" {
            if fs::remove_file(&image_path).is_ok() {
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
        std::mem::forget(daemon);
    }
}

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
    let required_width = i64::from(window.width.min(64));
    let required_height = i64::from(window.height.min(64));
    right - left >= required_width && bottom - top >= required_height
}

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
