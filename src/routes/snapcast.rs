//! `/api/snapcast`: Snapserver / Snapclient control.

use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use super::SharedState;
use crate::error::{ApiError, ApiResult};
use crate::snapcast;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(status))
        .route("/enable", post(enable))
        .route("/disable", post(disable))
        .route("/restart-server", post(restart_server))
        .route("/restart-client", post(restart_client))
}

async fn status() -> Json<snapcast::SnapcastStatus> {
    Json(snapcast::get_status().await)
}

/// `{ success, message, active, enabled }`
async fn with_status(message: &str) -> Json<Value> {
    let status = snapcast::get_status().await;
    Json(json!({ "success": true, "message": message, "active": status.active, "enabled": status.enabled }))
}

async fn enable() -> ApiResult<Json<Value>> {
    snapcast::enable().await.map_err(|_| ApiError::internal("Failed to enable snapserver"))?;
    Ok(with_status("Snapserver enabled").await)
}

async fn disable() -> ApiResult<Json<Value>> {
    snapcast::disable().await.map_err(|_| ApiError::internal("Failed to disable snapserver"))?;
    Ok(with_status("Snapserver disabled").await)
}

async fn restart_server() -> ApiResult<Json<Value>> {
    snapcast::restart_server().await.map_err(|_| ApiError::internal("Failed to restart snapserver"))?;
    Ok(Json(json!({ "success": true, "message": "Snapserver restarted" })))
}

async fn restart_client() -> ApiResult<Json<Value>> {
    snapcast::restart_client().await.map_err(|_| ApiError::internal("Failed to restart snapclient"))?;
    Ok(Json(json!({ "success": true, "message": "Snapclient restarted" })))
}
