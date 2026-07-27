// POST/DELETE /api/slots/{1..25}：更新或清空单个监控宫格。
use crate::http::protocol::{respond_json, Request};
use crate::image::safe_image_filename;
use crate::model::MonitorTile;
use crate::runtime::SharedRuntime;
use serde_json::{json, Value};
use std::{
    net::TcpStream,
    time::{SystemTime, UNIX_EPOCH},
};

// 命中则处理并返回 true；未命中返回 false，交给上一级继续尝试其他路由。
pub(crate) fn handle(
    request: &Request,
    path: &str,
    runtime: &SharedRuntime,
    stream: &mut TcpStream,
) -> bool {
    let Some(slot_text) = path.strip_prefix("/api/slots/") else {
        return false; // 不是宫格相关路径
    };
    // 宫格编号对外是 1-25（与 Android 版一致），内部数组下标是 0-24。
    let slot = slot_text.parse::<usize>().ok(); // 解析路径中的宫格编号
    if slot.is_none() || !(1..=25).contains(&slot.unwrap()) {
        respond_json(
            stream,
            404,
            json!({ "error": "slot must be between 1 and 25" }), // 编号不是数字，或超出 1-25 范围
        );
        return true;
    }
    let index = slot.unwrap() - 1; // 转换为 0 起始的数组下标
    if request.method == "DELETE" {
        runtime.state.write().expect("state lock poisoned").tiles[index] =
            MonitorTile::default(); // 用 Default 实现直接清空该宫格所有字段
        runtime.changed(); // 通知前端刷新
        respond_json(
            stream,
            200,
            json!({ "status": "cleared", "slot": index + 1 }), // 响应里的 slot 换算回对外的 1 起始编号
        );
        return true;
    }
    if request.method == "POST" {
        return handle_update(request, runtime, stream, index);
    }
    // 命中了 /api/slots/{n} 但方法既不是 DELETE 也不是 POST：交回上一级走通用 404。
    false
}

// 校验请求体并写入指定下标的宫格；所有分支都会响应并返回 true。
fn handle_update(
    request: &Request,
    runtime: &SharedRuntime,
    stream: &mut TcpStream,
    index: usize,
) -> bool {
    let Ok(body) = serde_json::from_slice::<Value>(&request.body) else {
        respond_json(stream, 400, json!({ "error": "invalid JSON" })); // 请求体不是合法 JSON
        return true;
    };
    let username = body
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim(); // 缺省或非字符串都当作空字符串处理，再统一裁剪空白
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
        respond_json(stream, 400, json!({ "error": "username is required" })); // 必填字段校验
        return true;
    }
    if ai_name.is_empty() {
        respond_json(stream, 400, json!({ "error": "aiName is required" }));
        return true;
    }
    if image.is_empty() {
        respond_json(stream, 400, json!({ "error": "image is required" }));
        return true;
    }
    // image 字段必须是此前 POST /api/images 上传接口返回的合法文件名，
    // 防止客户端传入任意路径引用到宫格上。
    if !safe_image_filename(image) {
        respond_json(
            stream,
            400,
            json!({ "error": "image must be a valid uploaded filename" }), // 文件名不合法（可能是伪造路径）
        );
        return true;
    }
    runtime.state.write().expect("state lock poisoned").tiles[index] = MonitorTile {
        username: username.into(),
        ai_name: ai_name.into(),
        content: body
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
    respond_json(
        stream,
        200,
        json!({ "status": "updated", "slot": index + 1 }),
    );
    true
}
