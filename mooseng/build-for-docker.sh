#!/bin/bash
# Build script to help with compilation errors and Docker builds

set -e

echo "🔧 Building MooseNG for Docker deployment..."

# Change to mooseng directory
cd "$(dirname "$0")"

# First, try to fix the most critical compilation errors
echo "📋 Checking compilation status..."

# Try building just the common lib first
echo "🔨 Building mooseng-common..."
if ! cargo check -p mooseng-common; then
    echo "❌ mooseng-common has compilation errors"
    exit 1
fi

echo "✅ mooseng-common builds successfully"

# Try building protocol
echo "🔨 Building mooseng-protocol..."
if ! cargo check -p mooseng-protocol; then
    echo "❌ mooseng-protocol has compilation errors"
    exit 1
fi

echo "✅ mooseng-protocol builds successfully"

# Try building each component individually
components=("mooseng-chunkserver" "mooseng-client" "mooseng-metalogger" "mooseng-cli")

for component in "${components[@]}"; do
    echo "🔨 Building $component..."
    if cargo check -p "$component"; then
        echo "✅ $component builds successfully"
    else
        echo "⚠️  $component has compilation errors (non-critical for Docker)"
    fi
done

# Try building master last (it has the most errors)
echo "🔨 Building mooseng-master..."
if cargo check -p mooseng-master; then
    echo "✅ mooseng-master builds successfully"
    echo "🎉 All components build successfully!"
    echo "📦 Ready for Docker build"
else
    echo "❌ mooseng-master has compilation errors"
    echo "🏗️  Docker build may fail, but will continue with development setup"
fi

echo ""
echo "🐳 Docker deployment commands:"
echo "  Development setup (single master):  docker-compose -f docker-compose.dev.yml up -d"
echo "  Production setup (HA cluster):     docker-compose up -d"
echo "  Build only:                        docker-compose build"
echo ""
echo "📊 Health check endpoints:"
echo "  Master:        http://localhost:9430/health"
echo "  Chunkserver-1: http://localhost:9429/health"
echo "  Chunkserver-2: http://localhost:9451/health"
echo "  Chunkserver-3: http://localhost:9461/health"
echo ""
echo "🔧 Management:"
echo "  CLI access:    docker exec -it mooseng-cli-dev /bin/sh"
echo "  Logs:          docker-compose logs -f"
echo "  Stop:          docker-compose down"
echo "  Clean:         docker-compose down -v"