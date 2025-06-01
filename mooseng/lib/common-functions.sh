#!/bin/bash
# Common functions for MooseNG demo scripts

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Logging functions
info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

success() {
    echo -e "${GREEN}✅ $1${NC}"
}

warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

error() {
    echo -e "${RED}❌ $1${NC}" >&2
}

debug() {
    if [ "${DEBUG:-false}" = "true" ]; then
        echo -e "${PURPLE}🔍 $1${NC}" >&2
    fi
}

# Print formatted banner
print_banner() {
    local title="$1"
    local width=${#title}
    local line=$(printf '%*s' "$width" | tr ' ' '=')
    
    echo ""
    echo -e "${CYAN}$title${NC}"
    echo -e "${CYAN}$line${NC}"
}

# Check if command exists
command_exists() {
    command -v "$1" &> /dev/null
}

# Wait for URL with timeout and retries
wait_for_url() {
    local url=$1
    local timeout=${2:-30}
    local retry_delay=${3:-1}
    local elapsed=0
    
    while [ $elapsed -lt $timeout ]; do
        if curl -s -f "$url" > /dev/null 2>&1; then
            return 0
        fi
        sleep $retry_delay
        ((elapsed += retry_delay))
    done
    
    return 1
}

# Wait for service with detailed status
wait_for_service() {
    local name=$1
    local url=$2
    local timeout=${3:-30}
    local attempts=0
    local max_attempts=$((timeout / 3))
    
    while [ $attempts -lt $max_attempts ]; do
        if curl -s -f "$url" > /dev/null 2>&1; then
            success "$name is healthy"
            return 0
        fi
        
        attempts=$((attempts + 1))
        if [ $attempts -lt $max_attempts ]; then
            echo -e "  ⏳ Waiting for $name (attempt $attempts/$max_attempts)..."
            sleep 3
        fi
    done
    
    error "$name failed to become healthy after $attempts attempts"
    return 1
}

# Get container health status
get_container_health() {
    local container=$1
    docker inspect --format='{{.State.Health.Status}}' "$container" 2>/dev/null || echo "unknown"
}

# Check if container is running
is_container_running() {
    local container=$1
    docker ps --format "table {{.Names}}" | grep -q "^${container}$"
}

# Get container logs tail
get_container_logs() {
    local container=$1
    local lines=${2:-20}
    docker logs --tail $lines "$container" 2>&1
}

# Check port availability
check_port() {
    local port=$1
    if command_exists lsof; then
        ! lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1
    elif command_exists netstat; then
        ! netstat -tuln | grep -q ":$port "
    else
        # Fallback: try to bind to the port
        (echo "" | nc -l 127.0.0.1 $port 2>/dev/null) &
        local pid=$!
        sleep 0.1
        if kill -0 $pid 2>/dev/null; then
            kill $pid 2>/dev/null
            wait $pid 2>/dev/null
            return 0
        else
            return 1
        fi
    fi
}

# Check multiple ports for conflicts
check_port_conflicts() {
    local ports=("$@")
    local conflicts=()
    
    for port in "${ports[@]}"; do
        if ! check_port $port; then
            conflicts+=($port)
        fi
    done
    
    if [ ${#conflicts[@]} -gt 0 ]; then
        error "Port conflicts detected: ${conflicts[*]}"
        echo "Please stop the services using these ports or change the port mappings."
        return 1
    fi
    
    return 0
}

# Print service endpoints
print_service_endpoints() {
    echo ""
    echo "🌐 Service Endpoints:"
    echo "==================="
    echo "Masters:"
    echo "  - Master 1: http://localhost:${MASTER1_CLIENT_PORT:-9421} (Raft: ${MASTER1_RAFT_PORT:-9422}, Metrics: ${MASTER1_METRICS_PORT:-9423})"
    echo "  - Master 2: http://localhost:${MASTER2_CLIENT_PORT:-9431} (Raft: ${MASTER2_RAFT_PORT:-9432}, Metrics: ${MASTER2_METRICS_PORT:-9433})"
    echo "  - Master 3: http://localhost:${MASTER3_CLIENT_PORT:-9441} (Raft: ${MASTER3_RAFT_PORT:-9442}, Metrics: ${MASTER3_METRICS_PORT:-9443})"
    echo ""
    echo "Chunkservers:"
    echo "  - Chunkserver 1: http://localhost:${CHUNKSERVER1_BIND_PORT:-9420} (Metrics: ${CHUNKSERVER1_METRICS_PORT:-9425})"
    echo "  - Chunkserver 2: http://localhost:${CHUNKSERVER2_BIND_PORT:-9450} (Metrics: ${CHUNKSERVER2_METRICS_PORT:-9455})"
    echo "  - Chunkserver 3: http://localhost:${CHUNKSERVER3_BIND_PORT:-9460} (Metrics: ${CHUNKSERVER3_METRICS_PORT:-9465})"
    echo ""
    echo "Clients:"
    echo "  - Client 1: Metrics at http://localhost:${CLIENT1_METRICS_PORT:-9427}"
    echo "  - Client 2: Metrics at http://localhost:${CLIENT2_METRICS_PORT:-9437}"
    echo "  - Client 3: Metrics at http://localhost:${CLIENT3_METRICS_PORT:-9447}"
    echo ""
    echo "Monitoring:"
    echo "  - Prometheus: http://localhost:${PROMETHEUS_PORT:-9090}"
    echo "  - Grafana: http://localhost:${GRAFANA_PORT:-3000} (admin/${GRAFANA_ADMIN_PASSWORD:-admin})"
    echo "  - Dashboard: http://localhost:${DASHBOARD_PORT:-8080}"
}

# Print service status in a formatted table
print_service_status() {
    echo ""
    echo "📊 Service Status:"
    echo "=================="
    printf "%-20s %-15s %-10s\n" "SERVICE" "STATUS" "HEALTH"
    printf "%-20s %-15s %-10s\n" "-------" "------" "------"
    
    for service in master-1 master-2 master-3 chunkserver-1 chunkserver-2 chunkserver-3 client-1 client-2 client-3 prometheus grafana dashboard; do
        container="mooseng-$service"
        if is_container_running "$container"; then
            status="${GREEN}Running${NC}"
            health=$(get_container_health "$container")
            case $health in
                healthy)
                    health="${GREEN}Healthy${NC}"
                    ;;
                unhealthy)
                    health="${RED}Unhealthy${NC}"
                    ;;
                starting)
                    health="${YELLOW}Starting${NC}"
                    ;;
                *)
                    health="${BLUE}N/A${NC}"
                    ;;
            esac
        else
            status="${RED}Stopped${NC}"
            health="${RED}N/A${NC}"
        fi
        printf "%-20s %-25b %-20b\n" "$service" "$status" "$health"
    done
}

# Test summary formatter
print_test_summary() {
    local test_results=("$@")
    local duration=${test_results[-1]}
    unset 'test_results[-1]'
    
    echo ""
    echo "📋 Test Summary:"
    echo "==============="
    
    local passed=0
    local failed=0
    
    for result in "${test_results[@]}"; do
        echo "  $result"
        if [[ $result == *"✅"* ]]; then
            ((passed++))
        else
            ((failed++))
        fi
    done
    
    echo ""
    echo "Total: $((passed + failed)) tests"
    echo "Passed: $passed"
    echo "Failed: $failed"
    echo "Duration: ${duration}s"
    
    if [ $failed -eq 0 ]; then
        success "All tests passed!"
        return 0
    else
        error "Some tests failed!"
        return 1
    fi
}

# Validate Docker and Docker Compose installation
validate_docker_installation() {
    info "Validating Docker installation..."
    
    if ! command_exists docker; then
        error "Docker is not installed. Please install Docker first."
        echo "Visit: https://docs.docker.com/get-docker/"
        return 1
    fi
    
    if ! docker info &> /dev/null; then
        error "Docker daemon is not running. Please start Docker."
        return 1
    fi
    
    # Check Docker Compose (v2)
    if ! docker compose version &> /dev/null; then
        warning "Docker Compose v2 is not available. Checking for v1..."
        if command_exists docker-compose; then
            warning "Found Docker Compose v1. Consider upgrading to v2."
            # Set a flag to use docker-compose instead of docker compose
            export DOCKER_COMPOSE_CMD="docker-compose"
        else
            error "Docker Compose is not installed. Please install Docker Compose."
            echo "Visit: https://docs.docker.com/compose/install/"
            return 1
        fi
    else
        export DOCKER_COMPOSE_CMD="docker compose"
    fi
    
    success "Docker and Docker Compose are properly installed"
    return 0
}

# Get Docker Compose command
get_compose_cmd() {
    echo "${DOCKER_COMPOSE_CMD:-docker compose}"
}

# Load environment file if exists
load_env_file() {
    local env_file="${1:-.env}"
    if [ -f "$env_file" ]; then
        debug "Loading environment from $env_file"
        set -a
        source "$env_file"
        set +a
    fi
}

# Create required directories
create_directories() {
    local dirs=("$@")
    for dir in "${dirs[@]}"; do
        if [ ! -d "$dir" ]; then
            debug "Creating directory: $dir"
            mkdir -p "$dir"
        fi
    done
}

# Backup volumes
backup_volumes() {
    local backup_dir="$1"
    local compose_file="${2:-docker-compose.yml}"
    
    info "Backing up volumes to $backup_dir..."
    mkdir -p "$backup_dir"
    
    # Get volume list from compose file
    local volumes=$($(get_compose_cmd) -f "$compose_file" config --volumes 2>/dev/null)
    
    if [ -z "$volumes" ]; then
        warning "No volumes found to backup"
        return 0
    fi
    
    local backed_up=0
    for volume in $volumes; do
        local full_volume_name="${COMPOSE_PROJECT_NAME:-mooseng}_${volume}"
        if docker volume inspect "$full_volume_name" &>/dev/null; then
            info "Backing up volume: $volume"
            if docker run --rm \
                -v "${full_volume_name}:/source:ro" \
                -v "$(realpath "$backup_dir"):/backup" \
                alpine tar -czf "/backup/${volume}.tar.gz" -C /source . 2>/dev/null; then
                ((backed_up++))
            else
                warning "Failed to backup volume: $volume"
            fi
        else
            debug "Volume not found: $full_volume_name"
        fi
    done
    
    if [ $backed_up -gt 0 ]; then
        success "Backed up $backed_up volumes to $backup_dir"
    else
        warning "No volumes were backed up"
    fi
}

# Restore volumes from backup
restore_volumes() {
    local backup_dir="$1"
    local compose_file="${2:-docker-compose.yml}"
    
    if [ ! -d "$backup_dir" ]; then
        error "Backup directory not found: $backup_dir"
        return 1
    fi
    
    info "Restoring volumes from $backup_dir..."
    
    local restored=0
    for backup_file in "$backup_dir"/*.tar.gz; do
        if [ -f "$backup_file" ]; then
            local volume_name=$(basename "$backup_file" .tar.gz)
            local full_volume_name="${COMPOSE_PROJECT_NAME:-mooseng}_${volume_name}"
            
            info "Restoring volume: $volume_name"
            
            # Create volume if it doesn't exist
            docker volume create "$full_volume_name" &>/dev/null
            
            if docker run --rm \
                -v "${full_volume_name}:/target" \
                -v "$(realpath "$backup_dir"):/backup:ro" \
                alpine tar -xzf "/backup/$(basename "$backup_file")" -C /target; then
                ((restored++))
            else
                warning "Failed to restore volume: $volume_name"
            fi
        fi
    done
    
    if [ $restored -gt 0 ]; then
        success "Restored $restored volumes from $backup_dir"
    else
        warning "No volumes were restored"
    fi
}

# Export all functions
export -f info success warning error debug
export -f print_banner command_exists wait_for_url wait_for_service
export -f get_container_health is_container_running get_container_logs
export -f check_port check_port_conflicts
export -f print_service_endpoints print_service_status print_test_summary
export -f validate_docker_installation get_compose_cmd
export -f load_env_file create_directories
export -f backup_volumes restore_volumes