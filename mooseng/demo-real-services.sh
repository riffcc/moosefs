#!/bin/bash
set -e

# MooseNG Real Services Demo Script
# This script demonstrates the MooseNG distributed filesystem with real compiled services

echo "🚀 Starting MooseNG Demo with Real Services"
echo "============================================="

# Check prerequisites
check_prerequisites() {
    echo "📋 Checking prerequisites..."
    
    if ! command -v docker &> /dev/null; then
        echo "❌ Docker is required but not installed"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null; then
        echo "❌ Docker Compose is required but not installed"
        exit 1
    fi
    
    if ! docker info &> /dev/null; then
        echo "❌ Docker daemon is not running"
        exit 1
    fi
    
    echo "✅ Prerequisites check passed"
}

# Clean up any existing containers
cleanup() {
    echo "🧹 Cleaning up existing containers..."
    docker-compose -f docker-compose.yml down --volumes --remove-orphans || true
    docker system prune -f || true
    echo "✅ Cleanup completed"
}

# Build services
build_services() {
    echo "🔨 Building MooseNG services..."
    echo "This may take a few minutes for the first build..."
    
    # Build services in parallel where possible
    docker-compose build --parallel --progress=plain
    
    if [ $? -eq 0 ]; then
        echo "✅ Services built successfully"
    else
        echo "❌ Build failed - some services may have compilation issues"
        echo "📋 Checking which services compiled successfully..."
        
        # Test each service individually
        services=("master" "chunkserver" "client")
        for service in "${services[@]}"; do
            echo "Testing $service build..."
            if docker-compose build $service-1 &> /dev/null; then
                echo "✅ $service builds successfully"
            else
                echo "❌ $service has build issues"
            fi
        done
        
        echo ""
        echo "🔧 For services with compilation issues, consider:"
        echo "   1. Using mock services temporarily"
        echo "   2. Fixing compilation errors in the Rust code"
        echo "   3. Contributing fixes to the MooseNG project"
        
        read -p "Continue with available services? (y/n): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
}

# Start services
start_services() {
    echo "🚀 Starting MooseNG cluster..."
    
    # Start services in order: masters first, then chunkservers, then clients
    echo "Starting master servers..."
    docker-compose up -d master-1 master-2 master-3
    
    echo "Waiting for masters to be ready..."
    sleep 30
    
    echo "Starting chunkservers..."
    docker-compose up -d chunkserver-1 chunkserver-2 chunkserver-3
    
    echo "Waiting for chunkservers to register..."
    sleep 20
    
    echo "Starting clients..."
    docker-compose up -d client-1 client-2 client-3
    
    echo "Starting monitoring stack..."
    docker-compose up -d prometheus grafana dashboard
    
    echo "✅ All services started"
}

# Check service health
check_health() {
    echo "🔍 Checking service health..."
    
    services=(
        "master-1:9421"
        "master-2:9421"
        "master-3:9421"
        "chunkserver-1:9420"
        "chunkserver-2:9420" 
        "chunkserver-3:9420"
    )
    
    for service in "${services[@]}"; do
        container=$(echo $service | cut -d: -f1)
        port=$(echo $service | cut -d: -f2)
        
        echo -n "Checking $container... "
        if docker-compose ps | grep -q "$container.*Up"; then
            echo "✅ Running"
        else
            echo "❌ Not running"
        fi
    done
    
    echo ""
    echo "📊 Service Status Dashboard: http://localhost:8080"
    echo "📈 Prometheus Metrics: http://localhost:9090"
    echo "📉 Grafana Dashboard: http://localhost:3000 (admin/admin)"
}

# Demonstrate filesystem operations
demo_operations() {
    echo "🔧 Demonstrating MooseNG filesystem operations..."
    
    # Check if any client is running and has mounted filesystem
    if docker-compose ps | grep -q "client-1.*Up"; then
        echo "Testing filesystem operations on client-1..."
        
        # Create test files
        echo "Creating test files..."
        docker-compose exec client-1 sh -c "
            echo 'Hello from MooseNG!' > /mnt/mooseng/test1.txt || echo 'Mount may not be ready'
            echo 'Distributed filesystem test' > /mnt/mooseng/test2.txt || echo 'Mount may not be ready'
            ls -la /mnt/mooseng/ || echo 'Mount point not accessible'
        "
        
        # Test replication across clients
        echo "Testing file visibility across clients..."
        for client in client-2 client-3; do
            if docker-compose ps | grep -q "$client.*Up"; then
                echo "Checking files on $client..."
                docker-compose exec $client sh -c "
                    ls -la /mnt/mooseng/ || echo 'Mount point not accessible on $client'
                "
            fi
        done
        
    else
        echo "⚠️  No clients are running - cannot demonstrate filesystem operations"
    fi
}

# Show logs
show_logs() {
    echo "📋 Recent service logs:"
    echo "======================="
    
    docker-compose logs --tail=20 --timestamps
}

# Main execution
main() {
    echo "MooseNG Demo - Real Services"
    echo "============================"
    
    check_prerequisites
    
    case "${1:-start}" in
        "clean")
            cleanup
            ;;
        "build")
            build_services
            ;;
        "start")
            cleanup
            build_services
            start_services
            check_health
            demo_operations
            ;;
        "stop")
            echo "🛑 Stopping MooseNG demo..."
            docker-compose down
            echo "✅ Demo stopped"
            ;;
        "status")
            check_health
            ;;
        "logs")
            show_logs
            ;;
        "demo")
            demo_operations
            ;;
        *)
            echo "Usage: $0 {start|stop|status|logs|demo|clean|build}"
            echo ""
            echo "Commands:"
            echo "  start  - Clean, build, and start all services"
            echo "  stop   - Stop all services"
            echo "  status - Check service health"
            echo "  logs   - Show recent service logs"
            echo "  demo   - Demonstrate filesystem operations"
            echo "  clean  - Clean up containers and volumes"
            echo "  build  - Build services only"
            exit 1
            ;;
    esac
}

# Handle Ctrl+C
trap cleanup EXIT

main "$@"