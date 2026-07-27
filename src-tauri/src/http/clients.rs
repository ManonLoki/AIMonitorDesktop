// POST /api/clients/{client_id}/heartbeat：刷新控制端的槽位租约。
use super::error_json;
use crate::{heartbeat::valid_client_id, runtime::SharedRuntime};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub(crate) async fn heartbeat(
    State(runtime): State<SharedRuntime>,
    Path(client_id): Path<String>,
) -> Response {
    if !valid_client_id(&client_id) {
        return error_json(StatusCode::BAD_REQUEST, "invalid clientId");
    }
    runtime
        .client_leases
        .lock()
        .expect("client lease lock poisoned")
        .heartbeat(&client_id);
    Json(json!({ "status": "alive", "clientId": client_id })).into_response()
}
