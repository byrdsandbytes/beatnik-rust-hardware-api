use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::Router;
use serde_json::Value;

use crate::boot_config::BootConfigService;
use crate::camilla::CamillaInstance;
use crate::cmd;
use crate::update::UpdateService;

mod audio;
mod snapcast;
mod system;
mod update;

pub struct AppState {
    pub boot_config: BootConfigService,
    pub camilla: CamillaInstance,
    pub preamp: CamillaInstance,
    pub update: UpdateService,
}

pub type SharedState = Arc<AppState>;

pub fn router(state: SharedState) -> Router {
    Router::new()
        .nest("/api/hardware", audio::router())
        .nest("/api/system", system::router())
        .nest("/api/snapcast", snapcast::router())
        .nest("/api/update", update::router())
        .with_state(state)
}

/// Reads a string field from a JSON body. Missing, invalid or non-JSON bodies yield `None`.
fn body_string(body: &Bytes, field: &str) -> Option<String> {
    let value: Value = serde_json::from_slice(body).ok()?;
    match value.get(field)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// Runs `program args` after a short delay, so the HTTP response can be sent first.
fn spawn_delayed(program: &'static str, args: &'static [&'static str]) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        if let Err(error) = cmd::run(program, args).await {
            tracing::error!("{program} failed: {error}");
        }
    });
}
