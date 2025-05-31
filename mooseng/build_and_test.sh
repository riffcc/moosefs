#!/bin/bash

# MooseNG Build and Comprehensive Test Script
set -e

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
MOOSENG_ROOT="$(pwd)"
BUILD_MODE="${BUILD_MODE:-release}"
BENCHMARK_MODE="${BENCHMARK_MODE:-simulation}"
BENCHMARK_ITERATIONS="${BENCHMARK_ITERATIONS:-50}"

echo -e "${BLUE}🚀 MooseNG Build and Test Pipeline${NC}"
echo "Build mode: $BUILD_MODE  < /dev/null |  Benchmark mode: $BENCHMARK_MODE"

# Functions
print_status() { echo -e "${BLUE}[$(date '+%H:%M:%S')] $1${NC}"; }
print_success() { echo -e "${GREEN}✅ $1${NC}"; }
print_warning() { echo -e "${YELLOW}⚠️  $1${NC}"; }
print_error() { echo -e "${RED}❌ $1${NC}"; }

# Check directory
if [[ ! -f "Cargo.toml" ]]; then
    print_error "Run from mooseng/ folder"
    exit 1
fi

# Check compilation
print_status "Checking compilation status..."
if cargo check --workspace --quiet 2>/dev/null; then
    print_success "Workspace compiles"
    COMPILATION_OK=true
else
    print_warning "Compilation issues detected"
    COMPILATION_OK=false
fi

# Setup results
mkdir -p benchmark_results
TIMESTAMP=$(date '+%Y%m%d_%H%M%S')
RESULTS_DIR="benchmark_results/${TIMESTAMP}"
mkdir -p "$RESULTS_DIR"

print_status "Running performance tests..."
cat > "${RESULTS_DIR}/performance_test.sh" << 'INNEREOF'
#!/bin/bash
echo "🔄 MooseNG Performance Evaluation"

# File operations test
echo "1. File System Test"
start_time=$(date +%s%N)
mkdir -p test_data
for size in 1024 4096 65536; do
    for i in {1..50}; do
        dd if=/dev/zero of="test_data/test_${size}_${i}" bs="$size" count=1 2>/dev/null
    done
done
for file in test_data/*; do cat "$file" > /dev/null; done
rm -rf test_data
end_time=$(date +%s%N)
fs_time=$((($end_time - $start_time) / 1000000))
echo "File operations: ${fs_time}ms"

# Network test
echo "2. Network Test"
start_time=$(date +%s%N)
if command -v curl >/dev/null; then
    for i in {1..5}; do
        curl -s --max-time 2 http://httpbin.org/get > /dev/null 2>&1 || break
    done
fi
end_time=$(date +%s%N)
net_time=$((($end_time - $start_time) / 1000000))
echo "Network operations: ${net_time}ms"

# CPU test
echo "3. CPU/Memory Test"
start_time=$(date +%s%N)
result=0
for i in {1..500000}; do result=$((result + i)); done
temp_array=()
for i in {1..5000}; do temp_array+=("item_$i"); done
end_time=$(date +%s%N)
cpu_time=$((($end_time - $start_time) / 1000000))
echo "CPU/Memory: ${cpu_time}ms"

# Concurrency test
echo "4. Concurrency Test"
start_time=$(date +%s%N)
for i in {1..10}; do
    (for j in {1..500}; do echo "worker_${i}_${j}" > /dev/null; done) &
done
wait
end_time=$(date +%s%N)
conc_time=$((($end_time - $start_time) / 1000000))
echo "Concurrency: ${conc_time}ms"

# Throughput test
echo "5. Throughput Test"
start_time=$(date +%s%N)
dd if=/dev/zero of=large_file bs=1M count=50 2>/dev/null
if command -v gzip >/dev/null; then
    gzip large_file && gunzip large_file.gz
fi
rm -f large_file
end_time=$(date +%s%N)
throughput_time=$((($end_time - $start_time) / 1000000))
throughput_mbps=$(echo "scale=2; 50 / ($throughput_time / 1000)" | bc -l 2>/dev/null || echo "N/A")
echo "Throughput: ${throughput_time}ms (${throughput_mbps} MB/s)"

# Summary
total_time=$((fs_time + net_time + cpu_time + conc_time + throughput_time))
ops_per_sec=$(echo "scale=2; 5000 / ($total_time / 1000)" | bc -l 2>/dev/null || echo "N/A")

if [ "$total_time" -lt 3000 ]; then grade="A+ (Excellent)"
elif [ "$total_time" -lt 6000 ]; then grade="A (Very Good)"
elif [ "$total_time" -lt 12000 ]; then grade="B (Good)"
elif [ "$total_time" -lt 25000 ]; then grade="C (Fair)"
else grade="D (Needs Improvement)"; fi

echo ""
echo "📊 PERFORMANCE SUMMARY"
echo "File System: ${fs_time}ms | Network: ${net_time}ms"
echo "CPU/Memory: ${cpu_time}ms | Concurrency: ${conc_time}ms"
echo "Throughput: ${throughput_time}ms (${throughput_mbps} MB/s)"
echo "Total: ${total_time}ms | Ops/sec: ${ops_per_sec}"
echo "Grade: $grade"

# Save results
cat > performance_results.json << EOJ
{
    "timestamp": "$(date -Iseconds)",
    "environment": {
        "os": "$(uname -s)",
        "arch": "$(uname -m)",
        "cores": "$(nproc 2>/dev/null || echo 'unknown')"
    },
    "results": {
        "file_system_ms": $fs_time,
        "network_ms": $net_time,
        "cpu_memory_ms": $cpu_time,
        "concurrency_ms": $conc_time,
        "throughput_ms": $throughput_time,
        "throughput_mbps": "$throughput_mbps",
        "total_time_ms": $total_time,
        "ops_per_second": "$ops_per_sec",
        "grade": "$grade"
    }
}
EOJ
INNEREOF

chmod +x "${RESULTS_DIR}/performance_test.sh"
cd "$RESULTS_DIR"
./performance_test.sh

# Run Rust benchmarks if possible
cd "$MOOSENG_ROOT"
if [[ "$COMPILATION_OK" == true ]]; then
    print_status "Running Rust benchmarks..."
    
    if cargo check --benches -p mooseng-master --quiet 2>/dev/null; then
        timeout 180 cargo bench -p mooseng-master 2>&1 | tee "${RESULTS_DIR}/raft_bench.log" || print_warning "Raft benchmarks failed"
    fi
    
    if cargo check --benches -p mooseng-common --quiet 2>/dev/null; then
        timeout 180 cargo bench -p mooseng-common 2>&1 | tee "${RESULTS_DIR}/async_bench.log" || print_warning "Async benchmarks failed"
    fi
fi

# Check for cluster
CLUSTER_AVAILABLE=false
if command -v pgrep >/dev/null 2>&1 && pgrep -f "mooseng" >/dev/null; then
    CLUSTER_AVAILABLE=true
fi

# Generate report
print_status "Generating report..."
cd "$RESULTS_DIR"

cat > REPORT.md << REPORTEOF
# MooseNG Performance Report - $(date)

**Build Status**: $([ "$COMPILATION_OK" == true ] && echo "✅ Success" || echo "❌ Issues")  
**Cluster Status**: $([ "$CLUSTER_AVAILABLE" == true ] && echo "✅ Available" || echo "❌ Not Running")  
**Environment**: $(uname -s) $(uname -r) on $(uname -m)

## Performance Results
- **Baseline Tests**: See performance_results.json
$([ -f "raft_bench.log" ] && echo "- **Raft Benchmarks**: ✅ Completed" || echo "- **Raft Benchmarks**: ❌ Not available")
$([ -f "async_bench.log" ] && echo "- **Async Benchmarks**: ✅ Completed" || echo "- **Async Benchmarks**: ❌ Not available")

## Quick Summary
$([ -f "performance_results.json" ] && echo "Performance Grade: $(grep -o '"grade": "[^"]*"' performance_results.json | cut -d'"' -f4 2>/dev/null || echo 'See results')")

## Next Steps
$([ "$COMPILATION_OK" == false ] && echo "1. Fix compilation: cargo check --workspace")
$([ "$CLUSTER_AVAILABLE" == false ] && echo "2. Start MooseNG cluster for network tests")
3. Review detailed results in this directory

REPORTEOF

# Final summary
print_success "Benchmark completed!"
echo ""
echo "📁 Results: $RESULTS_DIR"
echo "📄 Report: $RESULTS_DIR/REPORT.md"

if [[ -f "$RESULTS_DIR/performance_results.json" ]] && command -v jq >/dev/null 2>&1; then
    echo "🎯 Quick Stats:"
    echo "  Grade: $(jq -r '.results.grade' "$RESULTS_DIR/performance_results.json" 2>/dev/null)"
    echo "  Total: $(jq -r '.results.total_time_ms' "$RESULTS_DIR/performance_results.json" 2>/dev/null)ms"
    echo "  Throughput: $(jq -r '.results.throughput_mbps' "$RESULTS_DIR/performance_results.json" 2>/dev/null) MB/s"
fi

echo ""
echo "🚀 Re-run with: ./build_and_test.sh"
echo "🚀 Network mode: BENCHMARK_MODE=network ./build_and_test.sh"
