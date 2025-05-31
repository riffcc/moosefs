use anyhow::Result;
use clap::Parser;
use mooseng_common::config::MasterConfig;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

mod server;
mod metadata;
mod cache;
mod filesystem;
mod chunk_manager;
mod session;
mod storage_class;
mod grpc_services;
// TODO: Enable when raft module is fixed
// mod raft;
mod multiregion;

use server::MasterServer;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "/etc/mooseng/master.toml")]
    config: PathBuf,

    /// Data directory override
    #[arg(short, long)]
    data_dir: Option<PathBuf>,

    /// Enable debug logging
    #[arg(short = 'v', long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    info!("Starting MooseNG Master Server v{}", mooseng_common::MOOSENG_VERSION);

    // Load configuration
    let mut config = load_config(&args.config)?;
    if let Some(data_dir) = args.data_dir {
        config.data_dir = data_dir;
    }

    // Validate configuration
    validate_config(&config)?;

    info!("Configuration loaded from: {:?}", args.config);
    info!("Data directory: {:?}", config.data_dir);
    info!("Region: {} (ID: {})", config.region_name, config.region_id);
    info!("Listening on: {}:{} (client), {}:{} (admin), {}:{} (chunk server)", 
          config.bind_address, config.client_port,
          config.bind_address, config.admin_port,
          config.bind_address, config.cs_port);

    // Create and start the master server
    let server = MasterServer::new(config).await?;
    
    // Set up signal handlers
    let shutdown = setup_shutdown_handler();

    // Run the server
    match server.run(shutdown).await {
        Ok(_) => {
            info!("Master server shut down gracefully");
            Ok(())
        }
        Err(e) => {
            error!("Master server error: {}", e);
            Err(e)
        }
    }
}

fn load_config(path: &PathBuf) -> Result<MasterConfig> {
    if path.exists() {
        let settings = ::config::Config::builder()
            .add_source(::config::File::from(path.as_path()))
            .build()?;
        
        Ok(settings.try_deserialize()?)
    } else {
        info!("Configuration file not found, using defaults");
        Ok(MasterConfig::default())
    }
}

fn validate_config(config: &MasterConfig) -> Result<()> {
    // Validate data directory is writable
    if !config.data_dir.exists() {
        std::fs::create_dir_all(&config.data_dir)
            .map_err(|e| anyhow::anyhow!("Cannot create data directory {:?}: {}", config.data_dir, e))?;
    }

    // Check for port conflicts
    let ports = vec![
        config.client_port,
        config.cs_port,
        config.admin_port,
        config.metalogger_port,
        config.raft_port,
    ];
    
    for (i, &port1) in ports.iter().enumerate() {
        for &port2 in ports.iter().skip(i + 1) {
            if port1 == port2 {
                return Err(anyhow::anyhow!("Port conflict detected: port {} is used multiple times", port1));
            }
        }
    }

    // Validate cache size
    if config.metadata_cache_size < 1024 * 1024 {
        return Err(anyhow::anyhow!("Metadata cache size too small (minimum 1MB)"));
    }

    // Validate worker threads
    if config.worker_threads > 256 {
        return Err(anyhow::anyhow!("Too many worker threads specified (maximum 256)"));
    }

    // Validate timeouts
    if config.session_timeout_ms < 1000 {
        return Err(anyhow::anyhow!("Session timeout too short (minimum 1 second)"));
    }

    info!("Configuration validation passed");
    Ok(())
}

fn setup_shutdown_handler() -> tokio::sync::broadcast::Receiver<()> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
        
        info!("Received shutdown signal");
        let _ = shutdown_tx.send(());
    });

    shutdown_rx
}