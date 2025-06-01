#!/bin/bash

# MooseFS Parallel Health Check Demo Startup Script
# Optimized for parallel execution of health checks to reduce startup time

set -e

echo "🚀 Starting MooseFS Demo with Parallel Health Checks..."
echo "======================================================="

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

echo "📁 Working directory: $(pwd)"

# Start services
echo "🔧 Starting services with optimized health checks..."
docker compose up -d

# Parallel health check function
check_services_parallel() {
    local services=("$@")
    local pids=()
    local results=()
    
    # Start health checks in parallel
    for service in "${services[@]}"; do
        (
            local max_attempts=20
            local attempt=1
            
            echo "🔍 Checking $service health..."
            while [ $attempt -le $max_attempts ]; do
                if docker compose ps $service | grep -q "healthy"; then
                    echo "✅ $service is healthy"
                    exit 0
                elif docker compose ps $service | grep -q "unhealthy"; then
                    echo "❌ $service is unhealthy"
                    exit 1
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
            echo "❌ ${services[$i]} failed health check"
        fi
    done
    
    return $([[ "$all_success" == "true" ]] && echo 0 || echo 1)
}

echo ""
echo "🔍 Running parallel health checks..."

# Phase 1: Wait for master
echo "📋 Phase 1: Checking master..."
check_services_parallel "master"

# Phase 2: Check all chunkservers, metalogger, and cgiserver in parallel
echo ""
echo "📋 Phase 2: Checking chunkservers, metalogger, and cgiserver in parallel..."
check_services_parallel "chunkserver-1" "chunkserver-2" "chunkserver-3" "metalogger" "cgiserver"

# Phase 3: Check client (depends on at least one chunkserver)
echo ""
echo "📋 Phase 3: Checking client..."
check_services_parallel "client"

echo ""
echo "📊 Final Cluster Status:"
echo "========================"
docker compose ps

echo ""
echo "🎉 MooseFS Demo Started Successfully with Parallel Health Checks!"
echo "=================================================================="
echo ""
echo "📋 Service Endpoints:"
echo "-------------------"
echo "🔧 Master:          http://localhost:9419 (Client) | http://localhost:9420 (ChunkServer) | http://localhost:9421 (Admin/CGI)"
echo "💾 ChunkServer 1:   http://localhost:9422"
echo "💾 ChunkServer 2:   http://localhost:9432" 
echo "💾 ChunkServer 3:   http://localhost:9442"
echo "🌐 CGI Server:      http://localhost:9425"
echo "🖥️  Client Mount:    ./mnt/mfs"
echo ""
echo "🔍 Useful Commands:"
echo "------------------"
echo "View logs:         docker compose logs -f [service_name]"
echo "Check status:      docker compose ps"
echo "Stop cluster:      docker compose down"
echo "Clean up:          docker compose down -v (removes volumes)"
echo ""
echo "✨ Startup time optimized with parallel health checks!"