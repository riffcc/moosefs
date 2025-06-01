# Docker Compose and Helper Scripts - Improvement Analysis

## Executive Summary

This document provides a comprehensive analysis of the MooseNG Docker Compose setup and helper scripts, identifying areas for improvement in efficiency, error handling, and maintainability, along with specific code modifications.

## 1. Docker Compose File Analysis

### Current Issues

1. **Significant Code Duplication**
   - Master servers (master-1, master-2, master-3) share ~90% identical configuration
   - Chunkservers repeat the same pattern with only port and volume differences
   - Client configurations are nearly identical

2. **Hardcoded Values**
   - Port numbers are hardcoded throughout
   - Network subnet is fixed (172.20.0.0/24)
   - No easy way to scale beyond 3 instances of each type

3. **Missing Features**
   - No resource limits (memory, CPU)
   - No logging configuration
   - Missing dependency health checks (depends_on with conditions)

### Proposed Improvements

#### 1.1 Use YAML Anchors and Extensions

```yaml
# Define common configurations
x-master-common: &master-common
  build:
    context: .
    dockerfile: docker/Dockerfile.mock-master
  environment: &master-env
    NODE_ROLE: master
    CLUSTER_NODES: master-1:9421,master-2:9421,master-3:9421
    CLIENT_PORT: 9421
    RAFT_PORT: 9422
    METRICS_PORT: 9423
    LOG_LEVEL: ${LOG_LEVEL:-info}
  networks:
    - mooseng-net
  restart: unless-stopped
  healthcheck: &master-healthcheck
    test: ["CMD", "curl", "-f", "http://localhost:9421/health"]
    interval: 10s
    timeout: 5s
    retries: 3
    start_period: 10s

services:
  master-1:
    <<: *master-common
    container_name: mooseng-master-1
    hostname: master-1
    environment:
      <<: *master-env
      NODE_ID: 1
    ports:
      - "${MASTER1_CLIENT_PORT:-9421}:9421"
      - "${MASTER1_RAFT_PORT:-9422}:9422"
      - "${MASTER1_METRICS_PORT:-9423}:9423"
    volumes:
      - master-1-data:/data
```

#### 1.2 Add Resource Limits

```yaml
services:
  master-1:
    # ... other config ...
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 2G
        reservations:
          cpus: '0.5'
          memory: 512M
```

#### 1.3 Implement Proper Health Dependencies

```yaml
services:
  chunkserver-1:
    # ... other config ...
    depends_on:
      master-1:
        condition: service_healthy
      master-2:
        condition: service_healthy
      master-3:
        condition: service_healthy
```

#### 1.4 Add Logging Configuration

```yaml
x-logging: &default-logging
  driver: "json-file"
  options:
    max-size: "10m"
    max-file: "3"
    labels: "service"

services:
  master-1:
    # ... other config ...
    logging: *default-logging
```

## 2. Helper Scripts Analysis

### 2.1 start-demo.sh Issues and Improvements

**Current Issues:**
- No validation of Docker/Docker Compose installation
- No checking for port conflicts
- Limited error handling
- Health checks could fail silently

**Proposed Improvements:**

```bash
#!/bin/bash
# MooseNG Docker Demo Startup Script - Enhanced Version

set -euo pipefail  # Better error handling
trap cleanup EXIT  # Cleanup on exit

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Source shared functions
source "${SCRIPT_DIR}/lib/common-functions.sh"

# Configuration
COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
RETRY_ATTEMPTS=5
RETRY_DELAY=3

# Cleanup function
cleanup() {
    if [ $? -ne 0 ]; then
        echo "❌ Script failed. Check logs for details."
    fi
}

# Validate prerequisites
validate_prerequisites() {
    echo "🔍 Validating prerequisites..."
    
    # Check Docker
    if ! command -v docker &> /dev/null; then
        error "Docker is not installed. Please install Docker first."
        exit 1
    fi
    
    # Check Docker Compose
    if ! docker compose version &> /dev/null; then
        error "Docker Compose is not available. Please install Docker Compose v2."
        exit 1
    fi
    
    # Check Docker daemon
    if ! docker info &> /dev/null; then
        error "Docker daemon is not running. Please start Docker."
        exit 1
    fi
    
    # Check for port conflicts
    check_port_conflicts
}

# Check for port conflicts
check_port_conflicts() {
    local ports=(9421 9431 9441 9420 9450 9460 9090 3000 8080)
    local conflicts=()
    
    for port in "${ports[@]}"; do
        if lsof -Pi :$port -sTCP:LISTEN -t >/dev/null 2>&1; then
            conflicts+=($port)
        fi
    done
    
    if [ ${#conflicts[@]} -gt 0 ]; then
        error "Port conflicts detected: ${conflicts[*]}"
        echo "Please stop the services using these ports or change the port mappings."
        exit 1
    fi
}

# Wait for service with retries
wait_for_service() {
    local name=$1
    local url=$2
    local attempts=0
    
    while [ $attempts -lt $RETRY_ATTEMPTS ]; do
        if curl -s -f "$url" > /dev/null 2>&1; then
            success "$name is healthy"
            return 0
        fi
        
        attempts=$((attempts + 1))
        if [ $attempts -lt $RETRY_ATTEMPTS ]; then
            echo "  ⏳ Waiting for $name (attempt $attempts/$RETRY_ATTEMPTS)..."
            sleep $RETRY_DELAY
        fi
    done
    
    error "$name failed to become healthy after $RETRY_ATTEMPTS attempts"
    return 1
}

# Main execution
main() {
    print_banner "MooseNG Docker Demo - Starting 3 Masters, 3 Chunkservers, 3 Clients"
    
    # Validate prerequisites
    validate_prerequisites
    
    # Create necessary directories
    info "Creating mount directories..."
    mkdir -p mnt/client-{1,2,3}
    
    # Stop any existing containers
    info "Stopping any existing containers..."
    docker compose -f "$COMPOSE_FILE" down --remove-orphans 2>/dev/null || true
    
    # Build images with progress
    info "Building Docker images..."
    if ! docker compose -f "$COMPOSE_FILE" build --parallel --progress=plain; then
        error "Failed to build Docker images"
        exit 1
    fi
    
    # Start services
    info "Starting services..."
    if ! docker compose -f "$COMPOSE_FILE" up -d; then
        error "Failed to start services"
        exit 1
    fi
    
    # Wait for services to be healthy
    info "Waiting for services to be healthy..."
    
    # Check masters
    local masters_healthy=0
    for i in 1 2 3; do
        port=$((9421 + (i-1)*10))
        if wait_for_service "Master $i" "http://localhost:$port/health"; then
            ((masters_healthy++))
        fi
    done
    
    # Check chunkservers
    local chunks_healthy=0
    for i in 1 2 3; do
        port=$((9420 + (i-1)*30))
        [ $i -eq 2 ] && port=9450
        [ $i -eq 3 ] && port=9460
        if wait_for_service "Chunkserver $i" "http://localhost:$port/health"; then
            ((chunks_healthy++))
        fi
    done
    
    # Display service status
    print_service_status
    
    # Summary
    if [ $masters_healthy -eq 3 ] && [ $chunks_healthy -eq 3 ]; then
        success "MooseNG Demo is running successfully!"
        print_access_info
    else
        warning "Some services are not healthy. Check logs with: docker compose logs -f [service-name]"
    fi
}

# Run main function
main "$@"
```

### 2.2 test-demo.sh Improvements

**Proposed Enhanced Version:**

```bash
#!/bin/bash
# MooseNG Docker Demo Test Script - Enhanced Version

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Source shared functions
source "${SCRIPT_DIR}/lib/common-functions.sh"

# Test configuration
COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
TEST_TIMEOUT=300  # 5 minutes total timeout

# Run comprehensive tests
run_tests() {
    local test_results=()
    local start_time=$(date +%s)
    
    print_banner "MooseNG Docker Demo Test Suite"
    
    # Test 1: Service Availability
    info "Test 1: Checking service availability..."
    if test_service_availability; then
        test_results+=("✅ Service Availability")
    else
        test_results+=("❌ Service Availability")
    fi
    
    # Test 2: Raft Consensus
    info "Test 2: Testing Raft consensus..."
    if test_raft_consensus; then
        test_results+=("✅ Raft Consensus")
    else
        test_results+=("❌ Raft Consensus")
    fi
    
    # Test 3: Data Persistence
    info "Test 3: Testing data persistence..."
    if test_data_persistence; then
        test_results+=("✅ Data Persistence")
    else
        test_results+=("❌ Data Persistence")
    fi
    
    # Test 4: Monitoring Stack
    info "Test 4: Testing monitoring stack..."
    if test_monitoring_stack; then
        test_results+=("✅ Monitoring Stack")
    else
        test_results+=("❌ Monitoring Stack")
    fi
    
    # Test 5: Performance Baseline
    info "Test 5: Running performance baseline..."
    if test_performance_baseline; then
        test_results+=("✅ Performance Baseline")
    else
        test_results+=("❌ Performance Baseline")
    fi
    
    # Display results
    local end_time=$(date +%s)
    local duration=$((end_time - start_time))
    
    print_test_summary "${test_results[@]}" "$duration"
}

# Test data persistence
test_data_persistence() {
    # Create test file
    local test_file="test_$(date +%s).txt"
    local test_content="MooseNG persistence test"
    
    # Write through client-1
    docker exec mooseng-client-1 sh -c "echo '$test_content' > /mnt/mooseng/$test_file" 2>/dev/null
    
    # Read through client-2
    local read_content=$(docker exec mooseng-client-2 cat "/mnt/mooseng/$test_file" 2>/dev/null)
    
    if [ "$read_content" = "$test_content" ]; then
        # Cleanup
        docker exec mooseng-client-1 rm "/mnt/mooseng/$test_file" 2>/dev/null
        return 0
    fi
    return 1
}

# Run the tests
run_tests
```

### 2.3 stop-demo.sh Improvements

**Proposed Enhanced Version:**

```bash
#!/bin/bash
# MooseNG Docker Demo Stop Script - Enhanced Version

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Source shared functions
source "${SCRIPT_DIR}/lib/common-functions.sh"

# Configuration
COMPOSE_FILE="${COMPOSE_FILE:-docker-compose.yml}"
REMOVE_VOLUMES=false
BACKUP_DATA=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -v|--volumes)
            REMOVE_VOLUMES=true
            shift
            ;;
        -b|--backup)
            BACKUP_DATA=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            error "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Show help
show_help() {
    cat << EOF
Usage: $0 [OPTIONS]

Options:
    -v, --volumes    Remove all data volumes
    -b, --backup     Backup data before stopping
    -h, --help       Show this help message

Examples:
    $0                    # Stop services, keep data
    $0 -v                 # Stop services and remove all data
    $0 -b                 # Backup data before stopping
EOF
}

# Backup data volumes
backup_data() {
    local backup_dir="backups/$(date +%Y%m%d_%H%M%S)"
    info "Creating backup in $backup_dir..."
    
    mkdir -p "$backup_dir"
    
    # Get volume list
    local volumes=$(docker compose -f "$COMPOSE_FILE" config --volumes)
    
    for volume in $volumes; do
        if docker volume inspect "mooseng_$volume" &>/dev/null; then
            info "Backing up volume: $volume"
            docker run --rm -v "mooseng_$volume:/source:ro" \
                -v "$PWD/$backup_dir:/backup" \
                alpine tar -czf "/backup/$volume.tar.gz" -C /source .
        fi
    done
    
    success "Backup completed in $backup_dir"
}

# Main execution
main() {
    print_banner "Stopping MooseNG Docker Demo"
    
    # Check if services are running
    if ! docker compose -f "$COMPOSE_FILE" ps --quiet 2>/dev/null | grep -q .; then
        info "No services are currently running"
        exit 0
    fi
    
    # Show current status
    info "Current running services:"
    docker compose -f "$COMPOSE_FILE" ps
    
    # Backup if requested
    if [ "$BACKUP_DATA" = true ]; then
        backup_data
    fi
    
    # Stop services
    info "Stopping all services..."
    docker compose -f "$COMPOSE_FILE" down
    
    # Remove volumes if requested
    if [ "$REMOVE_VOLUMES" = true ]; then
        warning "Removing all data volumes..."
        docker compose -f "$COMPOSE_FILE" down -v
    fi
    
    success "All services stopped!"
    
    # Show next steps
    echo ""
    info "Next steps:"
    echo "  - To start again: ./start-demo.sh"
    if [ "$REMOVE_VOLUMES" = false ]; then
        echo "  - To remove all data: $0 -v"
    fi
}

# Run main function
main
```

## 3. Shared Library Functions

Create a new file `lib/common-functions.sh`:

```bash
#!/bin/bash
# Common functions for MooseNG demo scripts

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
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

# Print banner
print_banner() {
    local title="$1"
    local width=${#title}
    local line=$(printf '%*s' "$width" | tr ' ' '=')
    
    echo ""
    echo "$title"
    echo "$line"
}

# Check if command exists
command_exists() {
    command -v "$1" &> /dev/null
}

# Wait for URL with timeout
wait_for_url() {
    local url=$1
    local timeout=${2:-30}
    local elapsed=0
    
    while [ $elapsed -lt $timeout ]; do
        if curl -s -f "$url" > /dev/null 2>&1; then
            return 0
        fi
        sleep 1
        ((elapsed++))
    done
    
    return 1
}

# Get container health status
get_container_health() {
    local container=$1
    docker inspect --format='{{.State.Health.Status}}' "$container" 2>/dev/null || echo "unknown"
}

# Print service endpoints
print_service_endpoints() {
    echo ""
    echo "🌐 Service Endpoints:"
    echo "==================="
    echo "Masters:"
    echo "  - Master 1: http://localhost:9421 (Raft: 9422, Metrics: 9423)"
    echo "  - Master 2: http://localhost:9431 (Raft: 9432, Metrics: 9433)"
    echo "  - Master 3: http://localhost:9441 (Raft: 9442, Metrics: 9443)"
    echo ""
    echo "Chunkservers:"
    echo "  - Chunkserver 1: http://localhost:9420 (Metrics: 9425)"
    echo "  - Chunkserver 2: http://localhost:9450 (Metrics: 9455)"
    echo "  - Chunkserver 3: http://localhost:9460 (Metrics: 9465)"
    echo ""
    echo "Clients:"
    echo "  - Client 1: Metrics at http://localhost:9427"
    echo "  - Client 2: Metrics at http://localhost:9437"
    echo "  - Client 3: Metrics at http://localhost:9447"
    echo ""
    echo "Monitoring:"
    echo "  - Prometheus: http://localhost:9090"
    echo "  - Grafana: http://localhost:3000 (admin/admin)"
    echo "  - Dashboard: http://localhost:8080"
}

# Export functions
export -f info success warning error
export -f print_banner command_exists wait_for_url
export -f get_container_health print_service_endpoints
```

## 4. Additional Recommendations

### 4.1 Create a Makefile

```makefile
.PHONY: help start stop test clean logs status

help:
	@echo "MooseNG Docker Demo - Available commands:"
	@echo "  make start    - Start the demo environment"
	@echo "  make stop     - Stop the demo environment"
	@echo "  make test     - Run tests"
	@echo "  make clean    - Stop and remove all data"
	@echo "  make logs     - Follow logs for all services"
	@echo "  make status   - Show service status"

start:
	@./start-demo.sh

stop:
	@./stop-demo.sh

test:
	@./test-demo.sh

clean:
	@./stop-demo.sh -v

logs:
	@docker compose logs -f

status:
	@docker compose ps
```

### 4.2 Environment Configuration

Create `.env.example`:

```env
# MooseNG Demo Configuration

# Log level (debug, info, warn, error)
LOG_LEVEL=info

# Master ports
MASTER1_CLIENT_PORT=9421
MASTER1_RAFT_PORT=9422
MASTER1_METRICS_PORT=9423

# Chunkserver configuration
CHUNKSERVER_REPLICAS=3
CHUNKSERVER_DISK_COUNT=4

# Client configuration
CLIENT_CACHE_SIZE=1G

# Monitoring
PROMETHEUS_RETENTION=7d
GRAFANA_ADMIN_PASSWORD=admin

# Resource limits
MASTER_CPU_LIMIT=2
MASTER_MEMORY_LIMIT=2G
CHUNKSERVER_CPU_LIMIT=4
CHUNKSERVER_MEMORY_LIMIT=4G
```

### 4.3 Health Check Script

Create `scripts/health-check.sh`:

```bash
#!/bin/bash
# Comprehensive health check for MooseNG cluster

source "$(dirname "$0")/../lib/common-functions.sh"

check_cluster_health() {
    local all_healthy=true
    
    # Check each component type
    for component in master chunkserver client; do
        info "Checking ${component}s..."
        
        local healthy=0
        local total=3
        
        for i in 1 2 3; do
            container="mooseng-${component}-${i}"
            health=$(get_container_health "$container")
            
            if [ "$health" = "healthy" ]; then
                ((healthy++))
                success "$container is healthy"
            else
                error "$container is $health"
                all_healthy=false
            fi
        done
        
        echo "  ${component^}s: $healthy/$total healthy"
    done
    
    # Check Raft leader
    info "Checking Raft consensus..."
    if check_raft_leader; then
        success "Raft leader elected"
    else
        warning "No Raft leader found"
        all_healthy=false
    fi
    
    # Return overall status
    $all_healthy
}

check_raft_leader() {
    for i in 1 2 3; do
        port=$((9421 + (i-1)*10))
        if curl -s "http://localhost:$port/status" 2>/dev/null | grep -q '"role":"leader"'; then
            return 0
        fi
    done
    return 1
}

# Run health check
if check_cluster_health; then
    success "Cluster is healthy!"
    exit 0
else
    error "Cluster has issues"
    exit 1
fi
```

## 5. Summary of Key Improvements

1. **Docker Compose**:
   - Use YAML anchors to eliminate duplication
   - Add environment variable support for configuration
   - Implement proper health check dependencies
   - Add resource limits and logging configuration

2. **Helper Scripts**:
   - Add comprehensive error handling with `set -euo pipefail`
   - Implement prerequisite validation
   - Add retry logic for health checks
   - Create shared library functions
   - Add command-line argument parsing
   - Implement backup functionality

3. **Maintainability**:
   - Centralize common functions in a library
   - Add extensive documentation and help messages
   - Create a Makefile for common operations
   - Use environment files for configuration

4. **Error Handling**:
   - Check for port conflicts before starting
   - Validate Docker installation and daemon status
   - Implement proper cleanup on script failure
   - Add timeout handling for health checks

5. **User Experience**:
   - Colored output for better readability
   - Progress indicators during operations
   - Clear error messages with suggested fixes
   - Comprehensive status reporting

These improvements will make the Docker Compose demo more robust, maintainable, and user-friendly while reducing code duplication and improving error handling.