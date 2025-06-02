#!/bin/bash

# MooseNG Docker Setup Test Script
# Tests the complete docker-compose setup with 3 masters, 3 chunkservers, 3 clients

set -e

echo "🔧 MooseNG Docker Setup Test"
echo "=============================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if Docker is running
check_docker() {
    print_status "Checking Docker daemon..."
    if ! docker info > /dev/null 2>&1; then
        print_error "Docker daemon is not running. Please start Docker first."
        exit 1
    fi
    print_success "Docker daemon is running"
}

# Validate docker-compose file
validate_compose() {
    print_status "Validating docker-compose.yml..."
    if docker-compose config --quiet; then
        print_success "docker-compose.yml is valid"
    else
        print_error "docker-compose.yml validation failed"
        exit 1
    fi
}

# List services
list_services() {
    print_status "Services in docker-compose.yml:"
    services=$(docker-compose config --services)
    echo "$services" | while read -r service; do
        echo "  - $service"
    done
}

# Build all services
build_services() {
    print_status "Building all Docker images..."
    
    # Build in dependency order for better performance
    print_status "Building master services..."
    docker-compose build master-1 master-2 master-3
    
    print_status "Building chunkserver services..."
    docker-compose build chunkserver-1 chunkserver-2 chunkserver-3
    
    print_status "Building client services..."
    docker-compose build client-1 client-2 client-3
    
    print_success "All services built successfully"
}

# Test build only (without starting)
test_build() {
    print_status "Testing build for one service from each type..."
    
    print_status "Testing master build..."
    docker-compose build master-1
    
    print_status "Testing chunkserver build..."
    docker-compose build chunkserver-1
    
    print_status "Testing client build..."
    docker-compose build client-1
    
    print_success "Build tests completed successfully"
}

# Start the cluster
start_cluster() {
    print_status "Starting MooseNG cluster..."
    
    # Start masters first
    print_status "Starting master servers..."
    docker-compose up -d master-1 master-2 master-3
    
    # Wait for masters to be ready
    print_status "Waiting for masters to start..."
    sleep 10
    
    # Start chunkservers
    print_status "Starting chunkservers..."
    docker-compose up -d chunkserver-1 chunkserver-2 chunkserver-3
    
    # Wait for chunkservers to register
    print_status "Waiting for chunkservers to register..."
    sleep 10
    
    # Start clients
    print_status "Starting clients..."
    docker-compose up -d client-1 client-2 client-3
    
    # Start monitoring
    print_status "Starting monitoring stack..."
    docker-compose up -d prometheus grafana dashboard
    
    print_success "MooseNG cluster started"
}

# Check service health
check_health() {
    print_status "Checking service health..."
    
    # Get all running containers
    containers=$(docker-compose ps --services --filter "status=running")
    
    for container in $containers; do
        status=$(docker-compose ps $container --format "table {{.State}}" | tail -n +2)
        if [ "$status" = "running" ]; then
            print_success "$container is running"
        else
            print_warning "$container status: $status"
        fi
    done
}

# Show logs
show_logs() {
    print_status "Recent logs from all services:"
    docker-compose logs --tail=20
}

# Show service URLs
show_urls() {
    print_status "Service URLs:"
    echo "  🌐 Web Dashboard:   http://localhost:8080"
    echo "  📊 Grafana:        http://localhost:3000 (admin/admin)"
    echo "  📈 Prometheus:     http://localhost:9090"
    echo "  🖥️  Master APIs:"
    echo "     - Master 1:     http://localhost:9421"
    echo "     - Master 2:     http://localhost:9431"
    echo "     - Master 3:     http://localhost:9441"
    echo "  📦 Chunkserver Metrics:"
    echo "     - Chunkserver 1: http://localhost:9425"
    echo "     - Chunkserver 2: http://localhost:9455"
    echo "     - Chunkserver 3: http://localhost:9465"
}

# Stop the cluster
stop_cluster() {
    print_status "Stopping MooseNG cluster..."
    docker-compose down
    print_success "MooseNG cluster stopped"
}

# Cleanup volumes
cleanup_volumes() {
    print_status "Cleaning up volumes..."
    docker-compose down -v
    print_success "Volumes cleaned up"
}

# Main execution
main() {
    case "${1:-help}" in
        "test-build")
            check_docker
            validate_compose
            test_build
            ;;
        "build")
            check_docker
            validate_compose
            build_services
            ;;
        "start")
            check_docker
            validate_compose
            start_cluster
            check_health
            show_urls
            ;;
        "stop")
            stop_cluster
            ;;
        "restart")
            stop_cluster
            start_cluster
            check_health
            show_urls
            ;;
        "status")
            check_health
            ;;
        "logs")
            show_logs
            ;;
        "urls")
            show_urls
            ;;
        "cleanup")
            cleanup_volumes
            ;;
        "help"|*)
            echo "Usage: $0 {test-build|build|start|stop|restart|status|logs|urls|cleanup|help}"
            echo ""
            echo "Commands:"
            echo "  test-build  - Test building one service from each type"
            echo "  build       - Build all Docker images"
            echo "  start       - Start the complete MooseNG cluster"
            echo "  stop        - Stop the cluster"
            echo "  restart     - Restart the cluster"
            echo "  status      - Check service health"
            echo "  logs        - Show recent logs"
            echo "  urls        - Show service URLs"
            echo "  cleanup     - Stop and remove all volumes"
            echo "  help        - Show this help message"
            ;;
    esac
}

# Execute main function
main "$@"