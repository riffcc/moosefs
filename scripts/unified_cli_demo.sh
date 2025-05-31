#!/bin/bash

# MooseNG Unified CLI and Benchmark Suite Demonstration
# This script demonstrates the integrated CLI tools and unified benchmark suite

set -e

MOOSENG_CLI="mooseng"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "========================================"
echo "MooseNG Unified CLI & Benchmark Demo"
echo "========================================"
echo "Project Root: $PROJECT_ROOT"
echo "Working Directory: $(pwd)"
echo

# Color output functions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to run command with explanation
run_demo_command() {
    local description="$1"
    local command="$2"
    local show_output="${3:-true}"
    
    echo
    info "Demo: $description"
    echo "Command: $command"
    echo "----------------------------------------"
    
    if [ "$show_output" = "true" ]; then
        if eval "$command"; then
            success "Command completed successfully"
        else
            error "Command failed (this may be expected in demo mode)"
        fi
    else
        echo "(Command would execute: $command)"
        echo "Output suppressed for demo purposes"
    fi
    echo
}

# Check if we're in the right directory
if [ ! -d "$PROJECT_ROOT/mooseng" ]; then
    error "This script must be run from the MooseFS project root or scripts directory"
    exit 1
fi

echo "1. Basic CLI Information"
echo "========================"

run_demo_command "Show CLI help" "$MOOSENG_CLI --help" false
run_demo_command "Show CLI version" "$MOOSENG_CLI --version" false
run_demo_command "Show benchmark subcommand help" "$MOOSENG_CLI benchmark --help" false

echo "2. Benchmark List and Status Operations"
echo "======================================="

run_demo_command "List available benchmark types" "$MOOSENG_CLI benchmark list" false
run_demo_command "Show benchmark status" "$MOOSENG_CLI benchmark status" false
run_demo_command "Show detailed benchmark status" "$MOOSENG_CLI benchmark status --detailed --limit 5" false

echo "3. Quick Benchmark Execution"
echo "============================="

run_demo_command "Run quick benchmark suite" "$MOOSENG_CLI benchmark quick --output ./demo-results --iterations 5" false
run_demo_command "Run quick benchmark with network tests" "$MOOSENG_CLI benchmark quick --output ./demo-results --iterations 3 --network" false

echo "4. Specific Benchmark Types"
echo "============================"

run_demo_command "Run file operation benchmarks" "$MOOSENG_CLI benchmark file --sizes '1024,65536,1048576' --output ./demo-results --iterations 10" false
run_demo_command "Run metadata operation benchmarks" "$MOOSENG_CLI benchmark metadata --operations 500 --output ./demo-results" false
run_demo_command "Run network latency benchmarks" "$MOOSENG_CLI benchmark network --targets 'localhost:9421,localhost:9422' --output ./demo-results --duration 30" false

echo "5. Comprehensive Benchmark Suite"
echo "================================="

run_demo_command "Run full benchmark suite with unified runner" "$MOOSENG_CLI benchmark full --output ./demo-results --real-network --infrastructure" false
run_demo_command "Run full benchmark suite with custom config" "$MOOSENG_CLI benchmark full --output ./demo-results --config ./test_configs/benchmark_config.toml" false

echo "6. Benchmark Results Management"
echo "==============================="

run_demo_command "Generate HTML report from results" "$MOOSENG_CLI benchmark report --input ./demo-results/quick_benchmark_results.json --format html --output benchmark_report.html --charts" false
run_demo_command "Generate CSV report from results" "$MOOSENG_CLI benchmark report --input ./demo-results --format csv --output benchmark_data.csv" false
run_demo_command "Generate JSON report from results" "$MOOSENG_CLI benchmark report --input ./demo-results --format json --output benchmark_summary.json" false

echo "7. Benchmark Comparison"
echo "======================="

run_demo_command "Compare two benchmark result sets (JSON format)" "$MOOSENG_CLI benchmark compare --baseline ./demo-results/baseline_results.json --current ./demo-results/current_results.json --format json" false
run_demo_command "Compare benchmark results (table format)" "$MOOSENG_CLI benchmark compare --baseline ./demo-results/baseline_results.json --current ./demo-results/current_results.json --format table" false
run_demo_command "Compare benchmark results (HTML format)" "$MOOSENG_CLI benchmark compare --baseline ./demo-results/baseline_results.json --current ./demo-results/current_results.json --format html" false

echo "8. Unified Benchmark Dashboard"
echo "==============================="

info "Starting benchmark dashboard server (this would run interactively)"
echo "Command: $MOOSENG_CLI benchmark dashboard --port 8080 --database ./benchmark.db --open-browser"
echo "Dashboard Features:"
echo "  - Real-time benchmark monitoring"
echo "  - Historical result visualization"
echo "  - Trend analysis and reporting"
echo "  - Interactive charts and graphs"
echo "  - Session management"
echo "  - Performance metrics export"
echo

echo "9. Database Query Operations"
echo "============================"

run_demo_command "Query all benchmark results" "$MOOSENG_CLI benchmark query --limit 20 --format table" false
run_demo_command "Query results by operation" "$MOOSENG_CLI benchmark query --operation 'file_create' --limit 10 --format json" false
run_demo_command "Query results by suite" "$MOOSENG_CLI benchmark query --suite 'file_operations' --limit 15 --format csv" false
run_demo_command "Query results by date range" "$MOOSENG_CLI benchmark query --start-date '2024-01-01T00:00:00Z' --end-date '2024-12-31T23:59:59Z' --limit 25" false
run_demo_command "Query results by session ID" "$MOOSENG_CLI benchmark query --session-id 'bench_20240531_120000' --format json" false

echo "10. Trend Analysis"
echo "=================="

run_demo_command "Analyze trends for all operations (30 days)" "$MOOSENG_CLI benchmark trends --days 30 --confidence 0.7 --format table" false
run_demo_command "Analyze trends for specific operation" "$MOOSENG_CLI benchmark trends --operation 'file_create' --days 14 --confidence 0.5 --format json" false
run_demo_command "Analyze trends with high confidence" "$MOOSENG_CLI benchmark trends --days 60 --confidence 0.9 --format table" false

echo "11. Continuous Integration Benchmarks"
echo "======================================"

run_demo_command "Run continuous benchmarks (5 iterations, 10 min intervals)" "$MOOSENG_CLI benchmark continuous --iterations 5 --interval 10 --threshold 5.0 --output ./ci-results --database ./ci-benchmark.db" false
run_demo_command "Run continuous benchmarks with custom threshold" "$MOOSENG_CLI benchmark continuous --iterations 3 --interval 5 --threshold 2.0 --output ./ci-results" false

echo "12. Integration with Other CLI Commands"
echo "========================================"

info "The benchmark system integrates with other MooseNG CLI commands:"
echo
echo "Cluster Management Integration:"
echo "  $MOOSENG_CLI cluster status --include-benchmarks"
echo "  $MOOSENG_CLI cluster health --benchmark-validation"
echo
echo "Monitoring Integration:"
echo "  $MOOSENG_CLI monitor metrics --include-benchmark-data"
echo "  $MOOSENG_CLI monitor performance --benchmark-baseline"
echo
echo "Admin Integration:"
echo "  $MOOSENG_CLI admin validate --run-benchmarks"
echo "  $MOOSENG_CLI admin optimize --benchmark-guided"
echo

echo "13. Configuration and Setup"
echo "==========================="

run_demo_command "Show current benchmark configuration" "$MOOSENG_CLI config show --component benchmark" false
run_demo_command "Set benchmark configuration" "$MOOSENG_CLI config set --component benchmark --setting 'default_iterations' 100" false
run_demo_command "Validate benchmark configuration" "$MOOSENG_CLI config validate --component benchmark" false
run_demo_command "Export benchmark configuration" "$MOOSENG_CLI config export --component benchmark --output benchmark_config.yaml --format yaml" false

echo "14. Advanced Benchmark Operations"
echo "=================================="

run_demo_command "Run benchmarks with custom tags" "$MOOSENG_CLI benchmark full --output ./demo-results --tags 'core,performance'" false
run_demo_command "Run infrastructure benchmarks" "$MOOSENG_CLI benchmark full --output ./demo-results --infrastructure --include-long" false
run_demo_command "Run network-intensive benchmarks" "$MOOSENG_CLI benchmark full --output ./demo-results --include-network --real-network" false

echo "15. Benchmark Data Export and Import"
echo "====================================="

run_demo_command "Export benchmark data to multiple formats" "$MOOSENG_CLI benchmark query --limit 100 --format json > all_results.json && $MOOSENG_CLI benchmark query --limit 100 --format csv > all_results.csv" false
run_demo_command "Create benchmark summary report" "$MOOSENG_CLI benchmark report --input ./demo-results --format html --output summary_report.html --charts" false

echo "16. Cleanup and Maintenance"
echo "==========================="

info "Benchmark database maintenance commands:"
echo "  $MOOSENG_CLI benchmark dashboard --database ./benchmark.db stats    # Show database statistics"
echo "  $MOOSENG_CLI benchmark dashboard --database ./benchmark.db cleanup --retention-days 90    # Clean old data"
echo

echo "=========================================="
echo "Demo Script Complete!"
echo "=========================================="
echo
success "This demo showed all major features of the unified CLI and benchmark suite:"
echo "✓ Integrated CLI commands for all benchmark operations"
echo "✓ Unified benchmark runner with multiple execution modes"
echo "✓ Real-time dashboard with web interface"
echo "✓ Database-backed result storage and querying"
echo "✓ Trend analysis and performance monitoring"
echo "✓ Continuous integration support"
echo "✓ Multiple output formats (JSON, CSV, HTML, Table)"
echo "✓ Comprehensive reporting and comparison tools"
echo
echo "Key Benefits:"
echo "• Single CLI interface for all benchmark operations"
echo "• Consistent command structure across all tools"
echo "• Historical data tracking and analysis"
echo "• Integration with existing MooseNG commands"
echo "• Extensible framework for custom benchmarks"
echo
info "To get started, run: $MOOSENG_CLI benchmark --help"
info "For the dashboard: $MOOSENG_CLI benchmark dashboard --open-browser"
echo