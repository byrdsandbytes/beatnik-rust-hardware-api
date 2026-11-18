//! `/api/system`: system info and reboot.

use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use super::{spawn_delayed, SharedState};
use crate::system;

pub fn router() -> Router<SharedState> {
    Router::new().route("/info", get(info)).route("/reboot", post(reboot))
}

async fn info() -> Json<system::SystemInfo> {
    Json(system::get_info().await)
}

async fn reboot() -> Json<Value> {
    spawn_delayed("sudo", &["reboot"]);
    Json(json!({ "message": "Rebooting..." }))
}
