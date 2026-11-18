//! `/api/update`: background updates of bleno, the controller and this API.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use super::SharedState;
use crate::error::{ApiError, ApiResult};
use crate::update::{UpdateTarget, UPDATE_TARGETS};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(status))
        .route("/versions", get(versions))
        .route("/{target}", post(start))
}

/// State of the last update per target.
async fn status(State(state): State<SharedState>) -> Json<Value> {
    Json(json!({ "busy": state.update.is_busy(), "targets": state.update.get_status() }))
}

/// Installed versions of api, bleno and controller.
async fn versions(State(state): State<SharedState>) -> Json<Value> {
    Json(json!({ "versions": state.update.get_versions().await }))
}

/// Starts an update in the background: bleno | controller | api | all
async fn start(
    State(state): State<SharedState>,
    Path(target): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let request = match target.as_str() {
        "all" => None,
        name => Some(UpdateTarget::parse(name).ok_or_else(|| {
            let valid: Vec<&str> = UPDATE_TARGETS.iter().map(|t| t.as_str()).chain(["all"]).collect();
            ApiError::bad_request(format!("Unknown target \"{target}\". Use one of: {}", valid.join(", ")))
        })?),
    };

    let targets = state.update.start(request).await.map_err(|e| ApiError::new(e.status, e.message))?;
    let names: Vec<&str> = targets.iter().map(|t| t.as_str()).collect();
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "success": true,
            "message": format!("Update started for: {}", names.join(", ")),
            "targets": targets,
        })),
    ))
}
