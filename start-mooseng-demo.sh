#!/bin/bash

# MooseNG Docker Compose Demo Startup Script
# Starts a complete MooseNG cluster with 3 masters, 3 chunkservers, and 3 clients

set -e

echo "🚀 Starting MooseNG Demo Cluster..."
echo "======================================"

# Check if Docker is running
if ! docker info >/dev/null 2>&1; then
    echo "❌ Error: Docker is not running. Please start Docker first."
    exit 1
fi

# Check if docker compose is available
if ! docker compose version >/dev/null 2>&1; then
    echo "❌ Error: docker compose is not available."
    exit 1
fi

# Navigate to mooseng directory
cd mooseng

echo "📁 Working directory: $(pwd)"

# Create necessary directories
echo "📂 Creating mount directories..."
mkdir -p mnt/client-1 mnt/client-2 mnt/client-3

# Pull/build and start services
echo "🔧 Building and starting services..."
echo "This may take a while on first run as images need to be built..."

# Start services in order with health checks
echo "🏗️  Starting MooseNG cluster..."
docker compose up -d

# Monitor startup progress
echo "⏳ Waiting for services to become healthy..."

# Function to check service health
check_service_health() {
    local service=$1
    local max_attempts=${2:-30}
    local attempt=1
    
    echo "🔍 Checking $service health..."
    while [ $attempt -le $max_attempts ]; do
        if docker compose ps $service | grep -q "healthy"; then
            echo "✅ $service is healthy"
            return 0
        elif docker compose ps $service | grep -q "unhealthy"; then
            echo "❌ $service is unhealthy"
            return 1
        else
            echo "⏳ $service starting... (attempt $attempt/$max_attempts)"
            sleep 10
            ((attempt++))
        fi
    done
    
    echo "⚠️  $service health check timed out"
    return 1
}

# Parallel health check function
check_services_parallel() {
    local services=("$@")
    local pids=()
    
    # Start health checks in parallel
    for service in "${services[@]}"; do
        (
            local max_attempts=15
            local attempt=1
            
            echo "🔍 Checking $service health..."
            while [ $attempt -le $max_attempts ]; do
                if docker compose ps $service 2>/dev/null | grep -q "healthy"; then
                    echo "✅ $service is healthy"
                    exit 0
                elif docker compose ps $service 2>/dev/null | grep -q "unhealthy"; then
                    echo "❌ $service is unhealthy"
                    exit 1
                elif docker compose ps $service 2>/dev/null | grep -q "Up"; then
                    echo "⏳ $service running but no health check... (attempt $attempt/$max_attempts)"
                    sleep 3
                    ((attempt++))
                else
                    echo "⏳ $service starting... (attempt $attempt/$max_attempts)"
                    sleep 5
                    ((attempt++))
                fi
            done
            
            echo "⚠️  $service health check timed out"
            exit 1
        ) &
        pids+=($!)
    done
    
    # Wait for all parallel checks to complete
    local all_success=true
    for i in "${!pids[@]}"; do
        wait ${pids[$i]}
        if [ $? -ne 0 ]; then
            all_success=false
        fi
    done
    
    return $([[ "$all_success" == "true" ]] && echo 0 || echo 1)
}

# Check services with parallel health checks
echo ""
echo "🔍 Running optimized parallel health checks..."

# Get list of actual services
available_services=($(docker compose ps --services 2>/dev/null | grep -E "(chunkserver|metalogger|cli|dashboard|prometheus|grafana)" | head -10))

if [ ${#available_services[@]} -eq 0 ]; then
    echo "⚠️  No services found - checking with sequential fallback..."
    # Fallback to checking expected services
    for service in chunkserver-1 chunkserver-2 chunkserver-3 metalogger-1 cli dashboard prometheus grafana; do
        if docker compose ps $service >/dev/null 2>&1; then
            check_service_health $service 10
        fi
    done
else
    echo "📋 Parallel health check for: ${available_services[*]}"
    check_services_parallel "${available_services[@]}"
fi

echo ""
echo "📊 Cluster Status:"
echo "=================="
docker compose ps

echo ""
echo "🎉 MooseNG Demo Cluster Started Successfully!"
echo "============================================="
echo ""
echo "📋 Service Endpoints:"
echo "-------------------"
echo "🔧 Master 1:        http://localhost:9421 (Client) | http://localhost:9422 (ChunkServer) | http://localhost:9430 (Health)"
echo "🔧 Master 2:        http://localhost:9431 (Client) | http://localhost:9432 (ChunkServer) | http://localhost:9435 (Health)"
echo "🔧 Master 3:        http://localhost:9441 (Client) | http://localhost:9442 (ChunkServer) | http://localhost:9445 (Health)"
echo ""
echo "💾 ChunkServer 1:   http://localhost:9420 | http://localhost:9425 (Metrics) | http://localhost:9429 (Health)"
echo "💾 ChunkServer 2:   http://localhost:9450 | http://localhost:9455 (Metrics) | http://localhost:9459 (Health)"
echo "💾 ChunkServer 3:   http://localhost:9460 | http://localhost:9465 (Metrics) | http://localhost:9469 (Health)"
echo ""
echo "🖥️  Client 1:        http://localhost:9427 (Metrics) | Mount: ./mnt/client-1"
echo "🖥️  Client 2:        http://localhost:9437 (Metrics) | Mount: ./mnt/client-2"
echo "🖥️  Client 3:        http://localhost:9447 (Metrics) | Mount: ./mnt/client-3"
echo ""
echo "📊 Monitoring:"
echo "-------------"
echo "📈 Prometheus:      http://localhost:9090"
echo "📊 Grafana:         http://localhost:3000 (admin/admin)"
echo "🌐 Dashboard:       http://localhost:8080"
echo ""
echo "🛠️  Management:"
echo "-------------"
echo "To access CLI: docker compose exec cli /bin/sh"
echo ""
echo "🔍 Useful Commands:"
echo "------------------"
echo "View logs:         docker compose logs -f [service_name]"
echo "Check status:      docker compose ps"
echo "Stop cluster:      docker compose down"
echo "Clean up:          docker compose down -v (removes volumes)"
echo ""
echo "✨ Your MooseNG cluster is ready for testing!"