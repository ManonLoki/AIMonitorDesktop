// POST/DELETE /api/slots/{1..25}：更新或清空单个监控宫格。
use super::error_json;
use crate::image::safe_image_filename;
use crate::model::MonitorTile;
use crate::runtime::SharedRuntime;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

// 宫格编号对外是 1-25（与 Android 版一致），内部数组下标是 0-24；
// 解析失败或越界统一返回 404，与原手写实现保持一致（不区分是否合法数字）。
// 错误分支只带一个静态字符串而不是现成的 Response，避免 Result 的 Err 变体
// 因为携带整个 HTTP 响应类型而变得过大（clippy::result_large_err）。
fn parse_slot_index(slot_text: &str) -> Result<usize, &'static str> {
    match slot_text.parse::<usize>() {
        Ok(slot) if (1..=25).contains(&slot) => Ok(slot - 1),
        _ => Err("slot must be between 1 and 25"),
    }
}

// DELETE /api/slots/{slot}：用 Default 实现直接清空该宫格所有字段。
pub(crate) async fn clear_slot(
    State(runtime): State<SharedRuntime>,
    Path(slot_text): Path<String>,
) -> Response {
    let index = match parse_slot_index(&slot_text) {
        Ok(index) => index,
        Err(message) => return error_json(StatusCode::NOT_FOUND, message),
    };
    runtime.state.write().expect("state lock poisoned").tiles[index] = MonitorTile::default();
    runtime.changed(); // 通知前端刷新
    Json(json!({ "status": "cleared", "slot": index + 1 })).into_response() // 响应里的 slot 换算回对外的 1 起始编号
}

// POST /api/slots/{slot}：校验请求体并写入指定下标的宫格。
pub(crate) async fn update_slot(
    State(runtime): State<SharedRuntime>,
    Path(slot_text): Path<String>,
    body: Bytes,
) -> Response {
    let index = match parse_slot_index(&slot_text) {
        Ok(index) => index,
        Err(message) => return error_json(StatusCode::NOT_FOUND, message),
    };
    let Ok(payload) = serde_json::from_slice::<Value>(&body) else {
        return error_json(StatusCode::BAD_REQUEST, "invalid JSON"); // 请求体不是合法 JSON
    };
    let username = payload
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim(); // 缺省或非字符串都当作空字符串处理，再统一裁剪空白
    let ai_name = payload
        .get("aiName")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let image = payload
        .get("image")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if username.is_empty() {
        return error_json(StatusCode::BAD_REQUEST, "username is required"); // 必填字段校验
    }
    if ai_name.is_empty() {
        return error_json(StatusCode::BAD_REQUEST, "aiName is required");
    }
    if image.is_empty() {
        return error_json(StatusCode::BAD_REQUEST, "image is required");
    }
    // image 字段必须是此前 POST /api/images 上传接口返回的合法文件名，
    // 防止客户端传入任意路径引用到宫格上。
    if !safe_image_filename(image) {
        return error_json(
            StatusCode::BAD_REQUEST,
            "image must be a valid uploaded filename", // 文件名不合法（可能是伪造路径）
        );
    }
    runtime.state.write().expect("state lock poisoned").tiles[index] = MonitorTile {
        username: username.into(),
        ai_name: ai_name.into(),
        content: payload
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(), // content 是可选字段，缺省为空字符串
        image_filename: Some(image.into()),
        updated_at_millis: Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH) // 计算自 Unix 纪元以来经过的时间
                .unwrap_or_default() // 系统时钟异常（早于纪元）时兜底为 0
                .as_millis() as u64, // 转换为毫秒时间戳
        ),
    };
    runtime.changed(); // 通知前端该宫格已更新
    Json(json!({ "status": "updated", "slot": index + 1 })).into_response()
}
