# MooseNG Deployment Guide

This guide covers various deployment options for MooseNG, from development setups to production clusters.

## Quick Start with Docker Compose

The fastest way to get MooseNG running locally:

```bash
# Clone the repository
git clone https://github.com/moosefs/mooseng.git
cd mooseng

# Start the cluster
docker-compose up -d

# Check cluster status
docker-compose exec cli mooseng cluster status

# Access the filesystem
mkdir -p ./mnt
# The client container mounts MooseNG at /mnt/mooseng
```

## Docker Compose Deployment

### Services Included

- **3 Master Servers** (HA Raft cluster)
- **3 Chunk Servers** (distributed storage)
- **1 Metalogger** (metadata backup)
- **1 Client** (FUSE mount example)
- **CLI Tools** (cluster management)
- **Prometheus** (metrics collection)
- **Grafana** (visualization)

### Environment Variables

Key environment variables for configuration:

```bash
# Master servers
MOOSENG_CLUSTER_NAME=mooseng-dev
MOOSENG_RAFT_PEERS=master-1:9423,master-2:9423,master-3:9423

# Chunk servers
MOOSENG_MASTER_ENDPOINTS=master-1:9422,master-2:9422,master-3:9422
MOOSENG_CHUNK_SIZE_MB=64

# Client
MOOSENG_MASTER_ENDPOINTS=master-1:9421,master-2:9421,master-3:9421
MOOSENG_CACHE_SIZE_MB=512
```

### Scaling

Scale chunk servers:
```bash
docker-compose up -d --scale chunkserver=5
```

## Kubernetes Deployment

### Prerequisites

- Kubernetes 1.20+
- kubectl configured
- Storage classes: `fast-ssd`, `standard`

### Method 1: Raw Manifests

```bash
# Create namespace and basic resources
kubectl apply -f k8s/namespace.yaml

# Deploy master servers
kubectl apply -f k8s/master-statefulset.yaml

# Deploy chunk servers (DaemonSet)
kubectl apply -f k8s/chunkserver-daemonset.yaml

# Label nodes for chunk servers
kubectl label nodes worker-1 worker-2 worker-3 mooseng.io/chunkserver=true
```

### Method 2: Helm Chart

```bash
# Add the MooseNG Helm repository
helm repo add mooseng https://charts.mooseng.io
helm repo update

# Install with default values
helm install mooseng mooseng/mooseng -n mooseng --create-namespace

# Or with custom values
helm install mooseng mooseng/mooseng -n mooseng --create-namespace -f values-production.yaml
```

### Helm Configuration Examples

#### Development Setup
```yaml
# values-dev.yaml
master:
  replicaCount: 1
  resources:
    requests:
      memory: "512Mi"
      cpu: "250m"

chunkserver:
  deploymentType: "deployment"
  replicaCount: 2

monitoring:
  enabled: true
```

#### Production Setup
```yaml
# values-prod.yaml
master:
  replicaCount: 3
  persistence:
    storageClass: "fast-ssd"
    dataSize: 50Gi
  resources:
    requests:
      memory: "2Gi"
      cpu: "1000m"

chunkserver:
  deploymentType: "daemonset"
  nodeSelector:
    mooseng.io/chunkserver: "true"
  persistence:
    pvc:
      enabled: true
      storageClass: "fast-ssd"
      dataSize: 500Gi

metalogger:
  replicaCount: 2
  persistence:
    storageClass: "standard"
    dataSize: 100Gi

monitoring:
  enabled: true
  prometheus:
    enabled: true
    retention: "90d"
    storageSize: "50Gi"
```

## Storage Configuration

### Storage Classes

MooseNG supports multiple storage classes for different use cases:

```yaml
storage_classes:
- name: "default"
  copies: 2
  tier: "standard"
  
- name: "archive"
  copies: 1
  erasure_scheme: "8+4"
  tier: "cold"
  
- name: "critical"
  copies: 3
  tier: "hot"
```

### Chunk Server Storage

#### Docker Volumes
- Automatic volume management
- Good for development/testing

#### Host Paths (Kubernetes)
- Direct access to node storage
- Better performance
- Requires node preparation

#### Persistent Volume Claims
- Kubernetes-managed storage
- Portable across nodes
- Slightly higher overhead

## Networking

### Ports

| Service | Port | Description |
|---------|------|-------------|
| Master (Client) | 9421 | Client connections |
| Master (ChunkServer) | 9422 | ChunkServer connections |
| Master (Raft) | 9423 | Raft consensus |
| ChunkServer | 9420 | Chunk operations |
| Metalogger | 9419 | Metadata backup |
| Prometheus | 9090 | Metrics |
| Grafana | 3000 | Dashboard |

### Load Balancing

For production deployments, place a load balancer in front of master servers:

```yaml
# Kubernetes LoadBalancer service
apiVersion: v1
kind: Service
metadata:
  name: mooseng-master-lb
spec:
  type: LoadBalancer
  selector:
    app.kubernetes.io/component: master
  ports:
  - port: 9421
    targetPort: 9421
```

## Monitoring

### Prometheus Metrics

MooseNG exports metrics for:
- Master server operations
- Chunk server performance
- Client statistics
- Cluster health

### Grafana Dashboards

Pre-built dashboards for:
- Cluster overview
- Master server metrics
- Chunk server performance
- Client activity
- Storage utilization

### Health Checks

Built-in health checks available at:
- `http://master:9423/health` - Liveness probe
- `http://master:9423/ready` - Readiness probe

## CLI Management

### Cluster Operations

```bash
# Check cluster status
mooseng cluster status --verbose

# Scale chunk servers
mooseng cluster scale chunkserver --count 5

# Rolling upgrade
mooseng cluster upgrade --component all --version 1.1.0
```

### Administration

```bash
# List chunk servers
mooseng admin list-chunkservers --verbose

# Set maintenance mode
mooseng admin maintenance --id cs-001 --enable

# Balance cluster
mooseng admin balance --dry-run
```

### Monitoring

```bash
# Real-time metrics
mooseng monitor metrics --component all --interval 5

# Health status
mooseng monitor health --warnings

# Export metrics
mooseng monitor export --format json --output metrics.json
```

## Security

### Authentication

- Built-in authentication disabled by default
- Enable with `auth_enabled = true` in configuration
- Use TLS certificates for encrypted communication

### Network Policies

Example Kubernetes NetworkPolicy:

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: mooseng-network-policy
spec:
  podSelector:
    matchLabels:
      app.kubernetes.io/name: mooseng
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: mooseng
  egress:
  - to:
    - namespaceSelector:
        matchLabels:
          name: mooseng
```

## Backup and Recovery

### Metadata Backup

Metalogger automatically backs up metadata:
- Real-time changelog replication
- Configurable backup intervals
- Multiple backup retention policies

### Data Recovery

```bash
# Check data integrity
mooseng admin repair --dry-run

# Repair corrupted chunks
mooseng admin repair --chunk-id 12345

# Full cluster repair
mooseng admin repair
```

## Troubleshooting

### Common Issues

1. **Master won't start**
   - Check Raft peer configuration
   - Verify persistent storage
   - Review logs for errors

2. **Chunk server connection issues**
   - Verify master endpoints
   - Check network connectivity
   - Review firewall rules

3. **Client mount failures**
   - Ensure FUSE is installed
   - Check mount permissions
   - Verify master connectivity

### Log Locations

- Docker: `docker-compose logs <service>`
- Kubernetes: `kubectl logs -n mooseng <pod>`
- Host: `/var/log/mooseng/`

### Debug Mode

Enable debug logging:
```bash
export RUST_LOG=debug
# or set in configuration
log_level = "debug"
```

## Performance Tuning

### Master Server

- Increase cache size for metadata-heavy workloads
- Tune Raft parameters for network latency
- Scale horizontally with more replicas

### Chunk Server

- Use fast SSD storage for chunks
- Increase worker threads for high I/O
- Optimize chunk size for workload

### Client

- Increase cache sizes for read-heavy workloads
- Tune read-ahead for sequential access
- Configure appropriate timeouts

### Resource Allocation

Recommended resource allocation:

| Component | CPU | Memory | Storage |
|-----------|-----|--------|---------|
| Master | 1-2 cores | 2-4 GB | 10-50 GB |
| ChunkServer | 2-4 cores | 4-8 GB | 100GB-10TB |
| Metalogger | 0.5-1 core | 1-2 GB | 20-100 GB |
| Client | 0.5-1 core | 1-2 GB | - |

This completes the deployment guide for MooseNG across different environments and use cases.