#!/bin/bash
# Automated MooseNG Demo Script for Video Recording
# This script automates the demo with pauses for recording

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print with color and pause
demo_echo() {
    echo -e "${GREEN}$1${NC}"
    sleep 2
}

# Function to run command with echo
demo_run() {
    echo -e "${BLUE}$ $1${NC}"
    sleep 1
    eval $1
    sleep 3
}

# Clear screen
clear

demo_echo "=== MooseNG Docker Demo ==="
demo_echo "Welcome to the MooseNG distributed storage demo!"
sleep 3

demo_echo "\n📁 First, let's look at our demo directory structure..."
demo_run "ls -la"

demo_echo "\n📋 Let's examine our Docker Compose configuration..."
demo_run "head -20 docker-compose.yml"

demo_echo "\n🚀 Now, let's start the entire MooseNG cluster..."
demo_run "./start-demo.sh"

demo_echo "\n✅ Great! All services are starting up."
demo_echo "Let's wait a moment for everything to initialize..."
sleep 5

demo_echo "\n🧪 Now let's verify that all services are healthy..."
demo_run "./test-demo.sh"

demo_echo "\n📊 Let's check the Docker containers..."
demo_run "docker compose ps"

demo_echo "\n🌐 Here are our service endpoints:"
cat << EOF
Masters:
  - Master 1: http://localhost:9421
  - Master 2: http://localhost:9431  
  - Master 3: http://localhost:9441

Chunkservers:
  - Chunkserver 1: http://localhost:9420
  - Chunkserver 2: http://localhost:9450
  - Chunkserver 3: http://localhost:9460

Monitoring:
  - Prometheus: http://localhost:9090
  - Grafana: http://localhost:3000 (admin/admin)
EOF

sleep 5

demo_echo "\n🔍 Let's check Prometheus targets..."
echo -e "${YELLOW}Please open http://localhost:9090/targets in your browser${NC}"
sleep 5

demo_echo "\n📊 Now let's explore Grafana dashboards..."
echo -e "${YELLOW}Please open http://localhost:3000 in your browser${NC}"
echo -e "${YELLOW}Login with admin/admin${NC}"
sleep 10

demo_echo "\n📈 You can view various metrics including:"
echo "  - Cluster health status"
echo "  - Storage capacity and usage"
echo "  - Request rates and latency"
echo "  - Node performance metrics"
sleep 5

demo_echo "\n🛑 Finally, let's stop the demo..."
demo_run "./stop-demo.sh"

demo_echo "\n✅ Demo complete!"
demo_echo "Thank you for watching the MooseNG Docker demo!"
demo_echo "\nFor more information, check out our README and documentation."