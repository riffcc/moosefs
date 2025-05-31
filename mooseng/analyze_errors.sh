#!/bin/bash
# Analyze compilation errors to identify patterns

echo "=== MooseNG Error Analysis ==="
echo

cd /home/wings/projects/moosefs/mooseng

# Master errors
echo "=== MASTER MODULE ERRORS ==="
echo "Top error types:"
cargo check -p mooseng-master 2>&1 | grep "error\[" | sed 's/.*error\[\(.*\)\]:.*/\1/' | sort | uniq -c | sort -nr | head -10
echo

echo "Most problematic files:"
cargo check -p mooseng-master 2>&1 | grep "^ -->" | sed 's/.*src\///' | cut -d: -f1 | sort | uniq -c | sort -nr | head -10
echo

# ChunkServer errors  
echo "=== CHUNKSERVER MODULE ERRORS ==="
echo "Top error types:"
cargo check -p mooseng-chunkserver 2>&1 | grep "error\[" | sed 's/.*error\[\(.*\)\]:.*/\1/' | sort | uniq -c | sort -nr | head -10
echo

echo "Most problematic files:"
cargo check -p mooseng-chunkserver 2>&1 | grep "^ -->" | sed 's/.*src\///' | cut -d: -f1 | sort | uniq -c | sort -nr | head -10
echo

echo "=== COMMON ERROR PATTERNS ==="
echo "Missing imports:"
cargo check --all 2>&1 | grep "unresolved import" | wc -l

echo "Type mismatches:"
cargo check --all 2>&1 | grep "mismatched types" | wc -l

echo "Missing trait implementations:"
cargo check --all 2>&1 | grep "trait bound" | wc -l

echo "Lifetime issues:"
cargo check --all 2>&1 | grep "lifetime" | wc -l