#!/bin/bash

# MooseNG Test Environment Setup Script
# Sets up a comprehensive testing environment for MooseNG benchmarking

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR"

echo "🚀 Setting up MooseNG Test Environment..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check required tools
check_dependencies() {
    log_info "Checking dependencies..."
    
    if ! command_exists cargo; then
        log_error "Rust/Cargo not found. Please install Rust: https://rustup.rs/"
        exit 1
    fi
    
    if ! command_exists docker; then
        log_warning "Docker not found. Some tests may not work."
    fi
    
    if ! command_exists python3; then
        log_warning "Python3 not found. Some analysis scripts may not work."
    fi
    
    log_success "Dependencies checked"
}

# Setup test directories
setup_directories() {
    log_info "Setting up test directories..."
    
    mkdir -p "$PROJECT_ROOT/test_data/small_files"
    mkdir -p "$PROJECT_ROOT/test_data/large_files"
    mkdir -p "$PROJECT_ROOT/test_data/metadata_test"
    mkdir -p "$PROJECT_ROOT/test_results"
    mkdir -p "$PROJECT_ROOT/logs"
    mkdir -p "$PROJECT_ROOT/tmp"
    
    log_success "Test directories created"
}

# Generate test data
generate_test_data() {
    log_info "Generating test data..."
    
    # Small files (1KB, 4KB, 64KB)
    for size in 1024 4096 65536; do
        dd if=/dev/urandom of="$PROJECT_ROOT/test_data/small_files/test_${size}b.dat" bs=$size count=1 2>/dev/null
    done
    
    # Large files (1MB, 10MB, 100MB) - but smaller for CI
    for size in 1048576 10485760; do
        dd if=/dev/urandom of="$PROJECT_ROOT/test_data/large_files/test_${size}b.dat" bs=$size count=1 2>/dev/null
    done
    
    # Metadata test files (many small files)
    for i in {1..100}; do
        echo "test file $i" > "$PROJECT_ROOT/test_data/metadata_test/file_$i.txt"
    done
    
    log_success "Test data generated"
}

# Create test configuration
create_test_config() {
    log_info "Creating test configuration..."
    
    cat > "$PROJECT_ROOT/test_config.toml" << 'EOF'
[benchmark]
warmup_iterations = 5
measurement_iterations = 50
detailed_report = true

[file_sizes]
small = [1024, 4096, 65536]              # 1KB, 4KB, 64KB
medium = [1048576, 10485760]             # 1MB, 10MB
large = [104857600]                      # 100MB (disabled by default)

[concurrency]
levels = [1, 5, 10, 25]

[regions]
test_regions = ["region1", "region2", "region3"]

[network]
latency_simulation = true
packet_loss_simulation = false
bandwidth_limits = []

[performance]
enable_profiling = false
memory_tracking = true
cpu_tracking = true

[output]
results_dir = "test_results"
log_level = "info"
export_formats = ["json", "csv"]
EOF
    
    log_success "Test configuration created"
}

# Setup Rust environment
setup_rust_environment() {
    log_info "Setting up Rust environment..."
    
    cd "$PROJECT_ROOT"
    
    # Update Rust to latest stable
    if command_exists rustup; then
        rustup update stable
    fi
    
    # Install required tools
    cargo install --version 0.6 criterion || log_warning "Criterion already installed or failed to install"
    
    log_success "Rust environment ready"
}

# Build project
build_project() {
    log_info "Building MooseNG project..."
    
    cd "$PROJECT_ROOT"
    
    # Clean and build
    cargo clean
    cargo build --release --workspace
    
    log_success "Project built successfully"
}

# Run basic smoke tests
run_smoke_tests() {
    log_info "Running smoke tests..."
    
    cd "$PROJECT_ROOT"
    
    # Test basic functionality
    if cargo test --lib --workspace --quiet; then
        log_success "Basic tests passed"
    else
        log_warning "Some tests failed, but continuing..."
    fi
}

# Create benchmark runner script
create_benchmark_runner() {
    log_info "Creating benchmark runner script..."
    
    cat > "$PROJECT_ROOT/run_benchmarks.sh" << 'EOF'
#!/bin/bash

# MooseNG Benchmark Runner
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "🔥 Running MooseNG Benchmarks..."

# Run different benchmark suites
echo "📊 Running file operation benchmarks..."
cargo bench --package mooseng-benchmarks file_operations

echo "📈 Running metadata operation benchmarks..."
cargo bench --package mooseng-benchmarks metadata_operations

echo "🌐 Running multi-region benchmarks..."
cargo bench --package mooseng-benchmarks multiregion

echo "⚡ Running network simulation benchmarks..."
cargo bench --package mooseng-benchmarks network_simulation

echo "🏁 All benchmarks completed!"
echo "📄 Results saved to target/criterion/"
EOF
    
    chmod +x "$PROJECT_ROOT/run_benchmarks.sh"
    
    log_success "Benchmark runner created"
}

# Create performance monitoring script
create_monitoring_script() {
    log_info "Creating monitoring script..."
    
    cat > "$PROJECT_ROOT/monitor_performance.sh" << 'EOF'
#!/bin/bash

# Performance Monitoring Script
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOG_FILE="$SCRIPT_DIR/logs/performance.log"

mkdir -p "$SCRIPT_DIR/logs"

echo "🔍 Monitoring MooseNG Performance..."
echo "📝 Logging to: $LOG_FILE"

# Start monitoring in background
{
    while true; do
        timestamp=$(date '+%Y-%m-%d %H:%M:%S')
        
        # System metrics
        cpu_usage=$(top -bn1 | grep "Cpu(s)" | awk '{print $2}' | cut -d'%' -f1)
        memory_usage=$(free | grep Mem | awk '{printf "%.1f", $3/$2 * 100.0}')
        
        # Disk I/O
        disk_io=$(iostat -d 1 2 | tail -n +4 | awk '{total += $4 + $5} END {print total}')
        
        echo "$timestamp,CPU:$cpu_usage%,Memory:$memory_usage%,DiskIO:$disk_io"
        
        sleep 5
    done
} >> "$LOG_FILE" 2>&1 &

MONITOR_PID=$!
echo "🏃 Monitoring started (PID: $MONITOR_PID)"
echo "🛑 Stop with: kill $MONITOR_PID"
EOF
    
    chmod +x "$PROJECT_ROOT/monitor_performance.sh"
    
    log_success "Monitoring script created"
}

# Print summary
print_summary() {
    echo ""
    echo "🎉 MooseNG Test Environment Setup Complete!"
    echo ""
    echo "📁 Created directories:"
    echo "  - test_data/: Test files and data"
    echo "  - test_results/: Benchmark results"
    echo "  - logs/: Log files"
    echo ""
    echo "📄 Created files:"
    echo "  - test_config.toml: Benchmark configuration"
    echo "  - run_benchmarks.sh: Benchmark runner script"
    echo "  - monitor_performance.sh: Performance monitoring"
    echo ""
    echo "🚀 Next steps:"
    echo "  1. Run benchmarks: ./run_benchmarks.sh"
    echo "  2. Monitor performance: ./monitor_performance.sh"
    echo "  3. Check results in test_results/"
    echo ""
    echo "📚 For more information, see the documentation in mooseng-benchmarks/"
}

# Main execution
main() {
    check_dependencies
    setup_directories
    generate_test_data
    create_test_config
    setup_rust_environment
    build_project
    run_smoke_tests
    create_benchmark_runner
    create_monitoring_script
    print_summary
}

# Run main function
main "$@"