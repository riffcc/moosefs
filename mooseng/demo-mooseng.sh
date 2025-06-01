#!/bin/bash
# MooseNG Demo Management Script

set -e

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

usage() {
    echo "Usage: $0 {start|stop|restart|status|logs|build|clean}"
    echo ""
    echo "Commands:"
    echo "  start   - Start all MooseNG services"
    echo "  stop    - Stop all MooseNG services"
    echo "  restart - Restart all MooseNG services"
    echo "  status  - Show status of all services"
    echo "  logs    - Follow logs from all services"
    echo "  build   - Build all Docker images"
    echo "  clean   - Stop services and remove volumes"
    exit 1
}

start_services() {
    echo -e "${GREEN}Starting MooseNG services...${NC}"
    docker-compose up -d
    echo ""
    echo -e "${GREEN}Services started!${NC}"
    echo ""
    echo "Dashboard: http://localhost:8080"
    echo "Prometheus: http://localhost:9090"
    echo "Grafana: http://localhost:3000 (admin/admin)"
    echo ""
    echo "Master endpoints:"
    echo "  - Master 1: http://localhost:9421"
    echo "  - Master 2: http://localhost:9431"
    echo "  - Master 3: http://localhost:9441"
}

stop_services() {
    echo -e "${YELLOW}Stopping MooseNG services...${NC}"
    docker-compose down
    echo -e "${GREEN}Services stopped.${NC}"
}

restart_services() {
    stop_services
    echo ""
    start_services
}

show_status() {
    echo -e "${GREEN}MooseNG Service Status:${NC}"
    docker-compose ps
    echo ""
    echo -e "${GREEN}Health Status:${NC}"
    
    # Check master health
    for i in 1 2 3; do
        port=$((9421 + (i-1)*10))
        echo -n "Master $i: "
        if curl -s http://localhost:$port/health > /dev/null 2>&1; then
            echo -e "${GREEN}Healthy${NC}"
        else
            echo -e "${RED}Unhealthy${NC}"
        fi
    done
    
    # Check chunkserver health
    for i in 1 2 3; do
        port=$((9427 + (i-1)*30))
        echo -n "ChunkServer $i: "
        if curl -s http://localhost:$port/health > /dev/null 2>&1; then
            echo -e "${GREEN}Healthy${NC}"
        else
            echo -e "${RED}Unhealthy${NC}"
        fi
    done
}

show_logs() {
    echo -e "${GREEN}Following logs (Ctrl+C to exit)...${NC}"
    docker-compose logs -f
}

build_images() {
    echo -e "${GREEN}Building MooseNG Docker images...${NC}"
    
    # Build Rust components first
    echo "Building Rust components..."
    cargo build --release --all
    
    # Build Docker images
    echo "Building Docker images..."
    docker-compose build --parallel
    
    echo -e "${GREEN}Build completed!${NC}"
}

clean_all() {
    echo -e "${RED}WARNING: This will remove all data!${NC}"
    read -p "Are you sure? (yes/no) " -n 3 -r
    echo
    if [[ $REPLY =~ ^yes$ ]]; then
        echo -e "${YELLOW}Stopping services and removing volumes...${NC}"
        docker-compose down -v
        echo -e "${GREEN}Cleanup completed.${NC}"
    else
        echo "Aborted."
    fi
}

# Main command handling
case "$1" in
    start)
        start_services
        ;;
    stop)
        stop_services
        ;;
    restart)
        restart_services
        ;;
    status)
        show_status
        ;;
    logs)
        show_logs
        ;;
    build)
        build_images
        ;;
    clean)
        clean_all
        ;;
    *)
        usage
        ;;
esac