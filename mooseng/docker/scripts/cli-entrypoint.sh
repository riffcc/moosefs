#!/bin/sh
set -e

# Function to log messages
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] CLI: $1"
}

log "Starting MooseNG CLI entrypoint..."

# Check if config template exists and process it
CONFIG_TEMPLATE="/etc/mooseng/cli.toml.template"
CONFIG_FILE="/etc/mooseng/cli.toml"

if [ -f "$CONFIG_TEMPLATE" ]; then
    log "Processing configuration template..."
    envsubst < "$CONFIG_TEMPLATE" > "$CONFIG_FILE"
else
    log "Warning: No configuration template found at $CONFIG_TEMPLATE"
    if [ ! -f "$CONFIG_FILE" ]; then
        log "Error: No configuration file found at $CONFIG_FILE"
        exit 1
    fi
fi

# Validate configuration
log "Validating configuration..."
if [ -z "${MOOSENG_MASTER_ENDPOINTS:-}" ]; then
    log "Warning: MOOSENG_MASTER_ENDPOINTS not set, using default"
fi

# Set up CLI environment
export MOOSENG_CONFIG_PATH="$CONFIG_FILE"

# Show welcome message if running interactively
if [ -t 0 ] && [ "$#" -eq 0 ]; then
    cat << 'EOF'

╔══════════════════════════════════════════════════════════════════════════════╗
║                              MooseNG CLI                                     ║
║                         Distributed File System                             ║
╚══════════════════════════════════════════════════════════════════════════════╝

Available commands:
  mooseng cluster status         - Show cluster status
  mooseng monitor health         - Monitor cluster health
  mooseng admin user list        - List users
  mooseng admin node list        - List nodes
  
Type 'mooseng --help' for full command reference.
Type 'exit' to quit.

EOF
fi

# If no arguments provided, start interactive shell
if [ "$#" -eq 0 ]; then
    log "Starting interactive CLI session..."
    exec /bin/sh
else
    # Execute the provided command
    log "Executing command: $*"
    exec "$@"
fi