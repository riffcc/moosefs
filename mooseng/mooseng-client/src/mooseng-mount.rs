use clap::Parser;
use mooseng_client::{ClientConfig, MooseFuse};
use std::path::PathBuf;
use tracing::{error, info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mooseng_client=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let matches = Command::new("mooseng-mount")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Mount MooseNG distributed filesystem")
        .arg(
            Arg::new("master")
                .short('m')
                .long("master")
                .value_name("ADDRESS")
                .help("Master server address")
                .default_value("127.0.0.1:9421"),
        )
        .arg(
            Arg::new("mount-point")
                .value_name("MOUNT_POINT")
                .help("Mount point directory")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path"),
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .action(clap::ArgAction::Count)
                .help("Enable debug mode (can be specified multiple times)"),
        )
        .arg(
            Arg::new("read-only")
                .short('r')
                .long("read-only")
                .action(clap::ArgAction::SetTrue)
                .help("Mount in read-only mode"),
        )
        .arg(
            Arg::new("allow-other")
                .long("allow-other")
                .action(clap::ArgAction::SetTrue)
                .help("Allow other users to access the mount"),
        )
        .arg(
            Arg::new("allow-root")
                .long("allow-root")
                .action(clap::ArgAction::SetTrue)
                .help("Allow root to access the mount"),
        )
        .arg(
            Arg::new("cache-size")
                .long("cache-size")
                .value_name("MB")
                .help("Metadata cache size in MB")
                .default_value("256"),
        )
        .get_matches();

    // Load configuration
    let mut config = if let Some(config_path) = matches.get_one::<String>("config") {
        ClientConfig::from_file(config_path)?
    } else {
        ClientConfig::default()
    };

    // Override config with command line arguments
    if let Some(master) = matches.get_one::<String>("master") {
        config.master_addr = master.parse()?;
    }

    if let Some(mount_point) = matches.get_one::<String>("mount-point") {
        config.mount_point = PathBuf::from(mount_point);
    }

    config.debug_level = matches.get_count("debug") as u8;
    config.read_only = matches.get_flag("read-only");
    config.allow_other = matches.get_flag("allow-other");
    config.allow_root = matches.get_flag("allow-root");

    if let Some(cache_size) = matches.get_one::<String>("cache-size") {
        let size_mb: usize = cache_size.parse()?;
        config.cache.metadata_cache_size = size_mb * 1024; // Convert MB to entries (rough estimate)
        config.cache.data_cache_size = (size_mb as u64) * 1024 * 1024; // Convert to bytes
    }

    info!("MooseNG FUSE Mount starting");
    info!("Master server: {}", config.master_addr);
    info!("Mount point: {}", config.mount_point.display());
    info!("Read-only: {}", config.read_only);

    // Verify mount point exists
    if !config.mount_point.exists() {
        error!("Mount point does not exist: {}", config.mount_point.display());
        return Err("Mount point does not exist".into());
    }

    if !config.mount_point.is_dir() {
        error!("Mount point is not a directory: {}", config.mount_point.display());
        return Err("Mount point is not a directory".into());
    }

    // Connect to master server
    info!("Connecting to master server...");
    let client = match MooseNGClient::connect(config.clone()).await {
        Ok(client) => {
            info!("Successfully connected to master server");
            Arc::new(client)
        }
        Err(e) => {
            error!("Failed to connect to master server: {}", e);
            return Err(e.into());
        }
    };

    // Create FUSE filesystem
    let fs = MooseNGFS::new(client, config.clone());

    // Prepare mount options
    let mut options = vec![
        fuser::MountOption::FSName("mooseng".to_string()),
        fuser::MountOption::Subtype("mooseng".to_string()),
    ];

    if config.read_only {
        options.push(fuser::MountOption::RO);
    }

    if config.allow_other {
        options.push(fuser::MountOption::AllowOther);
    }

    if config.allow_root {
        options.push(fuser::MountOption::AllowRoot);
    }

    // Add default options
    options.push(fuser::MountOption::DefaultPermissions);
    options.push(fuser::MountOption::AutoUnmount);

    info!("Mounting filesystem at {}", config.mount_point.display());

    // Mount the filesystem
    match fuser::mount2(fs, &config.mount_point, &options) {
        Ok(()) => {
            info!("Filesystem mounted successfully");
        }
        Err(e) => {
            error!("Failed to mount filesystem: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = ClientConfig::default();
        assert_eq!(config.master_addr.to_string(), "127.0.0.1:9421");
        assert_eq!(config.mount_point, PathBuf::from("/mnt/moosefs"));
        assert!(!config.read_only);
        assert!(!config.allow_other);
        assert!(!config.allow_root);
    }
}