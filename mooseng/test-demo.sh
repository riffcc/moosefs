#!/bin/bash
# MooseNG Docker Demo Test Script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "🧪 MooseNG Docker Demo Test"
echo "==========================="

# Function to check endpoint
check_endpoint() {
    local name=$1
    local url=$2
    if curl -s -f "$url" > /dev/null 2>&1; then
        echo "  ✅ $name is responding at $url"
        return 0
    else
        echo "  ❌ $name is NOT responding at $url"
        return 1
    fi
}

# Check if services are running
echo ""
echo "📊 Checking Docker services..."
if docker compose ps --services | grep -q master-1; then
    echo "  ✅ Docker services are defined"
else
    echo "  ❌ Docker services not found. Run ./start-demo.sh first!"
    exit 1
fi

# Get running services
RUNNING_SERVICES=$(docker compose ps --services --filter "status=running" | wc -l)
TOTAL_SERVICES=$(docker compose ps --services | wc -l)
echo "  📈 Running services: $RUNNING_SERVICES/$TOTAL_SERVICES"

# Check master health endpoints
echo ""
echo "🔍 Testing Master Servers..."
MASTERS_OK=0
for i in 1 2 3; do
    PORT=$((9420 + (i-1)*10 + 1))
    if check_endpoint "Master $i" "http://localhost:$PORT/health"; then
        ((MASTERS_OK++))
    fi
done
echo "  📊 Masters healthy: $MASTERS_OK/3"

# Check chunkserver health endpoints
echo ""
echo "🔍 Testing Chunk Servers..."
CHUNKS_OK=0
for i in 1 2 3; do
    PORT=$((9420 + (i-1)*30))
    if [ $i -eq 1 ]; then PORT=9420; fi
    if [ $i -eq 2 ]; then PORT=9450; fi
    if [ $i -eq 3 ]; then PORT=9460; fi
    if check_endpoint "Chunkserver $i" "http://localhost:$PORT/health"; then
        ((CHUNKS_OK++))
    fi
done
echo "  📊 Chunkservers healthy: $CHUNKS_OK/3"

# Check monitoring
echo ""
echo "🔍 Testing Monitoring Stack..."
check_endpoint "Prometheus" "http://localhost:9090/-/healthy"
check_endpoint "Grafana" "http://localhost:3000/api/health"
check_endpoint "Dashboard" "http://localhost:8080"

# Test Raft consensus (check if a leader is elected)
echo ""
echo "🗳️  Testing Raft Consensus..."
LEADER_FOUND=false
for i in 1 2 3; do
    PORT=$((9420 + (i-1)*10 + 1))
    if curl -s "http://localhost:$PORT/status" 2>/dev/null | grep -q "leader"; then
        echo "  ✅ Leader election successful (Master $i)"
        LEADER_FOUND=true
        break
    fi
done
if [ "$LEADER_FOUND" = false ]; then
    echo "  ⚠️  No leader elected yet (this may be normal during startup)"
fi

# Summary
echo ""
echo "📋 Test Summary:"
echo "================"
echo "  Masters: $MASTERS_OK/3 healthy"
echo "  Chunkservers: $CHUNKS_OK/3 healthy"
echo "  Total services running: $RUNNING_SERVICES/$TOTAL_SERVICES"

if [ $MASTERS_OK -eq 3 ] && [ $CHUNKS_OK -eq 3 ]; then
    echo ""
    echo "✅ All core services are healthy!"
    exit 0
else
    echo ""
    echo "⚠️  Some services are not healthy. Check logs with:"
    echo "   docker compose logs -f [service-name]"
    exit 1
fi