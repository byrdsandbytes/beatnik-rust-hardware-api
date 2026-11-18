//! Advertises the API as `_beatnik._tcp` by writing an Avahi service file.

use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::system;

const SERVICE_FILENAME: &str = "beatnik-hardware.service";

pub struct MdnsService {
    service_file: PathBuf,
    port: u16,
    running: AtomicBool,
}

impl MdnsService {
    pub fn from_env(port: u16) -> Self {
        let dir = std::env::var("AVAHI_SERVICE_DIR").unwrap_or_else(|_| "/etc/avahi/services".into());
        Self {
            service_file: PathBuf::from(dir).join(SERVICE_FILENAME),
            port,
            running: AtomicBool::new(false),
        }
    }

    pub async fn start(&self) {
        if self.running.load(Ordering::SeqCst) {
            return;
        }

        let info = system::get_info().await;
        let mac = info.mac_address.as_deref().unwrap_or("00:00:00:00:00:00");
        let total = system::memory_info().await.total;
        let ram = format!("{:.1}GB", total as f64 / (1024.0 * 1024.0 * 1024.0));
        let version = env!("CARGO_PKG_VERSION");
        let port = self.port;

        tracing::info!("Starting mDNS advertisement for _beatnik._tcp on port {port}");
        tracing::info!("Host: {}, MAC: {mac}, Model: {}, RAM: {ram}, v{version}", info.hostname, info.model);

        // %h in the name is replaced by avahi-daemon with the hostname
        let xml = [
            r#"<?xml version="1.0" standalone='no'?><!--*-nxml-*-->"#.to_string(),
            r#"<!DOCTYPE service-group SYSTEM "avahi-service.dtd">"#.into(),
            "<service-group>".into(),
            r#"  <name replace-wildcards="yes">Beatnik Hardware API - %h</name>"#.into(),
            "  <service>".into(),
            "    <type>_beatnik._tcp</type>".into(),
            format!("    <port>{port}</port>"),
            format!("    <txt-record>version={}</txt-record>", xml_escape(version)),
            format!("    <txt-record>mac={}</txt-record>", xml_escape(mac)),
            format!("    <txt-record>model={}</txt-record>", xml_escape(&info.model)),
            format!("    <txt-record>ram={ram}</txt-record>"),
            "    <txt-record>build_ref=b8d9c2</txt-record>".into(),
            "  </service>".into(),
            "</service-group>".into(),
        ]
        .join("\n");

        match tokio::fs::write(&self.service_file, xml).await {
            Ok(()) => {
                tracing::info!("Service definition written to {}", self.service_file.display());
                self.running.store(true, Ordering::SeqCst);
            }
            Err(error) => {
                tracing::error!(
                    "Error writing Avahi service file to {}: {error}",
                    self.service_file.display()
                );
                if error.kind() == ErrorKind::PermissionDenied {
                    tracing::error!(
                        "Make sure the application has write permissions to the Avahi services directory."
                    );
                }
            }
        }
    }

    pub async fn stop(&self) {
        if !self.running.load(Ordering::SeqCst) {
            return;
        }
        tracing::info!("Stopping mDNS advertisement...");
        match tokio::fs::remove_file(&self.service_file).await {
            Ok(()) => {
                tracing::info!("Service definition removed from {}", self.service_file.display());
                self.running.store(false, Ordering::SeqCst);
            }
            Err(error) if error.kind() == ErrorKind::NotFound => self.running.store(false, Ordering::SeqCst),
            Err(error) => {
                tracing::error!("Error removing Avahi service file {}: {error}", self.service_file.display())
            }
        }
    }
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}
