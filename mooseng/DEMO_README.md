# MooseNG Distributed Filesystem Demo

This demo showcases a distributed filesystem implementation with 3 masters, 3 chunkservers, and 3 clients using Docker Compose.

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    MooseNG Cluster                      │
├─────────────────┬─────────────────┬─────────────────────┤
│   Master Ring   │  Chunkservers   │      Clients        │
│                 │                 │                     │
│   master-1      │  chunkserver-1  │     client-1        │
│   master-2      │  chunkserver-2  │     client-2        │
│   master-3      │  chunkserver-3  │     client-3        │
│                 │                 │                     │
│ (Raft Consensus)│ (Data Storage)  │ (FUSE Mounts)       │
└─────────────────┴─────────────────┴─────────────────────┘
```

### Services

- **3 Master Servers** (ports 9421-9423, 9431-9433, 9441-9443)
  - Raft consensus for high availability
  - Metadata management
  - Client coordination

- **3 Chunkservers** (ports 9420-9425, 9450-9455, 9460-9465)
  - Distributed data storage
  - 4 storage directories each
  - Replication management

- **3 Clients** (ports 9427, 9437, 9447)
  - FUSE filesystem interface
  - Concurrent access support
  - Caching layer

- **Monitoring Stack**
  - Prometheus (port 9090)
  - Grafana (port 3000)
  - Web Dashboard (port 8080)

## 🚀 Quick Start

### Prerequisites

- Docker and Docker Compose
- 8GB+ RAM recommended
- 10GB+ disk space

### Launch the Demo

```bash
# Start the complete cluster
./start-demo.sh

# Check service status  
./start-demo.sh status

# View logs
./start-demo.sh logs

# Run basic tests
./start-demo.sh test

# Stop the cluster
./start-demo.sh stop
```

### Individual Commands

```bash
# Build services only
./demo-real-services.sh build

# Clean up everything
./demo-real-services.sh clean
```

## 📊 Monitoring

- **Service Dashboard**: http://localhost:8080
- **Prometheus Metrics**: http://localhost:9090
- **Grafana Dashboard**: http://localhost:3000 (admin/admin)

## 🔧 Service Configuration

### Environment Variables

Each service can be configured via environment variables:

#### Master Services
```bash
MOOSENG_NODE_ID=master-1           # Unique node identifier
MOOSENG_CLUSTER_ID=mooseng-cluster # Cluster identifier
MOOSENG_RAFT_PEERS=master-1:9422,master-2:9422,master-3:9422
MOOSENG_LISTEN_PORT=9421           # Client API port
MOOSENG_GRPC_PORT=9422            # Raft consensus port
MOOSENG_METRICS_PORT=9423         # Prometheus metrics
MOOSENG_LOG_LEVEL=info            # Logging level
```

#### Chunkserver Services
```bash
MOOSENG_SERVER_ID=chunkserver-1    # Unique server identifier
MOOSENG_MASTER_ADDRS=master-1:9421,master-2:9421,master-3:9421
MOOSENG_LISTEN_PORT=9420           # Service port
MOOSENG_DATA_DIRS=/data/disk1,/data/disk2,/data/disk3,/data/disk4
MOOSENG_LOG_LEVEL=info            # Logging level
```

#### Client Services
```bash
MOOSENG_MASTER_ADDRS=master-1:9421,master-2:9421,master-3:9421
MOOSENG_MOUNT_POINT=/mnt/mooseng   # FUSE mount point
MOOSENG_CACHE_SIZE_MB=256          # Client cache size
MOOSENG_LOG_LEVEL=info            # Logging level
```

## 🧪 Testing Filesystem Operations

### Basic File Operations

```bash
# Enter a client container
docker-compose exec client-1 sh

# Create files
echo "Hello MooseNG!" > /mnt/mooseng/test.txt
echo "Distributed FS" > /mnt/mooseng/readme.txt

# List files
ls -la /mnt/mooseng/

# Read files
cat /mnt/mooseng/test.txt
```

### Test Replication

```bash
# Create file on client-1
docker-compose exec client-1 sh -c "echo 'Replicated data' > /mnt/mooseng/shared.txt"

# Verify on client-2
docker-compose exec client-2 sh -c "cat /mnt/mooseng/shared.txt"

# Verify on client-3
docker-compose exec client-3 sh -c "cat /mnt/mooseng/shared.txt"
```

### Performance Testing

```bash
# Large file creation
docker-compose exec client-1 sh -c "dd if=/dev/zero of=/mnt/mooseng/largefile bs=1M count=100"

# Concurrent access test
for i in {1..3}; do
  docker-compose exec client-$i sh -c "echo 'Client $i data' > /mnt/mooseng/client$i.txt" &
done
wait
```

## 🔍 Troubleshooting

### Service Health Checks

```bash
# Check all container status
docker-compose ps

# Check specific service logs
docker-compose logs master-1
docker-compose logs chunkserver-1
docker-compose logs client-1

# Check service health endpoints
curl http://localhost:9421/health  # master-1
curl http://localhost:9420/health  # chunkserver-1
```

### Common Issues

1. **Services won't start**
   - Check Docker daemon is running
   - Ensure ports aren't already in use
   - Verify adequate system resources

2. **Compilation errors**
   - The Rust codebase is under development
   - Some services may have unresolved dependencies
   - Use mock services as fallback

3. **Mount issues**
   - Clients need privileged mode for FUSE
   - Check /dev/fuse device availability
   - Verify mount point permissions

4. **Network connectivity**
   - All services use custom Docker network
   - Check inter-service DNS resolution
   - Verify firewall settings

### Log Analysis

```bash
# View all service logs
docker-compose logs --follow

# Filter specific service logs
docker-compose logs --follow master-1

# View recent errors
docker-compose logs --tail=50 | grep -i error
```

## 🚧 Development Status

### Compilation Status

- ✅ **Client Service**: Compiles successfully with warnings
- ⚠️ **Master Service**: Has compilation errors (21 errors, 152 warnings)
- ⚠️ **Chunkserver Service**: Has compilation errors (18 errors, 44 warnings)

### Known Issues

1. **Master Service**: Multiple struct definition conflicts, import resolution issues
2. **Chunkserver Service**: Import path resolution, type mismatches
3. **Protocol Definitions**: Some gRPC interfaces incomplete

### Next Steps

1. **Fix Compilation Errors**: Resolve Rust compilation issues
2. **Complete Protocol**: Finish gRPC service definitions
3. **Add Integration Tests**: Comprehensive test suite
4. **Performance Optimization**: Benchmark and optimize
5. **Documentation**: Complete API documentation

## 🤝 Contributing

To contribute to fixing compilation issues:

1. **Master Service**: Focus on `src/cache.rs`, `src/raft/membership.rs`, `src/grpc_services.rs`
2. **Chunkserver Service**: Fix import paths in `src/storage.rs`
3. **Tests**: Add unit and integration tests
4. **Documentation**: Improve code documentation

## 📝 Configuration Files

- `docker-compose.yml`: Main service orchestration
- `docker/configs/*.toml`: Service configuration templates
- `docker/scripts/*-entrypoint.sh`: Service startup scripts
- `docker/Dockerfile.*`: Service build definitions

## 🎯 Production Readiness

This demo is for **development and testing** purposes. For production deployment:

1. **Security**: Add TLS, authentication, authorization
2. **Persistence**: Configure persistent storage volumes
3. **Monitoring**: Set up comprehensive monitoring and alerting
4. **Backup**: Implement backup and disaster recovery
5. **Scaling**: Test with realistic workloads and cluster sizes

## 📚 Additional Resources

- [MooseFS Documentation](https://moosefs.com/documentation/)
- [Docker Compose Reference](https://docs.docker.com/compose/)
- [Rust Async Programming](https://rust-lang.github.io/async-book/)
- [gRPC Protocol Documentation](https://grpc.io/docs/)