// POST/GET /api/images 与 GET/DELETE /api/images/{filename}：图片上传、分页列表、下载与删除。
use crate::http::protocol::{extract_multipart, query_params, respond, respond_json, Request};
use crate::image::{detect_image, make_gif_loop_forever, probe_image_file, safe_image_filename};
use crate::runtime::SharedRuntime;
use serde_json::{json, Value};
use std::{fs, net::TcpStream};
use uuid::Uuid;

// 命中则处理并返回 true；未命中返回 false，交给上一级继续尝试其他路由。
pub(crate) fn handle(
    request: &Request,
    path: &str,
    runtime: &SharedRuntime,
    stream: &mut TcpStream,
) -> bool {
    if request.method == "POST" && path == "/api/images" {
        return handle_upload(request, runtime, stream);
    }
    if request.method == "GET" && path == "/api/images" {
        return handle_list(request, runtime, stream);
    }
    if let Some(filename) = path.strip_prefix("/api/images/") {
        return handle_single(request, filename, runtime, stream);
    }
    false
}

// 从 multipart 请求体里取出文件字节，再靠魔数校验确实是受支持的图片格式，
// 通过后以随机 UUID 命名落盘，避免信任客户端提供的原始文件名。
fn handle_upload(request: &Request, runtime: &SharedRuntime, stream: &mut TcpStream) -> bool {
    let Some(mut bytes) = extract_multipart(
        &request.body,
        request
            .headers
            .get("content-type")
            .map(String::as_str)
            .unwrap_or(""),
    ) else {
        respond_json(
            stream,
            400,
            json!({ "error": "an image file is required" }), // multipart 解析失败，说明没有携带有效文件字段
        );
        return true;
    };
    let Some((extension, _)) = detect_image(&bytes) else {
        respond_json(
            stream,
            400,
            json!({ "error": "image must be a valid JPG, PNG, or GIF file" }), // 魔数校验未通过，不是受支持的图片格式
        );
        return true;
    };
    if extension == "gif" {
        make_gif_loop_forever(&mut bytes); // GIF 落盘前先修正为无限循环播放
    }
    let filename = format!("{}.{}", Uuid::new_v4(), extension); // 随机 UUID 文件名，避免信任客户端文件名
    if fs::write(runtime.image_dir.join(&filename), bytes).is_err() {
        respond_json(
            stream,
            400,
            json!({ "error": "image must be a valid JPG, PNG, or GIF file" }), // 写盘失败（磁盘满/权限问题等）复用同一错误文案
        );
        return true;
    }
    respond_json(stream, 200, json!({ "filename": filename })); // 返回生成的文件名，客户端后续用它引用到具体宫格
    true
}

// 用 probe_image_file 只读文件头 + 元数据，而不是整份读入内存再丢弃——
// 见 image.rs 中 probe_image_file 上方注释。offset/limit 只影响返回的分页切片，
// 但排序和 total 计数仍需要遍历目录中的每一个文件。
fn handle_list(request: &Request, runtime: &SharedRuntime, stream: &mut TcpStream) -> bool {
    let query = query_params(&request.target); // 解析分页参数
    let offset = query.get("offset").map(|v| v.parse::<usize>()).transpose(); // 解析失败会在下面统一报错
    let limit = query.get("limit").map(|v| v.parse::<usize>()).transpose();
    let (offset, limit) = match (offset, limit) {
        (Ok(offset), Ok(limit)) => (offset.unwrap_or(0), limit.unwrap_or(50)), // 未提供时的默认分页参数
        _ => {
            respond_json(
                stream,
                400,
                json!({ "error": "offset must be >= 0 and limit must be between 1 and 100" }), // offset/limit 不是合法数字
            );
            return true;
        }
    };
    if !(1..=100).contains(&limit) {
        respond_json(
            stream,
            400,
            json!({ "error": "offset must be >= 0 and limit must be between 1 and 100" }), // limit 超出允许范围
        );
        return true;
    }
    let mut images: Vec<Value> = fs::read_dir(&runtime.image_dir)
        .into_iter() // Result<ReadDir> -> Iterator（失败则产生空迭代）
        .flatten() // 展开 ReadDir 本身的迭代
        .flatten() // 展开每个 Result<DirEntry>，忽略读取失败的条目
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned(); // 文件名（非法 UTF-8 会被替换字符处理）
            let (mime, size) = probe_image_file(&entry.path())?; // 探测失败（非图片文件等）的条目直接跳过
            Some(json!({ "filename": name, "mimeType": mime, "size": size, "url": format!("/api/images/{name}") }))
        })
        .collect();
    images.sort_by_key(|value| value["filename"].as_str().unwrap_or("").to_owned()); // 按文件名排序，保证分页结果稳定
    let total = images.len(); // 排序、过滤后的总数
    let page: Vec<_> = images.into_iter().skip(offset).take(limit).collect(); // 取出当前页
    respond_json(
        stream,
        200,
        json!({ "images": page, "offset": offset, "limit": limit, "total": total, "hasMore": offset + page.len() < total }), // hasMore 用于客户端判断是否还有下一页
    );
    true
}

// GET 下载单张图片 / DELETE 删除单张图片，两者共用文件名校验与路径拼接。
fn handle_single(
    request: &Request,
    filename: &str,
    runtime: &SharedRuntime,
    stream: &mut TcpStream,
) -> bool {
    if !safe_image_filename(filename) {
        respond_json(stream, 404, json!({ "error": "image not found" })); // 文件名不合法一律当作"不存在"处理，不泄露具体校验规则
        return true;
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
                respond(stream, 200, mime, &bytes); // 直接把图片原始字节写回，Content-Type 用探测到的 MIME
                return true;
            }
        }
        respond_json(stream, 404, json!({ "error": "image not found" })); // 文件不存在，或存在但不是受支持的图片格式
        return true;
    }
    if request.method == "DELETE" {
        if fs::remove_file(&image_path).is_ok() {
            // 删除图片文件后，同步清空所有引用了该文件名的宫格，避免出现悬空引用。
            let mut state = runtime.state.write().expect("state lock poisoned"); // 写锁保护，遍历并修改宫格数组
            for tile in &mut state.tiles {
                if tile.image_filename.as_deref() == Some(filename) {
                    tile.image_filename = None; // 清空对已删除图片的引用
                }
            }
            drop(state); // 显式提前释放写锁，再去广播事件，避免事件回调里再次读锁时死锁
            runtime.changed(); // 通知前端宫格数据已变化
            respond_json(
                stream,
                200,
                json!({ "status": "deleted", "filename": filename }),
            );
        } else {
            respond_json(stream, 404, json!({ "error": "image not found" })); // 文件本就不存在或删除失败（如权限问题）
        }
        return true;
    }
    // 命中了 /api/images/{filename} 但方法既不是 GET 也不是 DELETE：
    // 交回上一级，最终落到通用的 404 "not found"，与其余未匹配路由保持一致。
    false
}
