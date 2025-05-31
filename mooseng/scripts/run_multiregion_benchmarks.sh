#!/bin/bash

# MooseNG Multi-Region Benchmark Runner
# This script runs comprehensive benchmarks across multiple regions

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RESULTS_DIR="${PROJECT_ROOT}/benchmark-results/$(date +%Y%m%d_%H%M%S)"
LOG_FILE="${RESULTS_DIR}/benchmark.log"

# Default configuration
DEFAULT_REGIONS="us-east,eu-west,ap-south"
DEFAULT_DURATION="300"  # 5 minutes per test
DEFAULT_ITERATIONS="10"
DEFAULT_OUTPUT_FORMAT="json"

# Function to print usage
usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Run multi-region benchmarks for MooseNG distributed file system.

OPTIONS:
    -r, --regions REGIONS       Comma-separated list of regions (default: $DEFAULT_REGIONS)
    -d, --duration SECONDS      Duration for each benchmark (default: $DEFAULT_DURATION)
    -i, --iterations COUNT      Number of iterations per test (default: $DEFAULT_ITERATIONS)
    -o, --output FORMAT         Output format: json, csv, html (default: $DEFAULT_OUTPUT_FORMAT)
    -c, --config FILE           Custom benchmark configuration file
    -v, --verbose               Enable verbose logging
    -n, --network-simulation    Enable network condition simulation
    -p, --parallel              Run benchmarks in parallel where possible
    -f, --file-sizes SIZES      Comma-separated list of file sizes in bytes
    -t, --test-types TYPES      Comma-separated list of test types to run
    --cleanup                   Clean up test data after completion
    --docker                    Run benchmarks in Docker containers
    --kubernetes                Run benchmarks on Kubernetes cluster
    -h, --help                  Show this help message

EXAMPLES:
    # Run basic multi-region benchmark
    $0 --regions us-east,eu-west --duration 180

    # Run comprehensive benchmark with network simulation
    $0 --network-simulation --parallel --verbose

    # Run specific test types with custom file sizes
    $0 --test-types "file_ops,metadata,consistency" --file-sizes "1024,65536,1048576"

    # Run benchmarks using Docker containers
    $0 --docker --regions us-east,eu-west,ap-south

EOF
}

# Function to log messages
log() {
    local level="$1"
    shift
    local message="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[$timestamp] [$level] $message" | tee -a "$LOG_FILE"
}

# Function to setup benchmark environment
setup_environment() {
    log "INFO" "Setting up benchmark environment..."
    
    mkdir -p "$RESULTS_DIR"
    mkdir -p "$RESULTS_DIR/logs"
    mkdir -p "$RESULTS_DIR/data"
    
    # Copy benchmark configuration
    if [[ -f "$PROJECT_ROOT/mooseng-benchmarks/benchmark-config.toml" ]]; then
        cp "$PROJECT_ROOT/mooseng-benchmarks/benchmark-config.toml" "$RESULTS_DIR/"
    fi
    
    # Setup network tools if needed
    if [[ "$ENABLE_NETWORK_SIM" == "true" ]]; then
        log "INFO" "Setting up network simulation tools..."
        setup_network_simulation
    fi
    
    log "INFO" "Environment setup completed. Results will be stored in: $RESULTS_DIR"
}

# Function to setup network simulation
setup_network_simulation() {
    log "INFO" "Configuring network simulation..."
    
    # Check if running with sufficient privileges for tc
    if ! tc qdisc show > /dev/null 2>&1; then
        log "WARN" "Insufficient privileges for traffic control. Network simulation may not work properly."
        return 1
    fi
    
    # Create network simulation script
    cat > "$RESULTS_DIR/network-sim.sh" << 'EOF'
#!/bin/bash

# Network simulation helper script
setup_latency() {
    local interface="$1"
    local latency_ms="$2"
    local jitter_ms="${3:-10}"
    local loss_percent="${4:-0}"
    
    tc qdisc add dev "$interface" root netem delay "${latency_ms}ms" "${jitter_ms}ms" loss "${loss_percent}%"
}

cleanup_latency() {
    local interface="$1"
    tc qdisc del dev "$interface" root netem 2>/dev/null || true
}

# Simulate different network conditions
simulate_cross_continent() {
    setup_latency eth0 150 20 0.5
}

simulate_poor_connection() {
    setup_latency eth0 300 50 2.0
}

simulate_satellite() {
    setup_latency eth0 600 30 1.0
}

cleanup_all() {
    cleanup_latency eth0
}

case "$1" in
    "cross-continent") simulate_cross_continent ;;
    "poor") simulate_poor_connection ;;
    "satellite") simulate_satellite ;;
    "cleanup") cleanup_all ;;
    *) echo "Usage: $0 {cross-continent|poor|satellite|cleanup}" ;;
esac
EOF
    
    chmod +x "$RESULTS_DIR/network-sim.sh"
}

# Function to run Docker-based benchmarks
run_docker_benchmarks() {
    log "INFO" "Running Docker-based multi-region benchmarks..."
    
    # Build benchmark image
    log "INFO" "Building benchmark Docker image..."
    cd "$PROJECT_ROOT"
    docker build -t mooseng-benchmark:latest -f docker/Dockerfile.benchmark .
    
    # Start multi-region environment
    log "INFO" "Starting multi-region Docker environment..."
    docker-compose -f docker-compose.multiregion.yml up -d
    
    # Wait for services to be ready
    log "INFO" "Waiting for services to initialize..."
    sleep 60
    
    # Run benchmarks
    log "INFO" "Executing benchmark suite..."
    docker run --rm \
        --network mooseng_inter-region-net \
        -v "$RESULTS_DIR:/results" \
        -e REGIONS="$REGIONS" \
        -e DURATION="$DURATION" \
        -e ITERATIONS="$ITERATIONS" \
        mooseng-benchmark:latest \
        /usr/local/bin/run-benchmarks.sh
    
    # Collect results
    log "INFO" "Collecting benchmark results..."
    docker-compose -f docker-compose.multiregion.yml logs > "$RESULTS_DIR/logs/docker-compose.log"
    
    # Cleanup if requested
    if [[ "$CLEANUP" == "true" ]]; then
        log "INFO" "Cleaning up Docker environment..."
        docker-compose -f docker-compose.multiregion.yml down -v
    fi
}

# Function to run Kubernetes-based benchmarks
run_kubernetes_benchmarks() {
    log "INFO" "Running Kubernetes-based multi-region benchmarks..."
    
    # Check kubectl availability
    if ! command -v kubectl &> /dev/null; then
        log "ERROR" "kubectl not found. Please install kubectl to run Kubernetes benchmarks."
        exit 1
    fi
    
    # Apply Kubernetes manifests
    log "INFO" "Deploying MooseNG on Kubernetes..."
    kubectl apply -f k8s/namespace.yaml
    kubectl apply -f k8s/master-statefulset.yaml
    kubectl apply -f k8s/chunkserver-daemonset.yaml
    
    # Wait for deployment
    log "INFO" "Waiting for Kubernetes deployment to be ready..."
    kubectl wait --for=condition=ready pod -l app=mooseng-master -n mooseng --timeout=300s
    kubectl wait --for=condition=ready pod -l app=mooseng-chunkserver -n mooseng --timeout=300s
    
    # Run benchmarks
    log "INFO" "Running Kubernetes benchmarks..."
    kubectl apply -f - << EOF
apiVersion: batch/v1
kind: Job
metadata:
  name: mooseng-benchmark
  namespace: mooseng
spec:
  template:
    spec:
      containers:
      - name: benchmark
        image: mooseng-benchmark:latest
        env:
        - name: REGIONS
          value: "$REGIONS"
        - name: DURATION
          value: "$DURATION"
        - name: ITERATIONS
          value: "$ITERATIONS"
        volumeMounts:
        - name: results
          mountPath: /results
      volumes:
      - name: results
        hostPath:
          path: $RESULTS_DIR
      restartPolicy: Never
  backoffLimit: 3
EOF
    
    # Wait for completion
    kubectl wait --for=condition=complete job/mooseng-benchmark -n mooseng --timeout=1800s
    
    # Collect results
    kubectl logs job/mooseng-benchmark -n mooseng > "$RESULTS_DIR/logs/kubernetes-benchmark.log"
    
    # Cleanup if requested
    if [[ "$CLEANUP" == "true" ]]; then
        log "INFO" "Cleaning up Kubernetes resources..."
        kubectl delete namespace mooseng
    fi
}

# Function to run native benchmarks
run_native_benchmarks() {
    log "INFO" "Running native multi-region benchmarks..."
    
    cd "$PROJECT_ROOT/mooseng-benchmarks"
    
    # Build benchmark suite
    log "INFO" "Building benchmark suite..."
    cargo build --release
    
    # Prepare benchmark configuration
    cat > "$RESULTS_DIR/benchmark-config.toml" << EOF
[benchmark]
regions = [$(echo "$REGIONS" | sed 's/,/", "/g' | sed 's/^/"/; s/$/"/')]
duration_seconds = $DURATION
iterations = $ITERATIONS
output_format = "$OUTPUT_FORMAT"
enable_network_simulation = $ENABLE_NETWORK_SIM
parallel_execution = $PARALLEL_EXECUTION

[file_sizes]
small = [1024, 4096, 16384]
medium = [65536, 262144, 1048576]
large = [10485760, 104857600, 1073741824]

[test_types]
enabled = [$(echo "${TEST_TYPES:-file_ops,metadata,multiregion,network}" | sed 's/,/", "/g' | sed 's/^/"/; s/$/"/')]

[output]
directory = "$RESULTS_DIR"
detailed_logs = $VERBOSE
include_raw_data = true
EOF
    
    # Run different benchmark categories
    local benchmark_categories=(
        "file_operations"
        "metadata_operations"
        "multiregion"
        "network_simulation"
    )
    
    for category in "${benchmark_categories[@]}"; do
        if [[ "$TEST_TYPES" == *"$category"* ]] || [[ -z "$TEST_TYPES" ]]; then
            log "INFO" "Running $category benchmarks..."
            
            if [[ "$PARALLEL_EXECUTION" == "true" ]]; then
                cargo run --release --bin comprehensive_benchmark -- \
                    --config "$RESULTS_DIR/benchmark-config.toml" \
                    --category "$category" \
                    --output "$RESULTS_DIR/${category}-results.$OUTPUT_FORMAT" \
                    --parallel &
            else
                cargo run --release --bin comprehensive_benchmark -- \
                    --config "$RESULTS_DIR/benchmark-config.toml" \
                    --category "$category" \
                    --output "$RESULTS_DIR/${category}-results.$OUTPUT_FORMAT"
            fi
        fi
    done
    
    # Wait for parallel jobs to complete
    if [[ "$PARALLEL_EXECUTION" == "true" ]]; then
        log "INFO" "Waiting for parallel benchmark jobs to complete..."
        wait
    fi
}

# Function to generate comprehensive report
generate_report() {
    log "INFO" "Generating comprehensive benchmark report..."
    
    cd "$PROJECT_ROOT/mooseng-benchmarks"
    cargo run --release --bin report_generator -- \
        --input-dir "$RESULTS_DIR" \
        --output "$RESULTS_DIR/comprehensive-report.html" \
        --format html \
        --include-charts \
        --include-comparisons
    
    # Generate summary
    cat > "$RESULTS_DIR/summary.txt" << EOF
MooseNG Multi-Region Benchmark Summary
=====================================

Benchmark Configuration:
- Regions: $REGIONS
- Duration: $DURATION seconds per test
- Iterations: $ITERATIONS
- Test Types: ${TEST_TYPES:-"all"}
- Network Simulation: $ENABLE_NETWORK_SIM
- Parallel Execution: $PARALLEL_EXECUTION

Results Location: $RESULTS_DIR
Detailed Report: $RESULTS_DIR/comprehensive-report.html

Benchmark completed at: $(date)
EOF
    
    log "INFO" "Report generation completed"
    log "INFO" "Summary: $RESULTS_DIR/summary.txt"
    log "INFO" "Detailed Report: $RESULTS_DIR/comprehensive-report.html"
}

# Function to cleanup resources
cleanup_resources() {
    if [[ "$CLEANUP" == "true" ]]; then
        log "INFO" "Cleaning up benchmark resources..."
        
        # Cleanup network simulation
        if [[ "$ENABLE_NETWORK_SIM" == "true" ]] && [[ -f "$RESULTS_DIR/network-sim.sh" ]]; then
            "$RESULTS_DIR/network-sim.sh" cleanup
        fi
        
        # Cleanup temporary files
        rm -f /tmp/mooseng-benchmark-*
        
        log "INFO" "Cleanup completed"
    fi
}

# Main execution function
main() {
    local REGIONS="$DEFAULT_REGIONS"
    local DURATION="$DEFAULT_DURATION"
    local ITERATIONS="$DEFAULT_ITERATIONS"
    local OUTPUT_FORMAT="$DEFAULT_OUTPUT_FORMAT"
    local CONFIG_FILE=""
    local VERBOSE="false"
    local ENABLE_NETWORK_SIM="false"
    local PARALLEL_EXECUTION="false"
    local FILE_SIZES=""
    local TEST_TYPES=""
    local CLEANUP="false"
    local USE_DOCKER="false"
    local USE_KUBERNETES="false"
    
    # Parse command line arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            -r|--regions)
                REGIONS="$2"
                shift 2
                ;;
            -d|--duration)
                DURATION="$2"
                shift 2
                ;;
            -i|--iterations)
                ITERATIONS="$2"
                shift 2
                ;;
            -o|--output)
                OUTPUT_FORMAT="$2"
                shift 2
                ;;
            -c|--config)
                CONFIG_FILE="$2"
                shift 2
                ;;
            -v|--verbose)
                VERBOSE="true"
                shift
                ;;
            -n|--network-simulation)
                ENABLE_NETWORK_SIM="true"
                shift
                ;;
            -p|--parallel)
                PARALLEL_EXECUTION="true"
                shift
                ;;
            -f|--file-sizes)
                FILE_SIZES="$2"
                shift 2
                ;;
            -t|--test-types)
                TEST_TYPES="$2"
                shift 2
                ;;
            --cleanup)
                CLEANUP="true"
                shift
                ;;
            --docker)
                USE_DOCKER="true"
                shift
                ;;
            --kubernetes)
                USE_KUBERNETES="true"
                shift
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                log "ERROR" "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
    done
    
    # Setup environment
    setup_environment
    
    log "INFO" "Starting MooseNG multi-region benchmark suite"
    log "INFO" "Configuration: regions=$REGIONS, duration=$DURATION, iterations=$ITERATIONS"
    
    # Run benchmarks based on selected mode
    if [[ "$USE_DOCKER" == "true" ]]; then
        run_docker_benchmarks
    elif [[ "$USE_KUBERNETES" == "true" ]]; then
        run_kubernetes_benchmarks
    else
        run_native_benchmarks
    fi
    
    # Generate reports
    generate_report
    
    # Cleanup
    cleanup_resources
    
    log "INFO" "Multi-region benchmark suite completed successfully"
    echo ""
    echo "Results available at: $RESULTS_DIR"
    echo "Open $RESULTS_DIR/comprehensive-report.html in your browser to view detailed results"
}

# Trap for cleanup on script exit
trap cleanup_resources EXIT

# Run main function if script is executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    main "$@"
fi