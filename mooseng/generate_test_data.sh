#!/bin/bash

# Generate test data for MooseNG benchmarks

set -e

echo "Generating test data..."

# Create small test files
mkdir -p test_data/small_files
for i in {1..100}; do
    dd if=/dev/urandom of="test_data/small_files/file_${i}.dat" bs=1K count=$((1 + RANDOM % 64)) 2>/dev/null
done

# Create medium test files
mkdir -p test_data/medium_files
for i in {1..10}; do
    dd if=/dev/urandom of="test_data/medium_files/file_${i}.dat" bs=1M count=$((1 + RANDOM % 10)) 2>/dev/null
done

# Create large test files
mkdir -p test_data/large_files
for i in {1..3}; do
    dd if=/dev/urandom of="test_data/large_files/file_${i}.dat" bs=100M count=$((1 + RANDOM % 5)) 2>/dev/null
done

# Create directory structure for metadata tests
mkdir -p test_data/metadata_test
for depth in {1..5}; do
    mkdir -p "test_data/metadata_test/depth_${depth}"
    for dir in {1..10}; do
        mkdir -p "test_data/metadata_test/depth_${depth}/dir_${dir}"
        for file in {1..20}; do
            touch "test_data/metadata_test/depth_${depth}/dir_${dir}/file_${file}.txt"
        done
    done
done

echo "Test data generation completed!"
