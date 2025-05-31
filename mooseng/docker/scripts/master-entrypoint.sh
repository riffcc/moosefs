#!/bin/sh
set -e

# Function to substitute environment variables in config template
substitute_config() {
    envsubst < /etc/mooseng/master.toml.template > /etc/mooseng/master.toml
}

# Function to wait for dependencies
wait_for_dependencies() {
    if [ -n "$MOOSENG_WAIT_FOR" ]; then
        echo "Waiting for dependencies: $MOOSENG_WAIT_FOR"
        for dep in $(echo "$MOOSENG_WAIT_FOR" | tr ',' ' '); do
            host=$(echo "$dep" | cut -d':' -f1)
            port=$(echo "$dep" | cut -d':' -f2)
            
            echo "Waiting for $host:$port..."
            while ! nc -z "$host" "$port" 2>/dev/null; do
                sleep 1
            done
            echo "$host:$port is available"
        done
    fi
}

# Function to initialize master data directory
init_master() {
    if [ ! -f "$MOOSENG_DATA_DIR/metadata.mfs" ]; then
        echo "Initializing master data directory..."
        mkdir -p "$MOOSENG_DATA_DIR"
        
        # Create initial metadata file if it doesn't exist
        # This would normally be done by the master binary with --init flag
        touch "$MOOSENG_DATA_DIR/metadata.mfs"
        echo "Master data directory initialized"
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
    if [ ! -f "/etc/mooseng/master.toml" ]; then
        echo "Error: Configuration file not found at /etc/mooseng/master.toml"
        exit 1
    fi
    
    # Basic validation - check if required fields are present
    if ! grep -q "listen_port" /etc/mooseng/master.toml; then
        echo "Error: listen_port not configured"
        exit 1
    fi
    
    if ! grep -q "data_dir" /etc/mooseng/master.toml; then
        echo "Error: data_dir not configured"
        exit 1
    fi
}

# Main execution
main() {
    echo "Starting MooseNG Master Server..."
    echo "Node ID: ${MOOSENG_NODE_ID:-master-1}"
    echo "Cluster ID: ${MOOSENG_CLUSTER_ID:-mooseng-cluster}"
    
    # Setup
    substitute_config
    setup_logging
    validate_config
    wait_for_dependencies
    init_master
    
    # Handle special commands
    case "${1:-}" in
        --health-check)
            # Simple health check - verify the process can start and bind to port
            exec /usr/local/bin/mooseng-master --config /etc/mooseng/master.toml --health-check
            ;;
        --init)
            echo "Initializing master server..."
            exec /usr/local/bin/mooseng-master --config /etc/mooseng/master.toml --init
            ;;
        *)
            # Normal startup
            echo "Configuration validated successfully"
            echo "Starting master server with config: /etc/mooseng/master.toml"
            exec /usr/local/bin/mooseng-master --config /etc/mooseng/master.toml
            ;;
    esac
}

# Execute main function
main "$@"