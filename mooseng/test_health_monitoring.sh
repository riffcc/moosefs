#!/bin/bash

# End-to-end test script for health monitoring system
# This script tests the health monitoring and CLI integration

set -e

echo "=== MooseNG Health Monitoring End-to-End Test ==="
echo

# Test 1: Build all components to check for compilation errors
echo "Test 1: Building all components..."
cd /home/wings/projects/moosefs/mooseng

# Build common library first
echo "Building mooseng-common..."
cargo build -p mooseng-common || { echo "FAILED: mooseng-common build failed"; exit 1; }

# Build all components
echo "Building mooseng-master..."
cargo build -p mooseng-master || { echo "FAILED: mooseng-master build failed"; exit 1; }

echo "Building mooseng-chunkserver..."
cargo build -p mooseng-chunkserver || { echo "FAILED: mooseng-chunkserver build failed"; exit 1; }

echo "Building mooseng-client..."
cargo build -p mooseng-client || { echo "FAILED: mooseng-client build failed"; exit 1; }

echo "Building mooseng-metalogger..."
cargo build -p mooseng-metalogger || { echo "FAILED: mooseng-metalogger build failed"; exit 1; }

echo "Building mooseng-cli..."
cargo build -p mooseng-cli || { echo "FAILED: mooseng-cli build failed"; exit 1; }

echo "✓ All components built successfully!"
echo

# Test 2: Run CLI health check commands
echo "Test 2: Testing CLI health monitoring commands..."

CLI_BIN="./target/debug/mooseng"

# Test health status command
echo "Testing 'mooseng monitor health' command..."
$CLI_BIN monitor health || { echo "FAILED: monitor health command failed"; exit 1; }
echo "✓ Health monitoring command completed"
echo

# Test detailed health status
echo "Testing 'mooseng monitor health --detailed' command..."
$CLI_BIN monitor health --detailed || { echo "FAILED: detailed health command failed"; exit 1; }
echo "✓ Detailed health monitoring command completed"
echo

# Test manual health checks
echo "Testing manual health check commands..."

echo "Testing master health check..."
$CLI_BIN admin health check master || { echo "FAILED: master health check failed"; exit 1; }
echo "✓ Master health check completed"

echo "Testing chunkserver health check..."
$CLI_BIN admin health check chunkserver || { echo "FAILED: chunkserver health check failed"; exit 1; }
echo "✓ Chunkserver health check completed"

echo "Testing metalogger health check..."
$CLI_BIN admin health check metalogger || { echo "FAILED: metalogger health check failed"; exit 1; }
echo "✓ Metalogger health check completed"

echo "Testing client health check..."
$CLI_BIN admin health check client || { echo "FAILED: client health check failed"; exit 1; }
echo "✓ Client health check completed"
echo

# Test 3: Self-healing commands
echo "Test 3: Testing self-healing commands..."

echo "Testing cache clear action..."
$CLI_BIN admin health heal chunkserver --action clear_cache || { echo "FAILED: cache clear action failed"; exit 1; }
echo "✓ Cache clear action completed"

echo "Testing network reconnect action..."
$CLI_BIN admin health heal master --action reconnect || { echo "FAILED: network reconnect action failed"; exit 1; }
echo "✓ Network reconnect action completed"

echo "Testing custom action with parameters..."
$CLI_BIN admin health heal metalogger --action wal_rotate --params size=100MB || { echo "FAILED: custom action failed"; exit 1; }
echo "✓ Custom action completed"
echo

# Test 4: Health history and configuration
echo "Test 4: Testing health history and configuration..."

echo "Testing healing history..."
$CLI_BIN admin health history || { echo "FAILED: healing history failed"; exit 1; }
echo "✓ Healing history command completed"

echo "Testing auto-heal configuration..."
$CLI_BIN admin health auto-heal master --enable || { echo "FAILED: auto-heal configuration failed"; exit 1; }
echo "✓ Auto-heal configuration completed"

echo "Testing health settings configuration..."
$CLI_BIN admin health configure master --interval 30 --timeout 10 --failure-threshold 3 || { echo "FAILED: health settings configuration failed"; exit 1; }
echo "✓ Health settings configuration completed"
echo

# Test 5: Run unit tests for health checkers
echo "Test 5: Running unit tests for health monitoring components..."

echo "Testing mooseng-common health tests..."
cargo test -p mooseng-common health || { echo "FAILED: mooseng-common health tests failed"; exit 1; }

echo "Testing mooseng-master health tests..."
cargo test -p mooseng-master health || { echo "FAILED: mooseng-master health tests failed"; exit 1; }

echo "Testing mooseng-chunkserver health tests..."
cargo test -p mooseng-chunkserver health || { echo "FAILED: mooseng-chunkserver health tests failed"; exit 1; }

echo "Testing mooseng-client health tests..."
cargo test -p mooseng-client health || { echo "FAILED: mooseng-client health tests failed"; exit 1; }

echo "Testing mooseng-metalogger health tests..."
cargo test -p mooseng-metalogger health || { echo "FAILED: mooseng-metalogger health tests failed"; exit 1; }

echo "✓ All health monitoring tests passed!"
echo

# Summary
echo "=== TEST SUMMARY ==="
echo "✓ All components built successfully"
echo "✓ CLI health monitoring commands work"
echo "✓ Manual health checks function properly"
echo "✓ Self-healing actions execute correctly"
echo "✓ Health history and configuration commands work"
echo "✓ Unit tests pass for all health monitoring components"
echo
echo "🎉 END-TO-END HEALTH MONITORING TEST COMPLETED SUCCESSFULLY!"
echo
echo "Available CLI commands for health monitoring:"
echo "  mooseng monitor health [--detailed]"
echo "  mooseng admin health check <component> [--id <id>]"
echo "  mooseng admin health heal <component> --action <action> [--params key=value]"
echo "  mooseng admin health history [--component <comp>] [--count <n>]"
echo "  mooseng admin health auto-heal <component> --enable/--disable"
echo "  mooseng admin health configure <component> [--interval <sec>] [--timeout <sec>] [--failure-threshold <n>]"
echo