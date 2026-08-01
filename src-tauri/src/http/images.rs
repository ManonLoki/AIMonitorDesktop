// POST/GET /api/images 与 GET/DELETE /api/images/{filename}：图片上传、分页列表、下载与删除。
// 所有文件 I/O 都走 tokio::fs（见 image.rs 的 probe_image_file），保持整条请求链路
// 纯异步、不阻塞 Tokio 工作线程；multipart 的 boundary 与字段解析交给 Axum。
use super::error_json;
use crate::image::{detect_image, make_gif_loop_forever, probe_image_file, safe_image_filename};
use crate::runtime::SharedRuntime;
use axum::{
    body::Bytes,
    extract::{FromRequest, Multipart, Path, Query, Request, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::task::JoinSet;
use uuid::Uuid;

// 分页查询参数；字段保留为 Option<String> 而非 Option<usize>，
// 这样非法数字（如 "abc"）不会在提取阶段就被 Axum 拒绝并返回它自带的错误格式，
// 而是走下面手写的校验逻辑，继续产出与 Android 端约定一致的 `{"error": "..."}`。
#[derive(Deserialize)]
pub(crate) struct Pagination {
    offset: Option<String>,
    limit: Option<String>,
}

fn required_file_error() -> Response {
    error_json(StatusCode::BAD_REQUEST, "an image file is required")
}

// 这个分支只判断请求是否应交给 Multipart 提取器，不自行解析 boundary。
fn is_multipart_form(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("multipart/form-data"))
}

// 保留 Axum 提取器的 413，交给外层 normalize_body_limit_rejection 统一改写为 JSON；
// 其他 body/multipart 解析失败都映射为 Android API 约定的 required 错误。
fn normalize_upload_rejection(response: Response) -> Response {
    if response.status() == StatusCode::PAYLOAD_TOO_LARGE {
        response
    } else {
        required_file_error()
    }
}

// multipart 只接受 API 约定的 `file` 字段，或显式带 filename 的文件字段；
// 普通文本字段会被跳过。非 multipart 客户端仍可直接 POST 原始二进制。
async fn extract_upload_bytes(request: Request) -> Result<Bytes, Response> {
    if !is_multipart_form(request.headers()) {
        let bytes = Bytes::from_request(request, &())
            .await
            .map_err(|rejection| normalize_upload_rejection(rejection.into_response()))?;
        return (!bytes.is_empty())
            .then_some(bytes)
            .ok_or_else(required_file_error);
    }

    let mut multipart = Multipart::from_request(request, &())
        .await
        .map_err(|rejection| normalize_upload_rejection(rejection.into_response()))?;
    loop {
        let field = multipart
            .next_field()
            .await
            .map_err(|error| normalize_upload_rejection(error.into_response()))?;
        let Some(field) = field else {
            return Err(required_file_error());
        };
        if field.name() != Some("file") && field.file_name().is_none() {
            continue;
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|error| normalize_upload_rejection(error.into_response()))?;
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }
}

// POST /api/images：从请求体里取出文件字节，靠魔数校验是受支持的图片格式后，
// 以随机 UUID 命名落盘，避免信任客户端提供的原始文件名。
pub(crate) async fn upload_image(
    State(runtime): State<SharedRuntime>,
    request: Request,
) -> Response {
    let bytes = match extract_upload_bytes(request).await {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };
    let Some((extension, _)) = detect_image(&bytes) else {
        return error_json(
            StatusCode::BAD_REQUEST,
            "image must be a valid JPG, PNG, or GIF file", // 魔数校验未通过，不是受支持的图片格式
        );
    };
    // 只有 GIF 需要就地改写循环标记，才值得为它单独拷贝一份可写缓冲区；
    // JPG/PNG 直接落盘原始 Bytes，避免每次上传都多一次整份拷贝。
    let filename = format!("{}.{}", Uuid::new_v4(), extension); // 随机 UUID 文件名，避免信任客户端文件名
    let write_result = if extension == "gif" {
        let mut bytes = bytes.to_vec();
        make_gif_loop_forever(&mut bytes); // GIF 落盘前先修正为无限循环播放
        tokio::fs::write(runtime.image_dir.join(&filename), bytes).await
    } else {
        tokio::fs::write(runtime.image_dir.join(&filename), bytes).await
    };
    if write_result.is_err() {
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
    Query(query): Query<Pagination>,
) -> Response {
    let offset = query.offset.map(|v| v.parse::<usize>()).transpose(); // 解析失败会在下面统一报错
    let limit = query.limit.map(|v| v.parse::<usize>()).transpose();
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

    // 先收集目录项，再用 JoinSet 并发探测每个文件的魔数与大小——各文件的探测
    // 互不依赖，且结果反正要重新按文件名排序，谁先完成不影响最终顺序。
    let mut images: Vec<Value> = Vec::new();
    if let Ok(mut dir) = tokio::fs::read_dir(&runtime.image_dir).await {
        let mut probes = JoinSet::new();
        while let Ok(Some(entry)) = dir.next_entry().await {
            let name = entry.file_name().to_string_lossy().into_owned(); // 文件名（非法 UTF-8 会被替换字符处理）
            let path = entry.path();
            probes.spawn(async move { probe_image_file(&path).await.map(|probe| (name, probe)) });
        }
        while let Some(result) = probes.join_next().await {
            // Err 分支只会在探测任务 panic 时出现，属于不该发生的异常情况，直接丢弃该条目即可。
            if let Ok(Some((name, (mime, size)))) = result {
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
            // 直接把图片原始字节写回，Content-Type 用探测到的 MIME。
            return (StatusCode::OK, [(header::CONTENT_TYPE, mime)], bytes).into_response();
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
            let mut state = runtime.state.write(); // 写锁保护，遍历并修改宫格数组
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::extract::DefaultBodyLimit;

    fn request(content_type: Option<&str>, body: impl Into<Body>) -> Request {
        let mut builder = Request::builder();
        if let Some(content_type) = content_type {
            builder = builder.header(header::CONTENT_TYPE, content_type);
        }
        builder.body(body.into()).expect("test request is valid")
    }

    async fn assert_required(request: Request) {
        let response = extract_upload_bytes(request)
            .await
            .expect_err("request must not contain an upload");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("error body is readable");
        assert_eq!(body, r#"{"error":"an image file is required"}"#);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn accepts_raw_binary_upload() {
        let bytes = extract_upload_bytes(request(
            Some("application/octet-stream"),
            Body::from(&b"raw image bytes"[..]),
        ))
        .await
        .expect("raw request body is the upload");

        assert_eq!(bytes, &b"raw image bytes"[..]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn skips_text_field_before_named_file() {
        let body = concat!(
            "--boundary\r\n",
            "Content-Disposition: form-data; name=\"description\"\r\n\r\n",
            "not the file\r\n",
            "--boundary\r\n",
            "Content-Disposition: form-data; name=\"file\"\r\n\r\n",
            "actual file\r\n",
            "--boundary--\r\n",
        );

        let bytes = extract_upload_bytes(request(
            Some("multipart/form-data; boundary=boundary"),
            body,
        ))
        .await
        .expect("named file field is selected");

        assert_eq!(bytes, "actual file");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn accepts_quoted_boundary_and_filename_field() {
        let body = concat!(
            "--quoted-boundary\r\n",
            "Content-Disposition: form-data; name=\"upload\"; filename=\"image.png\"\r\n",
            "Content-Type: image/png\r\n\r\n",
            "file by filename\r\n",
            "--quoted-boundary--\r\n",
        );

        let bytes = extract_upload_bytes(request(
            Some("multipart/form-data; boundary=\"quoted-boundary\""),
            body,
        ))
        .await
        .expect("filename marks the field as a file upload");

        assert_eq!(bytes, "file by filename");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_multipart_without_file_field() {
        let body = concat!(
            "--boundary\r\n",
            "Content-Disposition: form-data; name=\"description\"\r\n\r\n",
            "text only\r\n",
            "--boundary--\r\n",
        );
        assert_required(request(
            Some("multipart/form-data; boundary=boundary"),
            body,
        ))
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn rejects_empty_file_and_invalid_boundary() {
        let empty_file = concat!(
            "--boundary\r\n",
            "Content-Disposition: form-data; name=\"file\"; filename=\"empty.png\"\r\n\r\n",
            "\r\n--boundary--\r\n",
        );
        assert_required(request(
            Some("multipart/form-data; boundary=boundary"),
            empty_file,
        ))
        .await;
        assert_required(request(Some("multipart/form-data"), "not multipart")).await;
        assert_required(request(
            Some("multipart/form-data; boundary=\"\""),
            "not multipart",
        ))
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn preserves_payload_too_large_for_outer_json_normalizer() {
        let body = concat!(
            "--boundary\r\n",
            "Content-Disposition: form-data; name=\"file\"; filename=\"large.png\"\r\n\r\n",
            "this payload is deliberately over the test limit\r\n",
            "--boundary--\r\n",
        );
        let mut request = request(Some("multipart/form-data; boundary=boundary"), body);
        DefaultBodyLimit::max(32).apply(&mut request);

        let response = extract_upload_bytes(request)
            .await
            .expect_err("body limit must reject the upload");
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
