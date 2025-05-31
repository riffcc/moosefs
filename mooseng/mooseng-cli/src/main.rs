use clap::{Parser, Subcommand};
use std::process;

mod admin;
mod cluster;
mod monitoring;
mod config;
mod data;
mod grpc_client;
mod benchmark;

#[derive(Parser)]
#[command(name = "mooseng")]
#[command(about = "MooseNG Distributed File System CLI")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Cluster management operations
    Cluster {
        #[command(subcommand)]
        action: cluster::ClusterCommands,
    },
    /// Administrative operations
    Admin {
        #[command(subcommand)]
        action: admin::AdminCommands,
    },
    /// Monitoring and status operations  
    Monitor {
        #[command(subcommand)]
        action: monitoring::MonitorCommands,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: config::ConfigCommands,
    },
    /// Data upload, download, and management operations
    Data {
        #[command(subcommand)]
        action: data::DataCommands,
    },
    /// Benchmark operations for performance testing
    Benchmark {
        #[command(subcommand)]
        action: benchmark::BenchmarkCommands,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Cluster { action } => cluster::handle_command(action).await,
        Commands::Admin { action } => admin::handle_command(action).await,
        Commands::Monitor { action } => monitoring::handle_command(action).await,
        Commands::Config { action } => config::handle_command(action).await,
        Commands::Data { action } => data::handle_command(action).await,
        Commands::Benchmark { action } => benchmark::handle_command(action).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}