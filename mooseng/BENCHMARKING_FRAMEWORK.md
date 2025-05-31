# MooseNG Comprehensive Benchmarking Framework

## Overview

This document describes the comprehensive benchmarking framework for MooseNG, which provides three main parallelizable components for testing distributed file system performance under real-world conditions.

## Framework Components

### 1. Real Network Test Implementation (`real_network.rs`)

**Purpose**: Test MooseNG performance under realistic network conditions including variable latency, bandwidth limitations, and packet loss.

**Key Features**:
- Traffic control (tc) integration for Linux network emulation
- Predefined network scenarios (satellite, mobile 4G, intercontinental, datacenter, congested)
- Cross-platform Docker-based testing for non-Linux environments
- Real TCP/UDP throughput and latency measurements
- Packet loss simulation and measurement

**Network Scenarios**:
- **Satellite**: 600ms latency, 1Mbps bandwidth, 1% packet loss
- **Mobile 4G**: 50ms latency, 10Mbps bandwidth, 0.5% packet loss  
- **Intercontinental**: 150ms latency, 100Mbps bandwidth, 0.1% packet loss
- **Datacenter**: 1ms latency, 1000Mbps bandwidth, 0.01% packet loss
- **Congested**: 200ms latency, 1Mbps bandwidth, 5% packet loss

### 2. Multi-region Testing Infrastructure (`infrastructure.rs`)

**Purpose**: Deploy and test MooseNG across multiple geographic regions using cloud infrastructure.

**Key Features**:
- Terraform-based infrastructure provisioning across multiple cloud regions
- Kubernetes orchestration for application deployment
- Automated latency testing between regions
- Resource provisioning and cleanup automation
- Support for AWS, GCP, and Azure deployments

**Infrastructure Components**:
- VPC and networking setup per region
- Security group configuration
- EC2/compute instance provisioning
- Kubernetes cluster deployment
- Inter-region connectivity testing

### 3. Performance Metrics Collection and Analysis (`metrics.rs`, `report.rs`)

**Purpose**: Comprehensive metrics collection, analysis, and visualization for benchmark results.

**Key Features**:
- Real-time metrics collection with system resource monitoring
- Statistical analysis (mean, median, p95, p99, standard deviation)
- Multiple export formats (JSON, CSV, Prometheus, InfluxDB)
- Performance regression detection and comparison
- HTML reports with interactive charts
- Grafana dashboard generation

**Metrics Collected**:
- Operation latency and throughput
- CPU and memory utilization
- Network and disk I/O
- Error rates and success rates
- Custom operation metadata

## CLI Tool Usage

The `benchmark_runner` CLI provides easy access to all framework capabilities:

### Network Testing
```bash
# Test with specific network conditions
cargo run --bin benchmark_runner -- network --interface eth0 --scenarios satellite,datacenter

# Test with custom data sizes
cargo run --bin benchmark_runner -- network --data-sizes 1024,65536,1048576
```

### Infrastructure Testing
```bash
# Deploy and test multi-region infrastructure
cargo run --bin benchmark_runner -- infrastructure --regions us-east-1,eu-west-1,ap-southeast-1

# Dry run (generate Terraform config without deployment)
cargo run --bin benchmark_runner -- infrastructure --dry-run
```

### Complete Benchmark Suite
```bash
# Run all benchmarks with real network and infrastructure testing
cargo run --bin benchmark_runner -- suite --real-network --infrastructure --iterations 100

# Run specific benchmarks only
cargo run --bin benchmark_runner -- suite --benchmarks small_files,metadata_operations
```

### Analysis and Reporting
```bash
# Analyze results with baseline comparison
cargo run --bin benchmark_runner -- analyze --results-dir ./results --baseline baseline.json --format prometheus

# Generate HTML report with charts
cargo run --bin benchmark_runner -- report --input results.json --format html --charts --title "Performance Report"

# Generate Grafana dashboard
cargo run --bin benchmark_runner -- dashboard --dashboard-type grafana --output-file dashboard.json
```

## Integration with CI/CD

The framework supports automated testing in CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
- name: Run MooseNG Benchmarks
  run: |
    cd mooseng-benchmarks
    cargo run --bin benchmark_runner -- suite --iterations 50 --output ./ci-results
    cargo run --bin benchmark_runner -- report --input ./ci-results/complete_benchmark_results.json --format html
```

## Performance Monitoring Integration

### Prometheus Metrics
The framework exports metrics in Prometheus format for integration with monitoring systems:

```bash
# Export to Prometheus format
cargo run --bin benchmark_runner -- analyze --format prometheus > metrics.prom
```

### Grafana Dashboards
Pre-configured Grafana dashboards for visualizing benchmark results:

```bash
# Generate Grafana dashboard configuration
cargo run --bin benchmark_runner -- dashboard --dashboard-type grafana --output-file mooseng-dashboard.json
```

### Alerting Rules
Prometheus alerting rules for performance regressions:

```bash
# Generate Prometheus alerting rules
cargo run --bin benchmark_runner -- dashboard --dashboard-type prometheus --output-file alerts.yml
```

## Testing the Framework

Run the comprehensive test suite:

```bash
./test_benchmark_framework.sh
```

This script validates:
- Compilation and dependencies
- Unit test coverage
- CLI functionality
- Basic benchmark execution
- Report generation
- Dashboard creation

## Architecture Benefits

### Parallelization
The three main components can be executed independently:
1. **Real Network Tests** can run on dedicated network testing infrastructure
2. **Multi-region Infrastructure** can deploy to different cloud accounts/regions
3. **Metrics Collection** can process results from multiple benchmark runs simultaneously

### Scalability
- Network tests scale to different interface types and conditions
- Infrastructure tests scale across multiple cloud providers and regions
- Metrics collection scales with memory limits and disk storage

### Extensibility
- New network scenarios can be easily added
- Additional cloud providers can be integrated
- Custom metrics and reports can be implemented

## Requirements

### System Requirements
- Linux system with `tc` (traffic control) support for network testing
- Docker for cross-platform network emulation
- Terraform for infrastructure provisioning
- kubectl for Kubernetes deployments

### Permissions
- Root access or CAP_NET_ADMIN for traffic control
- Cloud provider credentials for infrastructure deployment
- Kubernetes cluster access for application deployment

### Dependencies
All dependencies are managed through Cargo.toml:
- tokio for async runtime
- serde for serialization
- clap for CLI interface
- criterion for benchmarking
- plotters for chart generation
- tracing for logging

## Future Enhancements

1. **Additional Network Conditions**: Support for more complex network topologies
2. **Cloud Provider Expansion**: Add support for more cloud providers
3. **Real-time Monitoring**: Live dashboard updates during benchmark execution
4. **Machine Learning Analysis**: Automated performance anomaly detection
5. **Integration Testing**: End-to-end application workflow benchmarks