#!/bin/bash
# MooseFS Debian Package Builder Script
# Builds .deb packages for MooseFS with IPv6 support on Debian 12
# Author: Benjamin Arntzen <zorlin@gmail.com>

set -e

echo "=== MooseFS Debian Package Builder ==="
echo ""

# Create output directory
mkdir -p debian-packages

echo "Building MooseFS Debian packages with IPv6 support..."
echo "This will create .deb files in the debian-packages/ directory"
echo ""

# Build packages using Docker
echo "Starting Docker build..."
docker compose -f docker-compose.debian-builder.yml build moosefs-debian-builder

echo ""
echo "Running package build..."
docker compose -f docker-compose.debian-builder.yml run --rm moosefs-debian-builder

echo ""
echo "✓ Package build completed!"
echo ""
echo "Built packages are available in: debian-packages/"
ls -la debian-packages/

echo ""
echo "To serve packages via HTTP (for APT repository):"
echo "  docker compose -f docker-compose.debian-builder.yml up moosefs-package-server"
echo ""
echo "Then add to /etc/apt/sources.list:"
echo "  deb [trusted=yes] http://localhost:8080 ./"
echo ""
echo "And install with:"
echo "  apt update"
echo "  apt install moosefs-master moosefs-chunkserver moosefs-client moosefs-gui"
echo ""
echo "IPv6 features will be automatically enabled!"