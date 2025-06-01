#!/bin/bash
# MooseNG Docker Demo Stop Script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "🛑 Stopping MooseNG Docker Demo"
echo "==============================="

# Check if there are any running containers
if docker compose ps --services --filter "status=running" | grep -q .; then
    echo "📊 Current running services:"
    docker compose ps
    
    echo ""
    echo "🛑 Stopping all services..."
    docker compose down
    
    echo ""
    echo "✅ All services stopped!"
else
    echo "ℹ️  No services are currently running"
fi

echo ""
echo "💡 Options:"
echo "  - To remove all data volumes: docker compose down -v"
echo "  - To start again: ./start-demo.sh"
echo ""