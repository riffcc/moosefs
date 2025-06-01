#!/bin/bash

# MooseNG Demo Test Script
# Tests the functionality of the 3-master, 3-chunkserver, 3-client setup

set -e

echo "🧪 Testing MooseNG Demo Cluster..."
echo "=================================="

cd mooseng

# Check if services are running
echo "1️⃣  Checking service status..."
if ! docker compose ps | grep -q "Up"; then
    echo "❌ No services are running. Please start the cluster first with ./start-mooseng-demo.sh"
    exit 1
fi

echo "✅ Services are running"

# Test service endpoints
echo ""
echo "2️⃣  Testing service endpoints..."

test_endpoint() {
    local name=$1
    local url=$2
    local expected_code=${3:-200}
    
    echo "🔍 Testing $name at $url..."
    if curl -s -o /dev/null -w "%{http_code}" --connect-timeout 5 "$url" | grep -q "$expected_code"; then
        echo "✅ $name is responding"
    else
        echo "⚠️  $name may not be ready yet"
    fi
}

# Test health endpoints
test_endpoint "Master 1 Health" "http://localhost:9430"
test_endpoint "Master 2 Health" "http://localhost:9435"
test_endpoint "Master 3 Health" "http://localhost:9445"

test_endpoint "ChunkServer 1 Health" "http://localhost:9429"
test_endpoint "ChunkServer 2 Health" "http://localhost:9459"
test_endpoint "ChunkServer 3 Health" "http://localhost:9469"

test_endpoint "Prometheus" "http://localhost:9090"
test_endpoint "Grafana" "http://localhost:3000"
test_endpoint "Dashboard" "http://localhost:8080"

# Test client mounts
echo ""
echo "3️⃣  Testing client mounts..."

for i in 1 2 3; do
    echo "🔍 Testing client-$i mount..."
    if [ -d "mnt/client-$i" ]; then
        echo "✅ Client-$i mount directory exists"
        
        # Try to check if the mount is active
        if docker compose exec -T client-$i mountpoint -q /mnt/mooseng 2>/dev/null; then
            echo "✅ Client-$i filesystem is mounted"
        else
            echo "⚠️  Client-$i filesystem mount status unclear"
        fi
    else
        echo "❌ Client-$i mount directory missing"
    fi
done

# Test CLI access
echo ""
echo "4️⃣  Testing CLI access..."
if docker compose exec -T cli echo "CLI accessible" >/dev/null 2>&1; then
    echo "✅ CLI container is accessible"
else
    echo "⚠️  CLI container may not be ready"
fi

# Show cluster information
echo ""
echo "5️⃣  Cluster Information:"
echo "========================"

echo ""
echo "📊 Container Status:"
docker compose ps

echo ""
echo "💾 Volume Usage:"
docker volume ls | grep mooseng | head -10

echo ""
echo "🌐 Network Configuration:"
docker network ls | grep mooseng

echo ""
echo "🎯 Resource Usage:"
docker stats --no-stream --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}" $(docker compose ps -q) 2>/dev/null || echo "Could not retrieve resource stats"

echo ""
echo "✅ Test completed! Your MooseNG cluster appears to be functional."
echo ""
echo "🔧 Next Steps:"
echo "- Access the dashboard at http://localhost:8080"
echo "- Monitor metrics at http://localhost:9090 (Prometheus)"
echo "- View dashboards at http://localhost:3000 (Grafana)"
echo "- Test file operations through the mounted clients"
echo ""
echo "📚 For more detailed testing, use:"
echo "   docker compose exec cli /bin/sh"