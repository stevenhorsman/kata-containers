mod config;
mod libvirt;
mod server;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use config::Config;
use libvirt::LibvirtManager;

#[derive(Parser, Debug)]
#[command(name = "kata-remote-libvirt")]
#[command(about = "Minimal kata-remote hypervisor implementation using libvirt", long_about = None)]
struct Args {
    /// Path to the Unix socket for the ttrpc server
    #[arg(short, long)]
    socket: Option<PathBuf>,

    /// Path to configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Libvirt connection URI
    #[arg(short, long)]
    libvirt_uri: Option<String>,

    /// Base volume in format "pool/volume"
    #[arg(short, long)]
    base_volume: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize tracing
    let log_level = args.log_level.parse::<tracing::Level>()
        .unwrap_or(tracing::Level::INFO);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_level(true)
        )
        .with(
            tracing_subscriber::filter::LevelFilter::from_level(log_level)
        )
        .init();

    tracing::info!("Starting kata-remote-libvirt hypervisor");

    // Load configuration
    let mut config = if let Some(config_path) = args.config {
        tracing::info!("Loading configuration from: {:?}", config_path);
        Config::from_file(&config_path)?
    } else {
        Config::default()
    };

    // Override with command-line arguments
    if let Some(socket) = args.socket {
        config.socket_path = socket;
    }
    if let Some(uri) = args.libvirt_uri {
        config.libvirt_uri = uri;
    }
    if let Some(volume) = args.base_volume {
        config.base_volume = volume;
    }

    tracing::info!("Configuration:");
    tracing::info!("  Socket path: {:?}", config.socket_path);
    tracing::info!("  Libvirt URI: {}", config.libvirt_uri);
    tracing::info!("  Base volume: {}", config.base_volume);
    tracing::info!("  Default vCPUs: {}", config.default_vcpus);
    tracing::info!("  Default memory: {} MiB", config.default_memory);

    // Initialize libvirt manager
    let libvirt = LibvirtManager::new(config.clone())?;

    // Start ttrpc server
    let socket_path = config.socket_path.to_string_lossy().to_string();

    // Setup signal handler for graceful shutdown
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("Received shutdown signal");
        tx.send(()).await.ok();
    });

    // Start server in a separate task
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server::start_server(&socket_path, libvirt).await {
            tracing::error!("Server error: {}", e);
        }
    });

    // Wait for shutdown signal
    rx.recv().await;

    tracing::info!("Shutting down...");

    // Abort the server task
    server_handle.abort();

    // Clean up socket
    if let Err(e) = std::fs::remove_file(&config.socket_path) {
        tracing::warn!("Failed to remove socket file: {}", e);
    }

    tracing::info!("Shutdown complete");

    Ok(())
}
