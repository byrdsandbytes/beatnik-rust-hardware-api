//! `/api/hardware`: HATs, CamillaDSP configs, reboot.

use axum::body::Bytes;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

use super::{body_string, spawn_delayed, SharedState};
use crate::camilla::CamillaInstance;
use crate::error::{ApiError, ApiResult};
use crate::{detection, hats};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/hats", get(list_hats))
        .route("/camilla/configs", get(list_configs))
        .route("/camilla/configs/default", get(get_default).put(set_default))
        // Preamp instance: same logic, but no HAT patching (it plays into a Snapcast pipe)
        .route("/camilla/preamp/configs", get(list_preamp_configs))
        .route("/camilla/preamp/configs/default", get(get_preamp_default).put(set_preamp_default))
        .route("/status", get(status))
        .route("/apply", post(apply))
        .route("/reboot", post(reboot))
}

async fn list_hats() -> Json<&'static [hats::HatProfile]> {
    Json(hats::SUPPORTED_HATS)
}

async fn list_configs_of(camilla: &CamillaInstance, error: &str) -> ApiResult<Json<Value>> {
    match camilla.list_available_configs().await {
        Ok(configs) => Ok(Json(json!({ "configs": configs }))),
        Err(e) => {
            tracing::error!("{error}: {e}");
            Err(ApiError::internal(error))
        }
    }
}

async fn get_default_of(camilla: &CamillaInstance, error: &str) -> ApiResult<Json<Value>> {
    match camilla.get_default_config().await {
        Ok(file_name) => Ok(Json(json!({ "fileName": file_name }))),
        Err(e) => {
            tracing::error!("{error}: {e}");
            Err(ApiError::internal(error))
        }
    }
}

/// Points the active config link to `fileName` and restarts the instance's service.
async fn set_default_of(
    camilla: &CamillaInstance,
    body: &Bytes,
    hat_id: Option<&str>,
) -> ApiResult<Json<Value>> {
    let file_name =
        body_string(body, "fileName").ok_or_else(|| ApiError::bad_request("fileName is required"))?;

    let result = async {
        camilla.set_default_config(&file_name, hat_id).await?;
        camilla.restart_service().await
    }
    .await;

    match result {
        Ok(()) => Ok(Json(json!({ "status": "success", "fileName": file_name, "camillaRestarted": true }))),
        Err(e) => {
            tracing::error!("Failed to set default CamillaDSP config: {e}");
            Err(ApiError::bad_request(e.0))
        }
    }
}

async fn list_configs(State(state): State<SharedState>) -> ApiResult<Json<Value>> {
    list_configs_of(&state.camilla, "Failed to list CamillaDSP configs").await
}

async fn get_default(State(state): State<SharedState>) -> ApiResult<Json<Value>> {
    get_default_of(&state.camilla, "Failed to get default CamillaDSP config").await
}

async fn set_default(State(state): State<SharedState>, body: Bytes) -> ApiResult<Json<Value>> {
    let active = state.boot_config.get_active_config().await;
    set_default_of(&state.camilla, &body, active.map(|hat| hat.id)).await
}

async fn list_preamp_configs(State(state): State<SharedState>) -> ApiResult<Json<Value>> {
    list_configs_of(&state.preamp, "Failed to list preamp CamillaDSP configs").await
}

async fn get_preamp_default(State(state): State<SharedState>) -> ApiResult<Json<Value>> {
    get_default_of(&state.preamp, "Failed to get default preamp CamillaDSP config").await
}

async fn set_preamp_default(State(state): State<SharedState>, body: Bytes) -> ApiResult<Json<Value>> {
    set_default_of(&state.preamp, &body, None).await
}

/// What is configured, what was detected.
async fn status(State(state): State<SharedState>) -> ApiResult<Json<Value>> {
    let (active, detected, eeprom_disabled, camilla_config, preamp_config) = tokio::join!(
        state.boot_config.get_active_config(),
        detection::detect_connected_hat(),
        state.boot_config.is_eeprom_read_disabled(),
        state.camilla.get_default_config(),
        // The preamp is optional; a missing setup must not break the status route
        state.preamp.get_default_config(),
    );

    let camilla_config = camilla_config.map_err(|e| {
        tracing::error!("Failed to get default CamillaDSP config: {e}");
        ApiError::internal(format!("Failed to get default CamillaDSP config: {e}"))
    })?;

    Ok(Json(json!({
        "currentConfig": active,
        "detectedHardware": detected,
        "isMatch": active.map(|h| h.id) == detected.map(|h| h.id),
        "eepromReadDisabled": eeprom_disabled,
        "camillaConfigFile": camilla_config,
        "preampConfigFile": preamp_config.ok(),
    })))
}

/// Writes the boot overlay and the CamillaDSP output device for a HAT. Reboot required.
async fn apply(State(state): State<SharedState>, body: Bytes) -> ApiResult<Json<Value>> {
    let hat_id = body_string(&body, "hatId")
        .filter(|id| hats::find(id).is_some())
        .ok_or_else(|| ApiError::bad_request("Invalid HAT ID"))?;

    tracing::info!("Applying configuration for {hat_id}...");
    let result = async {
        state.boot_config.set_hat(&hat_id).await.map_err(|e| e.to_string())?;
        state.camilla.update_config(&hat_id).await.map_err(|e| e.0)?;
        state.camilla.restart_service().await.map_err(|e| e.0)
    }
    .await;

    match result {
        Ok(()) => Ok(Json(json!({
            "status": "success",
            "message": "Configuration applied. CamillaDSP restarted. Reboot required.",
            "camillaRestarted": true,
            "rebootRequired": true,
        }))),
        Err(e) => {
            tracing::error!("Failed to apply configuration: {e}");
            Err(ApiError::internal("Failed to apply configuration"))
        }
    }
}

async fn reboot() -> Json<Value> {
    spawn_delayed("reboot", &[]);
    Json(json!({ "status": "rebooting" }))
}
