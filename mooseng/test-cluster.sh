#!/bin/bash
# Test script for MooseNG cluster

echo "🔧 Testing MooseNG Cluster Setup"
echo "================================="

# Function to check if container is healthy
check_health() {
    local service=$1
    local container_name="mooseng-$service"
    
    echo -n "Checking $service... "
    if docker inspect "$container_name" &>/dev/null; then
        health=$(docker inspect --format='{{.State.Health.Status}}' "$container_name" 2>/dev/null)
        status=$(docker inspect --format='{{.State.Status}}' "$container_name")
        
        if [ "$status" = "running" ]; then
            if [ "$health" = "healthy" ] || [ "$health" = "" ]; then
                echo "✅ Running"
            else
                echo "⚠️  Running but unhealthy ($health)"
            fi
        else
            echo "❌ Not running ($status)"
        fi
    else
        echo "❌ Container not found"
    fi
}

# Wait for services to start
echo "Waiting for services to initialize..."
sleep 30

echo -e "\n📊 Service Status:"
echo "=================="
check_health "master-1"
check_health "master-2" 
check_health "master-3"
check_health "chunkserver-1"
check_health "chunkserver-2"
check_health "chunkserver-3"
check_health "client-1"
check_health "client-2"
check_health "client-3"

echo -e "\n🌐 Network Connectivity:"
echo "========================"
# Test master-to-master connectivity (Raft)
echo -n "Master Raft cluster... "
if docker exec mooseng-master-1 nc -z master-2 9422 && \
   docker exec mooseng-master-1 nc -z master-3 9422; then
    echo "✅ Connected"
else
    echo "❌ Failed"
fi

# Test chunkserver to master connectivity
echo -n "Chunkserver to masters... "
if docker exec mooseng-chunkserver-1 nc -z master-1 9421 && \
   docker exec mooseng-chunkserver-1 nc -z master-2 9421 && \
   docker exec mooseng-chunkserver-1 nc -z master-3 9421; then
    echo "✅ Connected"
else
    echo "❌ Failed"
fi

echo -e "\n📈 Monitoring Stack:"
echo "===================="
echo -n "Prometheus... "
if curl -s http://localhost:9090/api/v1/status/config >/dev/null 2>&1; then
    echo "✅ Available at http://localhost:9090"
else
    echo "❌ Not accessible"
fi

echo -n "Grafana... "
if curl -s http://localhost:3000 >/dev/null 2>&1; then
    echo "✅ Available at http://localhost:3000 (admin/admin)"
else
    echo "❌ Not accessible"
fi

echo -n "Dashboard... "
if curl -s http://localhost:8080 >/dev/null 2>&1; then
    echo "✅ Available at http://localhost:8080"
else
    echo "❌ Not accessible"
fi

echo -e "\n🔧 MooseNG Cluster Summary:"
echo "=========================="
echo "Masters: 3 instances with Raft consensus"
echo "Chunkservers: 3 instances with 4 storage disks each"
echo "Clients: 3 FUSE mount instances"
echo "Monitoring: Prometheus + Grafana + Custom Dashboard"
echo ""
echo "Access URLs:"
echo "- Prometheus: http://localhost:9090"
echo "- Grafana: http://localhost:3000 (admin/admin)"
echo "- Dashboard: http://localhost:8080"
echo ""
echo "Master API ports:"
echo "- Master-1: localhost:9421"
echo "- Master-2: localhost:9431" 
echo "- Master-3: localhost:9441"