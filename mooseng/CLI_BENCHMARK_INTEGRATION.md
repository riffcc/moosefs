# MooseNG CLI Tools and Unified Benchmark Suite Integration

This document describes the integration between MooseNG CLI tools, the unified benchmark suite, and the benchmark dashboard.

## Overview

The integration provides a seamless experience for:
- Managing MooseNG clusters via CLI tools
- Running benchmarks through both CLI and unified runner
- Viewing results through a web dashboard
- Historical analysis and trend detection

## Components

### 1. CLI Tools (`mooseng-cli/`)

**Features:**
- Cluster management (`mooseng cluster status`, `mooseng cluster init`)
- Real-time monitoring (`mooseng monitor metrics`, `mooseng monitor health`)
- Benchmark execution (`mooseng benchmark quick`, `mooseng benchmark full`)
- Configuration management (`mooseng config`)
- Data operations (`mooseng data upload`, `mooseng data download`)

**gRPC Integration:**
- Connects to MooseNG master servers
- Automatic failover and retry logic
- Robust error handling and fallback modes
- Configuration persistence

**Usage Examples:**
```bash
# Check cluster status
mooseng cluster status --verbose

# Run quick benchmark
mooseng benchmark quick --output ./results --iterations 100

# Monitor cluster in real-time
mooseng monitor metrics --interval 5

# Initialize new cluster
mooseng cluster init --masters "master1:9421,master2:9421" --name prod-cluster
```

### 2. Unified Benchmark Suite (`mooseng-benchmarks/`)

**Features:**
- Consolidated benchmark framework with modular architecture
- Multiple benchmark categories (file operations, metadata, network, multiregion)
- Real-time monitoring and live updates
- Historical result storage and analysis
- Configurable test parameters

**Command-line Interface:**
```bash
# Run all benchmarks
mooseng-bench all --output ./results

# Run specific categories
mooseng-bench category files metadata

# Run individual benchmarks
mooseng-bench benchmark small_files large_files --iterations 200

# Live monitoring
mooseng-bench monitor --interval 10 --duration 60

# List available benchmarks
mooseng-bench list --detailed

# Analyze existing results
mooseng-bench analyze ./results --compare --baseline ./baseline.json
```

**Configuration:**
```toml
# benchmark_config.toml
[benchmark]
warmup_iterations = 10
measurement_iterations = 100
file_sizes = [1024, 65536, 1048576, 10485760]
concurrency_levels = [1, 10, 50, 100]
detailed_report = true

[dashboard]
enabled = true
port = 8080
realtime_updates = true

[database]
url = "sqlite:benchmark_results.db"
```

### 3. Benchmark Dashboard (`mooseng-benchmarks/` dashboard server)

**Features:**
- Web-based interface for viewing benchmark results
- Real-time WebSocket updates during benchmark runs
- Historical trend analysis and regression detection
- Performance metrics visualization
- SQLite/PostgreSQL database backend

**API Endpoints:**
- `GET /api/sessions` - List benchmark sessions
- `GET /api/sessions/{id}` - Get specific session
- `GET /api/results` - Query benchmark results with filters
- `GET /api/metrics` - Get performance metrics
- `GET /api/trends` - Analyze performance trends
- `WebSocket /ws` - Real-time updates

**Usage:**
```bash
# Start dashboard server
mooseng-dashboard serve --bind 0.0.0.0:8080

# Initialize database
mooseng-dashboard init --database ./benchmark.db

# Show statistics
mooseng-dashboard stats

# Clean up old data
mooseng-dashboard cleanup --retention-days 90
```

## Integration Architecture

```
┌─────────────────┐    ┌───────────────────┐    ┌─────────────────┐
│   CLI Tools     │    │ Unified Benchmark │    │   Dashboard     │
│  (mooseng-cli)  │    │      Suite        │    │    Server       │
│                 │    │ (mooseng-bench)   │    │                 │
├─────────────────┤    ├───────────────────┤    ├─────────────────┤
│ • Cluster Mgmt  │◄──►│ • File Operations │◄──►│ • Web Interface │
│ • Monitoring    │    │ • Metadata Tests  │    │ • Real-time     │
│ • Benchmarking  │    │ • Network Tests   │    │ • Historical    │
│ • Configuration │    │ • Multi-region    │    │ • Trends        │
└─────────────────┘    └───────────────────┘    └─────────────────┘
         │                        │                        │
         └────────────────────────┼────────────────────────┘
                                  │
                    ┌─────────────▼──────────────┐
                    │      Database              │
                    │   (SQLite/PostgreSQL)     │
                    │                            │
                    │ • Sessions                 │
                    │ • Results                  │
                    │ • Metrics                  │
                    │ • Trends                   │
                    └────────────────────────────┘
```

## Workflow Integration

### 1. CLI-Driven Benchmarking
```bash
# CLI calls unified benchmark runner
mooseng benchmark full --config my-config.toml
# → Executes: cargo run --bin mooseng-bench -- all --config my-config.toml
# → Results stored in database
# → Viewable via dashboard
```

### 2. Direct Benchmark Execution
```bash
# Direct unified runner usage
mooseng-bench all --output ./results --format json
# → Results stored in database
# → Real-time updates via WebSocket
# → Historical analysis available
```

### 3. Dashboard Monitoring
```bash
# Start dashboard
mooseng-dashboard serve --bind 0.0.0.0:8080
# → Open http://localhost:8080
# → View real-time benchmark progress
# → Analyze historical trends
```

## Configuration Management

### CLI Configuration
- Location: `~/.mooseng/cli.toml`
- Environment variables: `MOOSENG_MASTERS`, `MOOSENG_CONNECT_TIMEOUT`
- Generated via: `mooseng config generate`

### Benchmark Configuration
- Location: `./benchmark_config.toml` or specified via `--config`
- Generated via: `mooseng-bench config generate`
- Validated via: `mooseng-bench config validate`

### Dashboard Configuration
- Embedded in benchmark configuration
- Database URL configurable
- Static files path configurable

## Error Handling and Fallbacks

### CLI Tools
- Automatic fallover between master servers
- Graceful degradation to placeholder data when gRPC unavailable
- Comprehensive error messages with suggestions

### Benchmark Suite
- Fallback to legacy implementations when unified runner unavailable
- Graceful handling of individual benchmark failures
- Continued execution with `--continue-on-error` flag

### Dashboard
- Handles database connection failures
- Provides read-only mode when write operations fail
- WebSocket reconnection logic

## Development and Testing

### Running the Integration Demo
```bash
# Run the comprehensive integration demo
./integration_demo.sh
```

### Building Components
```bash
# Build CLI tools
cd mooseng-cli && cargo build --release

# Build benchmark suite
cd mooseng-benchmarks && cargo build --release

# Build all binaries
cargo build --release --workspace
```

### Testing
```bash
# Test CLI functionality
cargo test --package mooseng-cli

# Test benchmark framework
cargo test --package mooseng-benchmarks

# Run integration tests
./test_integration.sh
```

## Production Deployment

### 1. CLI Tools Installation
```bash
# Install CLI tools
cargo install --path mooseng-cli

# Or use pre-built binaries
cp target/release/mooseng /usr/local/bin/
```

### 2. Dashboard Deployment
```bash
# Production dashboard setup
mooseng-dashboard serve \
  --bind 0.0.0.0:8080 \
  --database postgresql://user:pass@localhost/benchmarks \
  --static-dir /var/www/mooseng-dashboard
```

### 3. Automated Benchmarking
```bash
# CI/CD integration
mooseng-bench all \
  --output /var/log/benchmarks \
  --config /etc/mooseng/benchmark.toml \
  --format json
```

## Monitoring and Alerting

### Performance Regression Detection
- Automatic trend analysis
- Configurable thresholds
- Email/webhook notifications
- Integration with monitoring systems

### Real-time Monitoring
- WebSocket-based live updates
- Performance metrics streaming
- Resource utilization tracking
- Health status monitoring

## Troubleshooting

### Common Issues

1. **CLI connection failures**
   - Check master server addresses in configuration
   - Verify network connectivity
   - Check TLS certificate issues

2. **Benchmark compilation errors**
   - Ensure all dependencies are available
   - Check Rust version compatibility
   - Verify workspace configuration

3. **Dashboard not loading**
   - Check static files path
   - Verify database connection
   - Check port binding permissions

### Debug Mode
```bash
# Enable verbose logging
RUST_LOG=debug mooseng cluster status
RUST_LOG=debug mooseng-bench all
RUST_LOG=debug mooseng-dashboard serve
```

## Future Enhancements

### Planned Features
- Prometheus metrics export
- Grafana dashboard templates
- Kubernetes deployment manifests
- Advanced trend analysis algorithms
- Multi-cluster benchmarking support

### Integration Roadmap
- REST API for external integrations
- Plugin system for custom benchmarks
- Advanced reporting and analytics
- Performance regression alerts
- Automated performance tuning recommendations

## Contributing

### Development Workflow
1. Make changes to CLI tools in `mooseng-cli/`
2. Enhance benchmark suite in `mooseng-benchmarks/`
3. Update dashboard as needed
4. Run integration tests
5. Update documentation

### Testing Checklist
- [ ] CLI tools build and run
- [ ] Benchmark suite compiles
- [ ] Dashboard server starts
- [ ] Integration demo passes
- [ ] All tests pass
- [ ] Documentation updated