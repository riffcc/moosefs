use anyhow::Result;
use clap::Parser;
use mooseng_common::ShutdownCoordinator;
use tracing::{info, error};
use tracing_subscriber::EnvFilter;

mod config;
mod replication;
mod wal;
mod snapshot;
mod recovery;
mod server;

use config::MetaloggerConfig;
use server::MetaloggerServer;

#[derive(Parser)]
#[command(name = "mooseng-metalogger")]
#[command(about = "MooseNG Metalogger - Metadata backup and replication service")]
struct Args {
    /// Configuration file path
    #[arg(short, long, default_value = "/etc/mooseng/metalogger.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info"))
        )
        .json()
        .init();

    // Parse command line arguments
    let args = Args::parse();
    info!("Starting MooseNG Metalogger with config: {}", args.config);

    // Load configuration
    let config = MetaloggerConfig::load(&args.config)?;
    info!("Loaded configuration: {:?}", config);

    // Create shutdown coordinator
    let shutdown = ShutdownCoordinator::new();

    // Create and start metalogger server
    let server = Arc::new(MetaloggerServer::new(config, shutdown.clone()).await?);
    
    // Run the server
    if let Err(e) = server.run().await {
        error!("Metalogger server error: {}", e);
    }

    info!("MooseNG Metalogger shutdown complete");
    Ok(())
}