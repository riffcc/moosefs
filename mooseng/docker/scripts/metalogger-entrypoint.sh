#!/bin/sh
set -e

# Function to log messages
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] METALOGGER: $1"
}

log "Starting MooseNG Metalogger entrypoint..."

# Check if config template exists and process it
CONFIG_TEMPLATE="/etc/mooseng/metalogger.toml.template"
CONFIG_FILE="/etc/mooseng/metalogger.toml"

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

# Ensure data and log directories exist and are writable
mkdir -p "${MOOSENG_DATA_DIR:-/var/lib/mooseng/metalogger}"
mkdir -p "${MOOSENG_LOG_DIR:-/var/log/mooseng}"

# Validate configuration
log "Validating configuration..."
if [ -z "${MOOSENG_MASTER_ENDPOINTS:-}" ]; then
    log "Warning: MOOSENG_MASTER_ENDPOINTS not set, using default"
fi

# Wait for master servers to be available
if [ "${MOOSENG_WAIT_FOR_MASTERS:-true}" = "true" ]; then
    log "Waiting for master servers to be available..."
    
    # Extract first master endpoint for connectivity check
    FIRST_MASTER=$(echo "${MOOSENG_MASTER_ENDPOINTS:-master-1:9422}" | cut -d',' -f1)
    MASTER_HOST=$(echo "$FIRST_MASTER" | cut -d':' -f1)
    MASTER_PORT=$(echo "$FIRST_MASTER" | cut -d':' -f2)
    
    # Wait up to 60 seconds for master to be available
    TIMEOUT=60
    ELAPSED=0
    
    while [ $ELAPSED -lt $TIMEOUT ]; do
        if nc -z "$MASTER_HOST" "$MASTER_PORT" 2>/dev/null; then
            log "Master server $FIRST_MASTER is available"
            break
        fi
        log "Waiting for master server $FIRST_MASTER... ($ELAPSED/$TIMEOUT seconds)"
        sleep 2
        ELAPSED=$((ELAPSED + 2))
    done
    
    if [ $ELAPSED -ge $TIMEOUT ]; then
        log "Warning: Timeout waiting for master server, starting anyway..."
    fi
fi

# Handle graceful shutdown
shutdown_handler() {
    log "Received shutdown signal, stopping metalogger..."
    if [ -n "$METALOGGER_PID" ]; then
        kill -TERM "$METALOGGER_PID" 2>/dev/null || true
        wait "$METALOGGER_PID" 2>/dev/null || true
    fi
    log "Metalogger stopped gracefully"
    exit 0
}

# Set up signal handlers
trap shutdown_handler SIGTERM SIGINT

# Start the metalogger
log "Starting MooseNG Metalogger..."
log "Configuration file: $CONFIG_FILE"
log "Data directory: ${MOOSENG_DATA_DIR:-/var/lib/mooseng/metalogger}"
log "Log directory: ${MOOSENG_LOG_DIR:-/var/log/mooseng}"
log "Master endpoints: ${MOOSENG_MASTER_ENDPOINTS:-master-1:9422,master-2:9422,master-3:9422}"

exec /usr/local/bin/mooseng-metalogger \
    --config "$CONFIG_FILE" \
    "$@" &

METALOGGER_PID=$!
log "Metalogger started with PID $METALOGGER_PID"

# Wait for the process
wait $METALOGGER_PID