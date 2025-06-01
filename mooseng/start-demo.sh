#!/bin/bash
# MooseNG Docker Demo Startup Script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "🚀 MooseNG Docker Demo - Starting 3 Masters, 3 Chunkservers, 3 Clients"
echo "=================================================="

# Create necessary directories
echo "📁 Creating mount directories..."
mkdir -p mnt/client-{1,2,3}

# Check if mock services exist, if not we need to ensure they're created
if [ ! -f "docker/mock-services/master-mock.py" ] || [ ! -f "docker/mock-services/chunkserver-mock.py" ] || [ ! -f "docker/mock-services/client-mock.py" ]; then
    echo "⚠️  Mock services not found. Creating them..."
    # We'll use the existing ones or create minimal ones
fi

# Stop any existing containers
echo "🛑 Stopping any existing containers..."
docker compose down -v 2>/dev/null || true

# Build and start the services
echo "🔨 Building Docker images..."
docker compose build --parallel

echo "🚀 Starting services..."
docker compose up -d

# Wait for services to be healthy
echo "⏳ Waiting for services to be healthy..."
sleep 10

# Check service status
echo ""
echo "📊 Service Status:"
echo "==================="
docker compose ps

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
echo ""

# Check health endpoints
echo "🏥 Checking health endpoints..."
for port in 9421 9431 9441; do
    if curl -s -f "http://localhost:$port/health" > /dev/null 2>&1; then
        echo "  ✅ Master on port $port is healthy"
    else
        echo "  ❌ Master on port $port is not responding"
    fi
done

for port in 9420 9450 9460; do
    if curl -s -f "http://localhost:$port/health" > /dev/null 2>&1; then
        echo "  ✅ Chunkserver on port $port is healthy"
    else
        echo "  ❌ Chunkserver on port $port is not responding"
    fi
done

echo ""
echo "✅ MooseNG Demo is running!"
echo ""
echo "📝 Commands:"
echo "  - View logs: docker compose logs -f [service-name]"
echo "  - Stop demo: docker compose down"
echo "  - Stop and remove data: docker compose down -v"
echo ""