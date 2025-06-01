# MooseNG Docker Demo - Technical Documentation

This comprehensive guide provides detailed technical documentation for the MooseNG Docker Compose demonstration environment. MooseNG is a next-generation distributed file system built in Rust, designed as an evolution of MooseFS with enhanced performance, reliability, and cloud-native features.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Technical Architecture](#technical-architecture)
- [Docker Compose Infrastructure](#docker-compose-infrastructure)
- [Service Components](#service-components)
- [Helper Scripts](#helper-scripts)
- [Monitoring and Observability](#monitoring-and-observability)
- [Running the Demo](#running-the-demo)
- [Advanced Configuration](#advanced-configuration)
- [Development Guide](#development-guide)
- [Performance Optimization](#performance-optimization)
- [Troubleshooting](#troubleshooting)

## Architecture Overview

MooseNG implements a distributed file system with strong consistency guarantees, high availability, and horizontal scalability. The demo environment showcases a production-like deployment with the following topology:

```
                           ┌─────────────────┐
                           │   Grafana       │
                           │  (Port 3000)    │
                           └────────┬────────┘
                                    │
                           ┌────────▼────────┐
                           │   Prometheus    │
                           │  (Port 9090)    │
                           └────────┬────────┘
                                    │ Metrics Collection
         ┌──────────────────────────┴──────────────────────────┐
         │                                                      │
    ┌────▼─────┐         ┌──────────┐         ┌──────────┐    │
    │ Master 1 │◄────────┤ Master 2 ├────────►│ Master 3 │    │
    │  (9421)  │  Raft   │  (9431)  │  Raft   │  (9441)  │    │
    └────┬─────┘         └────┬─────┘         └────┬─────┘    │
         │                     │                     │          │
         └─────────────────────┴─────────────────────────┘          │
                               │ Metadata Operations            │
                   ┌───────────┴───────────┐                   │
                   │                       │                   │
         ┌─────────▼────┐       ┌─────────▼────┐    ┌────────▼─────┐
         │ ChunkServer 1│       │ ChunkServer 2│    │ ChunkServer 3│
         │    (9420)    │       │    (9450)    │    │    (9460)    │
         │  4 x Disks   │       │  4 x Disks   │    │  4 x Disks   │
         └──────────────┘       └──────────────┘    └──────────────┘
                   │                       │                   │
                   └───────────┬───────────┘                   │
                               │ Data Operations               │
                    ┌──────────┴──────────┐                   │
                    │                     │                   │
          ┌─────────▼───┐      ┌─────────▼───┐     ┌─────────▼───┐
          │  Client 1   │      │  Client 2   │     │  Client 3   │
          │   (9427)    │      │   (9437)    │     │   (9447)    │
          │ /mnt/client1│      │ /mnt/client2│     │ /mnt/client3│
          └─────────────┘      └─────────────┘     └─────────────┘
```

### Key Architectural Features

1. **High Availability**: 3-node master cluster with Raft consensus
2. **Distributed Storage**: 12 independent storage volumes across 3 chunk servers
3. **Client Access**: FUSE-based mounts with caching
4. **Observability**: Comprehensive metrics and dashboards

## Technical Architecture

### Master Server Cluster

The master servers implement a distributed metadata service with the following characteristics:

**Raft Consensus Implementation:**
- **Leader Election**: Automatic leader selection with configurable election timeout
- **Log Replication**: All metadata changes replicated to followers before acknowledgment
- **Snapshot Management**: Periodic snapshots for faster recovery
- **Split-brain Prevention**: Majority quorum required for operations

**Metadata Management:**
- **Namespace Operations**: Directory tree, file attributes, permissions
- **Chunk Mapping**: File-to-chunk mappings with location tracking
- **Session Management**: Client sessions with lease renewal
- **Transaction Log**: Atomic operations with rollback capability

**API Endpoints:**
| Endpoint | Method | Description |
|----------|---------|-------------|
| `/health` | GET | Health check with role information |
| `/status` | GET | Cluster status and statistics |
| `/metrics` | GET | Prometheus metrics export |
| `/api/v1/filesystem/stat` | POST | File/directory metadata |
| `/api/v1/filesystem/create` | POST | Create file with chunk allocation |
| `/api/v1/chunkservers` | GET | List chunk server status |

### Chunk Server Architecture

Each chunk server provides distributed storage with the following features:

**Storage Management:**
- **Multi-disk Support**: 4 independent disks per server
- **Chunk Size**: Configurable (default 64MB)
- **Replication**: Configurable replication factor (default 3)
- **Data Integrity**: CRC32 checksums on all chunks

**Performance Features:**
- **Parallel I/O**: Concurrent operations across disks
- **Write Buffering**: Aggregates small writes
- **Read Caching**: LRU cache for hot data
- **Zero-copy Operations**: Direct memory transfers

**Health Monitoring:**
- Disk usage monitoring
- I/O performance metrics
- Network throughput tracking
- Error rate monitoring

### Client Implementation

FUSE-based clients provide POSIX-compliant file system access:

**Caching Layers:**
- **Metadata Cache**: Reduces round-trips to masters
- **Data Cache**: Local caching of frequently accessed chunks
- **Write Buffer**: Aggregates writes before sending to chunk servers

**Connection Management:**
- **Master Connection Pool**: Load balancing across masters
- **Chunk Server Connections**: Direct connections for data transfer
- **Automatic Failover**: Seamless handling of server failures

## Docker Compose Infrastructure

### Network Architecture

The demo uses a custom bridge network with the following configuration:

```yaml
networks:
  mooseng-net:
    driver: bridge
    ipam:
      config:
        - subnet: 172.20.0.0/24
```

**Network Features:**
- Service discovery via container names
- Isolated from host network
- Inter-container communication optimization
- DNS resolution for all services

### Volume Management

**Persistent Volumes:**

| Volume Type | Purpose | Persistence |
|-------------|---------|-------------|
| Master Data | Metadata storage | Survives restarts |
| Chunk Storage | 12 independent volumes | Persistent data |
| Client Cache | Read cache | Temporary |
| Monitoring | Metrics history | Configurable retention |

### Service Configuration

**Master Server Configuration:**
```yaml
master-1:
  environment:
    - NODE_ID=1                    # Unique node identifier
    - NODE_ROLE=master            # Service role
    - CLUSTER_NODES=master-1:9421,master-2:9421,master-3:9421
    - CLIENT_PORT=9421            # Client API port
    - RAFT_PORT=9422              # Raft consensus port
    - METRICS_PORT=9423           # Prometheus metrics
    - LOG_LEVEL=info              # Logging verbosity
  healthcheck:
    test: ["CMD", "curl", "-f", "http://localhost:9421/health"]
    interval: 10s                 # Check frequency
    timeout: 5s                   # Request timeout
    retries: 3                    # Failure threshold
    start_period: 10s             # Grace period
```

**Chunk Server Configuration:**
```yaml
chunkserver-1:
  environment:
    - NODE_ID=1
    - MASTER_ENDPOINTS=master-1:9421,master-2:9421,master-3:9421
    - BIND_PORT=9420              # Data transfer port
    - METRICS_PORT=9425           # Metrics endpoint
    - DATA_DIRS=/data/disk1,/data/disk2,/data/disk3,/data/disk4
  volumes:
    - chunkserver-1-disk1:/data/disk1
    - chunkserver-1-disk2:/data/disk2
    - chunkserver-1-disk3:/data/disk3
    - chunkserver-1-disk4:/data/disk4
```

**Client Configuration:**
```yaml
client-1:
  privileged: true                # Required for FUSE
  cap_add:
    - SYS_ADMIN                   # Mount capability
  devices:
    - /dev/fuse                   # FUSE device
  volumes:
    - ./mnt/client-1:/mnt/mooseng:rshared
    - client-1-cache:/cache       # Local cache
```

## Service Components

### Mock Service Implementation

The demo uses Python Flask-based mock services that simulate real MooseNG behavior:

**Master Mock Service (`master-mock.py`):**
```python
# Key components:
- Flask HTTP server with gunicorn
- Raft state machine simulation
- RESTful API implementation
- Prometheus metrics export
- File system operation handlers
```

**Features Implemented:**
- Leader election simulation
- Metadata operations (create, stat, delete)
- Chunk allocation logic
- Client session management
- Metrics collection and export

**Chunk Server Mock (`chunkserver-mock.py`):**
- Chunk storage simulation
- Disk usage tracking
- Replication handling
- Health status reporting

**Client Mock (`client-mock.py`):**
- FUSE mount simulation
- File operation forwarding
- Cache management
- Metrics reporting

### Container Images

**Base Image Selection:**
- `python:3.11-alpine`: Minimal footprint
- Production-grade WSGI server (gunicorn)
- Health check integration
- Signal handling for graceful shutdown

**Dockerfile Structure:**
```dockerfile
FROM python:3.11-alpine
WORKDIR /app
COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt
COPY mock-services/master-mock.py .
EXPOSE 9421 9422 9423
HEALTHCHECK --interval=10s --timeout=5s --retries=3 \
    CMD wget -q -O- http://localhost:9421/health || exit 1
CMD ["gunicorn", "--bind", "0.0.0.0:9421", 
     "--workers", "2", "--threads", "4", "master-mock:app"]
```

## Helper Scripts

### start-demo.sh

The startup script orchestrates the complete demo environment initialization:

**Execution Flow:**
1. **Environment Validation**
   - Docker daemon check
   - Port availability verification
   - Directory structure creation

2. **Service Deployment**
   ```bash
   # Parallel image building
   docker compose build --parallel
   
   # Sequential startup with dependency order
   docker compose up -d
   ```

3. **Health Verification**
   - Service startup monitoring
   - Health endpoint checking
   - Leader election verification

4. **Status Reporting**
   - Service endpoint listing
   - Health status summary
   - Access instructions

**Advanced Features:**
```bash
# Environment variables
DEBUG=1                    # Enable debug output
COMPOSE_FILE=custom.yml   # Use custom compose file
SKIP_HEALTH_CHECK=1       # Skip health verification
NO_BUILD=1                # Skip image building
```

### test-demo.sh

Comprehensive testing script with multiple verification levels:

**Test Categories:**
1. **Infrastructure Tests**
   - Container status verification
   - Network connectivity checks
   - Port accessibility tests

2. **Service Health Tests**
   - HTTP endpoint availability
   - Response validation
   - Metric endpoint verification

3. **Cluster Functionality**
   - Raft leader election
   - Metadata operation testing
   - Chunk server connectivity

**Output Format:**
```
🧪 MooseNG Docker Demo Test
===========================
📊 Checking Docker services...
  ✅ Docker services are defined
  📈 Running services: 11/11

🔍 Testing Master Servers...
  ✅ Master 1 is responding
  ✅ Master 2 is responding
  ✅ Master 3 is responding
  📊 Masters healthy: 3/3

🗳️  Testing Raft Consensus...
  ✅ Leader election successful
```

### stop-demo.sh

Graceful shutdown with cleanup options:

**Features:**
- Running service detection
- Graceful container shutdown
- Optional volume cleanup
- Status reporting

**Usage Modes:**
```bash
./stop-demo.sh              # Stop containers, preserve data
./stop-demo.sh --clean      # Stop and remove all data
./stop-demo.sh --force      # Force stop hung containers
```

## Monitoring and Observability

### Prometheus Configuration

**Scrape Configuration:**
```yaml
scrape_configs:
  - job_name: 'mooseng-master'
    static_configs:
      - targets: ['master-1:9423', 'master-2:9423', 'master-3:9423']
    scrape_interval: 10s
    metrics_path: /metrics
    
  - job_name: 'mooseng-chunkserver'
    static_configs:
      - targets: ['chunkserver-1:9425', 'chunkserver-2:9425', 'chunkserver-3:9425']
    scrape_interval: 10s
```

**Available Metrics:**
| Metric | Type | Description |
|--------|------|-------------|
| `mooseng_master_up` | gauge | Service availability (0/1) |
| `mooseng_master_role` | gauge | Current role (1=leader, 0=follower) |
| `mooseng_master_term` | counter | Current Raft term |
| `mooseng_master_files` | gauge | Total file count |
| `mooseng_master_chunks` | gauge | Total chunk count |
| `mooseng_chunkserver_capacity_bytes` | gauge | Total storage capacity |
| `mooseng_chunkserver_used_bytes` | gauge | Used storage space |
| `mooseng_chunkserver_chunks` | gauge | Chunks per server |

### Grafana Dashboards

**Pre-configured Dashboards:**

1. **MooseNG Health Dashboard**
   - Service availability matrix
   - Cluster health score
   - Alert status overview
   - Recent events log

2. **MooseNG Performance Dashboard**
   - Operation latency histograms
   - Throughput graphs
   - Cache hit rates
   - Queue depths

3. **MooseNG Storage Dashboard**
   - Storage utilization per server
   - Chunk distribution heatmap
   - Replication status
   - I/O operations per second

### Example Queries

**Prometheus Query Examples:**
```promql
# Cluster health score (0-1)
avg(up{job=~"mooseng-.*"})

# Storage utilization percentage
100 * sum(mooseng_chunkserver_used_bytes) / sum(mooseng_chunkserver_capacity_bytes)

# Operations per second by type
sum(rate(mooseng_operations_total[5m])) by (operation)

# 95th percentile latency
histogram_quantile(0.95, sum(rate(mooseng_operation_duration_seconds_bucket[5m])) by (le))
```

## Running the Demo

### Prerequisites

**System Requirements:**
- Docker Engine 20.10+
- Docker Compose 2.0+
- 4GB RAM minimum
- 10GB free disk space
- Linux/macOS/Windows with WSL2

**Port Requirements:**
| Service | Ports | Purpose |
|---------|-------|---------|
| Masters | 9421-9443 | API, Raft, Metrics |
| Chunk Servers | 9420-9465 | Data, Metrics |
| Clients | 9427-9447 | Metrics |
| Monitoring | 3000, 9090, 8080 | Grafana, Prometheus, Dashboard |

### Quick Start Guide

```bash
# 1. Clone repository
git clone <repository-url>
cd moosefs/mooseng

# 2. Start the demo
./start-demo.sh

# 3. Verify health
./test-demo.sh

# 4. Access services
open http://localhost:3000    # Grafana
open http://localhost:9090    # Prometheus
open http://localhost:8080    # Dashboard

# 5. Test file operations
docker compose exec client-1 bash
cd /mnt/mooseng
echo "Hello MooseNG" > test.txt
dd if=/dev/zero of=large.bin bs=1M count=100

# 6. Stop demo
./stop-demo.sh
```

### Testing Scenarios

**Basic Functionality:**
```bash
# Create test files
for i in {1..10}; do
  docker compose exec client-1 dd if=/dev/urandom of=/mnt/mooseng/test$i.dat bs=1M count=10
done

# Verify replication
docker compose exec master-1 curl -s http://localhost:9421/api/v1/filesystem/stat -d '{"path":"/test1.dat"}'
```

**High Availability Testing:**
```bash
# Stop leader master
docker compose stop master-1

# Verify new leader election
./test-demo.sh

# Restart stopped master
docker compose start master-1
```

**Performance Testing:**
```bash
# Run benchmark
docker compose exec client-1 bash -c "
  cd /mnt/mooseng
  time dd if=/dev/zero of=bench.dat bs=1M count=1000
  time dd if=bench.dat of=/dev/null bs=1M
"
```

## Advanced Configuration

### Custom Environment Variables

**Master Configuration:**
```bash
# Advanced Raft tuning
RAFT_ELECTION_TIMEOUT=150ms
RAFT_HEARTBEAT_INTERVAL=50ms
RAFT_SNAPSHOT_INTERVAL=10000

# Performance tuning
METADATA_CACHE_SIZE=1GB
MAX_CONCURRENT_OPERATIONS=1000
```

**Chunk Server Configuration:**
```bash
# Storage tuning
CHUNK_SIZE=128MB
REPLICATION_FACTOR=3
COMPRESSION_ENABLED=true
COMPRESSION_ALGORITHM=lz4

# Performance
IO_THREADS=8
WRITE_BUFFER_SIZE=64MB
```

### Multi-Region Deployment

For multi-region testing:
```bash
docker compose -f docker-compose.multiregion.yml up -d
```

This configuration includes:
- Region-aware placement
- Cross-region replication
- Latency simulation
- Bandwidth limiting

## Development Guide

### Extending Mock Services

**Adding New Endpoints:**

1. **Edit mock service:**
```python
# docker/mock-services/master-mock.py
@app.route('/api/v1/custom/endpoint', methods=['POST'])
def custom_handler():
    data = request.json
    # Implementation
    return jsonify({"status": "success"})
```

2. **Add metrics:**
```python
# Add to metrics endpoint
custom_operations_total.inc()
custom_latency_histogram.observe(duration)
```

3. **Rebuild and test:**
```bash
docker compose build master-1
docker compose up -d master-1
curl -X POST http://localhost:9421/api/v1/custom/endpoint
```

### Debugging

**Enable Debug Logging:**
```bash
# Set environment variable
docker compose exec master-1 bash
export LOG_LEVEL=debug
export FLASK_ENV=development
```

**Trace Requests:**
```bash
# Enable request tracing
docker compose logs -f master-1 | grep -E "request|response"
```

**Performance Profiling:**
```bash
# Enable profiler
docker compose exec master-1 python -m cProfile -o profile.stats master-mock.py
```

## Performance Optimization

### Docker Optimization

**Resource Allocation:**
```yaml
# docker-compose.override.yml
services:
  master-1:
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 2G
        reservations:
          cpus: '1.0'
          memory: 1G
```

**Storage Performance:**
```bash
# Use local SSD for volumes
DOCKER_VOLUME_PATH=/fast-ssd docker compose up

# Pre-allocate storage
docker compose exec chunkserver-1 fallocate -l 10G /data/disk1/preallocated
```

### Network Optimization

**Host Networking (Performance Testing):**
```yaml
# docker-compose.perf.yml
services:
  master-1:
    network_mode: host
    environment:
      - BIND_ADDRESS=0.0.0.0
```

**TCP Tuning:**
```bash
# Increase buffer sizes
docker compose exec master-1 sysctl -w net.core.rmem_max=134217728
docker compose exec master-1 sysctl -w net.core.wmem_max=134217728
```

## Troubleshooting

### Common Issues and Solutions

**Port Conflicts:**
```bash
# Find conflicting processes
lsof -i :9420
netstat -tulpn | grep -E '9420|9090|3000'

# Change ports in .env file
echo "MASTER_PORT_BASE=9520" > .env
```

**Health Check Failures:**
```bash
# Detailed health check
docker compose exec master-1 wget -O- http://localhost:9421/health

# Check container logs
docker compose logs --tail=100 master-1 | grep -i error
```

**Memory Issues:**
```bash
# Check memory usage
docker stats --no-stream

# Increase Docker memory limit
# Docker Desktop: Settings > Resources > Memory
```

**Network Issues:**
```bash
# Test connectivity
docker compose exec master-1 ping master-2
docker compose exec master-1 nc -zv chunkserver-1 9420

# Inspect network
docker network inspect mooseng_mooseng-net
```

### Recovery Procedures

**Backup:**
```bash
# Backup script
#!/bin/bash
BACKUP_DIR="/backup/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$BACKUP_DIR"

# Backup metadata
for i in 1 2 3; do
  docker compose exec master-$i tar -czf - /data > "$BACKUP_DIR/master-$i.tar.gz"
done

# Backup chunks
for i in 1 2 3; do
  docker compose exec chunkserver-$i tar -czf - /data > "$BACKUP_DIR/chunks-$i.tar.gz"
done
```

**Restore:**
```bash
# Stop services
docker compose down

# Restore volumes
for i in 1 2 3; do
  docker run --rm -v mooseng_master-$i-data:/data -v "$BACKUP_DIR:/backup" \
    alpine tar -xzf /backup/master-$i.tar.gz -C /
done

# Start services
docker compose up -d
```

## Next Steps

1. **Production Deployment**: Adapt the configuration for production use
2. **Performance Testing**: Run benchmarks with real workloads
3. **Integration**: Connect with existing applications
4. **Customization**: Extend mock services for specific use cases
5. **Monitoring**: Set up alerts and SLOs

For detailed API documentation and advanced features, refer to the main MooseNG documentation.