#!/bin/bash
# Start MooseNG Docker Compose Demo

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=========================================="
echo "Starting MooseNG Docker Compose Demo"
echo "=========================================="
echo ""

# Stop any existing demo
echo "Stopping any existing demo containers..."
cd mooseng
docker compose down --remove-orphans 2>/dev/null || true

# Start the new demo
echo ""
echo "Starting MooseNG demo with:"
echo "- 3 Master servers (with Raft consensus)"
echo "- 3 ChunkServers (each with 4 storage volumes)"
echo "- 3 Clients (with FUSE mounts)"
echo "- Monitoring (Prometheus + Grafana)"
echo "- Dashboard (Web UI)"
echo ""

docker compose up -d --build

echo ""
echo "Waiting for services to be ready..."
sleep 10

echo ""
echo "=========================================="
echo "MooseNG Demo is starting!"
echo "=========================================="
echo ""
echo "Services:"
echo "- Masters: http://localhost:9421, :9431, :9441"
echo "- ChunkServers: http://localhost:9420, :9450, :9460"
echo "- Dashboard: http://localhost:8080"
echo "- Prometheus: http://localhost:9090"
echo "- Grafana: http://localhost:3000 (admin/admin)"
echo ""
echo "To view logs:"
echo "  docker compose logs -f"
echo ""
echo "To stop the demo:"
echo "  cd mooseng && docker compose down"
echo ""
echo "To test the demo:"
echo "  ./test-mooseng.sh"
echo ""