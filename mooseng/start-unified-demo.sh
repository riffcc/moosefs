#!/bin/bash

# MooseNG Unified Demo Startup Script
# Starts a complete cluster with 3 Masters, 3 Chunkservers, 3 Clients + Monitoring

set -euo pipefail  # Enhanced error handling: exit on error, undefined vars, pipe failures

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Configuration
LOG_FILE="${SCRIPT_DIR}/demo-startup.log"
MAX_WAIT_TIME=300  # 5 minutes max wait
SERVICE_CHECK_INTERVAL=5
HEALTH_CHECK_TIMEOUT=30

# Ensure log file exists and is writable
mkdir -p "$(dirname "$LOG_FILE")"
touch "$LOG_FILE" 2>/dev/null || {
    echo "Warning: Cannot create log file $LOG_FILE, logging to stdout only"
    LOG_FILE="/dev/null"
}

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Enhanced logging functions with file output
log() {
    local msg="[$(date +'%Y-%m-%d %H:%M:%S')] $1"
    echo -e "${GREEN}${msg}${NC}"
    echo "${msg}" >> "$LOG_FILE" 2>/dev/null || true
}

warn() {
    local msg="[$(date +'%Y-%m-%d %H:%M:%S')] WARNING: $1"
    echo -e "${YELLOW}${msg}${NC}"
    echo "${msg}" >> "$LOG_FILE" 2>/dev/null || true
}

error() {
    local msg="[$(date +'%Y-%m-%d %H:%M:%S')] ERROR: $1"
    echo -e "${RED}${msg}${NC}" >&2
    echo "${msg}" >> "$LOG_FILE" 2>/dev/null || true
}

info() {
    local msg="[$(date +'%Y-%m-%d %H:%M:%S')] $1"
    echo -e "${BLUE}${msg}${NC}"
    echo "${msg}" >> "$LOG_FILE" 2>/dev/null || true
}

# Enhanced error handling with cleanup
trap 'handle_error $? $LINENO' ERR
trap 'cleanup_on_exit' EXIT

handle_error() {
    local exit_code=$1
    local line_number=$2
    error "Script failed with exit code $exit_code at line $line_number"
    error "Check the log file at: $LOG_FILE"
    show_troubleshooting_info
    exit $exit_code
}

cleanup_on_exit() {
    local exit_code=$?
    if [[ $exit_code -ne 0 ]] && [[ $exit_code -ne 1 ]]; then
        warn "Script exited with error. Partial cleanup may be needed."
        warn "Run '$0 clean' to clean up any remaining containers."
    fi
}

# Enhanced prerequisite checks
check_docker() {
    info "Checking Docker availability..."
    
    if ! command -v docker >/dev/null 2>&1; then
        error "Docker command not found. Please install Docker and try again."
        exit 1
    fi
    
    if ! docker info >/dev/null 2>&1; then
        error "Docker is not running or not accessible. Please start Docker and try again."
        error "You may need to run: sudo systemctl start docker (Linux) or start Docker Desktop (macOS/Windows)"
        exit 1
    fi
    
    # Check Docker version
    local docker_version=$(docker --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
    log "Docker is running (version: $docker_version)"
    
    # Check available disk space
    local available_space=$(df "$SCRIPT_DIR" | tail -1 | awk '{print $4}')
    if [[ $available_space -lt 2097152 ]]; then  # Less than 2GB
        warn "Low disk space detected. Docker build may fail if space is insufficient."
    fi
}

check_docker_compose() {
    info "Checking Docker Compose availability..."
    
    if ! command -v docker-compose >/dev/null 2>&1; then
        error "Docker Compose is not installed. Please install Docker Compose and try again."
        error "Install instructions: https://docs.docker.com/compose/install/"
        exit 1
    fi
    
    # Check Docker Compose version
    local compose_version=$(docker-compose --version | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
    log "Docker Compose is available (version: $compose_version)"
    
    # Verify docker-compose.yml exists
    if [[ ! -f "docker-compose.yml" ]]; then
        error "docker-compose.yml file not found in current directory: $SCRIPT_DIR"
        exit 1
    fi
    
    # Basic syntax check for docker-compose.yml
    if ! docker-compose config >/dev/null 2>&1; then
        error "docker-compose.yml has syntax errors. Please check the file."
        exit 1
    fi
    
    log "Docker Compose configuration is valid"
}

check_port_availability() {
    info "Checking port availability..."
    local required_ports=("9090" "9421" "9422" "9423" "9431" "9441" "9420" "9450" "9460" "3000" "8080")
    local blocked_ports=()
    
    for port in "${required_ports[@]}"; do
        if command -v lsof >/dev/null 2>&1; then
            if lsof -i ":$port" >/dev/null 2>&1; then
                blocked_ports+=("$port")
            fi
        elif command -v netstat >/dev/null 2>&1; then
            if netstat -ln | grep ":$port " >/dev/null 2>&1; then
                blocked_ports+=("$port")
            fi
        fi
    done
    
    if [[ ${#blocked_ports[@]} -gt 0 ]]; then
        warn "The following ports are already in use: ${blocked_ports[*]}"
        warn "This may cause service startup failures. Consider stopping conflicting services."
    else
        log "All required ports are available"
    fi
}

# Enhanced cleanup with better error handling and reporting
cleanup() {
    info "Cleaning up existing containers..."
    
    # Stop and remove containers
    if docker-compose ps -q | grep -q .; then
        info "Stopping existing containers..."
        if docker-compose down --volumes --remove-orphans 2>>"$LOG_FILE"; then
            log "Containers stopped and removed successfully"
        else
            warn "Some containers may not have stopped cleanly"
        fi
    else
        info "No containers to clean up"
    fi
    
    # Optional: Clean up unused Docker resources
    if [[ "${CLEAN_DOCKER_SYSTEM:-false}" == "true" ]]; then
        info "Cleaning up unused Docker resources..."
        docker system prune -f >/dev/null 2>&1 || warn "Docker system prune failed"
    fi
    
    # Clean up any leftover mount points
    cleanup_mount_points
    
    log "Cleanup completed"
}

cleanup_mount_points() {
    info "Checking for leftover mount points..."
    local mount_dirs=("mnt/client-1" "mnt/client-2" "mnt/client-3")
    
    for mount_dir in "${mount_dirs[@]}"; do
        if [[ -d "$mount_dir" ]] && mountpoint -q "$mount_dir" 2>/dev/null; then
            warn "Found mounted directory: $mount_dir"
            if command -v umount >/dev/null 2>&1; then
                umount "$mount_dir" 2>/dev/null || warn "Could not unmount $mount_dir"
            fi
        fi
    done
}

# Enhanced image building with progress tracking
build_images() {
    info "Building MooseNG Docker images..."
    
    # Check if we can build in parallel
    local build_args="--parallel"
    if ! docker-compose build --help | grep -q "parallel"; then
        warn "Docker Compose version doesn't support parallel builds"
        build_args=""
    fi
    
    # Build with progress output
    info "Starting Docker image build process..."
    if docker-compose build $build_args 2>&1 | tee -a "$LOG_FILE"; then
        log "Images built successfully"
    else
        error "Image build failed. Check the log file for details: $LOG_FILE"
        return 1
    fi
    
    # Verify images were created
    verify_images_built
}

verify_images_built() {
    info "Verifying built images..."
    local expected_images=("mooseng_master-1" "mooseng_chunkserver-1" "mooseng_client-1")
    local missing_images=()
    
    for image in "${expected_images[@]}"; do
        if ! docker images | grep -q "$image"; then
            missing_images+=("$image")
        fi
    done
    
    if [[ ${#missing_images[@]} -gt 0 ]]; then
        warn "Some expected images were not found: ${missing_images[*]}"
    else
        log "All expected images are available"
    fi
}

# Enhanced service startup with health checks and detailed progress
start_services() {
    info "Starting MooseNG cluster services in proper order..."
    
    # Start masters first (they need to form Raft cluster)
    start_masters
    
    # Start chunkservers
    start_chunkservers
    
    # Start clients
    start_clients
    
    # Start monitoring stack
    start_monitoring_stack
    
    log "All services started successfully"
}

start_masters() {
    info "Starting Master servers..."
    
    local masters=("master-1" "master-2" "master-3")
    for master in "${masters[@]}"; do
        log "Starting $master..."
        if docker-compose up -d "$master" 2>>"$LOG_FILE"; then
            log "$master started"
        else
            error "Failed to start $master"
            return 1
        fi
    done
    
    # Wait for masters to initialize and form Raft cluster
    info "Waiting for masters to initialize and form Raft cluster..."
    wait_for_service_group "masters" "${masters[@]}"
    
    # Additional wait for Raft leader election
    info "Allowing time for Raft leader election..."
    sleep 10
    
    verify_raft_cluster
}

start_chunkservers() {
    info "Starting Chunk servers..."
    
    local chunkservers=("chunkserver-1" "chunkserver-2" "chunkserver-3")
    for chunkserver in "${chunkservers[@]}"; do
        log "Starting $chunkserver..."
        if docker-compose up -d "$chunkserver" 2>>"$LOG_FILE"; then
            log "$chunkserver started"
        else
            error "Failed to start $chunkserver"
            return 1
        fi
    done
    
    # Wait for chunkservers to register with masters
    info "Waiting for chunkservers to register with masters..."
    wait_for_service_group "chunkservers" "${chunkservers[@]}"
}

start_clients() {
    info "Starting Client instances..."
    
    local clients=("client-1" "client-2" "client-3")
    for client in "${clients[@]}"; do
        log "Starting $client..."
        if docker-compose up -d "$client" 2>>"$LOG_FILE"; then
            log "$client started"
        else
            error "Failed to start $client"
            return 1
        fi
    done
    
    # Wait for clients to connect to cluster
    info "Waiting for clients to connect to cluster..."
    wait_for_service_group "clients" "${clients[@]}"
}

start_monitoring_stack() {
    info "Starting monitoring stack..."
    
    local monitoring_services=("prometheus" "grafana" "dashboard")
    for service in "${monitoring_services[@]}"; do
        log "Starting $service..."
        if docker-compose up -d "$service" 2>>"$LOG_FILE"; then
            log "$service started"
        else
            warn "Failed to start $service (monitoring is optional)"
        fi
    done
    
    # Wait for monitoring services to be ready
    info "Waiting for monitoring services to be ready..."
    wait_for_service_group "monitoring" "${monitoring_services[@]}"
}

verify_raft_cluster() {
    info "Verifying Raft cluster formation..."
    
    local attempts=0
    local max_attempts=12  # 1 minute with 5-second intervals
    
    while [[ $attempts -lt $max_attempts ]]; do
        # Check if any master reports being a leader
        for port in 9421 9431 9441; do
            if curl -s "http://localhost:$port/health" 2>/dev/null | grep -q "leader\|healthy"; then
                log "Raft cluster appears to be healthy"
                return 0
            fi
        done
        
        ((attempts++))
        info "Raft cluster not ready yet, attempt $attempts/$max_attempts..."
        sleep 5
    done
    
    warn "Could not verify Raft cluster health, but continuing..."
    return 0
}

# Enhanced status display with detailed information
show_status() {
    info "=== MooseNG Cluster Status ==="
    
    # Basic service status
    echo ""
    info "Service Status:"
    if docker-compose ps 2>/dev/null; then
        echo ""
    else
        warn "Could not retrieve service status"
        return 1
    fi
    
    # Detailed health and port information
    echo ""
    info "Detailed Service Information:"
    show_detailed_service_info
    
    # Resource usage summary
    echo ""
    show_resource_usage
    
    # Connectivity test
    echo ""
    test_service_connectivity
}

show_detailed_service_info() {
    local services=("master-1" "master-2" "master-3" "chunkserver-1" "chunkserver-2" "chunkserver-3" "client-1" "client-2" "client-3" "prometheus" "grafana" "dashboard")
    
    printf "%-15s %-12s %-20s %-30s\n" "SERVICE" "STATUS" "HEALTH" "PORTS"
    printf "%-15s %-12s %-20s %-30s\n" "-------" "------" "------" "-----"
    
    for service in "${services[@]}"; do
        local status=$(docker-compose ps "$service" 2>/dev/null | tail -n +3 | awk '{print $5}' || echo "N/A")
        local health=$(docker inspect "mooseng-$service" --format='{{.State.Health.Status}}' 2>/dev/null || echo "N/A")
        local ports=$(docker-compose ps "$service" 2>/dev/null | tail -n +3 | awk '{print $6}' || echo "N/A")
        
        # Truncate long port strings
        if [[ ${#ports} -gt 28 ]]; then
            ports="${ports:0:25}..."
        fi
        
        printf "%-15s %-12s %-20s %-30s\n" "$service" "$status" "$health" "$ports"
    done
}

show_resource_usage() {
    info "Resource Usage Summary:"
    
    # Docker system info
    local containers_running=$(docker ps -q | wc -l | tr -d ' ')
    local images_count=$(docker images -q | wc -l | tr -d ' ')
    
    echo "  • Running containers: $containers_running"
    echo "  • Total images: $images_count"
    
    # Memory usage if available
    if command -v docker >/dev/null 2>&1; then
        local memory_usage=$(docker stats --no-stream --format "table {{.Container}}\t{{.MemUsage}}" 2>/dev/null | grep mooseng | head -3)
        if [[ -n "$memory_usage" ]]; then
            echo "  • Memory usage (top 3):"
            echo "$memory_usage" | while read -r line; do
                echo "    $line"
            done
        fi
    fi
}

test_service_connectivity() {
    info "Service Connectivity Test:"
    
    local endpoints=(
        "localhost:9421;Master-1 API"
        "localhost:9431;Master-2 API"
        "localhost:9441;Master-3 API"
        "localhost:9090;Prometheus"
        "localhost:3000;Grafana"
        "localhost:8080;Dashboard"
    )
    
    for endpoint_info in "${endpoints[@]}"; do
        local endpoint=$(echo "$endpoint_info" | cut -d';' -f1)
        local name=$(echo "$endpoint_info" | cut -d';' -f2)
        
        if timeout 5 nc -z ${endpoint/:/ } 2>/dev/null; then
            echo -e "  • $name ($endpoint): ${GREEN}✓ Reachable${NC}"
        else
            echo -e "  • $name ($endpoint): ${RED}✗ Unreachable${NC}"
        fi
    done
}

# Enhanced access information with validation
show_access_info() {
    echo ""
    info "=== MooseNG Cluster Access Information ==="
    echo ""
    
    show_service_endpoints
    show_usage_commands
    show_mount_information
    show_log_locations
    show_troubleshooting_tips
}

show_service_endpoints() {
    echo -e "${GREEN}🔧 Master Servers:${NC}"
    show_endpoint_with_status "Master-1 API" "http://localhost:9421" "/health"
    show_endpoint_with_status "Master-2 API" "http://localhost:9431" "/health"
    show_endpoint_with_status "Master-3 API" "http://localhost:9441" "/health"
    echo ""
    
    echo -e "${GREEN}💾 Chunk Servers:${NC}"
    show_endpoint_with_status "ChunkServer-1" "http://localhost:9420" "/health"
    show_endpoint_with_status "ChunkServer-2" "http://localhost:9450" "/health"
    show_endpoint_with_status "ChunkServer-3" "http://localhost:9460" "/health"
    echo ""
    
    echo -e "${GREEN}📊 Monitoring:${NC}"
    show_endpoint_with_status "Prometheus" "http://localhost:9090" "/"
    show_endpoint_with_status "Grafana" "http://localhost:3000" "/api/health" "(admin/admin)"
    show_endpoint_with_status "Dashboard" "http://localhost:8080" "/"
    echo ""
}

show_endpoint_with_status() {
    local name="$1"
    local url="$2"
    local health_path="$3"
    local extra_info="${4:-}"
    
    local status_indicator=""
    if timeout 3 curl -s "${url}${health_path}" >/dev/null 2>&1; then
        status_indicator="${GREEN}✓${NC}"
    else
        status_indicator="${RED}✗${NC}"
    fi
    
    echo -e "  • $status_indicator $name: $url $extra_info"
}

show_usage_commands() {
    echo -e "${GREEN}🔍 Useful Commands:${NC}"
    echo "  • View all logs: docker-compose logs -f"
    echo "  • View service logs: docker-compose logs -f [service-name]"
    echo "  • Check status: $0 status"
    echo "  • Restart service: docker-compose restart [service-name]"
    echo "  • Stop demo: $0 stop"
    echo "  • Cleanup: $0 clean"
    echo "  • Full restart: $0 start"
    echo ""
}

show_mount_information() {
    echo -e "${GREEN}📁 Mount Points:${NC}"
    echo "  • Client containers mount /mnt/mooseng internally"
    echo "  • Data is stored in Docker volumes (persistent across restarts)"
    echo "  • To access from host: docker exec -it mooseng-client-1 bash"
    echo ""
}

show_log_locations() {
    echo -e "${GREEN}📋 Log Information:${NC}"
    echo "  • Script logs: $LOG_FILE"
    echo "  • Container logs: docker-compose logs [service-name]"
    echo "  • Follow all logs: docker-compose logs -f"
    echo ""
}

show_troubleshooting_tips() {
    echo -e "${GREEN}🔧 Troubleshooting:${NC}"
    echo "  • If services fail to start: Check port conflicts with '$0 status'"
    echo "  • If builds fail: Try 'docker system prune -f' then rebuild"
    echo "  • For permission issues: Check Docker daemon permissions"
    echo "  • View detailed logs: tail -f $LOG_FILE"
    echo ""
}

show_troubleshooting_info() {
    echo ""
    error "=== Troubleshooting Information ==="
    echo ""
    echo "1. Check the detailed log file: $LOG_FILE"
    echo "2. Verify Docker and Docker Compose are running"
    echo "3. Check for port conflicts: netstat -tulpn | grep -E ':(9090|9421|9431|9441|9420|9450|9460|3000|8080)'"
    echo "4. Clean up and retry: $0 clean && $0 start"
    echo "5. Check available disk space: df -h"
    echo "6. Review Docker logs: docker-compose logs"
    echo ""
}

# Enhanced service health monitoring with detailed status
wait_for_service_group() {
    local group_name="$1"
    shift
    local services=("$@")
    
    info "Waiting for $group_name to become healthy..."
    
    local max_attempts=$((MAX_WAIT_TIME / SERVICE_CHECK_INTERVAL))
    local attempt=0
    
    while [[ $attempt -lt $max_attempts ]]; do
        local healthy_services=()
        local unhealthy_services=()
        
        for service in "${services[@]}"; do
            local status=$(docker-compose ps "$service" 2>/dev/null | tail -n +3 | awk '{print $5}' || echo "unknown")
            case "$status" in
                *"healthy"*|*"running"*)
                    healthy_services+=("$service")
                    ;;
                *"starting"*|*"Up"*)
                    # Service is up but may not be healthy yet
                    info "$service is starting..."
                    ;;
                *)
                    unhealthy_services+=("$service")
                    ;;
            esac
        done
        
        if [[ ${#healthy_services[@]} -eq ${#services[@]} ]]; then
            log "All $group_name services are healthy: ${healthy_services[*]}"
            return 0
        fi
        
        if [[ ${#unhealthy_services[@]} -gt 0 ]]; then
            warn "Unhealthy $group_name services: ${unhealthy_services[*]}"
        fi
        
        ((attempt++))
        info "$group_name health check: ${#healthy_services[@]}/${#services[@]} healthy (attempt $attempt/$max_attempts)"
        sleep $SERVICE_CHECK_INTERVAL
    done
    
    warn "$group_name services did not become healthy within timeout"
    return 1
}

wait_for_services() {
    info "Performing comprehensive service health checks..."
    
    # Get all running services
    local all_services=()
    mapfile -t all_services < <(docker-compose ps --services --filter "status=running" 2>/dev/null || docker-compose config --services 2>/dev/null)
    
    if [[ ${#all_services[@]} -eq 0 ]]; then
        warn "No services found to check"
        return 1
    fi
    
    wait_for_service_group "all services" "${all_services[@]}"
    
    # Detailed health report
    generate_health_report
}

generate_health_report() {
    info "Generating detailed health report..."
    
    local healthy_count=0
    local unhealthy_count=0
    local starting_count=0
    
    echo ""
    info "=== Service Health Report ==="
    
    # Check each service type
    check_service_type_health "Masters" "master-1 master-2 master-3"
    check_service_type_health "Chunkservers" "chunkserver-1 chunkserver-2 chunkserver-3"
    check_service_type_health "Clients" "client-1 client-2 client-3"
    check_service_type_health "Monitoring" "prometheus grafana dashboard"
    
    echo ""
}

check_service_type_health() {
    local service_type="$1"
    local services="$2"
    
    echo -e "\n${BLUE}$service_type:${NC}"
    
    for service in $services; do
        local status=$(docker-compose ps "$service" 2>/dev/null | tail -n +3 | awk '{print $5}' || echo "not found")
        local health_status=""
        
        case "$status" in
            *"healthy"*)
                health_status="${GREEN}✓ Healthy${NC}"
                ;;
            *"starting"*|*"Up"*)
                health_status="${YELLOW}⚠ Starting${NC}"
                ;;
            *"unhealthy"*)
                health_status="${RED}✗ Unhealthy${NC}"
                ;;
            "not found")
                health_status="${RED}✗ Not Found${NC}"
                ;;
            *)
                health_status="${YELLOW}? Unknown: $status${NC}"
                ;;
        esac
        
        echo -e "  • $service: $health_status"
    done
}

# Main execution
main() {
    echo -e "${BLUE}"
    cat << "EOF"
    __  ___                      _   ________
   /  |/  /___  ____  ________  / | / / ____/
  / /|_/ / __ \/ __ \/ ___/ _ \/  |/ / / __  
 / /  / / /_/ / /_/ (__  )  __/ /|  / /_/ /  
/_/  /_/\____/\____/____/\___/_/ |_/\____/   
                                            
         Unified Docker Demo                
    3 Masters + 3 ChunkServers + 3 Clients  
EOF
    echo -e "${NC}"
    
    # Pre-flight checks
    check_docker
    check_docker_compose
    
    # Process arguments
    case "${1:-start}" in
        "clean")
            cleanup
            ;;
        "build")
            build_images
            ;;
        "start")
            log "Starting MooseNG unified demo..."
            check_port_availability
            cleanup
            build_images
            start_services
            wait_for_services
            show_status
            show_access_info
            log "Demo startup completed successfully!"
            ;;
        "status")
            show_status
            ;;
        "stop")
            info "Stopping MooseNG cluster..."
            docker-compose down
            log "Cluster stopped"
            ;;
        "logs")
            if [[ -n "${2:-}" ]]; then
                info "Showing logs for service: $2"
                docker-compose logs -f "$2"
            else
                info "Showing logs for all services (use Ctrl+C to exit)"
                docker-compose logs -f
            fi
            ;;
        "health")
            info "Performing health check..."
            generate_health_report
            test_service_connectivity
            ;;
        "restart")
            if [[ -n "${2:-}" ]]; then
                info "Restarting service: $2"
                docker-compose restart "$2"
                log "Service $2 restarted"
            else
                info "Restarting all services..."
                docker-compose restart
                log "All services restarted"
            fi
            ;;
        *)
            echo "Usage: $0 {start|stop|build|clean|status|health|restart|logs} [service]"
            echo ""
            echo "Commands:"
            echo "  start   - Clean, build and start the complete cluster"
            echo "  stop    - Stop all services"
            echo "  build   - Build Docker images only"
            echo "  clean   - Clean up containers and volumes"
            echo "  status  - Show detailed service status"
            echo "  health  - Perform comprehensive health check"
            echo "  restart - Restart all services or specific service"
            echo "  logs    - Show logs (optionally for specific service)"
            echo ""
            echo "Examples:"
            echo "  $0 start           # Start the complete cluster"
            echo "  $0 logs master-1   # Show logs for master-1 only"
            echo "  $0 restart client-1 # Restart only client-1"
            echo "  $0 health          # Check all service health"
            echo ""
            echo "Environment variables:"
            echo "  CLEAN_DOCKER_SYSTEM=true  # Also run 'docker system prune' during cleanup"
            echo "  MAX_WAIT_TIME=300         # Maximum wait time for services (seconds)"
            exit 1
            ;;
    esac
}

# Execute main function with all arguments
main "$@"