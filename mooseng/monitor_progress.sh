#!/bin/bash
# Monitor compilation progress for MooseNG

echo "=== MooseNG Compilation Progress Monitor ==="
echo "Time: $(date)"
echo

# Count errors by module
echo "Error count by module:"
cd /home/wings/projects/moosefs/mooseng

# Master errors
MASTER_ERRORS=$(cargo check -p mooseng-master 2>&1 | grep -c "error\[" || echo "0")
echo "  mooseng-master: $MASTER_ERRORS errors"

# ChunkServer errors  
CHUNK_ERRORS=$(cargo check -p mooseng-chunkserver 2>&1 | grep -c "error\[" || echo "0")
echo "  mooseng-chunkserver: $CHUNK_ERRORS errors"

# Client errors
CLIENT_ERRORS=$(cargo check -p mooseng-client 2>&1 | grep -c "error\[" || echo "0")
echo "  mooseng-client: $CLIENT_ERRORS errors"

# Protocol errors
PROTO_ERRORS=$(cargo check -p mooseng-protocol 2>&1 | grep -c "error\[" || echo "0")
echo "  mooseng-protocol: $PROTO_ERRORS errors"

# Other modules
COMMON_ERRORS=$(cargo check -p mooseng-common 2>&1 | grep -c "error\[" || echo "0")
echo "  mooseng-common: $COMMON_ERRORS errors"

METAL_ERRORS=$(cargo check -p mooseng-metalogger 2>&1 | grep -c "error\[" || echo "0")
echo "  mooseng-metalogger: $METAL_ERRORS errors"

CLI_ERRORS=$(cargo check -p mooseng-cli 2>&1 | grep -c "error\[" || echo "0")
echo "  mooseng-cli: $CLI_ERRORS errors"

# Total
TOTAL_ERRORS=$(cargo check --all 2>&1 | grep -c "error\[" || echo "0")
echo
echo "TOTAL ERRORS: $TOTAL_ERRORS"

# Check if any module compiles successfully
echo
echo "Successfully compiling modules:"
cargo check --all 2>&1 | grep "Finished" && echo "  Build completed!" || echo "  None yet"

# Git status summary
echo
echo "Modified files:"
git status --porcelain | grep "^ M" | wc -l
echo
echo "New files:"
git status --porcelain | grep "^??" | wc -l