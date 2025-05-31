#!/bin/sh
set -e

# Function to substitute environment variables in config template
substitute_config() {
    envsubst < /etc/mooseng/chunkserver.toml.template > /etc/mooseng/chunkserver.toml
}

# Function to wait for master server
wait_for_master() {
    if [ -n "$MOOSENG_MASTER_ADDRS" ]; then
        echo "Waiting for master servers: $MOOSENG_MASTER_ADDRS"
        for master in $(echo "$MOOSENG_MASTER_ADDRS" | tr ',' ' '); do
            host=$(echo "$master" | cut -d':' -f1)
            port=$(echo "$master" | cut -d':' -f2)
            
            echo "Waiting for master $host:$port..."
            while ! nc -z "$host" "$port" 2>/dev/null; do
                sleep 1
            done
            echo "Master $host:$port is available"
        done
    fi
}

# Function to initialize data directories
init_data_dirs() {
    if [ -n "$MOOSENG_DATA_DIRS" ]; then
        echo "Initializing data directories: $MOOSENG_DATA_DIRS"
        for data_dir in $(echo "$MOOSENG_DATA_DIRS" | tr ',' ' '); do
            echo "Initializing data directory: $data_dir"
            mkdir -p "$data_dir"
            
            # Check if directory is writable
            if [ ! -w "$data_dir" ]; then
                echo "Error: Data directory $data_dir is not writable"
                exit 1
            fi
            
            # Check available space
            available_space=$(df "$data_dir" | awk 'NR==2 {print $4}')
            echo "Available space in $data_dir: ${available_space}KB"
            
            # Warn if less than 1GB available
            if [ "$available_space" -lt 1048576 ]; then
                echo "Warning: Low disk space in $data_dir (less than 1GB available)"
            fi
        done
    fi
}

# Function to setup logging
setup_logging() {
    mkdir -p "$MOOSENG_LOG_DIR"
    
    # Ensure log directory is writable
    if [ ! -w "$MOOSENG_LOG_DIR" ]; then
        echo "Warning: Log directory $MOOSENG_LOG_DIR is not writable"
    fi
}

# Function to validate configuration
validate_config() {
    if [ ! -f "/etc/mooseng/chunkserver.toml" ]; then
        echo "Error: Configuration file not found at /etc/mooseng/chunkserver.toml"
        exit 1
    fi
    
    # Basic validation
    if ! grep -q "listen_port" /etc/mooseng/chunkserver.toml; then
        echo "Error: listen_port not configured"
        exit 1
    fi
    
    if ! grep -q "master_addrs" /etc/mooseng/chunkserver.toml; then
        echo "Error: master_addrs not configured"
        exit 1
    fi
    
    if ! grep -q "data_dirs" /etc/mooseng/chunkserver.toml; then
        echo "Error: data_dirs not configured"
        exit 1
    fi
}

# Function to generate server ID if not provided
generate_server_id() {
    if [ -z "$MOOSENG_SERVER_ID" ]; then
        # Generate ID based on hostname and current time
        hostname=$(hostname)
        timestamp=$(date +%s)
        export MOOSENG_SERVER_ID="cs-${hostname}-${timestamp}"
        echo "Generated server ID: $MOOSENG_SERVER_ID"
    fi
}

# Function to check disk health
check_disk_health() {
    if [ -n "$MOOSENG_DATA_DIRS" ]; then
        for data_dir in $(echo "$MOOSENG_DATA_DIRS" | tr ',' ' '); do
            # Check if directory exists and is accessible
            if [ ! -d "$data_dir" ] || [ ! -r "$data_dir" ] || [ ! -w "$data_dir" ]; then
                echo "Error: Data directory $data_dir is not accessible"
                exit 1
            fi
            
            # Check filesystem type (optional warning)
            fs_type=$(df -T "$data_dir" | awk 'NR==2 {print $2}')
            echo "Data directory $data_dir filesystem: $fs_type"
        done
    fi
}

# Main execution
main() {
    echo "Starting MooseNG Chunk Server..."
    echo "Server ID: ${MOOSENG_SERVER_ID:-auto-generated}"
    echo "Master servers: ${MOOSENG_MASTER_ADDRS:-master:9421}"
    
    # Setup
    generate_server_id
    substitute_config
    setup_logging
    validate_config
    check_disk_health
    init_data_dirs
    wait_for_master
    
    # Handle special commands
    case "${1:-}" in
        --health-check)
            # Simple health check
            exec /usr/local/bin/mooseng-chunkserver --config /etc/mooseng/chunkserver.toml --health-check
            ;;
        --scan-disks)
            echo "Scanning disk health..."
            check_disk_health
            init_data_dirs
            echo "Disk scan completed successfully"
            exit 0
            ;;
        *)
            # Normal startup
            echo "Configuration validated successfully"
            echo "Starting chunk server with config: /etc/mooseng/chunkserver.toml"
            exec /usr/local/bin/mooseng-chunkserver --config /etc/mooseng/chunkserver.toml
            ;;
    esac
}

# Execute main function
main "$@"