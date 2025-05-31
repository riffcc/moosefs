#!/bin/bash

# Test MooseNG under various network conditions
# Requires root privileges for network manipulation

set -e

if [ "$EUID" -ne 0 ]; then
    echo "This script requires root privileges for network simulation"
    echo "Please run with sudo"
    exit 1
fi

RESULTS_DIR="test_results/network_tests/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

echo "Testing MooseNG under various network conditions..."

# Test different latency conditions
for latency in 10 50 100 200; do
    echo "Testing with ${latency}ms latency..."
    
    # Add network delay
    tc qdisc add dev lo root netem delay ${latency}ms
    
    # Run benchmark
    cargo run --release --package mooseng-benchmarks --bin network_bench \
        --latency ${latency} \
        --output "$RESULTS_DIR/latency_${latency}ms.json"
    
    # Remove network delay
    tc qdisc del dev lo root
done

echo "Network condition tests completed!"
