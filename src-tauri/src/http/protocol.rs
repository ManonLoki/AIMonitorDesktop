// HTTP 请求解析与响应写出的底层工具：不包含任何路由逻辑，只负责"字节 ↔ 结构化数据"。
use crate::constants::{MAX_BODY_BYTES, MAX_HEADER_BYTES};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    time::Duration,
};

// 解析后的最简 HTTP 请求：只保留后续路由逻辑需要的字段。
pub(crate) struct Request {
    pub(crate) method: String,               // 大写的 HTTP 方法，如 GET/POST/DELETE
    pub(crate) target: String,               // 原始请求目标（含查询字符串），如 "/api/images?limit=10"
    pub(crate) headers: HashMap<String, String>, // 小写化的请求头名称到值的映射
    pub(crate) body: Vec<u8>,                // 原始请求体字节
}

// 手写的最小 HTTP/1.1 请求解析：读请求行 → 逐行读请求头（遇到空行结束）→
// 按 Content-Length 读取请求体。之所以不用 hyper/axum 等框架，是为了保持
// 桌面应用启动零额外依赖、便于跨平台交叉编译（尤其是 Windows xwin 交叉构建）。
pub(crate) fn read_request(stream: &TcpStream) -> Result<Request, String> {
    // 5 秒读超时，防止慢速/挂起连接占用工作线程。
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    let mut reader = BufReader::new(stream); // 缓冲读取，配合 read_line 按行解析
    let mut request_line = String::new(); // 存放请求行，如 "GET /health HTTP/1.1"
    reader
        .read_line(&mut request_line)
        .map_err(|error| error.to_string())?;
    let request_parts: Vec<_> = request_line.split_whitespace().collect(); // 按空白拆分为 [方法, 路径, 协议版本]
    if request_parts.len() < 2 {
        return Err("invalid request line".into()); // 至少要有方法和路径两段
    }
    let mut headers: HashMap<String, String> = HashMap::new(); // 收集请求头
    let mut header_size = request_line.len(); // 已读字节数累计，含请求行本身
    loop {
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|error| error.to_string())?;
        header_size += line.len(); // 累加本行长度到总头部大小
        if header_size > MAX_HEADER_BYTES {
            return Err("headers too large".into()); // 超过头部上限，拒绝继续解析
        }
        // 空行（CRLF 或 LF）标志请求头结束。
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().into()); // 头名统一转小写，便于后续用 "content-type" 这样的固定 key 查找
        }
    }
    let content_length = headers
        .get("content-length")
        .map(|value| value.parse::<usize>().map_err(|_| "invalid content length")) // 头存在但不是合法数字则报错
        .transpose()? // Option<Result<_>> -> Result<Option<_>>，出错时提前返回
        .unwrap_or(0); // 没有 Content-Length 头视为空请求体
    if content_length > MAX_BODY_BYTES {
        return Err("request body too large".into()); // 请求体超过上限，拒绝读取
    }
    let mut body = vec![0; content_length]; // 按声明长度预分配缓冲区
    reader
        .read_exact(&mut body)
        .map_err(|_| "incomplete request body".to_string())?; // 必须读满声明的长度，否则视为请求体不完整
    Ok(Request {
        method: request_parts[0].to_ascii_uppercase(), // 方法统一大写，兼容大小写不规范的客户端
        target: request_parts[1].into(),
        headers,
        body,
    })
}

// 根据 HTTP 状态码返回对应的标准原因短语，用于拼接状态行。
fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        _ => "Error", // 本服务只会用到以上几种状态码，其余情况兜底
    }
}

// 写出最简 HTTP 响应；固定带上宽松的 CORS 头，方便局域网内任意来源的网页/客户端调用。
// Connection: close 表示每个请求独立开关一次 TCP 连接，与工作线程池按连接分发的模型一致。
pub(crate) fn respond(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n",
        status_reason(status), // 状态行里的原因短语
        body.len() // Content-Length 必须与实际写出的 body 长度一致
    );
    let _ = stream.write_all(header.as_bytes()); // 先写响应头
    let _ = stream.write_all(body); // 再写响应体
    let _ = stream.flush(); // 确保数据实际发出；写入失败在这里也一律静默忽略（连接可能已被对端关闭）
}

// respond 的 JSON 语法糖：自动设置 Content-Type 并序列化 Value。
pub(crate) fn respond_json(stream: &mut TcpStream, status: u16, value: Value) {
    respond(
        stream,
        status,
        "application/json; charset=utf-8",
        value.to_string().as_bytes(),
    );
}

// 从 multipart/form-data 请求体中截取第一个字段的原始内容（图片上传只有一个文件字段，
// 不需要完整的 multipart 解析器）。非 multipart 请求直接原样返回 body。
pub(crate) fn extract_multipart(body: &[u8], content_type: &str) -> Option<Vec<u8>> {
    if !content_type
        .to_ascii_lowercase()
        .starts_with("multipart/form-data")
    {
        return Some(body.to_vec()); // 非 multipart，直接把整个请求体当作文件内容
    }
    let boundary = content_type
        .split(';')
        .find_map(|part| part.trim().strip_prefix("boundary=")) // 从 Content-Type 头里提取 boundary 参数
        ?
        .trim_matches('"'); // 部分客户端会给 boundary 加引号，去掉它
    let marker = format!("--{boundary}").into_bytes(); // multipart 每个分段前的边界标记
    // 字段内容从第一个空行（分隔头部与正文）之后开始。
    let header_end = body.windows(4).position(|window| window == b"\r\n\r\n")? + 4;
    // 内容在下一个边界标记之前结束。
    let suffix = [b"\r\n--".as_slice(), boundary.as_bytes()].concat(); // 内容结尾处紧跟的下一个边界前缀
    let end = body[header_end..]
        .windows(suffix.len())
        .position(|window| window == suffix)? // 在字段内容里查找下一个边界的起始位置
        + header_end; // 转换回相对整个 body 的偏移
    if !body.starts_with(&marker) || end <= header_end {
        return None; // 请求体不是以边界开头，或找不到有效的内容区间，视为格式错误
    }
    Some(body[header_end..end].to_vec()) // 截取头部结束到下一个边界之间的原始字节
}

// 解析 URL 查询字符串为简单的 key-value 映射（分页参数 offset/limit 用）。
pub(crate) fn query_params(target: &str) -> HashMap<String, String> {
    target
        .split_once('?') // 只关心 '?' 之后的部分
        .map(|(_, query)| {
            query
                .split('&') // 按 & 拆分每个键值对
                .filter_map(|part| part.split_once('=')) // 忽略没有 '=' 的畸形片段
                .map(|(key, value)| (key.into(), value.into()))
                .collect()
        })
        .unwrap_or_default() // 没有查询字符串时返回空映射
}
