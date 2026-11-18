//! Beatnik Hardware API: configures Raspberry Pi audio HATs, CamillaDSP and
//! Snapcast, and updates the Beatnik components.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::http::Method;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

mod boot_config;
mod camilla;
mod cmd;
mod detection;
mod error;
mod hats;
mod mdns;
mod routes;
mod snapcast;
mod system;
mod update;

const DEFAULT_PORT: u16 = 3000;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        // No color codes in journald
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    if let Err(error) = camilla::seed_configs().await {
        tracing::error!("Failed to seed CamillaDSP configs: {error}");
    }

    let state = Arc::new(routes::AppState {
        boot_config: boot_config::BootConfigService::from_env(),
        camilla: camilla::CamillaInstance::main_from_env(),
        preamp: camilla::CamillaInstance::preamp_from_env(),
        update: update::UpdateService::from_env(),
    });

    // Reflect any origin, like the previous `origin: true` CORS setting
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_methods([Method::GET, Method::HEAD, Method::PUT, Method::PATCH, Method::POST, Method::DELETE])
        .allow_headers(AllowHeaders::mirror_request());

    let app = routes::router(state).layer(cors).layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
            .on_response(DefaultOnResponse::new().level(Level::INFO)),
    );

    let port = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(DEFAULT_PORT);
    // 0.0.0.0 allows access from the controller container and the LAN
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(error) => {
            tracing::error!("Failed to bind {addr}: {error}");
            std::process::exit(1);
        }
    };
    tracing::info!("🎧 Beatnik Hardware Service is running on port {port}");

    let mdns = Arc::new(mdns::MdnsService::from_env(port));
    tokio::spawn({
        let mdns = mdns.clone();
        async move { mdns.start().await }
    });

    if let Err(error) = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await {
        tracing::error!("Server error: {error}");
    }

    tracing::info!("Shutting down...");
    mdns.stop().await;
}

/// Resolves on SIGTERM or SIGINT.
async fn shutdown_signal() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM handler can be installed");
    let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT handler can be installed");
    tokio::select! {
        _ = sigterm.recv() => {}
        _ = sigint.recv() => {}
    }
}
