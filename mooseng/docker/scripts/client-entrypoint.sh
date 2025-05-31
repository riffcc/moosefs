#!/bin/sh
set -e

# Function to log messages
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] CLIENT: $1"
}

log "Starting MooseNG Client entrypoint..."

# Check if config template exists and process it
CONFIG_TEMPLATE="/etc/mooseng/client.toml.template"
CONFIG_FILE="/etc/mooseng/client.toml"

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

# Ensure mount point and log directories exist
MOUNT_POINT="${MOOSENG_MOUNT_POINT:-/mnt/mooseng}"
mkdir -p "$MOUNT_POINT"
mkdir -p "${MOOSENG_LOG_DIR:-/var/log/mooseng}"

# Validate configuration
log "Validating configuration..."
if [ -z "${MOOSENG_MASTER_ENDPOINTS:-}" ]; then
    log "Warning: MOOSENG_MASTER_ENDPOINTS not set, using default"
fi

# Check if FUSE is available
if [ ! -c /dev/fuse ]; then
    log "Error: /dev/fuse device not found. Make sure to run with --device /dev/fuse:/dev/fuse"
    exit 1
fi

# Wait for master servers to be available
if [ "${MOOSENG_WAIT_FOR_MASTERS:-true}" = "true" ]; then
    log "Waiting for master servers to be available..."
    
    # Extract first master endpoint for connectivity check
    FIRST_MASTER=$(echo "${MOOSENG_MASTER_ENDPOINTS:-master-1:9421}" | cut -d',' -f1)
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

# Cleanup function for unmounting
cleanup() {
    log "Received shutdown signal, unmounting filesystem..."
    if mountpoint -q "$MOUNT_POINT"; then
        umount "$MOUNT_POINT" 2>/dev/null || {
            log "Normal unmount failed, forcing unmount..."
            umount -f "$MOUNT_POINT" 2>/dev/null || true
        }
        log "Filesystem unmounted"
    fi
    exit 0
}

# Set up signal handlers
trap cleanup SIGTERM SIGINT

# Check if already mounted
if mountpoint -q "$MOUNT_POINT"; then
    log "Warning: $MOUNT_POINT is already mounted, unmounting first..."
    umount "$MOUNT_POINT" || {
        log "Failed to unmount $MOUNT_POINT, forcing unmount..."
        umount -f "$MOUNT_POINT" || true
    }
fi

# Start the FUSE client
log "Starting MooseNG FUSE client..."
log "Configuration file: $CONFIG_FILE"
log "Mount point: $MOUNT_POINT"
log "Log directory: ${MOOSENG_LOG_DIR:-/var/log/mooseng}"
log "Master endpoints: ${MOOSENG_MASTER_ENDPOINTS:-master-1:9421,master-2:9421,master-3:9421}"
log "Cache size: ${MOOSENG_CACHE_SIZE_MB:-512}MB"

# Run the client in foreground mode
exec /usr/local/bin/mooseng-mount \
    --config "$CONFIG_FILE" \
    --mount-point "$MOUNT_POINT" \
    --foreground \
    "$@"