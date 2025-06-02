# MooseNG Unified Docker Demo

🚀 **Complete MooseNG cluster demonstration with 3 Masters, 3 ChunkServers, and 3 Clients**

## 🏗️ Architecture Overview

This demo provides a complete, production-like MooseNG cluster setup:

### Core Services
- **3 Master Servers** (`master-1`, `master-2`, `master-3`)
  - Raft consensus for high availability
  - Ports: 9421-9423, 9431-9433, 9441-9443
  - Persistent metadata storage

- **3 Chunk Servers** (`chunkserver-1`, `chunkserver-2`, `chunkserver-3`)
  - 4 storage disks each (12 disks total)
  - Ports: 9420/9425, 9450/9455, 9460/9465
  - Distributed chunk storage

- **3 Client Instances** (`client-1`, `client-2`, `client-3`)
  - FUSE mount at `/mnt/mooseng`
  - Independent cache systems
  - Ports: 9427, 9437, 9447

### Monitoring Stack
- **Prometheus** (port 9090) - Metrics collection
- **Grafana** (port 3000) - Visualization dashboards
- **Web Dashboard** (port 8080) - Custom MooseNG UI

## 🚀 Quick Start

### Prerequisites
- Docker Engine 20.10+
- Docker Compose 2.0+
- 8GB+ RAM recommended
- 20GB+ free disk space

### Start the Complete Cluster
```bash
# Navigate to MooseNG directory
cd mooseng/

# Start everything (clean build + deploy)
./start-unified-demo.sh start
```

### Alternative Commands
```bash
# Check current status
./start-unified-demo.sh status

# View logs for all services
./start-unified-demo.sh logs

# View logs for specific service
./start-unified-demo.sh logs master-1

# Stop the cluster
./start-unified-demo.sh stop

# Clean up completely
./start-unified-demo.sh clean
```

## 📊 Access Points

### Core Services
| Service | URL | Purpose |
|---------|-----|---------|
| Master-1 API | `http://localhost:9421` | Primary master API |
| Master-2 API | `http://localhost:9431` | Secondary master API |
| Master-3 API | `http://localhost:9441` | Tertiary master API |
| ChunkServer-1 | `http://localhost:9420` | First chunk server |
| ChunkServer-2 | `http://localhost:9450` | Second chunk server |
| ChunkServer-3 | `http://localhost:9460` | Third chunk server |

### Monitoring & Management
| Service | URL | Credentials | Purpose |
|---------|-----|-------------|---------|
| Grafana | `http://localhost:3000` | admin/admin | Dashboards & alerts |
| Prometheus | `http://localhost:9090` | - | Metrics & queries |
| Dashboard | `http://localhost:8080` | - | MooseNG web UI |

## 🔧 Configuration

### Environment Variables
Key configuration options can be modified via environment variables:

```bash
# Master configuration
MOOSENG_LOG_LEVEL=info
MOOSENG_CACHE_SIZE_MB=1024
MOOSENG_RAFT_PEERS=master-1:9422,master-2:9422,master-3:9422

# ChunkServer configuration  
MOOSENG_DATA_DIRS=/data/disk1,/data/disk2,/data/disk3,/data/disk4
MOOSENG_MASTER_ADDRS=master-1:9421,master-2:9421,master-3:9421

# Client configuration
MOOSENG_MOUNT_POINT=/mnt/mooseng
MOOSENG_CACHE_SIZE_MB=256
```

### Storage Layout
Each chunkserver provides 4 storage disks:
- `chunkserver-1`: `/data/disk1-4` → 4 Docker volumes
- `chunkserver-2`: `/data/disk1-4` → 4 Docker volumes  
- `chunkserver-3`: `/data/disk1-4` → 4 Docker volumes

**Total**: 12 storage volumes providing distributed, redundant storage

## 🧪 Testing the Cluster

### Basic Connectivity Test
```bash
# Check if all services are running
docker-compose ps

# Test master API connectivity
curl http://localhost:9421/health
curl http://localhost:9431/health  
curl http://localhost:9441/health

# Check chunkserver registration
curl http://localhost:9420/status
```

### File System Operations
```bash
# Access client container
docker exec -it mooseng-client-1 /bin/sh

# Inside container - test mount
ls /mnt/mooseng
touch /mnt/mooseng/test.txt
echo "Hello MooseNG" > /mnt/mooseng/test.txt
cat /mnt/mooseng/test.txt
```

### Performance Testing
```bash
# Run built-in benchmarks
docker exec -it mooseng-client-1 /bin/sh -c "
  cd /mnt/mooseng && 
  dd if=/dev/zero of=test_1mb.dat bs=1M count=1 &&
  dd if=/dev/zero of=test_10mb.dat bs=1M count=10 &&
  ls -la
"
```

## 📈 Monitoring & Observability

The monitoring stack consists of three main components working together to provide comprehensive observability:

### Prometheus Metrics Collection (Port 9090)

**Access**: `http://localhost:9090`

**Configuration Summary**:
- Scrape interval: 15s globally, 10s for MooseNG services
- Configured targets:
  - Master servers: `master-1:9423`, `master-2:9423`, `master-3:9423`
  - Chunk servers: `chunkserver-1:9425`, `chunkserver-2:9425`, `chunkserver-3:9425`
  - Health endpoints: All services on port 9430
  - System metrics: All services on port 9100 (if node_exporter available)

**Key Metrics Available**:
- Master server metrics: `mooseng_master_*`
- ChunkServer metrics: `mooseng_chunkserver_*`
- Client metrics: `mooseng_client_*`
- Health status: `mooseng_health_*`
- System metrics: `node_*` (if configured)

**Validation Commands**:
```bash
# Check Prometheus targets status
curl http://localhost:9090/api/v1/targets

# Query sample metrics
curl 'http://localhost:9090/api/v1/query?query=up'

# Check configuration
curl http://localhost:9090/api/v1/status/config
```

### Grafana Visualization (Port 3000)

**Access**: `http://localhost:3000`  
**Credentials**: admin/admin (change on first login)

**Pre-configured Dashboards**:
1. **MooseNG Health Dashboard** - Service status and connectivity
2. **Performance Dashboard** - Throughput, latency, and IOPS
3. **Storage Dashboard** - Disk usage, chunk distribution
4. **Multi-region Performance** - Cross-region metrics

**Data Source Configuration**:
- Prometheus URL: `http://prometheus:9090`
- Query timeout: 60s
- Scrape interval: 15s

**First-time Setup**:
```bash
# Access Grafana
open http://localhost:3000

# Login with admin/admin
# Change password when prompted
# Navigate to Dashboards > Browse to view pre-configured dashboards
```

### MooseNG Web Dashboard (Port 8080)

**Access**: `http://localhost:8080`

**Features**:
- Real-time cluster status
- Interactive performance charts
- Benchmark results integration
- Custom MooseNG-specific UI components

**Technology Stack**:
- Frontend: HTML5, CSS3, JavaScript (ES6+)
- Charts: Chart.js, D3.js
- Served via: Nginx Alpine container

### Log Analysis & Aggregation

**Centralized Logging**:
```bash
# View all services logs
docker-compose logs -f

# Filter by service type
docker-compose logs -f master-1 master-2 master-3
docker-compose logs -f chunkserver-1 chunkserver-2 chunkserver-3
docker-compose logs -f client-1 client-2 client-3

# Filter by severity
docker-compose logs | grep -E "(ERROR|WARN)"

# Follow specific service with timestamps
docker-compose logs -f --timestamps master-1
```

**Log Locations Inside Containers**:
- Master logs: `/var/log/mooseng/master.log`
- ChunkServer logs: `/var/log/mooseng/chunkserver.log`
- Client logs: `/var/log/mooseng/client.log`

### Monitoring Stack Health Verification

**Complete Health Check Script**:
```bash
#!/bin/bash
echo "=== MooseNG Monitoring Stack Health Check ==="

# Check Prometheus
echo "Checking Prometheus..."
if curl -s http://localhost:9090/-/healthy > /dev/null; then
    echo "✅ Prometheus: Healthy"
else
    echo "❌ Prometheus: Unhealthy"
fi

# Check Grafana
echo "Checking Grafana..."
if curl -s http://localhost:3000/api/health > /dev/null; then
    echo "✅ Grafana: Healthy"
else
    echo "❌ Grafana: Unhealthy"
fi

# Check Dashboard
echo "Checking Web Dashboard..."
if curl -s http://localhost:8080 > /dev/null; then
    echo "✅ Dashboard: Healthy"
else
    echo "❌ Dashboard: Unhealthy"
fi

# Check metrics endpoints
echo "Checking metrics endpoints..."
for port in 9423 9433 9443 9425 9455 9465; do
    if curl -s http://localhost:$port/metrics > /dev/null; then
        echo "✅ Port $port: Metrics available"
    else
        echo "⚠️  Port $port: Metrics unavailable"
    fi
done
```

## 🔍 Troubleshooting

### Common Issues

**Services not starting**:
```bash
# Check Docker resources
docker system df
docker system prune -f

# Rebuild images
./start-unified-demo.sh build
```

**Port conflicts**:
```bash
# Check what's using ports
netstat -tulpn | grep :9421
lsof -i :9421

# Modify docker-compose.yml ports section
```

**FUSE mounting issues**:
```bash
# Ensure privileged mode for clients
# Check /dev/fuse availability
docker run --rm --privileged alpine ls -la /dev/fuse
```

**Raft cluster issues**:
```bash
# Check master connectivity
docker exec mooseng-master-1 nc -zv master-2 9422
docker exec mooseng-master-1 nc -zv master-3 9422

# View Raft logs
docker-compose logs master-1 | grep -i raft
```

### Monitoring Stack Troubleshooting

#### Prometheus Issues

**Prometheus not starting**:
```bash
# Check Prometheus container logs
docker-compose logs prometheus

# Validate configuration syntax
docker exec mooseng-prometheus promtool check config /etc/prometheus/prometheus.yml

# Check file permissions
docker exec mooseng-prometheus ls -la /etc/prometheus/
```

**Missing metrics/targets**:
```bash
# Check targets status in Prometheus UI
open http://localhost:9090/targets

# Manually test metrics endpoints
curl http://localhost:9423/metrics  # master-1
curl http://localhost:9425/metrics  # chunkserver-1

# Check network connectivity
docker exec mooseng-prometheus nc -zv master-1 9423
```

**Configuration issues**:
```bash
# Reload Prometheus configuration
curl -X POST http://localhost:9090/-/reload

# Check configuration status
curl http://localhost:9090/api/v1/status/config

# Validate scrape targets
curl http://localhost:9090/api/v1/targets
```

#### Grafana Issues

**Grafana not accessible**:
```bash
# Check Grafana container status
docker-compose ps grafana

# Check logs for errors
docker-compose logs grafana

# Verify port binding
netstat -tulpn | grep :3000
```

**Dashboard not loading**:
```bash
# Check data source connection
curl http://localhost:3000/api/datasources/proxy/1/api/v1/query?query=up

# Verify dashboard provisioning
docker exec mooseng-grafana ls -la /etc/grafana/provisioning/dashboards/

# Check dashboard files
docker exec mooseng-grafana ls -la /etc/grafana/provisioning/dashboards/*.json
```

**Login issues**:
```bash
# Reset admin password
docker exec -it mooseng-grafana grafana-cli admin reset-admin-password newpassword

# Check user authentication logs
docker-compose logs grafana | grep -i auth
```

#### Web Dashboard Issues

**Dashboard not loading**:
```bash
# Check nginx container
docker-compose logs dashboard

# Verify static files
docker exec mooseng-dashboard ls -la /usr/share/nginx/html/

# Test direct file access
curl http://localhost:8080/index.html
```

**JavaScript errors**:
```bash
# Check browser console for errors
# Verify all JavaScript dependencies load correctly
# Check network tab for failed requests

# Test API connectivity from dashboard
curl http://localhost:8080/src/ApiClient.js
```

**API connectivity issues**:
```bash
# Check if dashboard can reach Prometheus
docker exec mooseng-dashboard nc -zv prometheus 9090

# Verify CORS configuration if needed
# Check browser network tab for blocked requests
```

#### Network and Connectivity

**Inter-service connectivity**:
```bash
# Test network connectivity between monitoring components
docker exec mooseng-prometheus nc -zv grafana 3000
docker exec mooseng-grafana nc -zv prometheus 9090

# Check Docker network configuration
docker network inspect mooseng_mooseng-net

# Verify all services are on same network
docker inspect mooseng-prometheus | grep NetworkMode
docker inspect mooseng-grafana | grep NetworkMode
```

**DNS resolution issues**:
```bash
# Test hostname resolution
docker exec mooseng-prometheus nslookup master-1
docker exec mooseng-grafana nslookup prometheus

# Check /etc/hosts entries
docker exec mooseng-prometheus cat /etc/hosts
```

#### Performance and Resource Issues

**High memory usage**:
```bash
# Check container resource usage
docker stats

# Monitor Prometheus memory usage
curl http://localhost:9090/api/v1/query?query=process_resident_memory_bytes

# Adjust retention policies in prometheus.yml
# --storage.tsdb.retention.time=15d
```

**Slow dashboard loading**:
```bash
# Check query performance in Prometheus
curl 'http://localhost:9090/api/v1/query?query=up&timeout=5s'

# Monitor Grafana query performance
# Check browser developer tools Network tab

# Optimize dashboard queries and time ranges
```

#### Data and Metrics Issues

**Missing historical data**:
```bash
# Check Prometheus data directory
docker exec mooseng-prometheus ls -la /prometheus/

# Verify data retention settings
docker exec mooseng-prometheus cat /etc/prometheus/prometheus.yml | grep retention

# Check for data corruption
docker exec mooseng-prometheus promtool tsdb list /prometheus/
```

**Incorrect metric values**:
```bash
# Validate metric collection at source
curl http://localhost:9423/metrics | grep mooseng_

# Check metric types and labels
curl 'http://localhost:9090/api/v1/label/__name__/values'

# Verify scrape intervals and timing
curl http://localhost:9090/api/v1/targets
```

### Quick Diagnostic Commands

```bash
# Complete monitoring stack health check
echo "=== Monitoring Stack Diagnostic ==="
docker-compose ps prometheus grafana dashboard
echo ""

echo "=== Service Connectivity ==="
for service in prometheus:9090 grafana:3000 dashboard:8080; do
    if curl -s --max-time 5 http://localhost:${service##*:} >/dev/null; then
        echo "✅ $service: Reachable"
    else
        echo "❌ $service: Unreachable"
    fi
done
echo ""

echo "=== Metrics Endpoints ==="
for port in 9423 9433 9443 9425 9455 9465; do
    if curl -s --max-time 3 http://localhost:$port/metrics >/dev/null; then
        echo "✅ localhost:$port/metrics: Available"
    else
        echo "❌ localhost:$port/metrics: Unavailable"
    fi
done
```

### Health Checks
All services include health checks:
```bash
# Check health status
docker-compose ps --filter "health=healthy"
docker-compose ps --filter "health=unhealthy"

# Manual health check
docker exec mooseng-master-1 /usr/local/bin/mooseng-master --health-check
```

## 🚧 Development & Customization

### Building from Source
```bash
# Build specific service
docker-compose build master-1

# Force rebuild without cache
docker-compose build --no-cache master-1

# Build in parallel
docker-compose build --parallel
```

### Custom Configuration
1. Modify `docker/configs/*.toml` templates
2. Update `docker-compose.yml` environment variables
3. Rebuild affected services

### Adding Services
1. Add service definition to `docker-compose.yml`
2. Create Dockerfile in `docker/`
3. Add monitoring configuration to `prometheus.yml`

## 📚 References

- [MooseNG Architecture](README.md)
- [Docker Compose Reference](https://docs.docker.com/compose/)
- [Prometheus Configuration](https://prometheus.io/docs/prometheus/latest/configuration/configuration/)
- [Grafana Documentation](https://grafana.com/docs/)

## 🎯 Next Steps

After running the demo:
1. **Explore Grafana dashboards** - Understanding metrics and performance
2. **Test file operations** - Create, read, update, delete files
3. **Simulate failures** - Stop services and observe failover behavior
4. **Scale testing** - Add more clients or chunkservers
5. **Performance benchmarking** - Run comprehensive load tests

---

*Generated by MooseNG Docker Demo System - For production deployment, review security settings and resource limits.*