//! aifed-daemon - Background daemon for aifed
//!
//! This daemon provides LSP services for a single workspace via HTTP over Unix socket.
//! Each daemon instance is bound to exactly one workspace.

mod args;
mod detection;
mod error;
mod idle;
mod languages;
mod lsp;
mod server;

use aifed_common::socket_path;
use args::Args;
use clap::Parser;
use detection::detect;
use idle::IdleMonitor;
use languages::RustAnalyzerConfig;
use lsp::LanguageServerManager;
use server::{DaemonState, build_router};
use std::sync::Arc;
use tokio::net::UnixListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry().with(tracing_subscriber::fmt::layer()).init();

    // Parse CLI arguments
    let args = Args::parse();

    // Canonicalize workspace path
    let workspace = args.workspace.canonicalize().map_err(|e| {
        anyhow::anyhow!(
            "Failed to canonicalize workspace path '{}': {}",
            args.workspace.display(),
            e
        )
    })?;

    // Determine socket path
    let socket = args
        .socket
        .unwrap_or_else(|| socket_path(&workspace).expect("Failed to generate socket path"));

    // Ensure socket directory exists
    if let Some(parent) = socket.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Remove existing socket if present
    if socket.exists() {
        std::fs::remove_file(&socket)?;
    }

    tracing::info!("Starting aifed-daemon for workspace: {}", workspace.display());
    tracing::info!("Socket path: {}", socket.display());

    // Initialize LSP manager
    let lsp_manager = Arc::new(LanguageServerManager::new());

    // Register language server configs
    lsp_manager.register_config(RustAnalyzerConfig).await;
    tracing::debug!("Registered language server configs: rust (rust-analyzer)");

    // Auto-detect languages and start LSP servers
    let detected = detect(&workspace);
    tracing::info!("Detected languages: {:?}", detected);

    for lang in &detected {
        match lsp_manager.start(lang, workspace.clone()).await {
            Ok(()) => tracing::info!("Started LSP server for: {}", lang),
            Err(e) => tracing::error!("Failed to start LSP server for {}: {}", lang, e),
        }
    }

    // Initialize idle monitor
    let idle_monitor = Arc::new(IdleMonitor::new(args.idle_timeout_secs));
    let monitor_clone = idle_monitor.clone();
    monitor_clone.start_monitor();

    // Create shared state
    let state = DaemonState {
        workspace: workspace.clone(),
        lsp_manager,
        idle_monitor: idle_monitor.clone(),
    };

    // Build router
    let app = build_router(state);

    // Bind to Unix socket
    let listener = UnixListener::bind(&socket)?;
    tracing::info!("Listening on socket: {}", socket.display());

    // Setup graceful shutdown
    let idle_for_shutdown = idle_monitor.clone();
    let shutdown_signal = async move {
        let mut shutdown_rx = idle_for_shutdown.subscribe_shutdown();
        tokio::select! {
            _ = shutdown_rx.recv() => {
                tracing::info!("Idle timeout shutdown triggered");
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Ctrl+C received, shutting down");
            }
        }
    };

    // Run server
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal).await?;

    // Cleanup
    tracing::info!("Shutting down daemon");
    if let Err(e) = std::fs::remove_file(&socket) {
        tracing::warn!("Failed to remove socket file: {}", e);
    }

    Ok(())
}
