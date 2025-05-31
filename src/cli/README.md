# MooseNG Command Line Interface (CLI)

The MooseNG CLI is a comprehensive command-line tool for managing and interacting with MooseNG distributed file system clusters.

## Location

The CLI implementation is located in `/home/wings/projects/moosefs/mooseng/mooseng-cli/`

## Features

### 1. Cluster Management
- **Status**: View cluster health and component status
- **Initialization**: Initialize new clusters
- **Scaling**: Scale cluster components up or down
- **Topology**: Manage network topology and region discovery
- **Upgrades**: Perform rolling upgrades

### 2. Data Operations
- **Upload**: Upload files and directories to MooseNG
- **Download**: Download files and directories from MooseNG
- **Sync**: Bidirectional synchronization between local and remote
- **List**: Browse MooseNG filesystem
- **Copy/Move/Delete**: File operations within MooseNG
- **Info**: Get detailed file information and chunk distribution

### 3. Administrative Operations
- **Chunk Server Management**: Add, remove, and manage chunk servers
- **Storage Classes**: Create and manage storage classes
- **Quotas**: Set and manage disk quotas
- **Health Checks**: Manual health checks and self-healing
- **Repair**: Chunk repair and consistency operations
- **Balancing**: Data balancing across chunk servers

### 4. Monitoring & Metrics
- **Real-time Metrics**: Live cluster performance monitoring
- **Health Status**: Component health with detailed diagnostics
- **Operations Monitoring**: Track active operations
- **Event Logs**: View system events and alerts
- **Statistics**: Historical performance statistics

### 5. Configuration Management
- **Show/Set/Get**: Configuration management
- **Validation**: Configuration validation
- **Import/Export**: Configuration backup and restore
- **Client Configuration**: CLI client settings management

## Architecture

### Core Components

1. **gRPC Client** (`grpc_client.rs`)
   - Connection management with failover
   - Master server discovery and load balancing
   - Retry logic and error handling
   - Configuration persistence

2. **Command Modules**
   - `cluster.rs` - Cluster management operations
   - `admin.rs` - Administrative operations
   - `data.rs` - Data upload/download/management
   - `monitoring.rs` - Monitoring and metrics
   - `config.rs` - Configuration management

3. **Health Integration**
   - Direct integration with MooseNG health checking system
   - Real-time component health monitoring
   - Self-healing action triggers

## Usage Examples

### Cluster Operations
```bash
# View cluster status
mooseng cluster status --verbose

# Initialize new cluster
mooseng cluster init --masters "master1:9421,master2:9421" --name "production"

# Scale chunk servers
mooseng cluster scale chunkserver --count 10
```

### Data Operations
```bash
# Upload directory with compression
mooseng data upload /local/data /remote/backup --recursive --compress

# Download with parallel streams
mooseng data download /remote/files /local/download --parallel 8

# Sync directories
mooseng data sync /local/dir /remote/dir --direction bidirectional
```

### Administration
```bash
# Add chunk server
mooseng admin add-chunkserver --host chunk1.example.com --capacity 1000

# Create storage class
mooseng config storage-class create archive --copies 1 --erasure "8+4"

# Set quota
mooseng admin quota set /project1 --size 100GB --inodes 1000000
```

### Monitoring
```bash
# Real-time metrics
mooseng monitor metrics --component cluster --interval 5

# Health check
mooseng monitor health --detailed

# Show operations
mooseng monitor operations --slow --threshold 1000
```

## Configuration

The CLI uses a hierarchical configuration system:

1. Command-line arguments (highest priority)
2. Environment variables
3. Configuration files:
   - `~/.mooseng/cli.toml`
   - `/etc/mooseng/cli.toml`
   - `./mooseng-cli.toml`

### Configuration Example
```toml
master_addresses = ["master1:9421", "master2:9421", "master3:9421"]
connect_timeout_secs = 10
request_timeout_secs = 30
enable_tls = true
tls_cert_path = "/etc/ssl/mooseng/client.pem"
retry_attempts = 3
retry_delay_ms = 1000
```

## Implementation Status

### ✅ Completed
- Complete CLI framework with clap
- gRPC client with failover and retry
- All major command categories implemented
- Health system integration
- Configuration management
- Comprehensive help and documentation

### 🔄 In Progress
- Data upload/download implementation
- gRPC service integration
- Error handling enhancements

### 📋 Pending
- Integration testing with live cluster
- Performance optimization
- Advanced scripting capabilities
- Bash completion

## Dependencies

The CLI depends on the following MooseNG components:
- `mooseng-common` - Common utilities and health framework
- `mooseng-protocol` - gRPC protocol definitions
- `mooseng-master` - Master server health integration
- `mooseng-chunkserver` - Chunk server health integration
- `mooseng-client` - Client health integration
- `mooseng-metalogger` - Metalogger health integration

## Build Instructions

```bash
cd /home/wings/projects/moosefs/mooseng
cargo build --bin mooseng
```

The compiled binary will be available as `target/debug/mooseng` or `target/release/mooseng`.

## Testing

Run tests with:
```bash
cargo test -p mooseng-cli
```

## Future Enhancements

1. **Scripting Support**: Enable script execution for automation
2. **Interactive Mode**: Interactive shell mode for extended sessions
3. **Plugin System**: Support for custom command plugins
4. **Advanced Filtering**: Enhanced filtering and search capabilities
5. **Visualization**: ASCII charts and graphs for metrics
6. **Configuration Templates**: Predefined configuration templates