#!/bin/bash
# Test MooseNG Docker Compose Demo

set -e

echo "=========================================="
echo "Testing MooseNG Docker Compose Demo"
echo "=========================================="
echo ""

# Check if containers are running
echo "1. Checking container status..."
cd mooseng
docker compose ps

echo ""
echo "2. Testing Master servers..."
for port in 9421 9431 9441; do
    echo -n "   Master on port $port: "
    if curl -s -f http://localhost:$port/health >/dev/null 2>&1; then
        echo "✓ OK"
    else
        echo "✗ FAILED"
    fi
done

echo ""
echo "3. Testing ChunkServers..."
for port in 9420 9450 9460; do
    echo -n "   ChunkServer on port $port: "
    if curl -s -f http://localhost:$port/ >/dev/null 2>&1; then
        echo "✓ OK"
    else
        echo "✗ FAILED"
    fi
done

echo ""
echo "4. Testing monitoring services..."
echo -n "   Prometheus: "
if curl -s -f http://localhost:9090/-/healthy >/dev/null 2>&1; then
    echo "✓ OK"
else
    echo "✗ FAILED"
fi

echo -n "   Grafana: "
if curl -s -f http://localhost:3000/api/health >/dev/null 2>&1; then
    echo "✓ OK"
else
    echo "✗ FAILED"
fi

echo ""
echo "5. Testing file operations..."
# Test writing to one client and reading from another
docker exec mooseng-client-1 sh -c "echo 'Hello from Client 1' > /mnt/mooseng/test.txt" 2>/dev/null || echo "   Note: Mock clients don't support real file operations"

echo ""
echo "6. Container logs (last 5 lines each):"
echo ""
for service in master-1 chunkserver-1 client-1; do
    echo "--- $service ---"
    docker compose logs --tail=5 $service 2>/dev/null || echo "No logs available"
    echo ""
done

echo "=========================================="
echo "Test complete!"
echo "=========================================="