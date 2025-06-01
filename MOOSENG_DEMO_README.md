# MooseNG Docker Demo

A comprehensive Docker Compose setup demonstrating a complete MooseNG distributed filesystem cluster.

## Architecture

This demo provides:
- **3 Master Servers** with Raft consensus for high availability
- **3 Chunk Servers** each with 4 data directories for storage distribution
- **3 Client Instances** with FUSE mounts for filesystem access
- **1 Metalogger** for metadata backup and recovery
- **CLI Tools** for cluster management
- **Monitoring Stack** (Prometheus + Grafana + Custom Dashboard)

## Quick Start

### 1. Start the Cluster
```bash
./start-mooseng-demo.sh
```

This will:
- Build all necessary Docker images
- Start all services with proper dependencies
- Wait for health checks to pass
- Display service endpoints

### 2. Test the Setup
```bash
./test-mooseng-demo.sh
```

This will validate that all services are responding and functional.

## Service Endpoints

### Master Servers (HA Cluster)
- **Master 1**: http://localhost:9421 (Client) | http://localhost:9422 (ChunkServer) | http://localhost:9430 (Health)
- **Master 2**: http://localhost:9431 (Client) | http://localhost:9432 (ChunkServer) | http://localhost:9435 (Health)  
- **Master 3**: http://localhost:9441 (Client) | http://localhost:9442 (ChunkServer) | http://localhost:9445 (Health)

### Chunk Servers (Distributed Storage)
- **ChunkServer 1**: http://localhost:9420 | http://localhost:9425 (Metrics) | http://localhost:9429 (Health)
- **ChunkServer 2**: http://localhost:9450 | http://localhost:9455 (Metrics) | http://localhost:9459 (Health)
- **ChunkServer 3**: http://localhost:9460 | http://localhost:9465 (Metrics) | http://localhost:9469 (Health)

### Client Access Points
- **Client 1**: http://localhost:9427 (Metrics) | Mount: `./mnt/client-1`
- **Client 2**: http://localhost:9437 (Metrics) | Mount: `./mnt/client-2`
- **Client 3**: http://localhost:9447 (Metrics) | Mount: `./mnt/client-3`

### Monitoring & Management
- **Prometheus**: http://localhost:9090 (Metrics collection)
- **Grafana**: http://localhost:3000 (admin/admin) (Visualization dashboards)
- **Dashboard**: http://localhost:8080 (Custom MooseNG dashboard)
- **CLI**: `docker-compose exec cli /bin/sh` (Management tools)

## Features Demonstrated

### High Availability
- Raft consensus among 3 master servers
- Automatic failover capabilities
- Distributed metadata management

### Scalable Storage
- 3 chunk servers with 4 data directories each
- Distributed chunk placement
- Erasure coding and self-healing capabilities

### Client Access
- Multiple FUSE-mounted clients
- Concurrent access to shared filesystem
- Client-side caching and optimization

### Advanced Features
- Health monitoring and metrics collection
- Zero-copy optimizations
- Tiered storage capabilities
- Comprehensive logging

## Testing the Filesystem

### Basic File Operations
```bash
# Access any client mount
cd mnt/client-1

# Create test files
echo "Hello from Client 1" > test1.txt
mkdir test-directory
echo "Shared data" > test-directory/shared.txt

# Verify from other clients
cat ../client-2/test1.txt
ls ../client-3/test-directory/
```

### CLI Management
```bash
# Access the CLI container
docker-compose exec cli /bin/sh

# Use MooseNG CLI tools (when available)
mooseng cluster status
mooseng storage info
mooseng health check
```

## CLI Tools Reference

The MooseNG CLI provides comprehensive management capabilities for your distributed filesystem cluster. Below is a complete reference for all available commands and their usage.

### CLI Tool Categories

The MooseNG CLI is organized into 6 main command categories:

1. **Cluster Management** (`mooseng cluster`)
2. **Administrative Operations** (`mooseng admin`)
3. **Monitoring and Status** (`mooseng monitor`)
4. **Configuration Management** (`mooseng config`)
5. **Data Operations** (`mooseng data`)
6. **Benchmarking** (`mooseng benchmark`)

### 1. Cluster Management (`mooseng cluster`)

**Purpose**: Manage cluster-wide operations and network topology

```bash
# Display cluster health and statistics
mooseng cluster status
mooseng cluster status --verbose
mooseng cluster status --json

# Initialize a new cluster
mooseng cluster init --name my-cluster --masters master-1:9421,master-2:9421,master-3:9421

# Join nodes to existing cluster  
mooseng cluster join --master master-1:9421 --node-type chunkserver
mooseng cluster join --master master-1:9421 --node-type client

# Remove nodes from cluster
mooseng cluster leave --node chunkserver-3 --graceful
mooseng cluster leave --node chunkserver-3 --force

# Scale cluster components
mooseng cluster scale --chunkservers 5
mooseng cluster scale --clients 10

# Perform rolling upgrades
mooseng cluster upgrade --version 2.1.0 --rolling
mooseng cluster upgrade --masters-first --wait-healthy

# Network topology management
mooseng cluster topology show
mooseng cluster topology discover --auto
mooseng cluster topology regions
mooseng cluster topology connections
mooseng cluster topology test --region us-east-1 --region us-west-2
```

### 2. Administrative Operations (`mooseng admin`)

**Purpose**: Administrative functions for cluster maintenance

```bash
# Manage chunk servers
mooseng admin add-chunkserver --address chunkserver-4:9420 --data-dirs /data1,/data2
mooseng admin remove-chunkserver --address chunkserver-4:9420 --migrate-data
mooseng admin list-chunkservers --detailed
mooseng admin maintenance --node chunkserver-2 --enable
mooseng admin maintenance --node chunkserver-2 --disable

# Data management
mooseng admin balance --dry-run
mooseng admin balance --execute --target-utilization 80
mooseng admin repair --chunk-id 12345 --verify-only
mooseng admin repair --all --auto-heal

# Storage classes
mooseng admin storage-class set --path /important --class premium
mooseng admin storage-class set --path /archive --class cold-storage
mooseng admin storage-class get --path /important

# Quota management
mooseng admin quota set --path /projects/team1 --size 100GB --files 10000
mooseng admin quota get --path /projects/team1
mooseng admin quota remove --path /projects/team1
mooseng admin quota list --all

# Health management
mooseng admin health check --all-nodes
mooseng admin health heal --chunk-id 12345
mooseng admin health heal --auto --damaged-only
mooseng admin health history --last-24h
mooseng admin health auto-heal --enable
mooseng admin health configure --check-interval 30s --heal-parallelism 5
```

### 3. Monitoring and Status (`mooseng monitor`)

**Purpose**: Real-time monitoring and alerting

```bash
# Show real-time cluster metrics
mooseng monitor metrics --live
mooseng monitor metrics --history 1h
mooseng monitor metrics --export json --output metrics.json

# Performance statistics
mooseng monitor stats --read-write
mooseng monitor stats --network
mooseng monitor stats --storage

# Health monitoring
mooseng monitor health --all
mooseng monitor health --masters
mooseng monitor health --chunkservers
mooseng monitor health --clients

# Operation monitoring  
mooseng monitor operations --operation read --live
mooseng monitor operations --operation write --top 10
mooseng monitor operations --operation metadata --slow

# Event monitoring
mooseng monitor events --follow
mooseng monitor events --filter error
mooseng monitor events --since 1h

# Export metrics
mooseng monitor export --format prometheus --output metrics.prom
mooseng monitor export --format csv --output metrics.csv
mooseng monitor export --format json --output metrics.json
```

### 4. Configuration Management (`mooseng config`)

**Purpose**: Manage CLI and cluster configurations

```bash
# Display current configuration
mooseng config show
mooseng config show --section cluster
mooseng config show --section storage

# Set/get configuration values
mooseng config set cluster.name "production-cluster"
mooseng config set storage.default-class "standard"
mooseng config get cluster.name
mooseng config get storage.default-class

# Configuration management
mooseng config reset --section cluster
mooseng config reset --all
mooseng config validate
mooseng config export --output cluster-config.yaml
mooseng config import --input cluster-config.yaml

# Storage class management
mooseng config storage-class list
mooseng config storage-class create premium --copies 3 --erasure "8+4" --tier fast
mooseng config storage-class create archive --copies 1 --erasure "8+4" --tier cold
mooseng config storage-class modify premium --tier nvme
mooseng config storage-class delete test-class

# CLI client configuration
mooseng config client show
mooseng config client set-masters master-1:9421,master-2:9421,master-3:9421
mooseng config client set-timeout --connect 10s --request 30s
mooseng config client set-tls --enable --cert-file client.crt --key-file client.key
mooseng config client set-retry --max-attempts 3 --backoff exponential
mooseng config client reset
mooseng config client test --verbose
```

### 5. Data Operations (`mooseng data`)

**Purpose**: File and directory operations within MooseNG

```bash
# Upload/download files
mooseng data upload /local/path /remote/path --recursive
mooseng data upload /local/file.txt /remote/file.txt --compress --storage-class premium
mooseng data download /remote/path /local/path --recursive
mooseng data download /remote/file.txt /local/file.txt --verify-checksum

# Synchronization
mooseng data sync /local/source /remote/dest --recursive
mooseng data sync /local/source /remote/dest --bidirectional
mooseng data sync /local/source /remote/dest --delete-extra
mooseng data sync /local/source /remote/dest --dry-run

# File operations
mooseng data list /remote/path --detailed
mooseng data list /remote/path --recursive --filter "*.txt"
mooseng data copy /remote/source /remote/dest
mooseng data move /remote/source /remote/dest
mooseng data delete /remote/path --recursive
mooseng data mkdir /remote/new-directory

# File information and attributes
mooseng data info /remote/file.txt
mooseng data info /remote/directory --storage-stats
mooseng data set-attr /remote/file.txt --storage-class premium
mooseng data set-attr /remote/file.txt --replication 3
mooseng data set-attr /remote/directory --recursive --storage-class archive
```

### 6. Benchmarking (`mooseng benchmark`)

**Purpose**: Performance testing and analysis

```bash
# Quick benchmark suite
mooseng benchmark quick
mooseng benchmark quick --network --metadata
mooseng benchmark quick --output results.json

# Comprehensive benchmarking
mooseng benchmark full --duration 30m
mooseng benchmark full --concurrency 10 --output detailed-results.json

# Specific benchmark types
mooseng benchmark file --operation read --size 1MB --count 1000
mooseng benchmark file --operation write --size 1GB --count 100
mooseng benchmark metadata --operation create --count 10000
mooseng benchmark metadata --operation list --directory-size 1000
mooseng benchmark network --test-latency --test-bandwidth

# Benchmark analysis
mooseng benchmark compare results1.json results2.json
mooseng benchmark compare --baseline baseline.json --current current.json
mooseng benchmark report --input results.json --format html --output report.html
mooseng benchmark report --input results.json --format csv --output report.csv

# Benchmark management
mooseng benchmark list --available
mooseng benchmark status --running
mooseng benchmark status --history
mooseng benchmark dashboard --port 8081 --live
mooseng benchmark query --metric throughput --since 1h
mooseng benchmark trends --metric latency --period 7d

# Continuous benchmarking (CI mode)
mooseng benchmark continuous --schedule "0 */6 * * *" --store-results
mooseng benchmark continuous --stop
```

### Common Usage Examples

```bash
# Check overall cluster health
docker-compose exec cli mooseng cluster status --verbose

# Upload data with specific storage class
docker-compose exec cli mooseng data upload /local/important-data /backup \
  --recursive --storage-class premium --verify-checksum

# Monitor real-time performance
docker-compose exec cli mooseng monitor metrics --live

# Run performance benchmarks  
docker-compose exec cli mooseng benchmark quick --network --output /tmp/bench.json

# Set up quotas for project directories
docker-compose exec cli mooseng admin quota set --path /projects/team1 \
  --size 500GB --files 100000

# Configure auto-healing
docker-compose exec cli mooseng admin health auto-heal --enable
docker-compose exec cli mooseng admin health configure --check-interval 60s

# Export configuration for backup
docker-compose exec cli mooseng config export --output /tmp/cluster-config.yaml

# Scale the cluster
docker-compose exec cli mooseng cluster scale --chunkservers 5

# Monitor events in real-time
docker-compose exec cli mooseng monitor events --follow --filter warning,error
```

### Environment Variables

Configure the CLI behavior using environment variables:

```bash
# Master endpoints (comma-separated)
export MOOSENG_MASTER_ENDPOINTS="master-1:9421,master-2:9421,master-3:9421"

# Connection timeout
export MOOSENG_CONNECT_TIMEOUT="10s"

# Request timeout  
export MOOSENG_REQUEST_TIMEOUT="30s"

# Default storage class
export MOOSENG_DEFAULT_STORAGE_CLASS="standard"

# Enable debug logging
export MOOSENG_LOG_LEVEL="debug"

# TLS configuration
export MOOSENG_TLS_ENABLED="true"
export MOOSENG_TLS_CERT_FILE="/etc/ssl/client.crt"
export MOOSENG_TLS_KEY_FILE="/etc/ssl/client.key"
```

### Configuration Files

The CLI looks for configuration files in these locations (in order):
1. `./mooseng.yaml` (current directory)
2. `~/.mooseng/config.yaml` (user home)
3. `/etc/mooseng/config.yaml` (system-wide)

Example configuration file:
```yaml
cluster:
  name: "mooseng-demo"
  masters:
    - "master-1:9421"
    - "master-2:9421" 
    - "master-3:9421"

connection:
  timeout: "10s"
  retry_attempts: 3
  retry_backoff: "exponential"

storage:
  default_class: "standard"
  classes:
    premium:
      copies: 3
      erasure: "8+4"
      tier: "fast"
    archive:
      copies: 1
      erasure: "8+4"
      tier: "cold"

logging:
  level: "info"
  format: "json"
```

## Resource Requirements

### Minimum Requirements
- **CPU**: 8 cores recommended
- **Memory**: 16GB RAM recommended  
- **Storage**: 20GB free space for volumes
- **Network**: Docker networking enabled

### Resource Allocation
- **Masters**: 2GB RAM, 2 CPU cores each
- **ChunkServers**: 4GB RAM, 4 CPU cores each
- **Clients**: 1GB RAM, 1 CPU core each
- **Monitoring**: 1.5GB RAM total

## Management Commands

### Cluster Operations
```bash
# View all services
docker-compose ps

# Check specific service logs
docker-compose logs -f master-1
docker-compose logs -f chunkserver-1

# Restart a service
docker-compose restart chunkserver-2

# Scale services (where applicable)
docker-compose up -d --scale chunkserver=5
```

### Maintenance
```bash
# Stop the cluster
docker-compose down

# Clean up everything (including data)
docker-compose down -v

# View resource usage
docker stats
```

## Troubleshooting

### Common Issues

#### 1. **Services Won't Start**

**Docker Environment Issues:**
```bash
# Check Docker is running
docker info

# Verify available resources
docker system df
docker system prune -f  # Clean up if needed

# Check for port conflicts
netstat -tulpn | grep :94
lsof -i :9421-9469  # Check if ports are in use

# Check Docker daemon logs
journalctl -u docker.service --since "1 hour ago"
```

**Resource Constraints:**
```bash
# Check system resources
free -h  # Memory usage
df -h    # Disk space
uptime   # System load

# Check Docker resource limits
docker system info | grep -A 5 "Resource Limits"
```

**Build Issues:**
```bash
# Clean build cache and rebuild
docker-compose down -v
docker system prune -f
docker-compose build --no-cache
```

#### 2. **Raft Election Failures**

**Symptoms:**
- Masters stuck in "starting" state
- No clear leader elected
- Frequent leader changes
- Split-brain scenarios

**Diagnosis:**
```bash
# Check master logs for election issues
docker-compose logs master-1 | grep -i "election\|leader\|vote"
docker-compose logs master-2 | grep -i "election\|leader\|vote"
docker-compose logs master-3 | grep -i "election\|leader\|vote"

# Check network connectivity between masters
docker-compose exec master-1 ping master-2
docker-compose exec master-1 ping master-3
docker-compose exec master-2 ping master-3

# Check Raft ports are accessible
docker-compose exec master-1 nc -zv master-2 9423
docker-compose exec master-1 nc -zv master-3 9423
```

**Solutions:**
```bash
# Option 1: Restart masters in sequence
docker-compose restart master-1
sleep 30
docker-compose restart master-2
sleep 30
docker-compose restart master-3

# Option 2: Clean restart all masters
docker-compose stop master-1 master-2 master-3
docker volume rm mooseng_master-1-data mooseng_master-2-data mooseng_master-3-data
docker-compose up -d master-1 master-2 master-3

# Option 3: Check cluster configuration
docker-compose exec master-1 env | grep MOOSENG_RAFT_PEERS
```

**Prevention:**
- Ensure stable network connectivity
- Use proper DNS resolution or IP addresses
- Verify all masters can communicate on Raft ports (9423)
- Check system clock synchronization: `timedatectl status`

#### 3. **Chunkserver Data Directory Errors**

**Symptoms:**
- Chunkserver crashes on startup
- "Permission denied" errors
- "No space left on device" errors
- Corrupted chunk data warnings

**Diagnosis:**
```bash
# Check chunkserver logs
docker-compose logs chunkserver-1 | grep -i "error\|failed\|panic"

# Check data directory permissions and space
docker-compose exec chunkserver-1 ls -la /data1 /data2 /data3 /data4
docker-compose exec chunkserver-1 df -h /data1 /data2 /data3 /data4

# Check for disk errors
docker-compose exec chunkserver-1 dmesg | grep -i "error\|fail"

# Verify data directory initialization
docker-compose exec chunkserver-1 find /data1 -type f -name "*.chunk" | head -5
```

**Solutions:**
```bash
# Fix permissions (if needed)
docker-compose exec chunkserver-1 chown -R mooseng:mooseng /data1 /data2 /data3 /data4
docker-compose exec chunkserver-1 chmod -R 755 /data1 /data2 /data3 /data4

# Clean up corrupted data (CAUTION: Data loss!)
docker-compose stop chunkserver-1
docker volume rm mooseng_chunkserver-1-data1 mooseng_chunkserver-1-data2 mooseng_chunkserver-1-data3 mooseng_chunkserver-1-data4
docker-compose up -d chunkserver-1

# Expand data directory size (for local volumes)
docker volume inspect mooseng_chunkserver-1-data1

# Check chunk integrity
docker-compose exec chunkserver-1 /usr/local/bin/mooseng-chunkserver --verify-chunks /data1
```

**Prevention:**
- Monitor disk space: `df -h` regularly
- Set up proper volume sizes in docker-compose.yml
- Use external storage for production deployments
- Implement automated backup strategies

#### 4. **Client Mount Problems**

**Symptoms:**
- FUSE mount fails
- "Transport endpoint not connected" errors
- Files not visible across clients
- Permission denied on mounted filesystem

**Diagnosis:**
```bash
# Check client health and mount status
docker-compose ps client-1 client-2 client-3
docker-compose exec client-1 mountpoint -q /mnt/mooseng && echo "Mounted" || echo "Not mounted"

# Check FUSE availability
docker-compose exec client-1 ls -la /dev/fuse
docker-compose exec client-1 cat /proc/filesystems | grep fuse

# Check client logs
docker-compose logs client-1 | grep -i "fuse\|mount\|error"

# Test master connectivity from client
docker-compose exec client-1 nc -zv master-1 9421
docker-compose exec client-1 nc -zv master-2 9421
docker-compose exec client-1 nc -zv master-3 9421

# Check host mount point
ls -la mnt/client-1/
```

**Solutions:**
```bash
# Remount filesystem
docker-compose exec client-1 umount -f /mnt/mooseng
docker-compose restart client-1

# Check kernel modules (on host)
lsmod | grep fuse
modprobe fuse  # Load FUSE module if missing

# Verify container privileges
docker-compose exec client-1 cat /proc/self/status | grep Cap

# Manual mount test
docker-compose exec client-1 /usr/local/bin/mooseng-mount \
  -o master1=master-1:9421,master2=master-2:9421,master3=master-3:9421 \
  /mnt/mooseng

# Check file permissions
docker-compose exec client-1 touch /mnt/mooseng/test.txt
ls -la mnt/client-1/test.txt
```

**Prevention:**
- Ensure `/dev/fuse` is available on host system
- Verify container runs with `privileged: true`
- Test FUSE functionality before deployment
- Monitor client health endpoints regularly

#### 5. **Network Connectivity Issues**

**Symptoms:**
- Services can't reach each other
- Intermittent connection failures
- DNS resolution problems

**Diagnosis:**
```bash
# Check Docker network
docker network ls | grep mooseng
docker network inspect mooseng_mooseng-net

# Test service-to-service connectivity
docker-compose exec master-1 ping chunkserver-1
docker-compose exec chunkserver-1 ping master-1
docker-compose exec client-1 ping master-1

# Check DNS resolution
docker-compose exec master-1 nslookup chunkserver-1
docker-compose exec client-1 nslookup master-1

# Test specific ports
docker-compose exec client-1 telnet master-1 9421
docker-compose exec chunkserver-1 telnet master-1 9422
```

**Solutions:**
```bash
# Recreate network
docker-compose down
docker network prune -f
docker-compose up -d

# Check firewall settings (host)
ufw status
iptables -L | grep DOCKER

# Manual network troubleshooting
docker-compose exec master-1 netstat -tlnp
docker-compose exec master-1 ss -tlnp
```

#### 6. **Performance Issues**

**Symptoms:**
- Slow file operations
- High CPU or memory usage
- Network bottlenecks

**Diagnosis:**
```bash
# Monitor resource usage
docker stats --no-stream
docker-compose exec master-1 top -p 1
docker-compose exec chunkserver-1 iostat -x 1 5

# Check network performance
docker-compose exec client-1 iperf3 -c chunkserver-1 -p 5201
docker-compose exec master-1 ping -c 10 chunkserver-1

# Monitor filesystem performance
docker-compose exec client-1 dd if=/dev/zero of=/mnt/mooseng/test bs=1M count=100
docker-compose exec client-1 dd if=/mnt/mooseng/test of=/dev/null bs=1M
```

**Solutions:**
```bash
# Adjust resource limits in docker-compose.yml
# Increase memory limits for memory-intensive services
# Add CPU limits to prevent resource contention

# Tune filesystem parameters
docker-compose exec client-1 mount -o remount,cache=strict /mnt/mooseng

# Monitor and optimize
# Use Prometheus/Grafana dashboards at http://localhost:3000
# Check http://localhost:9090 for raw metrics
```

### Health Checks Failing

**Master Health Checks:**
```bash
# Check master health endpoint directly
curl -f http://localhost:9430/health || echo "Master 1 unhealthy"
curl -f http://localhost:9435/health || echo "Master 2 unhealthy"
curl -f http://localhost:9445/health || echo "Master 3 unhealthy"

# Manual health check
docker-compose exec master-1 /usr/local/bin/mooseng-master --health-check
```

**Chunkserver Health Checks:**
```bash
# Check chunkserver health endpoints
curl -f http://localhost:9429/health || echo "Chunkserver 1 unhealthy"
curl -f http://localhost:9459/health || echo "Chunkserver 2 unhealthy"
curl -f http://localhost:9469/health || echo "Chunkserver 3 unhealthy"

# Manual health check
docker-compose exec chunkserver-1 /usr/local/bin/mooseng-chunkserver --health-check
```

**Client Health Checks:**
```bash
# Check if clients can mount filesystem
docker-compose exec client-1 mountpoint -q /mnt/mooseng
docker-compose exec client-2 mountpoint -q /mnt/mooseng  
docker-compose exec client-3 mountpoint -q /mnt/mooseng

# Test basic file operations
docker-compose exec client-1 echo "test" > /mnt/mooseng/health-test.txt
docker-compose exec client-2 cat /mnt/mooseng/health-test.txt
docker-compose exec client-1 rm /mnt/mooseng/health-test.txt
```

### Debug Commands
```bash
# Check overall cluster status
docker-compose ps
docker-compose top

# View detailed service information
docker-compose config --services
docker-compose config --volumes

# Access service shells for debugging
docker-compose exec master-1 /bin/sh
docker-compose exec chunkserver-1 /bin/sh
docker-compose exec client-1 /bin/sh
docker-compose exec cli /bin/sh

# Monitor real-time logs
docker-compose logs -f
docker-compose logs -f master-1
docker-compose logs -f chunkserver-1 chunkserver-2 chunkserver-3

# View service configuration
docker-compose exec master-1 env | grep MOOSENG
docker-compose exec chunkserver-1 env | grep MOOSENG

# Check resource utilization
docker stats
docker system df
docker volume ls | grep mooseng

# Network debugging
docker network inspect mooseng_mooseng-net
docker-compose exec master-1 netstat -tlnp | grep 94
```

### Log Analysis

**Important Log Locations:**
- Master logs: `docker-compose logs master-1`
- Chunkserver logs: `docker-compose logs chunkserver-1`
- Client logs: `docker-compose logs client-1`
- All logs: `docker-compose logs -f`

**Common Error Patterns:**
```bash
# Raft consensus issues
docker-compose logs master-1 | grep -i "election timeout\|leader\|vote"

# Storage issues
docker-compose logs chunkserver-1 | grep -i "storage\|disk\|chunk\|corruption"

# Network issues
docker-compose logs | grep -i "connection refused\|timeout\|unreachable"

# Permission issues
docker-compose logs | grep -i "permission\|access denied\|forbidden"

# Resource issues
docker-compose logs | grep -i "memory\|oom\|space\|resource"
```

## Advanced Configuration

### Customization
Edit `mooseng/docker-compose.yml` to:
- Adjust resource limits
- Modify environment variables
- Add additional services
- Change port mappings

### Production Considerations
- Use external volumes for data persistence
- Configure proper secrets management
- Set up external monitoring
- Implement backup strategies
- Configure network security

## Contributing

To extend this demo:
1. Fork the repository
2. Make your changes in the `mooseng/` directory
3. Test with `./test-mooseng-demo.sh`
4. Submit a pull request

## Architecture Diagram

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│    Master-1     │    │    Master-2     │    │    Master-3     │
│   (Leader)      │◄──►│   (Follower)    │◄──►│   (Follower)    │
│   :9421-9430    │    │   :9431-9435    │    │   :9441-9445    │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         │              Raft Consensus                  │
         │                       │                       │
    ┌────┴────┐             ┌────┴────┐             ┌────┴────┐
    │ChunkSrv1│             │ChunkSrv2│             │ChunkSrv3│
    │:9420-9429│             │:9450-9459│             │:9460-9469│
    │4x Data  │             │4x Data  │             │4x Data  │
    └─────────┘             └─────────┘             └─────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌────────────┴────────────┐
                    │                         │
               ┌────┴────┐              ┌────┴────┐
               │Client-1 │              │Client-2 │
               │:9427    │              │:9437    │
               │FUSE     │              │FUSE     │
               └─────────┘              └─────────┘
                         │
                    ┌────┴────┐
                    │Client-3 │
                    │:9447    │
                    │FUSE     │
                    └─────────┘
```

This demo provides a complete, production-like environment for testing and developing with MooseNG distributed filesystem.