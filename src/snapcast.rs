//! Controls the Snapserver / Snapclient systemd services.

use serde::Serialize;

use crate::cmd;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SnapcastStatus {
    pub active: bool,
    pub enabled: bool,
}

/// Whether snapserver is running and whether it starts on boot.
pub async fn get_status() -> SnapcastStatus {
    // A non-zero exit code means inactive / disabled
    let active = cmd::run("systemctl", &["is-active", "snapserver"]).await.is_ok();
    let enabled = cmd::run("systemctl", &["is-enabled", "snapserver"]).await.is_ok();
    SnapcastStatus { active, enabled }
}

/// Enables and starts snapserver, then starts the controller container if it exists.
pub async fn enable() -> Result<(), cmd::CmdError> {
    cmd::run("sudo", &["systemctl", "enable", "--now", "snapserver"])
        .await
        .inspect_err(|e| tracing::error!("Error enabling snapserver: {e}"))?;
    if let Err(error) = cmd::run("sudo", &["docker", "start", "beatnik-controller"]).await {
        tracing::warn!("Failed to start beatnik-controller container: {error}");
    }
    Ok(())
}

/// Stops the controller container, disables snapserver and restarts snapclient
/// so it can search for another server right away.
pub async fn disable() -> Result<(), cmd::CmdError> {
    if let Err(error) = cmd::run("sudo", &["docker", "stop", "beatnik-controller"]).await {
        tracing::warn!("Failed to stop beatnik-controller container: {error}");
    }
    let result = async {
        cmd::run("sudo", &["systemctl", "disable", "--now", "snapserver"]).await?;
        cmd::run("sudo", &["systemctl", "restart", "snapclient"]).await
    }
    .await;
    result
        .map(drop)
        .inspect_err(|e| tracing::error!("Error disabling snapserver or restarting snapclient: {e}"))
}

pub async fn restart_server() -> Result<(), cmd::CmdError> {
    cmd::run("sudo", &["systemctl", "restart", "snapserver"])
        .await
        .map(drop)
        .inspect_err(|e| tracing::error!("Error restarting snapserver: {e}"))
}

pub async fn restart_client() -> Result<(), cmd::CmdError> {
    cmd::run("sudo", &["systemctl", "restart", "snapclient"])
        .await
        .map(drop)
        .inspect_err(|e| tracing::error!("Error restarting snapclient: {e}"))
}
