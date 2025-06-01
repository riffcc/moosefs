#!/bin/bash
# Start MooseNG Working Demo with Mock Services

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/mooseng"

echo "🚀 Starting MooseNG Working Demo..."
echo "=================================="
echo
echo "This demo uses mock services that simulate the MooseNG architecture:"
echo "- 3 Master servers (Raft HA simulation)"
echo "- 3 ChunkServers (4 storage dirs each)"
echo "- 3 Clients (simulated FUSE mounts)"
echo "- Prometheus + Grafana monitoring"
echo

# Create mount directories
echo "📁 Creating mount directories..."
mkdir -p mnt/client-{1,2,3}

# Build and start services
echo "🔨 Building mock services..."
docker-compose -f docker-compose.working-demo.yml build

echo
echo "🚀 Starting services..."
docker-compose -f docker-compose.working-demo.yml up -d

# Wait for services to be ready
echo
echo "⏳ Waiting for services to start..."
sleep 10

# Check service health
echo
echo "🏥 Checking service health..."
echo

# Check masters
echo "Masters:"
for i in 1 2 3; do
    port=$((9421 + (i-1)*10))
    if curl -s "http://localhost:$port/health" >/dev/null 2>&1; then
        status=$(curl -s "http://localhost:$port/status" | grep -o '"role":"[^"]*"' | cut -d'"' -f4)
        echo "  ✅ Master-$i: healthy (role: $status)"
    else
        echo "  ❌ Master-$i: not responding"
    fi
done

echo
echo "ChunkServers:"
for i in 1 2 3; do
    port=$((9420 + (i-1)*40))
    if curl -s "http://localhost:$port/health" >/dev/null 2>&1; then
        chunks=$(curl -s "http://localhost:$port/status" | grep -o '"chunks":[0-9]*' | cut -d':' -f2)
        echo "  ✅ ChunkServer-$i: healthy (chunks: $chunks)"
    else
        echo "  ❌ ChunkServer-$i: not responding"
    fi
done

echo
echo "Clients:"
for i in 1 2 3; do
    port=$((9427 + (i-1)*10))
    if curl -s "http://localhost:$port/health" >/dev/null 2>&1; then
        mounted=$(curl -s "http://localhost:$port/status" | grep -o '"mounted":[^,]*' | cut -d':' -f2)
        echo "  ✅ Client-$i: healthy (mounted: $mounted)"
    else
        echo "  ❌ Client-$i: not responding"
    fi
done

echo
echo "📊 Service URLs:"
echo "==============="
echo "Masters:"
echo "  - Master-1: http://localhost:9421/status"
echo "  - Master-2: http://localhost:9431/status"  
echo "  - Master-3: http://localhost:9441/status"
echo
echo "ChunkServers:"
echo "  - ChunkServer-1: http://localhost:9420/status"
echo "  - ChunkServer-2: http://localhost:9450/status"
echo "  - ChunkServer-3: http://localhost:9460/status"
echo
echo "Clients:"
echo "  - Client-1: http://localhost:9427/status"
echo "  - Client-2: http://localhost:9437/status"
echo "  - Client-3: http://localhost:9447/status"
echo
echo "Monitoring:"
echo "  - Prometheus: http://localhost:9090"
echo "  - Grafana: http://localhost:3000 (admin/admin)"
echo

echo "🎯 Demo Commands:"
echo "================="
echo "# View logs"
echo "docker-compose -f docker-compose.working-demo.yml logs -f"
echo
echo "# Check cluster status"
echo "curl http://localhost:9421/status | jq"
echo
echo "# List chunk servers"
echo "curl http://localhost:9421/api/v1/chunkservers | jq"
echo
echo "# Check metrics"
echo "curl http://localhost:9421/metrics"
echo
echo "# Stop demo"
echo "docker-compose -f docker-compose.working-demo.yml down"
echo
echo "✅ MooseNG Working Demo is running!"