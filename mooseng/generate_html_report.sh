#!/bin/bash

# MooseNG HTML Report Generator Script
# This script builds the benchmark reporter and generates comprehensive HTML reports

set -e

echo "🚀 MooseNG HTML Report Generator"
echo "=================================="

# Configuration
INPUT_DIR="${1:-benchmark_results}"
OUTPUT_DIR="${2:-html_reports}"
THEME="${3:-auto}"
CHART_LIB="${4:-plotly}"

echo "📁 Input Directory: $INPUT_DIR"
echo "📊 Output Directory: $OUTPUT_DIR"
echo "🎨 Theme: $THEME"
echo "📈 Chart Library: $CHART_LIB"
echo ""

# Check if input directory exists
if [ ! -d "$INPUT_DIR" ]; then
    echo "❌ Input directory '$INPUT_DIR' does not exist"
    echo "💡 Creating sample benchmark results..."
    
    # Create sample benchmark results for demonstration
    mkdir -p "$INPUT_DIR/20250531_$(date +%H%M%S)"
    cat > "$INPUT_DIR/20250531_$(date +%H%M%S)/performance_results.json" << 'EOF'
{
    "timestamp": "2025-05-31T15:54:21+01:00",
    "environment": {
        "os": "Linux",
        "arch": "x86_64",
        "cores": "16"
    },
    "results": {
        "file_system_ms": 125,
        "network_ms": 890,
        "cpu_memory_ms": 445,
        "concurrency_ms": 67,
        "throughput_ms": 234,
        "throughput_mbps": "198.45",
        "total_time_ms": 1761,
        "ops_per_second": "2134.56",
        "grade": "A (Excellent)"
    }
}
EOF
    
    # Create additional sample data
    mkdir -p "$INPUT_DIR/20250531_$(date +%H%M%S)_multiregion"
    cat > "$INPUT_DIR/20250531_$(date +%H%M%S)_multiregion/performance_results.json" << 'EOF'
{
    "timestamp": "2025-05-31T16:15:30+01:00",
    "environment": {
        "os": "Linux",
        "arch": "x86_64",
        "cores": "16"
    },
    "results": {
        "cross_region_latency_ms": 145,
        "replication_lag_ms": 78,
        "failover_time_ms": 2890,
        "consistency_check_ms": 156,
        "multiregion_throughput_ms": 345,
        "throughput_mbps": "167.23",
        "total_time_ms": 3614,
        "ops_per_second": "1845.67",
        "grade": "B+ (Very Good)"
    }
}
EOF

    echo "✅ Sample benchmark results created"
fi

echo "🔨 Building benchmark reporter..."

# Build the benchmark reporter
if ! cargo build --bin benchmark_reporter --release; then
    echo "❌ Failed to build benchmark reporter"
    echo "💡 Trying debug build..."
    if ! cargo build --bin benchmark_reporter; then
        echo "❌ Debug build also failed. Checking dependencies..."
        cargo check --bin benchmark_reporter
        exit 1
    fi
    BINARY_PATH="target/debug/benchmark_reporter"
else
    BINARY_PATH="target/release/benchmark_reporter"
fi

echo "✅ Benchmark reporter built successfully"

# Clean previous reports
if [ -d "$OUTPUT_DIR" ]; then
    echo "🧹 Cleaning previous reports..."
    rm -rf "$OUTPUT_DIR"
fi

echo "📊 Generating HTML reports..."

# Generate the HTML report
if ./"$BINARY_PATH" \
    --input "$INPUT_DIR" \
    --output "$OUTPUT_DIR" \
    --title "MooseNG Performance Analysis" \
    --theme "$THEME" \
    --chart-library "$CHART_LIB" \
    --real-time; then
    
    echo "✅ HTML reports generated successfully!"
    echo ""
    echo "📁 Reports location: $OUTPUT_DIR"
    echo "🌐 Main dashboard: $OUTPUT_DIR/index.html"
    echo ""
    
    # List generated files
    echo "📄 Generated files:"
    find "$OUTPUT_DIR" -type f -name "*.html" | sort | while read -r file; do
        echo "   - $(basename "$file")"
    done
    
    echo ""
    echo "🚀 To view the reports:"
    echo "   1. Open $OUTPUT_DIR/index.html in your browser"
    echo "   2. Or run: python3 -m http.server 8000 -d $OUTPUT_DIR"
    echo "   3. Then visit: http://localhost:8000"
    echo ""
    
    # Check if we can open the browser automatically
    if command -v xdg-open > /dev/null 2>&1; then
        echo "🌐 Opening in default browser..."
        xdg-open "$OUTPUT_DIR/index.html" 2>/dev/null &
    elif command -v open > /dev/null 2>&1; then
        echo "🌐 Opening in default browser..."
        open "$OUTPUT_DIR/index.html" 2>/dev/null &
    fi
    
else
    echo "❌ Failed to generate HTML reports"
    exit 1
fi

echo "🎉 HTML report generation completed!"