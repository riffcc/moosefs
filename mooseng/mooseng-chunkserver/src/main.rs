use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tokio::signal;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

mod cache;
mod chunk;
mod config;
mod error;
mod mmap;
mod server;
mod storage;
mod grpc_server;

use crate::config::ChunkServerConfig;
use crate::server::ChunkServer;
use crate::grpc_server::ChunkServerGrpc;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,
    
    /// Override bind address
    #[arg(long)]
    bind_address: Option<String>,
    
    /// Override port
    #[arg(short, long)]
    port: Option<u16>,
    
    /// Override data directory
    #[arg(short, long)]
    data_dir: Option<PathBuf>,
    
    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize logging
    let filter = if args.debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::from_default_env()
            .add_directive("mooseng_chunkserver=info".parse()?)
            .add_directive("mooseng_protocol=info".parse()?)
            .add_directive("mooseng_common=info".parse()?)
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
    
    info!("Starting MooseNG Chunk Server");
    
    // Load configuration
    let mut config = if let Some(config_path) = args.config {
        info!("Loading configuration from: {}", config_path.display());
        ChunkServerConfig::from_file(&config_path)?
    } else {
        info!("Using default configuration");
        ChunkServerConfig::default()
    };
    
    // Apply command line overrides
    if let Some(bind_address) = args.bind_address {
        config.bind_address = bind_address;
    }
    if let Some(port) = args.port {
        config.port = port;
    }
    if let Some(data_dir) = args.data_dir {
        config.data_dir = data_dir;
    }
    
    info!("Configuration: {:?}", config);
    
    // Ensure data directory exists
    std::fs::create_dir_all(&config.data_dir)?;
    info!("Data directory: {}", config.data_dir.display());
    
    // Create chunk server instance
    let chunk_server = ChunkServer::new(config.clone()).await?;
    
    // Start the chunk server
    chunk_server.start().await?;
    
    // Create and start gRPC server
    let grpc_server = ChunkServerGrpc::new(chunk_server);
    let addr = format!("{}:{}", config.bind_address, config.port).parse()?;
    
    info!("Starting gRPC server on {}", addr);
    
    // Create shutdown signal handler
    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    
    // Spawn signal handler task
    tokio::spawn(async move {
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("Received shutdown signal");
                let _ = shutdown_tx.send(());
            }
            Err(e) => {
                error!("Failed to listen for shutdown signal: {}", e);
            }
        }
    });
    
    // Start gRPC server with graceful shutdown
    grpc_server.serve(addr, shutdown_rx).await.map_err(|e| anyhow::anyhow!("gRPC server error: {}", e))?;
    
    info!("Chunk server shut down gracefully");
    Ok(())
}