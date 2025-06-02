# MooseNG Unified Demo Script Documentation

## Overview

The `start-unified-demo.sh` script is a comprehensive management tool for starting and managing a complete MooseNG cluster using Docker. It orchestrates 3 Master servers, 3 Chunkservers, 3 Clients, and a complete monitoring stack (Prometheus, Grafana, Dashboard).

## Script Architecture

### Core Components

1. **Configuration Management** (Lines 11-15)
2. **Logging Infrastructure** (Lines 17-54)
3. **Error Handling & Cleanup** (Lines 56-76)
4. **Prerequisite Validation** (Lines 77-154)
5. **Service Management** (Lines 156-362)
6. **Status & Monitoring** (Lines 364-456)
7. **User Interface** (Lines 458-674)
8. **Main Control Flow** (Lines 675-774)

## Detailed Command Analysis

### Initial Setup & Configuration

#### Script Header (Lines 1-10)
```bash
#!/bin/bash
set -euo pipefail  # Enhanced error handling
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"
```

**Purpose**: 
- Sets strict error handling mode (exit on error, undefined variables, pipe failures)
- Determines and changes to script directory for consistent execution context
- Ensures all relative paths work correctly regardless of where script is called from

#### Configuration Variables (Lines 11-22)
```bash
LOG_FILE="${SCRIPT_DIR}/demo-startup.log"
MAX_WAIT_TIME=300  # 5 minutes max wait
SERVICE_CHECK_INTERVAL=5
HEALTH_CHECK_TIMEOUT=30
```

**Purpose**: 
- Establishes logging destination with fallback to `/dev/null`
- Sets timeout parameters for service health checks
- Creates configurable intervals for polling service status

### Logging System (Lines 24-54)

#### Color Definitions (Lines 24-29)
```bash
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color
```

**Purpose**: ANSI color codes for improved terminal output readability

#### Logging Functions (Lines 32-54)
- `log()`: Success messages in green
- `warn()`: Warning messages in yellow  
- `error()`: Error messages in red to stderr
- `info()`: Informational messages in blue

**Key Features**:
- Timestamps all messages
- Dual output (console + log file)
- Graceful fallback if log file unavailable

### Error Handling & Cleanup (Lines 56-196)

#### Signal Traps (Lines 57-58)
```bash
trap 'handle_error $? $LINENO' ERR
trap 'cleanup_on_exit' EXIT
```

**Purpose**: 
- Automatic error reporting with exit code and line number
- Ensures cleanup runs on script exit (success or failure)

#### Enhanced Cleanup (Lines 157-196)
- **Container Cleanup**: Stops and removes all Docker containers and volumes
- **Resource Cleanup**: Optional Docker system pruning
- **Mount Point Cleanup**: Unmounts any leftover filesystem mounts
- **Orphan Removal**: Removes orphaned containers

### Prerequisite Validation (Lines 77-154)

#### Docker Availability Check (Lines 78-101)
```bash
check_docker() {
    if ! command -v docker >/dev/null 2>&1; then
        error "Docker command not found..."
    fi
    if ! docker info >/dev/null 2>&1; then
        error "Docker is not running..."
    fi
}
```

**Validation Steps**:
1. Verifies Docker command exists
2. Tests Docker daemon accessibility
3. Checks Docker version
4. Validates available disk space (warns if < 2GB)

#### Docker Compose Validation (Lines 103-129)
```bash
check_docker_compose() {
    # Version check
    # docker-compose.yml existence
    # Syntax validation
}
```

**Validation Steps**:
1. Confirms Docker Compose installation
2. Verifies `docker-compose.yml` file exists
3. Performs syntax validation of compose file

#### Port Availability Check (Lines 131-154)
```bash
check_port_availability() {
    local required_ports=("9090" "9421" "9422" "9423" "9431" "9441" "9420" "9450" "9460" "3000" "8080")
}
```

**Port Mapping**:
- **9090**: Prometheus
- **9421**: Master-1 Client API (9422: Raft, 9423: Metrics)
- **9431**: Master-2 Client API (9432: Raft, 9433: Metrics)
- **9441**: Master-3 Client API (9442: Raft, 9443: Metrics)
- **9420**: ChunkServer-1 API (9425: Metrics)
- **9450**: ChunkServer-2 API (9455: Metrics)
- **9460**: ChunkServer-3 API (9465: Metrics)
- **9427**: Client-1 (client-2 and client-3 use internal ports only)
- **3000**: Grafana
- **8080**: Dashboard

### Image Building (Lines 198-238)

#### Build Process (Lines 199-220)
```bash
build_images() {
    # Parallel build support detection
    # Progress tracking with tee
    # Build verification
}
```

**Features**:
- Automatic parallel build detection
- Real-time progress output to console and log
- Post-build image verification

### Service Startup Orchestration (Lines 240-362)

#### Startup Sequence (Lines 241-257)
```bash
start_services() {
    start_masters        # First - form Raft cluster
    start_chunkservers   # Second - register with masters
    start_clients        # Third - connect to cluster
    start_monitoring_stack # Last - monitoring overlay
}
```

**Critical Ordering**:
1. **Masters**: Must form Raft consensus cluster first
2. **Chunkservers**: Register with established master cluster
3. **Clients**: Connect to operational storage cluster
4. **Monitoring**: Overlays metrics collection on running cluster

#### Master Startup (Lines 259-282)
```bash
start_masters() {
    # Sequential master startup
    # Raft cluster formation wait
    # Leader election verification
}
```

**Raft Cluster Formation**:
- Starts all 3 masters sequentially
- Waits for basic container health
- Additional 10-second wait for leader election
- Verifies cluster health via HTTP endpoints

#### Health Verification (Lines 340-362)
```bash
verify_raft_cluster() {
    # Polls master health endpoints
    # Looks for "leader" or "healthy" status
    # 12 attempts with 5-second intervals
}
```

### Status & Monitoring (Lines 364-456)

#### Comprehensive Status Display (Lines 365-390)
```bash
show_status() {
    # Basic service status via docker-compose ps
    # Detailed service information table
    # Resource usage summary
    # Connectivity testing
}
```

#### Service Information Table (Lines 392-410)
Displays formatted table with:
- Service name
- Container status  
- Health check status
- Port mappings

#### Connectivity Testing (Lines 434-456)
Tests network connectivity to:
- Master API endpoints (9421, 9431, 9441)
- Monitoring endpoints (9090, 3000, 8080)
- Uses `nc` (netcat) for port connectivity verification

### User Interface (Lines 458-674)

#### Access Information Display (Lines 459-469)
Comprehensive information display including:
- Service endpoint URLs with live status
- Common management commands
- Mount point information
- Log locations
- Troubleshooting tips

#### Service Health Monitoring (Lines 558-673)
```bash
wait_for_service_group() {
    # Configurable timeout and interval
    # Per-service health tracking
    # Detailed progress reporting
}
```

**Health Check Logic**:
- Polls service status at configured intervals
- Categorizes services as healthy/starting/unhealthy
- Provides detailed progress feedback
- Continues with warnings if timeout exceeded

### Main Control Flow (Lines 675-774)

#### Command Processing (Lines 694-771)
```bash
case "${1:-start}" in
    "clean")    cleanup ;;
    "build")    build_images ;;
    "start")    # Full startup sequence
    "status")   show_status ;;
    "stop")     docker-compose down ;;
    "logs")     # Log viewing with optional service filter
    "health")   # Health check and connectivity test
    "restart")  # Service restart with optional targeting
esac
```

## Available Commands

### Primary Commands

#### `start` (Default)
**Full startup sequence**:
1. Pre-flight validation (Docker, Compose, ports)
2. Cleanup existing containers
3. Build fresh images
4. Start services in proper order
5. Wait for health confirmation
6. Display status and access information

#### `stop`
**Graceful shutdown**:
- Stops all containers
- Preserves volumes and networks
- Quick and clean shutdown

#### `clean`
**Complete cleanup**:
- Stops and removes all containers
- Removes volumes (data loss)
- Cleans up mount points
- Optional Docker system pruning

### Operational Commands

#### `status`
**Comprehensive status report**:
- Service container status
- Health check results
- Port mappings
- Resource usage summary
- Connectivity verification

#### `health`
**Detailed health analysis**:
- Per-service-type health report
- Network connectivity testing
- Component-specific status

#### `logs [service]`
**Log management**:
- View all logs: `./start-unified-demo.sh logs`
- Service-specific: `./start-unified-demo.sh logs master-1`
- Real-time following with `-f` flag

#### `restart [service]`
**Service restart**:
- All services: `./start-unified-demo.sh restart`
- Specific service: `./start-unified-demo.sh restart master-1`

#### `build`
**Image building only**:
- Builds Docker images without starting services
- Useful for development and testing
- Includes build verification

## Environment Variables

### Configuration Options

#### `CLEAN_DOCKER_SYSTEM=true`
**Purpose**: Enables Docker system pruning during cleanup
**Effect**: Removes unused images, networks, build cache
**Usage**: `CLEAN_DOCKER_SYSTEM=true ./start-unified-demo.sh clean`

#### `MAX_WAIT_TIME=300`
**Purpose**: Maximum time (seconds) to wait for services
**Default**: 300 seconds (5 minutes)
**Usage**: `MAX_WAIT_TIME=600 ./start-unified-demo.sh start`

## Service Architecture

### Master Servers (3 instances)
- **Purpose**: Metadata management and cluster coordination
- **Ports**: 
  - Master-1: 9421 (API), 9422 (Raft), 9423 (Metrics)
  - Master-2: 9431 (API), 9432 (Raft), 9433 (Metrics)  
  - Master-3: 9441 (API), 9442 (Raft), 9443 (Metrics)
- **Technology**: Raft consensus protocol
- **Startup**: Must achieve quorum before other services
- **Storage**: Persistent data volumes for metadata

### Chunk Servers (3 instances)  
- **Purpose**: Data storage and retrieval
- **Ports**:
  - ChunkServer-1: 9420 (API), 9425 (Metrics)
  - ChunkServer-2: 9450 (API), 9455 (Metrics)
  - ChunkServer-3: 9460 (API), 9465 (Metrics)
- **Dependencies**: Requires operational master cluster
- **Storage**: 4 disk volumes per instance for data distribution
- **Configuration**: Connects to all 3 masters for high availability

### Client Instances (3 instances)
- **Purpose**: File system interface and data access via FUSE
- **Ports**: Client-1: 9427 (others internal only)
- **Dependencies**: Requires operational master and chunk servers
- **Mount Points**: Internal `/mnt/mooseng` mount points
- **Privileges**: Requires privileged mode and `/dev/fuse` access for FUSE mounting
- **Cache**: Local cache volumes for performance optimization

### Monitoring Stack
- **Prometheus** (9090): Metrics collection and storage
- **Grafana** (3000): Visualization dashboard (admin/admin)
- **Dashboard** (8080): Custom MooseNG dashboard

## Troubleshooting Guide

### Common Startup Issues

#### 1. Port Conflicts
**Symptoms**: Services fail to start, "port already in use" errors

**Diagnosis**:
```bash
# Check port usage
./start-unified-demo.sh status
netstat -tulpn | grep -E ':(9090|9421|9431|9441|9420|9450|9460|3000|8080)'
lsof -i :9090  # Check specific port
```

**Solutions**:
- Stop conflicting services
- Change port mappings in `docker-compose.yml`
- Use different host ports

#### 2. Docker Build Failures
**Symptoms**: Image build fails, "No space left on device"

**Diagnosis**:
```bash
df -h                    # Check disk space
docker system df         # Check Docker space usage
docker images            # List images
```

**Solutions**:
```bash
# Clean Docker resources
docker system prune -f
docker image prune -a
CLEAN_DOCKER_SYSTEM=true ./start-unified-demo.sh clean

# Free up disk space
# Clean up large files, logs, temporary files
```

#### 3. Service Health Check Failures
**Symptoms**: Services start but never become healthy

**Diagnosis**:
```bash
# Check individual service logs
docker-compose logs master-1
docker-compose logs chunkserver-1

# Check service status
./start-unified-demo.sh health
./start-unified-demo.sh status

# Check container processes
docker exec -it mooseng-master-1 ps aux
```

**Solutions**:
```bash
# Increase wait timeouts
MAX_WAIT_TIME=600 ./start-unified-demo.sh start

# Restart specific problematic services
./start-unified-demo.sh restart master-1

# Check configuration files
docker exec -it mooseng-master-1 cat /config/master.toml
```

#### 4. Raft Cluster Formation Issues
**Symptoms**: Masters start but cannot form cluster consensus

**Diagnosis**:
```bash
# Check master logs for Raft messages
docker-compose logs master-1 | grep -i raft
docker-compose logs master-2 | grep -i raft
docker-compose logs master-3 | grep -i raft

# Test master connectivity
curl http://localhost:9421/health
curl http://localhost:9431/health  
curl http://localhost:9441/health
```

**Solutions**:
```bash
# Restart masters in sequence
./start-unified-demo.sh restart master-1
sleep 10
./start-unified-demo.sh restart master-2
sleep 10
./start-unified-demo.sh restart master-3

# Complete cluster restart
./start-unified-demo.sh stop
./start-unified-demo.sh clean
./start-unified-demo.sh start
```

#### 5. Permission Issues
**Symptoms**: "Permission denied" errors, Docker daemon inaccessible

**Diagnosis**:
```bash
docker info              # Test Docker access
groups $USER             # Check user groups
sudo docker info         # Test with sudo
```

**Solutions**:
```bash
# Add user to docker group (requires logout/login)
sudo usermod -aG docker $USER

# Start Docker daemon
sudo systemctl start docker      # Linux
# or start Docker Desktop         # macOS/Windows

# Run script with sudo (not recommended)
sudo ./start-unified-demo.sh start
```

#### 6. Memory/Resource Constraints
**Symptoms**: Services OOM killed, slow performance

**Diagnosis**:
```bash
# Check system resources
free -h                  # Memory usage
docker stats             # Container resource usage
./start-unified-demo.sh status  # Resource summary
```

**Solutions**:
```bash
# Increase Docker memory limits
# Modify docker-compose.yml memory constraints
# Close other applications
# Restart Docker daemon

# Scale down services temporarily
docker-compose up -d master-1 chunkserver-1 client-1
```

### Advanced Troubleshooting

#### Network Connectivity Issues
```bash
# Test container networking
docker network ls
docker network inspect mooseng_default

# Test inter-container connectivity  
docker exec -it mooseng-client-1 ping mooseng-master-1
docker exec -it mooseng-chunkserver-1 nc -z mooseng-master-1 9421
```

#### Configuration Problems
```bash
# Examine configuration files
docker exec -it mooseng-master-1 ls -la /config/
docker exec -it mooseng-chunkserver-1 cat /config/chunkserver.toml

# Check environment variables
docker exec -it mooseng-master-1 env | grep MOOSE
```

#### Data Persistence Issues
```bash
# Check Docker volumes
docker volume ls | grep mooseng
docker volume inspect mooseng_master-1-data
docker volume inspect mooseng_chunkserver-1-disk1

# Examine mounted data and storage structure
docker exec -it mooseng-master-1 ls -la /data/
docker exec -it mooseng-chunkserver-1 ls -la /data/disk1 /data/disk2 /data/disk3 /data/disk4

# Check client cache
docker exec -it mooseng-client-1 ls -la /cache/

# Verify FUSE mount inside client
docker exec -it mooseng-client-1 mountpoint /mnt/mooseng
docker exec -it mooseng-client-1 ls -la /mnt/mooseng/
```

#### Storage Configuration Issues
```bash
# Verify chunkserver storage configuration
docker exec -it mooseng-chunkserver-1 env | grep MOOSENG_DATA_DIRS
docker exec -it mooseng-chunkserver-2 env | grep MOOSENG_DATA_DIRS
docker exec -it mooseng-chunkserver-3 env | grep MOOSENG_DATA_DIRS

# Check storage space in each disk
docker exec -it mooseng-chunkserver-1 df -h /data/disk1 /data/disk2 /data/disk3 /data/disk4

# Verify disk permissions
docker exec -it mooseng-chunkserver-1 ls -la /data/
```

#### FUSE Mount Issues (Client-specific)
```bash
# Check if FUSE device is available
docker exec -it mooseng-client-1 ls -la /dev/fuse

# Check FUSE module on host
lsmod | grep fuse
modprobe fuse  # Load if missing

# Check client privileges
docker inspect mooseng-client-1 | grep -i privileged

# Test FUSE mount manually
docker exec -it mooseng-client-1 bash
# Inside container:
fusermount -u /mnt/mooseng  # Unmount if mounted
/usr/local/bin/mooseng-client --mount-point /mnt/mooseng --master-endpoints master-1:9421,master-2:9421,master-3:9421
```

#### Common Issue Scenarios

##### Scenario 1: "Cannot create log file" Warning
**Symptoms**: Script shows warning about log file creation
**Cause**: Insufficient permissions or disk space
**Solution**:
```bash
# Check permissions and create directory manually
mkdir -p "$(dirname /path/to/mooseng/demo-startup.log)"
touch /path/to/mooseng/demo-startup.log
chmod 644 /path/to/mooseng/demo-startup.log
```

##### Scenario 2: ChunkServer Cannot Connect to Masters
**Symptoms**: ChunkServers start but don't register with masters
**Diagnosis**:
```bash
# Check master availability from chunkserver
docker exec -it mooseng-chunkserver-1 ping master-1
docker exec -it mooseng-chunkserver-1 nc -z master-1 9421

# Check master endpoints configuration
docker exec -it mooseng-chunkserver-1 env | grep MASTER_ADDRS
```
**Solution**:
```bash
# Restart masters first, then chunkservers
./start-unified-demo.sh restart master-1 master-2 master-3
sleep 30
./start-unified-demo.sh restart chunkserver-1 chunkserver-2 chunkserver-3
```

##### Scenario 3: Client FUSE Mount Fails
**Symptoms**: Client starts but cannot mount filesystem
**Diagnosis**:
```bash
# Check FUSE availability
docker exec -it mooseng-client-1 ls -la /dev/fuse
# Should show: crw-rw-rw- 1 root root 10, 229 [date] /dev/fuse

# Check privileged mode
docker inspect mooseng-client-1 | jq '.[0].HostConfig.Privileged'
# Should return: true
```
**Solution**:
```bash
# Ensure FUSE module is loaded on host
sudo modprobe fuse

# Restart client with proper privileges
./start-unified-demo.sh restart client-1
```

##### Scenario 4: Grafana Shows "Data source not found"
**Symptoms**: Grafana loads but shows no data or connection errors
**Diagnosis**:
```bash
# Check Prometheus connectivity from Grafana
docker exec -it mooseng-grafana nc -z prometheus 9090

# Check Prometheus targets
curl http://localhost:9090/api/v1/targets
```
**Solution**:
```bash
# Restart monitoring stack in order
./start-unified-demo.sh restart prometheus
sleep 10
./start-unified-demo.sh restart grafana
```

##### Scenario 5: "No healthy masters found" Error
**Symptoms**: Services report no available masters
**Diagnosis**:
```bash
# Check Raft cluster status
for port in 9421 9431 9441; do
    echo "Testing master on port $port:"
    curl -s "http://localhost:$port/health" || echo "Master unreachable"
done

# Check Raft leader election
docker-compose logs master-1 master-2 master-3 | grep -i "leader\|election\|raft"
```
**Solution**:
```bash
# Sequential master restart for Raft recovery
./start-unified-demo.sh stop
./start-unified-demo.sh clean
./start-unified-demo.sh start
```

### Recovery Procedures

#### Complete Reset
```bash
# Full cleanup and restart (destroys all data)
./start-unified-demo.sh stop
./start-unified-demo.sh clean
docker system prune -f
docker volume prune -f  # WARNING: Destroys all data
./start-unified-demo.sh start
```

#### Partial Recovery
```bash
# Restart specific service group
docker-compose restart master-1 master-2 master-3
./start-unified-demo.sh health

# Rebuild problematic images
./start-unified-demo.sh build
./start-unified-demo.sh restart
```

#### Data Recovery (if volumes still exist)
```bash
# Stop services but preserve volumes
./start-unified-demo.sh stop

# Check volume integrity
docker volume ls | grep mooseng
docker volume inspect mooseng_master-1-data

# Restart with existing data
./start-unified-demo.sh start
# Data should be preserved across restarts
```

### Log Analysis

#### Script Logs
```bash
# View startup script logs
tail -f mooseng/demo-startup.log

# Search for specific errors
grep -i error mooseng/demo-startup.log
grep -i warning mooseng/demo-startup.log
```

#### Container Logs
```bash
# View all container logs
docker-compose logs -f

# Service-specific logs with timestamps
docker-compose logs -f -t master-1

# Filter logs for errors
docker-compose logs master-1 2>&1 | grep -i error
```

#### System Logs
```bash
# Docker daemon logs
journalctl -u docker.service -f     # Linux systemd
# or check Docker Desktop logs        # macOS/Windows

# System resource logs
dmesg | grep -i memory              # Memory-related issues
dmesg | grep -i docker              # Docker-related kernel messages
```

## Performance Optimization

### Resource Tuning
- Adjust memory limits in `docker-compose.yml`
- Configure CPU limits for balanced resource usage
- Use SSD storage for better I/O performance

### Network Optimization  
- Use Docker's bridge networking efficiently
- Configure proper network timeouts
- Monitor network bandwidth usage

### Monitoring Integration
- Use Prometheus metrics for performance insights
- Configure Grafana alerts for proactive monitoring
- Leverage custom dashboards for operational visibility

## Best Practices

### Development Workflow
1. Use `build` command for iterative development
2. Use `logs` command with service names for debugging
3. Use `health` command for systematic troubleshooting
4. Use `clean` command sparingly (destroys data)

### Production Considerations
- Set appropriate timeout values for your environment
- Monitor disk space regularly
- Implement proper backup strategies for Docker volumes
- Use external monitoring for production deployments

### Security Considerations
- Change default Grafana credentials (admin/admin)
- Secure network access to exposed ports
- Regularly update Docker images
- Implement proper access controls for Docker daemon

## Quick Reference

### Essential Commands
```bash
# Start complete cluster
./start-unified-demo.sh start

# Check cluster status
./start-unified-demo.sh status

# View all logs
./start-unified-demo.sh logs

# Restart specific service
./start-unified-demo.sh restart master-1

# Clean shutdown
./start-unified-demo.sh stop

# Complete cleanup (destroys data)
./start-unified-demo.sh clean
```

### Emergency Commands
```bash
# Force stop all containers
docker-compose down --remove-orphans

# Emergency cleanup
docker system prune -af
docker volume prune -f

# Check what's using ports
lsof -i :9421
netstat -tulpn | grep :9090
```

### Health Check Commands
```bash
# Quick health check
curl http://localhost:9421/health  # Master-1
curl http://localhost:9090/api/v1/targets  # Prometheus

# Service connectivity
docker exec -it mooseng-client-1 ping master-1

# Check service logs
docker-compose logs -f master-1
```

### File System Operations (from within client)
```bash
# Access client container
docker exec -it mooseng-client-1 bash

# Inside client container:
ls /mnt/mooseng/                    # List MooseNG filesystem
echo "test" > /mnt/mooseng/test.txt # Write test file
cat /mnt/mooseng/test.txt           # Read test file
df -h /mnt/mooseng/                 # Check filesystem usage
```

### Monitoring URLs
- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3000 (admin/admin)
- **Dashboard**: http://localhost:8080
- **Master APIs**: http://localhost:9421, http://localhost:9431, http://localhost:9441

### Environment Variables
```bash
# Extend service wait time
MAX_WAIT_TIME=600 ./start-unified-demo.sh start

# Enable Docker cleanup during clean operation
CLEAN_DOCKER_SYSTEM=true ./start-unified-demo.sh clean
```

---

This documentation provides comprehensive guidance for operating the MooseNG unified demo cluster. For additional support, consult the container logs and the script's built-in troubleshooting information.