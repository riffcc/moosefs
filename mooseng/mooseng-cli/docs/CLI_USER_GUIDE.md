# MooseNG CLI User Guide

This guide provides comprehensive documentation for using the MooseNG Command Line Interface (CLI) tools to manage and monitor your MooseNG distributed file system cluster.

## Table of Contents

- [Installation](#installation)
- [Configuration](#configuration)
- [Cluster Management](#cluster-management)
- [Administrative Operations](#administrative-operations)
- [Monitoring and Status](#monitoring-and-status)
- [Data Operations](#data-operations)
- [Benchmarking](#benchmarking)
- [Configuration Management](#configuration-management)
- [Troubleshooting](#troubleshooting)
- [Examples](#examples)

## Installation

The MooseNG CLI is distributed as part of the MooseNG suite. To install:

```bash
# From source
git clone https://github.com/mooseng/mooseng.git
cd mooseng/mooseng-cli
cargo build --release

# The binary will be available at: target/release/mooseng
```

Add the binary to your PATH for convenient access:

```bash
export PATH=$PATH:/path/to/mooseng/target/release
```

## Configuration

### Initial Setup

Before using the CLI, configure connection to your MooseNG cluster:

```bash
# Set master server addresses
export MOOSENG_MASTERS="master1.example.com:9421,master2.example.com:9421"

# Or create a configuration file
mkdir -p ~/.mooseng
cat > ~/.mooseng/cli.toml << EOF
master_addresses = ["http://master1.example.com:9421", "http://master2.example.com:9421"]
connect_timeout_secs = 10
request_timeout_secs = 30
enable_tls = false
retry_attempts = 3
retry_delay_ms = 1000
EOF
```

### Configuration Options

| Option | Description | Default |
|--------|-------------|---------|
| `master_addresses` | List of master server URLs | `["http://localhost:9421"]` |
| `connect_timeout_secs` | Connection timeout in seconds | `10` |
| `request_timeout_secs` | Request timeout in seconds | `30` |
| `enable_tls` | Enable TLS encryption | `false` |
| `tls_cert_path` | Path to custom TLS certificate | `None` |
| `retry_attempts` | Number of retry attempts | `3` |
| `retry_delay_ms` | Delay between retries in milliseconds | `1000` |

## Cluster Management

### Cluster Status

View the current status of your MooseNG cluster:

```bash
# Basic cluster status
mooseng cluster status

# Detailed status with server information
mooseng cluster status --verbose
```

### Initialize a New Cluster

Set up a new MooseNG cluster:

```bash
# Initialize cluster with specific masters
mooseng cluster init \
  --masters "master1.example.com:9421,master2.example.com:9421,master3.example.com:9421" \
  --name "production-cluster"

# Force initialization (overwrites existing cluster)
mooseng cluster init \
  --masters "master1.example.com:9421" \
  --name "test-cluster" \
  --force
```

### Join a Cluster

Add a new node to an existing cluster:

```bash
# Join as a chunk server
mooseng cluster join \
  --master master1.example.com:9421 \
  --role chunkserver

# Join as a metalogger
mooseng cluster join \
  --master master1.example.com:9421 \
  --role metalogger

# Join as an additional master
mooseng cluster join \
  --master master1.example.com:9421 \
  --role master
```

### Leave a Cluster

Gracefully remove a node from the cluster:

```bash
# Graceful departure (waits for data migration)
mooseng cluster leave

# Force departure (immediate, may cause data loss)
mooseng cluster leave --force
```

### Scale Cluster Components

Adjust the number of cluster components:

```bash
# Scale chunk servers to 10 instances
mooseng cluster scale chunkserver --count 10

# Scale masters to 3 instances
mooseng cluster scale master --count 3
```

### Cluster Upgrades

Perform rolling upgrades of cluster components:

```bash
# Upgrade all components to version 1.1.0
mooseng cluster upgrade --version 1.1.0

# Upgrade only chunk servers
mooseng cluster upgrade --component chunkserver --version 1.1.0
```

## Network Topology Management

### View Network Topology

```bash
# Show current topology (text format)
mooseng cluster topology show

# Show topology in JSON format
mooseng cluster topology show --format json --verbose

# Show detailed connection information
mooseng cluster topology show --verbose
```

### Discover Network Topology

```bash
# Automatic topology discovery
mooseng cluster topology discover

# Force rediscovery of all regions
mooseng cluster topology discover --force --timeout 60
```

### Manage Regions

```bash
# List all discovered regions
mooseng cluster topology regions

# Show regions with health status
mooseng cluster topology regions --health
```

### Test Connectivity

```bash
# Test connectivity between specific regions
mooseng cluster topology test --from 1 --to 2 --iterations 10

# View connection details
mooseng cluster topology connections --from 1 --to 2

# View all connections
mooseng cluster topology connections
```

### Configure Topology Discovery

```bash
# Configure discovery methods
mooseng cluster topology configure \
  --methods "ping,traceroute,iperf" \
  --interval 300 \
  --auto-assign true
```

## Administrative Operations

### User Management

```bash
# List all users
mooseng admin users list

# Create a new user
mooseng admin users create \
  --username johndoe \
  --email john@example.com \
  --role user

# Delete a user
mooseng admin users delete --username johndoe

# Update user permissions
mooseng admin users update \
  --username johndoe \
  --role admin
```

### Storage Class Management

```bash
# List storage classes
mooseng admin storage-classes list

# Create a new storage class
mooseng admin storage-classes create \
  --name "high-durability" \
  --replication-factor 3 \
  --erasure-coding "8+4"

# Update storage class
mooseng admin storage-classes update \
  --name "high-durability" \
  --replication-factor 4

# Delete storage class
mooseng admin storage-classes delete --name "old-class"
```

### System Maintenance

```bash
# Perform system cleanup
mooseng admin cleanup --dry-run

# Apply cleanup changes
mooseng admin cleanup --execute

# Backup cluster metadata
mooseng admin backup --output /backup/cluster-metadata.tar.gz

# Restore from backup
mooseng admin restore --input /backup/cluster-metadata.tar.gz
```

## Monitoring and Status

### Real-time Monitoring

```bash
# Monitor cluster health continuously
mooseng monitor health --continuous

# Monitor specific metrics
mooseng monitor metrics --category performance --interval 30

# Monitor network connectivity
mooseng monitor network --targets all --interval 60
```

### Performance Monitoring

```bash
# Show performance summary
mooseng monitor performance

# Monitor specific operations
mooseng monitor performance --operations "read,write,metadata"

# Generate performance report
mooseng monitor performance --report --output performance-report.html
```

### Log Analysis

```bash
# View recent logs
mooseng monitor logs --tail 100

# Filter logs by level
mooseng monitor logs --level error --since "1 hour ago"

# Follow logs in real-time
mooseng monitor logs --follow --component chunkserver
```

### Alerts and Notifications

```bash
# List active alerts
mooseng monitor alerts list

# Acknowledge an alert
mooseng monitor alerts ack --id ALERT-12345

# Configure alert thresholds
mooseng monitor alerts configure \
  --metric cpu_usage \
  --threshold 80 \
  --action email
```

## Data Operations

### File Operations

```bash
# Upload files to the cluster
mooseng data upload \
  --source /local/path/file.txt \
  --destination /mooseng/path/file.txt

# Download files from the cluster
mooseng data download \
  --source /mooseng/path/file.txt \
  --destination /local/path/file.txt

# List files in a directory
mooseng data list --path /mooseng/data/ --recursive

# Delete files
mooseng data delete --path /mooseng/old-data/ --recursive
```

### Bulk Operations

```bash
# Bulk upload from directory
mooseng data upload \
  --source /local/data/ \
  --destination /mooseng/data/ \
  --recursive \
  --parallel 10

# Sync directories
mooseng data sync \
  --source /local/data/ \
  --destination /mooseng/data/ \
  --delete-extra
```

### Data Integrity

```bash
# Verify file integrity
mooseng data verify --path /mooseng/important-data/ --recursive

# Repair corrupted files
mooseng data repair --path /mooseng/corrupted-file.txt

# Check data distribution
mooseng data distribution --path /mooseng/data/
```

## Benchmarking

### Run Quick Benchmarks

```bash
# Run basic benchmark suite
mooseng benchmark quick --output ./results --iterations 100

# Include network tests
mooseng benchmark quick --network --output ./results
```

### Comprehensive Benchmarking

```bash
# Run full benchmark suite
mooseng benchmark full \
  --output ./benchmark-results \
  --config benchmark-config.toml \
  --real-network \
  --infrastructure

# Run specific benchmark types
mooseng benchmark file \
  --sizes "1024,65536,1048576" \
  --iterations 1000 \
  --output ./file-results

mooseng benchmark metadata \
  --operations 10000 \
  --output ./metadata-results

mooseng benchmark network \
  --targets "master1.example.com,master2.example.com" \
  --duration 300 \
  --output ./network-results
```

### Benchmark Analysis

```bash
# Compare benchmark results
mooseng benchmark compare \
  --baseline baseline-results.json \
  --current current-results.json \
  --format table

# Generate benchmark reports
mooseng benchmark report \
  --input benchmark-results.json \
  --format html \
  --output performance-report.html \
  --charts

# List available benchmarks
mooseng benchmark list

# Show benchmark status and history
mooseng benchmark status --detailed --limit 20
```

## Configuration Management

### View Configuration

```bash
# Show current configuration
mooseng config show

# Show specific configuration section
mooseng config show --section network

# Export configuration
mooseng config export --output cluster-config.toml
```

### Update Configuration

```bash
# Set configuration values
mooseng config set network.timeout 30
mooseng config set storage.replication_factor 3

# Import configuration from file
mooseng config import --input new-config.toml

# Reset configuration to defaults
mooseng config reset --section network
```

### Configuration Profiles

```bash
# Create configuration profile
mooseng config profile create \
  --name production \
  --from-current

# Switch to a profile
mooseng config profile use --name production

# List available profiles
mooseng config profile list

# Delete a profile
mooseng config profile delete --name old-profile
```

## Troubleshooting

### Common Issues

#### Connection Problems

```bash
# Test connectivity to masters
mooseng cluster status --verbose

# Check configuration
mooseng config show

# Verify network connectivity
ping master1.example.com
telnet master1.example.com 9421
```

#### Authentication Issues

```bash
# Check TLS configuration
mooseng config show --section tls

# Verify certificates
openssl x509 -in /path/to/cert.pem -text -noout

# Test with different settings
mooseng --config test-config.toml cluster status
```

#### Performance Issues

```bash
# Run diagnostics
mooseng monitor performance --detailed

# Check system resources
mooseng monitor health --resources

# Analyze logs for errors
mooseng monitor logs --level error --since "1 hour ago"
```

### Debug Mode

Enable verbose logging for troubleshooting:

```bash
# Enable debug logging
export RUST_LOG=debug
mooseng cluster status

# Or use the verbose flag
mooseng --verbose cluster status
```

### Getting Help

```bash
# General help
mooseng --help

# Command-specific help
mooseng cluster --help
mooseng benchmark --help

# Show version information
mooseng --version
```

## Examples

### Example 1: Setting up a 3-node cluster

```bash
# 1. Initialize cluster on first master
mooseng cluster init \
  --masters "master1.example.com:9421" \
  --name "production"

# 2. Join additional masters
mooseng --config master2-config.toml cluster join \
  --master master1.example.com:9421 \
  --role master

mooseng --config master3-config.toml cluster join \
  --master master1.example.com:9421 \
  --role master

# 3. Add chunk servers
for i in {1..6}; do
  mooseng --config chunk${i}-config.toml cluster join \
    --master master1.example.com:9421 \
    --role chunkserver
done

# 4. Add metaloggers
mooseng --config metalogger1-config.toml cluster join \
  --master master1.example.com:9421 \
  --role metalogger

# 5. Verify cluster status
mooseng cluster status --verbose
```

### Example 2: Performance monitoring and benchmarking

```bash
# 1. Run initial benchmark
mooseng benchmark full \
  --output ./baseline-results \
  --config production-benchmark.toml

# 2. Set up continuous monitoring
mooseng monitor performance --continuous &

# 3. Run weekly benchmarks
while true; do
  date=$(date +%Y%m%d)
  mooseng benchmark quick \
    --output ./weekly-results-${date} \
    --network
  sleep $((7 * 24 * 3600))  # 1 week
done
```

### Example 3: Data migration and maintenance

```bash
# 1. Upload data with verification
mooseng data upload \
  --source /important/data/ \
  --destination /mooseng/backup/ \
  --recursive \
  --verify

# 2. Create high-durability storage class
mooseng admin storage-classes create \
  --name "critical-data" \
  --replication-factor 4 \
  --erasure-coding "8+4"

# 3. Move critical files to high-durability storage
mooseng data set-storage-class \
  --path /mooseng/backup/critical/ \
  --storage-class "critical-data" \
  --recursive

# 4. Verify data integrity
mooseng data verify \
  --path /mooseng/backup/ \
  --recursive \
  --report integrity-report.txt
```

### Example 4: Cluster maintenance

```bash
# 1. Check cluster health
mooseng monitor health --detailed

# 2. Run system cleanup
mooseng admin cleanup --dry-run
mooseng admin cleanup --execute

# 3. Backup cluster metadata
mooseng admin backup \
  --output /backups/cluster-backup-$(date +%Y%m%d).tar.gz

# 4. Perform rolling upgrade
mooseng cluster upgrade \
  --version 1.2.0 \
  --component chunkserver

# 5. Verify upgrade success
mooseng cluster status --verbose
```

## Configuration File Templates

### Basic Configuration (`~/.mooseng/cli.toml`)

```toml
master_addresses = [
  "http://master1.example.com:9421",
  "http://master2.example.com:9421",
  "http://master3.example.com:9421"
]
connect_timeout_secs = 10
request_timeout_secs = 30
enable_tls = false
retry_attempts = 3
retry_delay_ms = 1000
```

### Production Configuration with TLS

```toml
master_addresses = [
  "https://master1.example.com:9421",
  "https://master2.example.com:9421",
  "https://master3.example.com:9421"
]
connect_timeout_secs = 15
request_timeout_secs = 60
enable_tls = true
tls_cert_path = "/etc/ssl/certs/mooseng-ca.pem"
retry_attempts = 5
retry_delay_ms = 2000
```

### Benchmark Configuration (`benchmark-config.toml`)

```toml
warmup_iterations = 10
measurement_iterations = 100
file_sizes = [1024, 4096, 65536, 1048576, 10485760]
concurrency_levels = [1, 10, 50, 100]
regions = ["us-east", "eu-west", "ap-south"]
detailed_report = true
```

For more information and advanced usage, consult the [MooseNG Documentation](https://docs.mooseng.io) or contact support.