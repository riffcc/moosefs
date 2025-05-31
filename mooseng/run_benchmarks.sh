#!/bin/bash

# MooseNG Benchmark Runner
# Coordinates all benchmark executions

set -e

RESULTS_DIR="test_results/benchmarks/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

echo "Starting MooseNG benchmark suite..."
echo "Results will be saved to: $RESULTS_DIR"

# Build the project first
echo "Building MooseNG..."
cargo build --release --workspace

# Run file operation benchmarks
echo "Running file operation benchmarks..."
cargo run --release --package mooseng-benchmarks --bin file_ops_bench \
    --config test_configs/benchmark_config.toml \
    --output "$RESULTS_DIR/file_operations.json"

# Run metadata benchmarks
echo "Running metadata benchmarks..."
cargo run --release --package mooseng-benchmarks --bin metadata_bench \
    --config test_configs/benchmark_config.toml \
    --output "$RESULTS_DIR/metadata_operations.json"

# Run multi-region benchmarks
echo "Running multi-region benchmarks..."
cargo run --release --package mooseng-benchmarks --bin multiregion_bench \
    --config test_configs/benchmark_config.toml \
    --output "$RESULTS_DIR/multiregion.json"

# Generate comprehensive report
echo "Generating benchmark report..."
cargo run --release --package mooseng-benchmarks --bin report_generator \
    --input "$RESULTS_DIR" \
    --output "$RESULTS_DIR/comprehensive_report.html"

echo "Benchmarks completed! Check $RESULTS_DIR for results."
