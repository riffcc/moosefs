#!/bin/bash
# MooseNG Docker Demo Startup Script - Enhanced Version

set -euo pipefail
trap cleanup EXIT

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Source shared functions
source "${SCRIPT_DIR}/lib/common-functions.sh"

# Configuration
COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
ENV_FILE="${ENV_FILE:-.env}"
RETRY_ATTEMPTS=5
RETRY_DELAY=3
PARALLEL_BUILD=${PARALLEL_BUILD:-true}
FORCE_REBUILD=${FORCE_REBUILD:-false}

# Script variables
CLEANUP_ON_ERROR=true
SERVICES_STARTED=false

# Show usage
show_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Start the MooseNG Docker demo environment with 3 masters, 3 chunkservers, and 3 clients.

Options:
    -h, --help              Show this help message
    -f, --file FILE         Use alternative docker-compose file (default: docker-compose.yml)
    -e, --env FILE          Use alternative environment file (default: .env)
    --no-build              Skip building images
    --force-rebuild         Force rebuild of all images
    --no-cleanup            Don't cleanup on error
    --debug                 Enable debug output

Examples:
    $0                                    # Start with defaults
    $0 -f docker-compose.improved.yml     # Use improved compose file
    $0 --force-rebuild                    # Force rebuild all images

EOF
}

# Parse command line arguments
parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            -h|--help)
                show_usage
                exit 0
                ;;
            -f|--file)
                COMPOSE_FILE="$2"
                shift 2
                ;;
            -e|--env)
                ENV_FILE="$2"
                shift 2
                ;;
            --no-build)
                SKIP_BUILD=true
                shift
                ;;
            --force-rebuild)
                FORCE_REBUILD=true
                shift
                ;;
            --no-cleanup)
                CLEANUP_ON_ERROR=false
                shift
                ;;
            --debug)
                DEBUG=true
                shift
                ;;
            *)
                error "Unknown option: $1"
                show_usage
                exit 1
                ;;
        esac
    done
}

# Cleanup function
cleanup() {
    local exit_code=$?
    
    if [ $exit_code -ne 0 ] && [ "$CLEANUP_ON_ERROR" = true ] && [ "$SERVICES_STARTED" = true ]; then
        warning "Script failed. Cleaning up..."
        $(get_compose_cmd) -f "$COMPOSE_FILE" down 2>/dev/null || true
    fi
    
    if [ $exit_code -ne 0 ]; then
        error "Startup failed. Check the logs for details."
        echo "To see service logs: $(get_compose_cmd) -f $COMPOSE_FILE logs"
    fi
}

# Validate compose file
validate_compose_file() {
    if [ ! -f "$COMPOSE_FILE" ]; then
        error "Docker Compose file not found: $COMPOSE_FILE"
        exit 1
    fi
    
    debug "Validating compose file: $COMPOSE_FILE"
    if ! $(get_compose_cmd) -f "$COMPOSE_FILE" config > /dev/null 2>&1; then
        error "Invalid Docker Compose file: $COMPOSE_FILE"
        $(get_compose_cmd) -f "$COMPOSE_FILE" config
        exit 1
    fi
}

# Check for required ports
check_required_ports() {
    info "Checking for port conflicts..."
    
    # Define required ports based on environment or defaults
    local master_ports=(
        ${MASTER1_CLIENT_PORT:-9421}
        ${MASTER1_RAFT_PORT:-9422}
        ${MASTER1_METRICS_PORT:-9423}
        ${MASTER2_CLIENT_PORT:-9431}
        ${MASTER2_RAFT_PORT:-9432}
        ${MASTER2_METRICS_PORT:-9433}
        ${MASTER3_CLIENT_PORT:-9441}
        ${MASTER3_RAFT_PORT:-9442}
        ${MASTER3_METRICS_PORT:-9443}
    )
    
    local chunkserver_ports=(
        ${CHUNKSERVER1_BIND_PORT:-9420}
        ${CHUNKSERVER1_METRICS_PORT:-9425}
        ${CHUNKSERVER2_BIND_PORT:-9450}
        ${CHUNKSERVER2_METRICS_PORT:-9455}
        ${CHUNKSERVER3_BIND_PORT:-9460}
        ${CHUNKSERVER3_METRICS_PORT:-9465}
    )
    
    local client_ports=(
        ${CLIENT1_METRICS_PORT:-9427}
        ${CLIENT2_METRICS_PORT:-9437}
        ${CLIENT3_METRICS_PORT:-9447}
    )
    
    local monitoring_ports=(
        ${PROMETHEUS_PORT:-9090}
        ${GRAFANA_PORT:-3000}
        ${DASHBOARD_PORT:-8080}
    )
    
    local all_ports=("${master_ports[@]}" "${chunkserver_ports[@]}" "${client_ports[@]}" "${monitoring_ports[@]}")
    
    if ! check_port_conflicts "${all_ports[@]}"; then
        exit 1
    fi
    
    success "All required ports are available"
}

# Build Docker images
build_images() {
    if [ "${SKIP_BUILD:-false}" = true ]; then
        info "Skipping image build (--no-build specified)"
        return 0
    fi
    
    info "Building Docker images..."
    
    local build_args=""
    if [ "$FORCE_REBUILD" = true ]; then
        build_args="--no-cache"
    fi
    
    if [ "$PARALLEL_BUILD" = true ]; then
        build_args="$build_args --parallel"
    fi
    
    if ! $(get_compose_cmd) -f "$COMPOSE_FILE" build $build_args --progress=plain; then
        error "Failed to build Docker images"
        exit 1
    fi
    
    success "Docker images built successfully"
}

# Start services
start_services() {
    info "Starting services..."
    
    SERVICES_STARTED=true
    
    if ! $(get_compose_cmd) -f "$COMPOSE_FILE" up -d; then
        error "Failed to start services"
        exit 1
    fi
    
    success "Services started"
}

# Wait for masters to be healthy
wait_for_masters() {
    info "Waiting for Master servers to be healthy..."
    
    local masters_healthy=0
    local master_ports=(
        ${MASTER1_CLIENT_PORT:-9421}
        ${MASTER2_CLIENT_PORT:-9431}
        ${MASTER3_CLIENT_PORT:-9441}
    )
    
    for i in 0 1 2; do
        local port=${master_ports[$i]}
        local master_num=$((i + 1))
        
        if wait_for_service "Master $master_num" "http://localhost:$port/health" 30; then
            ((masters_healthy++))
        fi
    done
    
    if [ $masters_healthy -lt 3 ]; then
        warning "Only $masters_healthy/3 masters are healthy"
        return 1
    fi
    
    success "All masters are healthy"
    return 0
}

# Wait for chunkservers to be healthy
wait_for_chunkservers() {
    info "Waiting for Chunk servers to be healthy..."
    
    local chunks_healthy=0
    local chunk_ports=(
        ${CHUNKSERVER1_BIND_PORT:-9420}
        ${CHUNKSERVER2_BIND_PORT:-9450}
        ${CHUNKSERVER3_BIND_PORT:-9460}
    )
    
    for i in 0 1 2; do
        local port=${chunk_ports[$i]}
        local chunk_num=$((i + 1))
        
        if wait_for_service "Chunkserver $chunk_num" "http://localhost:$port/health" 30; then
            ((chunks_healthy++))
        fi
    done
    
    if [ $chunks_healthy -lt 3 ]; then
        warning "Only $chunks_healthy/3 chunkservers are healthy"
        return 1
    fi
    
    success "All chunkservers are healthy"
    return 0
}

# Check Raft leader election
check_raft_leader() {
    info "Checking Raft leader election..."
    
    local leader_found=false
    local master_ports=(
        ${MASTER1_CLIENT_PORT:-9421}
        ${MASTER2_CLIENT_PORT:-9431}
        ${MASTER3_CLIENT_PORT:-9441}
    )
    
    # Give some time for leader election
    sleep 5
    
    for i in 0 1 2; do
        local port=${master_ports[$i]}
        local master_num=$((i + 1))
        
        local status=$(curl -s "http://localhost:$port/status" 2>/dev/null || echo "{}")
        if echo "$status" | grep -q '"role":"leader"'; then
            success "Master $master_num is the Raft leader"
            leader_found=true
            break
        fi
    done
    
    if [ "$leader_found" = false ]; then
        warning "No Raft leader elected yet (this may be normal during startup)"
    fi
}

# Check monitoring stack
check_monitoring() {
    info "Checking monitoring stack..."
    
    if wait_for_url "http://localhost:${PROMETHEUS_PORT:-9090}/-/healthy" 20; then
        success "Prometheus is healthy"
    else
        warning "Prometheus is not responding"
    fi
    
    if wait_for_url "http://localhost:${GRAFANA_PORT:-3000}/api/health" 20; then
        success "Grafana is healthy"
    else
        warning "Grafana is not responding"
    fi
    
    if wait_for_url "http://localhost:${DASHBOARD_PORT:-8080}" 10; then
        success "Dashboard is accessible"
    else
        warning "Dashboard is not responding"
    fi
}

# Main execution
main() {
    # Parse arguments
    parse_args "$@"
    
    # Load environment file
    load_env_file "$ENV_FILE"
    
    # Print banner
    print_banner "🚀 MooseNG Docker Demo - Starting Environment"
    
    # Validate prerequisites
    if ! validate_docker_installation; then
        exit 1
    fi
    
    # Validate compose file
    validate_compose_file
    
    # Create necessary directories
    info "Creating required directories..."
    create_directories mnt/client-{1,2,3}
    
    # Check for port conflicts
    check_required_ports
    
    # Stop any existing containers
    info "Stopping any existing containers..."
    $(get_compose_cmd) -f "$COMPOSE_FILE" down --remove-orphans 2>/dev/null || true
    
    # Build images
    build_images
    
    # Start services
    start_services
    
    # Wait for services to be healthy
    sleep 5  # Initial delay
    
    # Check service health
    local all_healthy=true
    
    if ! wait_for_masters; then
        all_healthy=false
    fi
    
    if ! wait_for_chunkservers; then
        all_healthy=false
    fi
    
    # Check additional components
    check_raft_leader
    check_monitoring
    
    # Display service status
    print_service_status
    
    # Display endpoints
    print_service_endpoints
    
    # Summary
    echo ""
    if [ "$all_healthy" = true ]; then
        print_banner "✅ MooseNG Demo is running successfully!"
    else
        print_banner "⚠️  MooseNG Demo started with warnings"
        echo "Some services may not be fully healthy. Check the logs for details."
    fi
    
    # Display useful commands
    echo ""
    info "Useful commands:"
    echo "  - View logs: $(get_compose_cmd) -f $COMPOSE_FILE logs -f [service-name]"
    echo "  - Stop demo: ./stop-demo.sh"
    echo "  - Run tests: ./test-demo.sh"
    echo "  - Check status: $(get_compose_cmd) -f $COMPOSE_FILE ps"
}

# Run main function
main "$@"