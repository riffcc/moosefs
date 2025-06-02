# MooseNG Unified Cluster Setup

## 🎯 Overview

This directory contains a complete, production-ready Docker Compose setup for MooseNG distributed file system with:

- **3 Master Servers** with Raft consensus
- **3 Chunk Servers** with 4 storage disks each  
- **3 Client Instances** with FUSE mounting
- **Monitoring Stack** (Prometheus + Grafana + Dashboard)

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         MASTER CLUSTER                         │
├─────────────────┬─────────────────┬─────────────────────────────┤
│   Master-1      │   Master-2      │      Master-3               │
│   :9421-9423    │   :9431-9433    │      :9441-9443             │
│   (Leader)      │   (Follower)    │      (Follower)             │
└─────────────────┴─────────────────┴─────────────────────────────┘
                              │
                         Raft Consensus
                              │
┌─────────────────────────────────────────────────────────────────┐
│                      CHUNK SERVERS                             │
├─────────────────┬─────────────────┬─────────────────────────────┤
│  Chunkserver-1  │  Chunkserver-2  │     Chunkserver-3           │
│     :9420       │     :9450       │        :9460                │
│   4 disks each  │   4 disks each  │      4 disks each           │
└─────────────────┴─────────────────┴─────────────────────────────┘
                              │
                         Data Storage
                              │
┌─────────────────────────────────────────────────────────────────┐
│                          CLIENTS                               │
├─────────────────┬─────────────────┬─────────────────────────────┤
│    Client-1     │    Client-2     │       Client-3              │
│     :9427       │     :9437       │        :9447                │
│  FUSE Mount     │  FUSE Mount     │     FUSE Mount              │
└─────────────────┴─────────────────┴─────────────────────────────┘
```

## 🚀 Quick Start

### Prerequisites
- Docker Engine 20.10+
- Docker Compose v2.0+
- At least 8GB RAM available
- 20GB free disk space

### Start the Cluster
```bash
# Start everything
./demo-unified-cluster.sh start

# Check status  
./demo-unified-cluster.sh status

# View service URLs
./demo-unified-cluster.sh urls
```

### Test the Filesystem
```bash
# Run filesystem tests
./demo-unified-cluster.sh test

# View logs
./demo-unified-cluster.sh logs
```

### Stop the Cluster
```bash
# Stop services
./demo-unified-cluster.sh stop

# Complete cleanup
./demo-unified-cluster.sh cleanup
```

## 📋 Service Details

### Master Servers
| Service | Ports | Role | Purpose |
|---------|-------|------|---------|
| master-1 | 9421-9423 | Leader | Primary metadata server |
| master-2 | 9431-9433 | Follower | Backup metadata server |
| master-3 | 9441-9443 | Follower | Backup metadata server |

**Ports:**
- `942X`: Client API
- `942X+1`: Raft communication
- `942X+2`: Metrics endpoint

### Chunk Servers
| Service | Port | Metrics | Storage |
|---------|------|---------|---------|
| chunkserver-1 | 9420 | 9425 | 4 virtual disks |
| chunkserver-2 | 9450 | 9455 | 4 virtual disks |
| chunkserver-3 | 9460 | 9465 | 4 virtual disks |

### Clients
| Service | Port | Mount Point | Cache |
|---------|------|-------------|-------|
| client-1 | 9427 | /mnt/mooseng | 256MB |
| client-2 | 9437 | /mnt/mooseng | 256MB |
| client-3 | 9447 | /mnt/mooseng | 256MB |

### Monitoring
| Service | Port | Purpose |
|---------|------|---------|
| Prometheus | 9090 | Metrics collection |
| Grafana | 3000 | Dashboards (admin/admin) |
| Dashboard | 8080 | Web interface |

## 🔧 Configuration

### Environment Variables
Key environment variables used across services:

**Masters:**
- `MOOSENG_NODE_ID`: Unique node identifier
- `MOOSENG_CLUSTER_ID`: Cluster identifier
- `MOOSENG_RAFT_PEERS`: Raft peer addresses
- `MOOSENG_CACHE_SIZE_MB`: Metadata cache size

**Chunkservers:**
- `MOOSENG_SERVER_ID`: Unique server identifier
- `MOOSENG_MASTER_ADDRS`: Master server addresses
- `MOOSENG_DATA_DIRS`: Storage directory paths

**Clients:**
- `MOOSENG_MASTER_ENDPOINTS`: Master endpoints
- `MOOSENG_MOUNT_POINT`: FUSE mount point
- `MOOSENG_CACHE_SIZE_MB`: Client cache size

### Storage Classes
- **default**: 2 copies, standard tier
- **archive**: 1 copy with 8+4 erasure coding, cold tier
- **fast**: 3 copies, hot tier

## 🔍 Troubleshooting

### Common Issues

**1. Docker daemon not running**
```bash
# On macOS
open -a Docker

# On Linux
sudo systemctl start docker
```

**2. Port conflicts**
```bash
# Check for conflicting processes
lsof -i :9421
lsof -i :3000
lsof -i :9090
```

**3. FUSE mounting issues**
```bash
# Check client logs
docker-compose logs client-1

# Verify FUSE device
docker exec mooseng-client-1 ls -la /dev/fuse
```

**4. Service startup failures**
```bash
# Check specific service logs
docker-compose logs master-1
docker-compose logs chunkserver-1

# Restart specific service
docker-compose restart master-1
```

### Health Checks

**Check all services:**
```bash
docker-compose ps
```

**Check master consensus:**
```bash
# Should show Raft cluster status
curl -s localhost:9423/metrics | grep raft
```

**Check chunkserver registration:**
```bash
# Should show registered chunkservers
curl -s localhost:9423/metrics | grep chunk
```

### Log Analysis

**View all logs:**
```bash
docker-compose logs -f
```

**Filter by service:**
```bash
docker-compose logs -f master-1
docker-compose logs -f chunkserver-1
docker-compose logs -f client-1
```

**Search for errors:**
```bash
docker-compose logs | grep -i error
docker-compose logs | grep -i failed
```

## 📊 Monitoring

### Grafana Dashboards
Access Grafana at http://localhost:3000 (admin/admin):

1. **MooseNG Health Dashboard**: Overall cluster health
2. **Performance Dashboard**: Throughput and latency metrics
3. **Storage Dashboard**: Disk usage and capacity

### Prometheus Metrics
Access Prometheus at http://localhost:9090:

- Master metrics: `:9423/metrics`, `:9433/metrics`, `:9443/metrics`
- Chunkserver metrics: `:9425/metrics`, `:9455/metrics`, `:9465/metrics`

### Key Metrics to Monitor
- `mooseng_raft_leader`: Current Raft leader
- `mooseng_chunks_total`: Total chunks stored
- `mooseng_bytes_written_total`: Data written
- `mooseng_client_operations_total`: Client operations

## 🧪 Testing

### Filesystem Operations
```bash
# Create a test file
docker exec mooseng-client-1 sh -c "echo 'Hello MooseNG' > /mnt/mooseng/test.txt"

# Read from another client
docker exec mooseng-client-2 cat /mnt/mooseng/test.txt

# List files
docker exec mooseng-client-3 ls -la /mnt/mooseng/
```

### Performance Testing
```bash
# Large file test
docker exec mooseng-client-1 dd if=/dev/zero of=/mnt/mooseng/bigfile bs=1M count=100

# Concurrent access test
docker exec mooseng-client-1 dd if=/dev/zero of=/mnt/mooseng/file1 bs=1M count=50 &
docker exec mooseng-client-2 dd if=/dev/zero of=/mnt/mooseng/file2 bs=1M count=50 &
docker exec mooseng-client-3 dd if=/dev/zero of=/mnt/mooseng/file3 bs=1M count=50 &
wait
```

## 🔒 Security

### Development Mode
- TLS disabled for easier debugging
- All services run as non-root user
- Network isolation via Docker bridge
- No authentication configured

### Production Considerations
- Enable TLS encryption
- Set up proper authentication
- Use secrets management
- Configure firewall rules
- Enable audit logging

## 📁 Files

### Main Files
- `docker-compose.yml`: Main orchestration file
- `demo-unified-cluster.sh`: Demo script with full functionality
- `test-docker-setup.sh`: Testing utilities

### Docker Files
- `docker/Dockerfile.master`: Master server image
- `docker/Dockerfile.chunkserver`: Chunkserver image  
- `docker/Dockerfile.client`: Client image

### Configuration
- `docker/configs/master.toml`: Master configuration template
- `docker/configs/chunkserver.toml`: Chunkserver configuration template
- `docker/configs/client.toml`: Client configuration template

### Scripts
- `docker/scripts/master-entrypoint.sh`: Master startup script
- `docker/scripts/chunkserver-entrypoint.sh`: Chunkserver startup script
- `docker/scripts/client-entrypoint.sh`: Client startup script

## 🆘 Support

### Getting Help
1. Check the logs: `./demo-unified-cluster.sh logs`
2. Verify health: `./demo-unified-cluster.sh status`
3. Review configuration files in `docker/configs/`
4. Check Docker daemon status

### Common Commands
```bash
# Quick start
./demo-unified-cluster.sh start

# Full restart
./demo-unified-cluster.sh restart

# Check everything
./demo-unified-cluster.sh status

# Clean start
./demo-unified-cluster.sh cleanup
./demo-unified-cluster.sh start
```

---

✅ **Ready to Go!** Your MooseNG cluster is configured and ready for testing.