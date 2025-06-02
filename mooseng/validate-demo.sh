#!/bin/bash

# MooseNG Demo Validation Script
# Validates the Docker Compose setup before running

set -e

echo "🔍 MooseNG Demo Validation"
echo "=========================="

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

ERRORS=0

check_result() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✅ $2${NC}"
    else
        echo -e "${RED}❌ $2${NC}"
        ERRORS=$((ERRORS + 1))
    fi
}

warn() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

info() {
    echo -e "ℹ️  $1"
}

# Check 1: Docker availability
echo ""
info "Checking Docker availability..."
docker version > /dev/null 2>&1
check_result $? "Docker is available"

# Check 2: Docker Compose availability
if docker compose version > /dev/null 2>&1; then
    check_result 0 "Docker Compose (v2) is available"
elif command -v docker-compose > /dev/null 2>&1; then
    check_result 0 "Docker Compose (v1) is available"
else
    check_result 1 "Docker Compose is not available"
fi

# Check 3: Docker daemon running
docker info > /dev/null 2>&1
check_result $? "Docker daemon is running"

# Check 4: Sufficient disk space (check for at least 10GB)
available_space=$(df . | awk 'NR==2 {print $4}')
if [ "$available_space" -gt 10485760 ]; then  # 10GB in KB
    check_result 0 "Sufficient disk space available (>10GB)"
else
    check_result 1 "Insufficient disk space (need at least 10GB)"
fi

# Check 5: Docker Compose file validation
echo ""
info "Validating Docker Compose configuration..."
docker compose config --quiet
check_result $? "Docker Compose file is valid"

# Check 6: Required files exist
echo ""
info "Checking required files..."

required_files=(
    "docker-compose.yml"
    "docker/Dockerfile.master"
    "docker/Dockerfile.chunkserver" 
    "docker/Dockerfile.client"
    "docker/configs/master.toml"
    "docker/configs/chunkserver.toml"
    "docker/configs/client.toml"
    "docker/scripts/master-entrypoint.sh"
    "docker/scripts/chunkserver-entrypoint.sh"
    "docker/scripts/client-entrypoint.sh"
    "start-demo.sh"
)

for file in "${required_files[@]}"; do
    if [ -f "$file" ]; then
        check_result 0 "File exists: $file"
    else
        check_result 1 "Missing file: $file"
    fi
done

# Check 7: Port availability
echo ""
info "Checking port availability..."

ports=(9090 3000 8080 9421 9431 9441 9420 9450 9460 9423 9433 9443 9425 9455 9465 9427 9437 9447)

for port in "${ports[@]}"; do
    if ! netstat -tuln 2>/dev/null | grep -q ":$port "; then
        check_result 0 "Port $port is available"
    else
        warn "Port $port is already in use - may cause conflicts"
    fi
done

# Check 8: FUSE support (for client containers)
echo ""
info "Checking FUSE support..."
if [ -c /dev/fuse ]; then
    check_result 0 "FUSE device available at /dev/fuse"
else
    warn "FUSE device not found - client containers need privileged mode"
fi

# Check 9: System resources
echo ""
info "Checking system resources..."

# Check available memory (need at least 4GB)
if command -v free > /dev/null 2>&1; then
    available_mem=$(free -m | awk 'NR==2{printf "%.0f", $7}')
    if [ "$available_mem" -gt 4096 ]; then
        check_result 0 "Sufficient memory available (>4GB)"
    else
        warn "Limited memory available: ${available_mem}MB (recommend 4GB+)"
    fi
elif command -v vm_stat > /dev/null 2>&1; then
    # macOS
    free_pages=$(vm_stat | grep "Pages free" | awk '{print $3}' | sed 's/\.//')
    free_mb=$((free_pages * 4096 / 1024 / 1024))
    if [ "$free_mb" -gt 4096 ]; then
        check_result 0 "Sufficient memory available (>4GB)"
    else
        warn "Limited memory available: ${free_mb}MB (recommend 4GB+)"
    fi
fi

# Summary
echo ""
echo "=========================="
if [ $ERRORS -eq 0 ]; then
    echo -e "${GREEN}🎉 All checks passed! Demo is ready to run.${NC}"
    echo ""
    echo "To start the demo:"
    echo "  ./start-demo.sh"
    echo ""
    echo "To get help:"
    echo "  ./start-demo.sh help"
    exit 0
else
    echo -e "${RED}❌ $ERRORS validation errors found.${NC}"
    echo ""
    echo "Please fix the issues above before running the demo."
    exit 1
fi