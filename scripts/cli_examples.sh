#!/bin/bash

# MooseNG CLI Examples
# This script demonstrates common CLI usage patterns

set -e

MOOSENG_CLI="mooseng"
MASTER_ADDR="localhost:9421"

echo "=== MooseNG CLI Examples ==="
echo

# Configuration
echo "1. CLI Configuration"
echo "===================="

# Show current configuration
echo "Show current CLI configuration:"
echo "$MOOSENG_CLI config client show"
echo

# Set master addresses
echo "Set master server addresses:"
echo "$MOOSENG_CLI config client set-masters 'master1:9421,master2:9421,master3:9421'"
echo

# Enable TLS
echo "Enable TLS with certificate:"
echo "$MOOSENG_CLI config client set-tls --enable --cert-path /etc/ssl/mooseng/client.pem"
echo

# Test connectivity
echo "Test connectivity to configured masters:"
echo "$MOOSENG_CLI config client test"
echo

# Cluster Management
echo "2. Cluster Management"
echo "===================="

# Basic cluster status
echo "View basic cluster status:"
echo "$MOOSENG_CLI cluster status"
echo

# Detailed cluster status
echo "View detailed cluster status:"
echo "$MOOSENG_CLI cluster status --verbose"
echo

# Initialize new cluster (example - don't run on existing cluster)
echo "Initialize new cluster (CAUTION - only for new deployments):"
echo "$MOOSENG_CLI cluster init --masters 'master1:9421,master2:9421' --name production --force"
echo

# Scale operations
echo "Scale chunk servers to 10 instances:"
echo "$MOOSENG_CLI cluster scale chunkserver --count 10"
echo

# Rolling upgrade
echo "Perform rolling upgrade:"
echo "$MOOSENG_CLI cluster upgrade --component chunkserver --version 1.1.0"
echo

# Topology management
echo "Show network topology:"
echo "$MOOSENG_CLI cluster topology show --verbose"
echo

echo "Discover topology:"
echo "$MOOSENG_CLI cluster topology discover --force"
echo

echo "List regions:"
echo "$MOOSENG_CLI cluster topology regions --health"
echo

echo "Test connectivity between regions:"
echo "$MOOSENG_CLI cluster topology test --from 1 --to 2 --iterations 10"
echo

# Data Operations
echo "3. Data Operations"
echo "=================="

# Upload examples
echo "Upload single file:"
echo "$MOOSENG_CLI data upload /local/file.txt /remote/file.txt"
echo

echo "Upload directory recursively with compression:"
echo "$MOOSENG_CLI data upload /local/data /remote/backup --recursive --compress --storage-class archive"
echo

echo "Upload with parallel streams:"
echo "$MOOSENG_CLI data upload /local/bigdata /remote/bigdata --recursive --parallel 8"
echo

echo "Resume interrupted upload:"
echo "$MOOSENG_CLI data upload /local/data /remote/data --recursive --resume"
echo

# Download examples
echo "Download file:"
echo "$MOOSENG_CLI data download /remote/file.txt /local/file.txt"
echo

echo "Download directory with parallel streams:"
echo "$MOOSENG_CLI data download /remote/backup /local/restore --recursive --parallel 4"
echo

echo "Resume interrupted download:"
echo "$MOOSENG_CLI data download /remote/bigfile.dat /local/bigfile.dat --resume"
echo

# Sync examples
echo "Upload sync (local to remote):"
echo "$MOOSENG_CLI data sync /local/project /remote/project --direction up --delete"
echo

echo "Download sync (remote to local):"
echo "$MOOSENG_CLI data sync /local/backup /remote/backup --direction down"
echo

echo "Bidirectional sync:"
echo "$MOOSENG_CLI data sync /local/shared /remote/shared --direction bidirectional"
echo

echo "Dry run sync (show what would be transferred):"
echo "$MOOSENG_CLI data sync /local/data /remote/data --dry-run"
echo

# File management
echo "List files in directory:"
echo "$MOOSENG_CLI data list /remote/path"
echo

echo "List files with detailed information:"
echo "$MOOSENG_CLI data list /remote/path --long --human-readable"
echo

echo "List files recursively:"
echo "$MOOSENG_CLI data list /remote/path --recursive --all"
echo

echo "Copy files within MooseNG:"
echo "$MOOSENG_CLI data copy /remote/source /remote/destination --recursive"
echo

echo "Move files within MooseNG:"
echo "$MOOSENG_CLI data move /remote/old_name /remote/new_name"
echo

echo "Delete files (with confirmation):"
echo "$MOOSENG_CLI data delete /remote/unwanted_file"
echo

echo "Force delete directory recursively:"
echo "$MOOSENG_CLI data delete /remote/old_directory --recursive --force"
echo

echo "Create directory:"
echo "$MOOSENG_CLI data mkdir /remote/new_directory --parents"
echo

echo "Get file information:"
echo "$MOOSENG_CLI data info /remote/file.dat --storage --chunks"
echo

echo "Set file attributes:"
echo "$MOOSENG_CLI data set-attr /remote/archive --storage-class archive --replication 1 --recursive"
echo

# Administrative Operations
echo "4. Administrative Operations"
echo "============================"

# Chunk server management
echo "List chunk servers:"
echo "$MOOSENG_CLI admin list-chunkservers --verbose"
echo

echo "Add chunk server:"
echo "$MOOSENG_CLI admin add-chunkserver --host chunk5.example.com --port 9420 --capacity 2000"
echo

echo "Remove chunk server (graceful):"
echo "$MOOSENG_CLI admin remove-chunkserver --id cs-003"
echo

echo "Force remove chunk server:"
echo "$MOOSENG_CLI admin remove-chunkserver --id cs-003 --force"
echo

echo "Set maintenance mode:"
echo "$MOOSENG_CLI admin maintenance --id cs-002 --enable"
echo

echo "Disable maintenance mode:"
echo "$MOOSENG_CLI admin maintenance --id cs-002 --disable"
echo

# Data balancing and repair
echo "Balance cluster (dry run):"
echo "$MOOSENG_CLI admin balance --dry-run"
echo

echo "Balance cluster with limits:"
echo "$MOOSENG_CLI admin balance --max-move 100"
echo

echo "Repair all chunks:"
echo "$MOOSENG_CLI admin repair"
echo

echo "Repair specific chunk:"
echo "$MOOSENG_CLI admin repair --chunk-id chunk_12345 --force"
echo

# Storage classes
echo "Set storage class for path:"
echo "$MOOSENG_CLI admin storage-class /project/archive archive --recursive"
echo

# Quota management
echo "Set quota for path:"
echo "$MOOSENG_CLI admin quota set /project1 --size 100GB --inodes 1000000"
echo

echo "Get quota information:"
echo "$MOOSENG_CLI admin quota get /project1"
echo

echo "List all quotas:"
echo "$MOOSENG_CLI admin quota list"
echo

echo "List exceeded quotas only:"
echo "$MOOSENG_CLI admin quota list --exceeded"
echo

echo "Remove quota:"
echo "$MOOSENG_CLI admin quota remove /project1"
echo

# Health management
echo "Manual health check for master:"
echo "$MOOSENG_CLI admin health check master"
echo

echo "Manual health check for specific chunk server:"
echo "$MOOSENG_CLI admin health check chunkserver --id cs-001"
echo

echo "Trigger self-healing (restart component):"
echo "$MOOSENG_CLI admin health heal chunkserver --id cs-002 --action restart"
echo

echo "Trigger cache clear:"
echo "$MOOSENG_CLI admin health heal master --action clear_cache"
echo

echo "Show healing history:"
echo "$MOOSENG_CLI admin health history --component chunkserver --count 10"
echo

echo "Configure auto-healing:"
echo "$MOOSENG_CLI admin health auto-heal chunkserver --enable"
echo

echo "Configure health check settings:"
echo "$MOOSENG_CLI admin health configure master --interval 30 --timeout 10 --failure-threshold 3"
echo

# Monitoring and Metrics
echo "5. Monitoring and Metrics"
echo "========================="

# Real-time metrics
echo "Show real-time cluster metrics (5 iterations, 10 second intervals):"
echo "$MOOSENG_CLI monitor metrics --component cluster --interval 10 --count 5"
echo

echo "Monitor specific chunk server:"
echo "$MOOSENG_CLI monitor metrics --component chunkserver --id cs-001 --interval 5"
echo

# Statistics
echo "Show performance statistics for last 24 hours:"
echo "$MOOSENG_CLI monitor stats --range 24h --verbose"
echo

echo "Show summary statistics:"
echo "$MOOSENG_CLI monitor stats --range 1h"
echo

# Health monitoring
echo "Show overall cluster health:"
echo "$MOOSENG_CLI monitor health"
echo

echo "Show detailed health information:"
echo "$MOOSENG_CLI monitor health --detailed"
echo

# Operations monitoring
echo "Show all active operations:"
echo "$MOOSENG_CLI monitor operations"
echo

echo "Show only slow operations (>1 second):"
echo "$MOOSENG_CLI monitor operations --slow --threshold 1000"
echo

echo "Show only write operations:"
echo "$MOOSENG_CLI monitor operations --operation write"
echo

# Event logs
echo "Show recent events:"
echo "$MOOSENG_CLI monitor events --count 20"
echo

echo "Show error events only:"
echo "$MOOSENG_CLI monitor events --level error --count 10"
echo

echo "Show events for specific component:"
echo "$MOOSENG_CLI monitor events --component chunkserver --count 15"
echo

# Export metrics
echo "Export metrics to JSON:"
echo "$MOOSENG_CLI monitor export --format json --output metrics.json --range 24h"
echo

echo "Export metrics to CSV:"
echo "$MOOSENG_CLI monitor export --format csv --output metrics.csv --range 1h"
echo

# Configuration Management
echo "6. Configuration Management"
echo "==========================="

# Show configuration
echo "Show all component configurations:"
echo "$MOOSENG_CLI config show"
echo

echo "Show master configuration only:"
echo "$MOOSENG_CLI config show --component master"
echo

echo "Show specific setting:"
echo "$MOOSENG_CLI config show --component master --setting listen_port"
echo

# Set configuration
echo "Set configuration value:"
echo "$MOOSENG_CLI config set --component master --setting cache_size_mb 2048"
echo

echo "Set configuration with immediate effect:"
echo "$MOOSENG_CLI config set --component chunkserver --setting max_connections 2000 --immediate"
echo

# Get configuration
echo "Get specific configuration value:"
echo "$MOOSENG_CLI config get --component master --setting workers"
echo

# Validate configuration
echo "Validate all configurations:"
echo "$MOOSENG_CLI config validate"
echo

echo "Validate specific component:"
echo "$MOOSENG_CLI config validate --component chunkserver"
echo

echo "Validate configuration file:"
echo "$MOOSENG_CLI config validate --file /etc/mooseng/master.toml"
echo

# Export/import configuration
echo "Export configuration to YAML:"
echo "$MOOSENG_CLI config export --output cluster_config.yaml --format yaml"
echo

echo "Export specific component configuration:"
echo "$MOOSENG_CLI config export --output master_config.json --format json --component master"
echo

echo "Import configuration (dry run):"
echo "$MOOSENG_CLI config import --input cluster_config.yaml --dry-run"
echo

echo "Import configuration:"
echo "$MOOSENG_CLI config import --input cluster_config.yaml"
echo

# Reset configuration
echo "Reset component configuration (with confirmation):"
echo "$MOOSENG_CLI config reset --component chunkserver"
echo

echo "Force reset configuration:"
echo "$MOOSENG_CLI config reset --component chunkserver --force"
echo

echo "Reset specific setting:"
echo "$MOOSENG_CLI config reset --component master --setting cache_size_mb --force"
echo

# Storage class management
echo "List storage classes:"
echo "$MOOSENG_CLI config storage-class list --verbose"
echo

echo "Create storage class:"
echo "$MOOSENG_CLI config storage-class create archive --copies 1 --erasure '8+4' --tier cold"
echo

echo "Modify storage class:"
echo "$MOOSENG_CLI config storage-class modify archive --copies 2"
echo

echo "Delete storage class:"
echo "$MOOSENG_CLI config storage-class delete old_class --force"
echo

echo
echo "=== End of Examples ==="
echo
echo "Note: These are example commands. Some operations may require"
echo "appropriate permissions or may not be applicable to your"
echo "specific cluster configuration."
echo
# Performance Benchmarking
echo "7. Performance Benchmarking"
echo "==========================="

# Quick benchmarks
echo "Run quick benchmark suite:"
echo "$MOOSENG_CLI benchmark quick --output ./results --iterations 10"
echo

echo "Run quick benchmarks with network tests:"
echo "$MOOSENG_CLI benchmark quick --output ./results --iterations 5 --network"
echo

# Comprehensive benchmarks
echo "Run comprehensive benchmark suite:"
echo "$MOOSENG_CLI benchmark full --output ./results --real-network --infrastructure"
echo

echo "Run benchmarks with custom configuration:"
echo "$MOOSENG_CLI benchmark full --output ./results --config /etc/mooseng/benchmark.toml"
echo

# Specific benchmark types
echo "Run file operation benchmarks:"
echo "$MOOSENG_CLI benchmark file --sizes '1024,65536,1048576,10485760' --output ./results --iterations 50"
echo

echo "Run metadata operation benchmarks:"
echo "$MOOSENG_CLI benchmark metadata --operations 1000 --output ./results"
echo

echo "Run network latency benchmarks:"
echo "$MOOSENG_CLI benchmark network --targets 'master1:9421,master2:9421' --output ./results --duration 60"
echo

# Benchmark management
echo "List available benchmark types:"
echo "$MOOSENG_CLI benchmark list"
echo

echo "Show benchmark status and history:"
echo "$MOOSENG_CLI benchmark status --detailed --limit 20"
echo

# Results and reporting
echo "Generate HTML report from benchmark results:"
echo "$MOOSENG_CLI benchmark report --input ./results --format html --output report.html --charts"
echo

echo "Generate CSV export of benchmark data:"
echo "$MOOSENG_CLI benchmark report --input ./results --format csv --output data.csv"
echo

echo "Compare two benchmark result sets:"
echo "$MOOSENG_CLI benchmark compare --baseline ./results/baseline.json --current ./results/current.json --format table"
echo

# Dashboard and monitoring
echo "Start live benchmark dashboard:"
echo "$MOOSENG_CLI benchmark dashboard --port 8080 --database ./benchmark.db --open-browser"
echo

echo "Query benchmark results from database:"
echo "$MOOSENG_CLI benchmark query --operation 'file_create' --limit 50 --format table"
echo

echo "Query results by date range:"
echo "$MOOSENG_CLI benchmark query --start-date '2024-01-01T00:00:00Z' --end-date '2024-12-31T23:59:59Z' --format json"
echo

echo "Analyze performance trends:"
echo "$MOOSENG_CLI benchmark trends --operation 'metadata_lookup' --days 30 --confidence 0.8"
echo

echo "Analyze all trends for last 60 days:"
echo "$MOOSENG_CLI benchmark trends --days 60 --confidence 0.7 --format json"
echo

# Continuous integration
echo "Run continuous benchmarks for CI:"
echo "$MOOSENG_CLI benchmark continuous --iterations 5 --interval 30 --threshold 5.0 --output ./ci-results"
echo

echo "Run continuous benchmarks with regression detection:"
echo "$MOOSENG_CLI benchmark continuous --iterations 10 --interval 15 --threshold 2.0 --database ./ci-benchmark.db"
echo

echo
echo "=== End of Examples ==="
echo
echo "Note: These are example commands. Some operations may require"
echo "appropriate permissions or may not be applicable to your"
echo "specific cluster configuration."
echo
echo "Use 'mooseng --help' or 'mooseng <command> --help' for detailed"
echo "information about any command."