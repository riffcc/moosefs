# MooseNG - Next Generation Distributed File System

MooseNG is a high-performance, fault-tolerant, distributed file system built in Rust, designed to be the next evolution of MooseFS. It provides scalable storage with advanced features like erasure coding, multi-region replication, and tiered storage.

## =€ Features

### Core Features
- **High Availability**: Raft consensus algorithm for master server redundancy
- **Scalability**: Horizontal scaling across multiple regions
- **Performance**: Zero-copy data paths and async I/O throughout
- **Reliability**: Reed-Solomon erasure coding with configurable redundancy
- **Flexibility**: Tiered storage with automatic data movement
- **Security**: TLS encryption for all inter-component communication

### Advanced Features
- **Multi-Region Support**: Active-active-active 3-region deployment
- **Metadata Caching**: Advanced caching with LRU eviction and prefetching
- **Compression**: Transparent compression with multiple algorithms (LZ4, Zstd, Gzip)
- **Health Monitoring**: Comprehensive health checks with self-healing capabilities
- **Observability**: Prometheus metrics and Grafana dashboards
- **Container Ready**: Docker and Kubernetes deployment support

## <× Architecture

MooseNG consists of five main components:

### 1. Master Server (`mooseng-master`)
- **Role**: Metadata management and cluster coordination
- **Features**: 
  - Raft consensus for high availability
  - Multi-region consistency management
  - Advanced metadata caching
  - Storage class management
  - Topology discovery

### 2. Chunk Server (`mooseng-chunkserver`)
- **Role**: Data storage and retrieval
- **Features**:
  - Reed-Solomon erasure coding
  - Tiered storage management
  - Zero-copy data operations
  - Compression services
  - Health monitoring

### 3. Client (`mooseng-client`)
- **Role**: FUSE-based file system interface
- **Features**:
  - POSIX-compliant file operations
  - Client-side caching
  - Connection pooling
  - Health monitoring

### 4. Metalogger (`mooseng-metalogger`)
- **Role**: Metadata backup and replication
- **Features**:
  - Real-time metadata replication
  - Write-ahead logging (WAL)
  - Snapshot management
  - Recovery capabilities

### 5. CLI Tools (`mooseng-cli`)
- **Role**: Cluster management and monitoring
- **Features**:
  - Cluster administration
  - Performance monitoring
  - Health status checking
  - Configuration management

## =' Building from Source

### Prerequisites
- Rust 1.70 or later
- Protocol Buffers compiler (`protoc`)
- OpenSSL development libraries
- CMake (for some dependencies)

### Build Instructions

```bash
# Clone the repository
git clone <repository-url>
cd mooseng

# Build all components
cargo build --release

# Run tests (Note: Some tests may need fixing)
cargo test --workspace --lib

# Build individual components
cargo build -p mooseng-master --release
cargo build -p mooseng-chunkserver --release
cargo build -p mooseng-client --release
cargo build -p mooseng-metalogger --release
cargo build -p mooseng-cli --release
```

## =3 Docker Deployment

### Quick Start with Docker Compose

```bash
# Basic deployment
docker-compose up -d

# Multi-region deployment
docker-compose -f docker-compose.multiregion.yml up -d

# Development/testing environment
docker-compose -f docker-compose.test.yml up -d
```