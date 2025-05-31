#!/bin/bash

# Comprehensive MooseNG test runner
# Coordinates compilation, benchmarks, and multi-region testing

set -e

echo "=== MooseNG Comprehensive Test Suite ==="
echo "Instance 2: Running full test environment validation"

# Step 1: Verify compilation
echo "Step 1: Verifying compilation..."
if ! cargo check --workspace; then
    echo "❌ Compilation failed! Please fix errors first."
    exit 1
fi
echo "✅ Compilation successful"

# Step 2: Generate test data
echo "Step 2: Generating test data..."
./generate_test_data.sh

# Step 3: Run local benchmarks
echo "Step 3: Running local benchmarks..."
./run_benchmarks.sh

# Step 4: Test with Docker environment
echo "Step 4: Testing with Docker environment..."
docker-compose -f docker-compose.test.yml build --no-cache
docker-compose -f docker-compose.test.yml up --abort-on-container-exit

# Step 5: Network condition testing (if root)
if [ "$EUID" -eq 0 ]; then
    echo "Step 5: Testing network conditions..."
    ./test_network_conditions.sh
else
    echo "Step 5: Skipping network tests (requires root)"
fi

# Step 6: Generate final report
echo "Step 6: Generating comprehensive report..."
FINAL_REPORT="test_results/final_comprehensive_report_$(date +%Y%m%d_%H%M%S).html"
cargo run --release --package mooseng-benchmarks --bin comprehensive_report \
    --input test_results \
    --output "$FINAL_REPORT"

echo ""
echo "🎉 Comprehensive testing completed!"
echo "📊 Final report: $FINAL_REPORT"
echo ""
echo "Summary of test environment setup:"
echo "✅ Compilation fixes applied"
echo "✅ Test configurations created"
echo "✅ Benchmark infrastructure ready"
echo "✅ Docker test environment configured"
echo "✅ Network simulation tools prepared"
echo "✅ Comprehensive reporting system ready"
