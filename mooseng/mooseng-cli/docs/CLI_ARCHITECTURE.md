# MooseNG CLI Architecture

## Overview

The MooseNG CLI (`mooseng`) is a comprehensive command-line interface for managing and monitoring MooseNG distributed file system clusters. It provides a structured, modular design with four main command categories: cluster management, administrative operations, monitoring, and configuration.

## Project Structure

```
mooseng-cli/
├── src/
│   ├── main.rs           # Entry point and command routing
│   ├── grpc_client.rs    # gRPC client for communication with MooseNG services
│   ├── admin.rs          # Administrative commands
│   ├── cluster.rs        # Cluster management commands
│   ├── monitoring.rs     # Monitoring and status commands
│   └── config.rs         # Configuration management commands
├── docs/
│   ├── CLI_ARCHITECTURE.md  # This file
│   └── USER_GUIDE.md        # User guide and examples
└── Cargo.toml           # Dependencies and build configuration
```

## Architecture Components

### 1. Main Entry Point (`main.rs`)

The main module serves as the CLI entry point and command router:

- **Command Parsing**: Uses `clap` for declarative CLI parsing
- **Command Routing**: Dispatches commands to appropriate handlers
- **Error Handling**: Provides consistent error reporting
- **Async Runtime**: Manages the tokio async runtime for gRPC operations

```rust
#[derive(Parser)]
#[command(name = "mooseng")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Cluster { action: cluster::ClusterCommands },
    Admin { action: admin::AdminCommands },
    Monitor { action: monitoring::MonitorCommands },
    Config { action: config::ConfigCommands },
}
```

### 2. gRPC Client (`grpc_client.rs`)

The gRPC client module provides robust communication with MooseNG services:

**Key Features:**
- **Automatic Failover**: Switches between multiple master servers
- **Connection Pooling**: Maintains persistent connections
- **Retry Logic**: Configurable retry with exponential backoff
- **TLS Support**: Optional TLS encryption for secure communication
- **Configuration Management**: File and environment-based configuration

**Core Types:**
```rust
pub struct MooseNGClient {
    config: GrpcClientConfig,
    master_clients: Vec<MasterServiceClient<Channel>>,
    current_master_index: usize,
}

pub struct GrpcClientConfig {
    pub master_addresses: Vec<String>,
    pub connect_timeout_secs: u64,
    pub request_timeout_secs: u64,
    pub enable_tls: bool,
    // ... additional configuration
}
```

**Connection Management:**
- Maintains connections to multiple master servers
- Automatic failover on connection failures
- Health checking and reconnection logic
- Load balancing across available masters

### 3. Command Modules

#### Cluster Management (`cluster.rs`)

Handles cluster-wide operations and topology management:

**Commands:**
- `status` - Display cluster health and statistics
- `init` - Initialize a new cluster
- `join` - Join nodes to existing cluster
- `leave` - Remove nodes from cluster
- `scale` - Scale cluster components up/down
- `upgrade` - Perform rolling upgrades
- `topology` - Network topology discovery and management

**Key Features:**
- Real-time cluster status retrieval via gRPC
- Fallback to placeholder data if services unavailable
- Detailed and summary display modes
- Multi-region topology visualization

#### Administrative Operations (`admin.rs`)

Provides administrative functions for cluster maintenance:

**Commands:**
- `servers` - Manage chunk servers, masters, metaloggers
- `storage-classes` - Create and manage storage classes
- `quotas` - Set and manage user/group quotas
- `repair` - Data repair and consistency operations
- `maintenance` - Maintenance mode operations
- `cleanup` - Cleanup orphaned data and metadata

**Key Features:**
- Server lifecycle management
- Storage class configuration with erasure coding
- Quota enforcement and reporting
- Data integrity operations

#### Monitoring (`monitoring.rs`)

Real-time monitoring and alerting capabilities:

**Commands:**
- `status` - Real-time cluster status
- `metrics` - Prometheus metrics export
- `health` - Health check operations
- `events` - Event log monitoring
- `performance` - Performance statistics
- `alerts` - Alert configuration and status

**Key Features:**
- Live metrics streaming
- Configurable alerting rules
- Performance profiling
- Event log aggregation

#### Configuration Management (`config.rs`)

Configuration management for CLI and cluster settings:

**Commands:**
- `show` - Display current configuration
- `set` - Set configuration values
- `get` - Get specific configuration values
- `list` - List all available settings
- `validate` - Validate configuration syntax
- `export`/`import` - Configuration backup/restore

**Key Features:**
- Hierarchical configuration management
- Environment variable integration
- Configuration validation
- Template-based configuration generation

## Data Flow Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────────┐
│   CLI Command   │───▶│   gRPC Client    │───▶│   Master Server     │
│                 │    │                  │    │                     │
│ - Parse args    │    │ - Load config    │    │ - Process request   │
│ - Validate      │    │ - Connect        │    │ - Return response   │
│ - Route         │    │ - Retry/failover │    │ - Update state      │
└─────────────────┘    └──────────────────┘    └─────────────────────┘
         │                       │                         │
         │                       ▼                         │
         │              ┌──────────────────┐               │
         │              │ Connection Pool  │               │
         │              │                  │               │
         │              │ - Master 1       │               │
         │              │ - Master 2       │               │
         │              │ - Master 3       │               │
         │              └──────────────────┘               │
         │                                                 │
         ▼                                                 ▼
┌─────────────────┐                               ┌─────────────────────┐
│   Output        │                               │   Other Services    │
│                 │                               │                     │
│ - Format data   │                               │ - Chunk servers     │
│ - Display       │                               │ - Metaloggers       │
│ - Error handle  │                               │ - Region peers      │
└─────────────────┘                               └─────────────────────┘
```

## Configuration System

### Configuration Hierarchy

1. **Command Line Arguments** (highest priority)
2. **Environment Variables**
3. **Configuration Files**
4. **Default Values** (lowest priority)

### Configuration File Locations

The CLI searches for configuration files in the following order:
1. `$MOOSENG_CLI_CONFIG` (environment variable)
2. `~/.mooseng/cli.toml`
3. `/etc/mooseng/cli.toml`
4. `./mooseng-cli.toml`

### Sample Configuration

```toml
[client]
master_addresses = ["grpc://master1:9421", "grpc://master2:9421"]
connect_timeout_secs = 10
request_timeout_secs = 30
enable_tls = true
retry_attempts = 3

[display]
default_format = "table"
verbose = false
color = true

[logging]
level = "info"
file = "~/.mooseng/cli.log"
```

## Error Handling Strategy

### Error Types

1. **Network Errors**: Connection failures, timeouts
2. **Authentication Errors**: Invalid credentials, permissions
3. **Validation Errors**: Invalid command arguments
4. **Service Errors**: Backend service failures

### Error Recovery

- **Automatic Retry**: For transient network errors
- **Failover**: Switch to alternative master servers
- **Graceful Degradation**: Show cached/placeholder data when possible
- **User Feedback**: Clear error messages with suggested actions

### Example Error Handling

```rust
match client.get_cluster_status().await {
    Ok(response) => display_status(response),
    Err(e) => {
        eprintln!("Failed to get cluster status: {}", e);
        match e.downcast_ref::<tonic::Status>() {
            Some(status) if status.code() == Code::Unavailable => {
                println!("Service unavailable, showing cached data...");
                display_cached_status()
            }
            _ => return Err(e),
        }
    }
}
```

## Extension Points

### Adding New Commands

1. **Define Command Structure**: Add to appropriate `*Commands` enum
2. **Implement Handler**: Create async handler function
3. **Add gRPC Calls**: Extend `MooseNGClient` if needed
4. **Add Tests**: Unit and integration tests
5. **Update Documentation**: User guide and help text

### Custom Output Formats

```rust
pub trait OutputFormatter {
    fn format_cluster_status(&self, status: &ClusterStatus) -> String;
    fn format_server_list(&self, servers: &[Server]) -> String;
}

// Implementations: TableFormatter, JsonFormatter, YamlFormatter
```

### Plugin System (Future)

```rust
pub trait CliPlugin {
    fn commands(&self) -> Vec<Command>;
    fn handle_command(&self, cmd: &str, args: &[String]) -> Result<()>;
}
```

## Performance Considerations

### Connection Pooling

- Maintain persistent gRPC connections
- Connection health monitoring
- Automatic reconnection on failures
- Load balancing across masters

### Caching Strategy

- Cache frequently accessed data (cluster status, server lists)
- Configurable cache TTL
- Cache invalidation on updates
- Fallback to cache on service unavailability

### Async Operations

- Non-blocking I/O for all network operations
- Concurrent requests where possible
- Streaming for large data sets
- Progress reporting for long operations

## Security Considerations

### Authentication

- Service account tokens
- User credential management
- Role-based access control
- Session management

### Transport Security

- TLS encryption for all communications
- Certificate validation
- Mutual TLS (mTLS) support
- Secure credential storage

### Authorization

- Command-level permissions
- Resource-level access control
- Audit logging
- Privilege escalation protection

## Testing Strategy

### Unit Tests

- Command parsing and validation
- gRPC client functionality
- Output formatting
- Error handling

### Integration Tests

- End-to-end command execution
- gRPC service integration
- Configuration loading
- Failover scenarios

### Performance Tests

- Connection pooling efficiency
- Large data set handling
- Concurrent operation limits
- Memory usage profiling

## Monitoring and Observability

### Metrics

- Command execution times
- gRPC request latencies
- Error rates by command
- Connection pool statistics

### Logging

- Structured logging with tracing
- Configurable log levels
- Request/response logging
- Performance metrics

### Debugging

- Verbose mode for troubleshooting
- Request tracing
- Connection state inspection
- Configuration dumping

## Future Enhancements

### Planned Features

1. **Interactive Mode**: Shell-like interface for multiple commands
2. **Autocompletion**: Bash/zsh completion scripts
3. **Configuration Wizard**: Guided setup for new deployments
4. **Plugin System**: Third-party command extensions
5. **Web Dashboard Integration**: Launch web UI from CLI
6. **Backup/Restore**: Cluster state backup and restoration
7. **Performance Profiling**: Built-in profiling tools
8. **Multi-Cluster Support**: Manage multiple clusters simultaneously

### Roadmap

- **v1.1**: Interactive mode and autocompletion
- **v1.2**: Plugin system and configuration wizard
- **v1.3**: Advanced monitoring and alerting
- **v2.0**: Multi-cluster management and web integration