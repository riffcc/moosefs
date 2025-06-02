#!/bin/bash

# MooseNG Unified Cluster Demo
# Demonstrates the complete 3M+3C+3Cl cluster setup

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
PURPLE='\033[0;35m'
NC='\033[0m'

print_header() {
    echo -e "${PURPLE}╔════════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${PURPLE}║                        MooseNG Demo                            ║${NC}"
    echo -e "${PURPLE}║           3 Masters + 3 Chunkservers + 3 Clients             ║${NC}"
    echo -e "${PURPLE}╚════════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

print_section() {
    echo -e "${CYAN}▶ $1${NC}"
    echo -e "${CYAN}$(printf '─%.0s' {1..60})${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

# Check prerequisites
check_prerequisites() {
    print_section "Checking Prerequisites"
    
    # Check Docker
    if ! command -v docker &> /dev/null; then
        print_error "Docker is not installed"
        exit 1
    fi
    print_success "Docker is installed"
    
    # Check Docker Compose
    if ! command -v docker-compose &> /dev/null; then
        print_error "Docker Compose is not installed"
        exit 1
    fi
    print_success "Docker Compose is installed"
    
    # Check Docker daemon
    if ! docker info > /dev/null 2>&1; then
        print_error "Docker daemon is not running"
        exit 1
    fi
    print_success "Docker daemon is running"
    
    echo ""
}

# Show architecture
show_architecture() {
    print_section "MooseNG Cluster Architecture"
    echo ""
    echo "┌─────────────────────────────────────────────────────────────────┐"
    echo "│                         MASTER CLUSTER                         │"
    echo "├─────────────────┬─────────────────┬─────────────────────────────┤"
    echo "│   Master-1      │   Master-2      │      Master-3               │"
    echo "│   :9421-9423    │   :9431-9433    │      :9441-9443             │"
    echo "│   (Leader)      │   (Follower)    │      (Follower)             │"
    echo "└─────────────────┴─────────────────┴─────────────────────────────┘"
    echo "                              │"
    echo "                         Raft Consensus"
    echo "                              │"
    echo "┌─────────────────────────────────────────────────────────────────┐"
    echo "│                      CHUNK SERVERS                             │"
    echo "├─────────────────┬─────────────────┬─────────────────────────────┤"
    echo "│  Chunkserver-1  │  Chunkserver-2  │     Chunkserver-3           │"
    echo "│     :9420       │     :9450       │        :9460                │"
    echo "│   4 disks each  │   4 disks each  │      4 disks each           │"
    echo "└─────────────────┴─────────────────┴─────────────────────────────┘"
    echo "                              │"
    echo "                         Data Storage"
    echo "                              │"
    echo "┌─────────────────────────────────────────────────────────────────┐"
    echo "│                          CLIENTS                               │"
    echo "├─────────────────┬─────────────────┬─────────────────────────────┤"
    echo "│    Client-1     │    Client-2     │       Client-3              │"
    echo "│     :9427       │     :9437       │        :9447                │"
    echo "│  FUSE Mount     │  FUSE Mount     │     FUSE Mount              │"
    echo "└─────────────────┴─────────────────┴─────────────────────────────┘"
    echo ""
    echo "┌─────────────────────────────────────────────────────────────────┐"
    echo "│                       MONITORING                               │"
    echo "├─────────────────┬─────────────────┬─────────────────────────────┤"
    echo "│   Prometheus    │     Grafana     │      Dashboard              │"
    echo "│     :9090       │     :3000       │        :8080                │"
    echo "└─────────────────┴─────────────────┴─────────────────────────────┘"
    echo ""
}

# Show configuration summary
show_configuration() {
    print_section "Configuration Summary"
    echo ""
    echo "🎯 Cluster Configuration:"
    echo "   • Cluster ID: mooseng-cluster"
    echo "   • Total Storage: 12 virtual disks (4 per chunkserver)"
    echo "   • Replication: 2x default, 3x for 'fast' class"
    echo "   • Erasure Coding: 4+2, 8+4 schemes available"
    echo "   • Cache: Enabled on all services"
    echo ""
    echo "🔒 Security:"
    echo "   • TLS: Disabled (development mode)"
    echo "   • Non-root containers"
    echo "   • Isolated network: 172.20.0.0/24"
    echo ""
    echo "📊 Monitoring:"
    echo "   • Prometheus metrics collection"
    echo "   • Grafana dashboards"
    echo "   • Health checks on all services"
    echo ""
}

# Start demo
start_demo() {
    print_section "Starting MooseNG Cluster"
    
    print_info "Validating docker-compose configuration..."
    if ! docker-compose config --quiet; then
        print_error "Invalid docker-compose.yml configuration"
        exit 1
    fi
    print_success "Configuration validated"
    
    print_info "Starting master servers..."
    docker-compose up -d master-1 master-2 master-3
    
    print_info "Waiting for masters to initialize (30s)..."
    sleep 30
    
    print_info "Starting chunkservers..."
    docker-compose up -d chunkserver-1 chunkserver-2 chunkserver-3
    
    print_info "Waiting for chunkservers to register (20s)..."
    sleep 20
    
    print_info "Starting clients..."
    docker-compose up -d client-1 client-2 client-3
    
    print_info "Starting monitoring stack..."
    docker-compose up -d prometheus grafana dashboard
    
    print_success "All services started!"
    echo ""
}

# Check cluster health
check_health() {
    print_section "Cluster Health Check"
    
    # Check container status
    print_info "Checking container status..."
    running_containers=$(docker-compose ps --services --filter "status=running" | wc -l)
    total_containers=12  # 3 masters + 3 chunkservers + 3 clients + 3 monitoring
    
    echo "   Running containers: $running_containers/$total_containers"
    
    if [ "$running_containers" -eq "$total_containers" ]; then
        print_success "All containers are running"
    else
        print_warning "Some containers may not be running properly"
        docker-compose ps
    fi
    
    # Check network connectivity
    print_info "Checking network connectivity..."
    if docker exec mooseng-master-1 nc -z master-2 9422 2>/dev/null; then
        print_success "Raft network connectivity OK"
    else
        print_warning "Raft network connectivity issues detected"
    fi
    
    echo ""
}

# Show service URLs
show_urls() {
    print_section "Service Access URLs"
    echo ""
    echo "🌐 Web Interfaces:"
    echo "   • Dashboard:        http://localhost:8080"
    echo "   • Grafana:         http://localhost:3000 (admin/admin)"
    echo "   • Prometheus:      http://localhost:9090"
    echo ""
    echo "🔧 API Endpoints:"
    echo "   • Master 1:        http://localhost:9421"
    echo "   • Master 2:        http://localhost:9431"
    echo "   • Master 3:        http://localhost:9441"
    echo ""
    echo "📊 Metrics Endpoints:"
    echo "   • Master 1:        http://localhost:9423/metrics"
    echo "   • Master 2:        http://localhost:9433/metrics"
    echo "   • Master 3:        http://localhost:9443/metrics"
    echo "   • Chunkserver 1:   http://localhost:9425/metrics"
    echo "   • Chunkserver 2:   http://localhost:9455/metrics"
    echo "   • Chunkserver 3:   http://localhost:9465/metrics"
    echo ""
}

# Show logs
show_recent_logs() {
    print_section "Recent Service Logs"
    echo ""
    print_info "Master logs:"
    docker-compose logs --tail=5 master-1 master-2 master-3 || true
    echo ""
    
    print_info "Chunkserver logs:"
    docker-compose logs --tail=5 chunkserver-1 chunkserver-2 chunkserver-3 || true
    echo ""
    
    print_info "Client logs:"
    docker-compose logs --tail=5 client-1 client-2 client-3 || true
    echo ""
}

# Run filesystem tests
test_filesystem() {
    print_section "Filesystem Functionality Tests"
    
    print_info "Testing file operations via client containers..."
    
    # Test file creation
    if docker exec mooseng-client-1 sh -c "echo 'Hello MooseNG' > /mnt/mooseng/test-file.txt" 2>/dev/null; then
        print_success "File creation test passed"
    else
        print_warning "File creation test failed (FUSE may not be mounted)"
    fi
    
    # Test file reading
    if docker exec mooseng-client-2 sh -c "cat /mnt/mooseng/test-file.txt" 2>/dev/null | grep -q "Hello MooseNG"; then
        print_success "File reading test passed"
    else
        print_warning "File reading test failed"
    fi
    
    # Test file listing
    if docker exec mooseng-client-3 sh -c "ls -la /mnt/mooseng/" 2>/dev/null | grep -q "test-file.txt"; then
        print_success "File listing test passed"
    else
        print_warning "File listing test failed"
    fi
    
    echo ""
}

# Stop demo
stop_demo() {
    print_section "Stopping MooseNG Cluster"
    
    print_info "Stopping all services..."
    docker-compose down
    print_success "All services stopped"
    echo ""
}

# Cleanup
cleanup_demo() {
    print_section "Cleaning Up Demo Environment"
    
    print_info "Stopping and removing containers..."
    docker-compose down -v
    
    print_info "Removing unused Docker images..."
    docker image prune -f
    
    print_success "Cleanup completed"
    echo ""
}

# Main menu
show_menu() {
    echo ""
    print_section "Demo Commands"
    echo ""
    echo "1. start      - Start the complete cluster"
    echo "2. stop       - Stop the cluster"
    echo "3. status     - Check cluster health"
    echo "4. logs       - Show recent logs"
    echo "5. test       - Run filesystem tests"
    echo "6. urls       - Show service URLs"
    echo "7. cleanup    - Stop and remove everything"
    echo "8. restart    - Restart the cluster"
    echo "9. help       - Show this menu"
    echo ""
}

# Main execution
main() {
    print_header
    
    case "${1:-help}" in
        "start")
            check_prerequisites
            show_architecture
            show_configuration
            start_demo
            check_health
            show_urls
            ;;
        "stop")
            stop_demo
            ;;
        "restart")
            stop_demo
            start_demo
            check_health
            show_urls
            ;;
        "status")
            check_health
            ;;
        "logs")
            show_recent_logs
            ;;
        "test")
            test_filesystem
            ;;
        "urls")
            show_urls
            ;;
        "cleanup")
            cleanup_demo
            ;;
        "arch")
            show_architecture
            ;;
        "help"|*)
            show_architecture
            show_menu
            echo -e "${YELLOW}Usage: $0 {start|stop|restart|status|logs|test|urls|cleanup|help}${NC}"
            echo ""
            echo -e "${CYAN}Quick Start:${NC}"
            echo "  1. $0 start    # Start the cluster"
            echo "  2. $0 status   # Check health"
            echo "  3. $0 test     # Test filesystem"
            echo "  4. $0 urls     # Get service URLs"
            echo ""
            ;;
    esac
}

# Execute main function
main "$@"