#!/bin/bash
# Build script for MooseNG Docker services

set -e

echo "Building MooseNG Docker services..."

# Build all Rust components first
echo "Building Rust components..."
cargo build --release --all

# Build Docker images
echo "Building Docker images..."
docker-compose build --parallel

echo "Build completed successfully!"
echo ""
echo "To start the services, run:"
echo "  docker-compose up -d"
echo ""
echo "To view logs:"
echo "  docker-compose logs -f"
echo ""
echo "To stop services:"
echo "  docker-compose down"