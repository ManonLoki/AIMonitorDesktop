// POST/GET /api/images 与 GET/DELETE /api/images/{filename}：图片上传、分页列表、下载与删除。
// 所有文件 I/O 都走 tokio::fs（见 image.rs 的 probe_image_file），保持整条请求链路
// 纯异步、不阻塞 Tokio 工作线程；multipart 解析仍是手写的小函数，因为图片上传接口
// 只有一个文件字段，不需要为此引入完整的 multipart 依赖。
use crate::image::{detect_image, make_gif_loop_forever, probe_image_file, safe_image_filename};
use crate::runtime::SharedRuntime;
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

// 统一构造 `{"error": "..."}` 形式的 JSON 错误响应，与手写实现时期的错误结构保持一致，
// 这样 Android 端已有的错误处理逻辑不需要改动。
fn error_json(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

// 从 multipart/form-data 请求体中截取第一个字段的原始内容。非 multipart 请求
// 直接把整个 body 当作文件内容（兼容非浏览器客户端直接 POST 二进制的场景）。
fn extract_multipart(body: &[u8], content_type: &str) -> Option<Vec<u8>> {
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
    let header_end = body.windows(4).position(|window| window == b"\r\n\r\n")? + 4; // 字段内容从第一个空行之后开始
    let suffix = [b"\r\n--".as_slice(), boundary.as_bytes()].concat(); // 内容结尾处紧跟的下一个边界前缀
    let end = body[header_end..]
        .windows(suffix.len())
        .position(|window| window == suffix)?
        + header_end;
    if !body.starts_with(&marker) || end <= header_end {
        return None; // 请求体不是以边界开头，或找不到有效的内容区间，视为格式错误
    }
    Some(body[header_end..end].to_vec()) // 截取头部结束到下一个边界之间的原始字节
}

// POST /api/images：从请求体里取出文件字节，靠魔数校验是受支持的图片格式后，
// 以随机 UUID 命名落盘，避免信任客户端提供的原始文件名。
pub(crate) async fn upload_image(
    State(runtime): State<SharedRuntime>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let Some(mut bytes) = extract_multipart(&body, content_type) else {
        return error_json(StatusCode::BAD_REQUEST, "an image file is required"); // multipart 解析失败，说明没有携带有效文件字段
    };
    let Some((extension, _)) = detect_image(&bytes) else {
        return error_json(
            StatusCode::BAD_REQUEST,
            "image must be a valid JPG, PNG, or GIF file", // 魔数校验未通过，不是受支持的图片格式
        );
    };
    if extension == "gif" {
        make_gif_loop_forever(&mut bytes); // GIF 落盘前先修正为无限循环播放
    }
    let filename = format!("{}.{}", Uuid::new_v4(), extension); // 随机 UUID 文件名，避免信任客户端文件名
    if tokio::fs::write(runtime.image_dir.join(&filename), bytes)
        .await
        .is_err()
    {
        return error_json(
            StatusCode::BAD_REQUEST,
            "image must be a valid JPG, PNG, or GIF file", // 写盘失败（磁盘满/权限问题等）复用同一错误文案
        );
    }
    Json(json!({ "filename": filename })).into_response() // 返回生成的文件名，客户端后续用它引用到具体宫格
}

// GET /api/images：分页列出已上传图片。用 probe_image_file 只读文件头 + 元数据，
// 而不是把每个文件整体读入内存——见 image.rs 中该函数上方的注释。
pub(crate) async fn list_images(
    State(runtime): State<SharedRuntime>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let offset = query.get("offset").map(|v| v.parse::<usize>()).transpose(); // 解析失败会在下面统一报错
    let limit = query.get("limit").map(|v| v.parse::<usize>()).transpose();
    let (offset, limit) = match (offset, limit) {
        (Ok(offset), Ok(limit)) => (offset.unwrap_or(0), limit.unwrap_or(50)), // 未提供时的默认分页参数
        _ => {
            return error_json(
                StatusCode::BAD_REQUEST,
                "offset must be >= 0 and limit must be between 1 and 100", // offset/limit 不是合法数字
            );
        }
    };
    if !(1..=100).contains(&limit) {
        return error_json(
            StatusCode::BAD_REQUEST,
            "offset must be >= 0 and limit must be between 1 and 100", // limit 超出允许范围
        );
    }

    let mut images: Vec<Value> = Vec::new();
    if let Ok(mut dir) = tokio::fs::read_dir(&runtime.image_dir).await {
        while let Ok(Some(entry)) = dir.next_entry().await {
            // 异步遍历目录，逐个探测；探测失败（非图片文件等）的条目直接跳过
            let name = entry.file_name().to_string_lossy().into_owned(); // 文件名（非法 UTF-8 会被替换字符处理）
            if let Some((mime, size)) = probe_image_file(&entry.path()).await {
                images.push(
                    json!({ "filename": name, "mimeType": mime, "size": size, "url": format!("/api/images/{name}") }),
                );
            }
        }
    }
    images.sort_by_key(|value| value["filename"].as_str().unwrap_or("").to_owned()); // 按文件名排序，保证分页结果稳定
    let total = images.len(); // 排序、过滤后的总数
    let page: Vec<_> = images.into_iter().skip(offset).take(limit).collect(); // 取出当前页
    Json(
        json!({ "images": page, "offset": offset, "limit": limit, "total": total, "hasMore": offset + page.len() < total }), // hasMore 用于客户端判断是否还有下一页
    )
    .into_response()
}

// GET /api/images/{filename}：下载单张图片原始字节，GIF 会先修正为无限循环。
pub(crate) async fn get_image(
    State(runtime): State<SharedRuntime>,
    Path(filename): Path<String>,
) -> Response {
    if !safe_image_filename(&filename) {
        return error_json(StatusCode::NOT_FOUND, "image not found"); // 文件名不合法一律当作"不存在"处理，不泄露具体校验规则
    }
    let image_path = runtime.image_dir.join(&filename);
    if let Ok(mut bytes) = tokio::fs::read(&image_path).await {
        if let Some((_, mime)) = detect_image(&bytes) {
            // 兼容升级前已经落盘的 GIF：响应时同样修正为无限循环，无需用户重新上传旧图片。
            if mime == "image/gif" {
                make_gif_loop_forever(&mut bytes);
            }
            return (StatusCode::OK, [(header::CONTENT_TYPE, mime)], bytes).into_response(); // 直接把图片原始字节写回，Content-Type 用探测到的 MIME
        }
    }
    error_json(StatusCode::NOT_FOUND, "image not found") // 文件不存在，或存在但不是受支持的图片格式
}

// DELETE /api/images/{filename}：删除图片文件，并同步清空所有引用了该文件名的宫格。
pub(crate) async fn delete_image(
    State(runtime): State<SharedRuntime>,
    Path(filename): Path<String>,
) -> Response {
    if !safe_image_filename(&filename) {
        return error_json(StatusCode::NOT_FOUND, "image not found");
    }
    let image_path = runtime.image_dir.join(&filename);
    if tokio::fs::remove_file(&image_path).await.is_ok() {
        {
            let mut state = runtime.state.write().expect("state lock poisoned"); // 写锁保护，遍历并修改宫格数组
            for tile in &mut state.tiles {
                if tile.image_filename.as_deref() == Some(filename.as_str()) {
                    tile.image_filename = None; // 清空对已删除图片的引用
                }
            }
        } // 写锁在此处离开作用域被释放，再去广播事件，避免事件回调里再次读锁时死锁
        runtime.changed(); // 通知前端宫格数据已变化
        Json(json!({ "status": "deleted", "filename": filename })).into_response()
    } else {
        error_json(StatusCode::NOT_FOUND, "image not found") // 文件本就不存在或删除失败（如权限问题）
    }
}
