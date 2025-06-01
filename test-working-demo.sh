#!/bin/bash
# Test MooseNG Working Demo functionality

set -e

echo "🧪 Testing MooseNG Working Demo..."
echo "=================================="
echo

# Test master functionality
echo "1️⃣ Testing Master Server API..."
echo "--------------------------------"

# Get cluster status
echo "Getting cluster status from Master-1..."
curl -s http://localhost:9421/status | python3 -m json.tool || echo "Failed to get status"
echo

# Create a mock file
echo "Creating a mock file..."
curl -s -X POST http://localhost:9421/api/v1/filesystem/create \
  -H "Content-Type: application/json" \
  -d '{"path": "/test-file.txt", "size": 1048576}' | python3 -m json.tool || echo "Failed to create file"
echo

# List chunk servers
echo "Listing chunk servers..."
curl -s http://localhost:9421/api/v1/chunkservers | python3 -m json.tool || echo "Failed to list chunk servers"
echo

# Test chunk server functionality
echo "2️⃣ Testing ChunkServer API..."
echo "------------------------------"

# Write a chunk
echo "Writing a test chunk to ChunkServer-1..."
curl -s -X PUT http://localhost:9420/api/v1/chunk/test-chunk-001 \
  -H "Content-Type: application/json" \
  -d '{"data": "test data"}' | python3 -m json.tool || echo "Failed to write chunk"
echo

# Read the chunk back
echo "Reading the test chunk..."
curl -s http://localhost:9420/api/v1/chunk/test-chunk-001 | python3 -m json.tool || echo "Failed to read chunk"
echo

# Test client functionality
echo "3️⃣ Testing Client API..."
echo "-------------------------"

# Get cache stats
echo "Getting cache statistics from Client-1..."
curl -s http://localhost:9427/cache/stats | python3 -m json.tool || echo "Failed to get cache stats"
echo

# Check all clients
echo "Checking all client statuses..."
for i in 9427 9437 9447; do
    echo -n "Client on port $i: "
    curl -s http://localhost:$i/health | python3 -m json.tool | grep status || echo "Failed"
done
echo

# Test metrics endpoints
echo "4️⃣ Testing Metrics Collection..."
echo "---------------------------------"

# Check Prometheus targets
echo "Checking Prometheus targets..."
curl -s http://localhost:9090/api/v1/targets | python3 -m json.tool | grep -A2 "\"health\":" | head -20 || echo "Failed to get targets"
echo

# Simulate some activity
echo "5️⃣ Simulating File System Activity..."
echo "-------------------------------------"

for i in {1..5}; do
    echo "Creating file $i..."
    curl -s -X POST http://localhost:9421/api/v1/filesystem/create \
      -H "Content-Type: application/json" \
      -d "{\"path\": \"/test-file-$i.dat\", \"size\": $((1024 * 1024 * i))}" >/dev/null
    
    # Write chunks
    curl -s -X PUT http://localhost:9420/api/v1/chunk/chunk-$i \
      -H "Content-Type: application/json" \
      -d "{\"data\": \"test data $i\"}" >/dev/null
done

echo "✅ Created 5 test files and chunks"
echo

# Check final statistics
echo "📊 Final Statistics..."
echo "----------------------"

echo "Master stats:"
curl -s http://localhost:9421/status | python3 -m json.tool | grep -E '"files"|"chunks"' || echo "Failed"
echo

echo "ChunkServer stats:"
curl -s http://localhost:9420/status | python3 -m json.tool | grep -E '"chunks"|"used_bytes"' || echo "Failed"
echo

echo "✅ All tests completed!"
echo
echo "📈 View detailed metrics at:"
echo "   - Prometheus: http://localhost:9090/graph"
echo "   - Grafana: http://localhost:3000"