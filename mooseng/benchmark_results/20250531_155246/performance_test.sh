#!/bin/bash
echo "🔄 MooseNG Performance Evaluation"

# File operations test
echo "1. File System Test"
start_time=$(date +%s%N)
mkdir -p test_data
for size in 1024 4096 65536; do
    for i in {1..50}; do
        dd if=/dev/zero of="test_data/test_${size}_${i}" bs="$size" count=1 2>/dev/null
    done
done
for file in test_data/*; do cat "$file" > /dev/null; done
rm -rf test_data
end_time=$(date +%s%N)
fs_time=$((($end_time - $start_time) / 1000000))
echo "File operations: ${fs_time}ms"

# Network test
echo "2. Network Test"
start_time=$(date +%s%N)
if command -v curl >/dev/null; then
    for i in {1..5}; do
        curl -s --max-time 2 http://httpbin.org/get > /dev/null 2>&1 || break
    done
fi
end_time=$(date +%s%N)
net_time=$((($end_time - $start_time) / 1000000))
echo "Network operations: ${net_time}ms"

# CPU test
echo "3. CPU/Memory Test"
start_time=$(date +%s%N)
result=0
for i in {1..500000}; do result=$((result + i)); done
temp_array=()
for i in {1..5000}; do temp_array+=("item_$i"); done
end_time=$(date +%s%N)
cpu_time=$((($end_time - $start_time) / 1000000))
echo "CPU/Memory: ${cpu_time}ms"

# Concurrency test
echo "4. Concurrency Test"
start_time=$(date +%s%N)
for i in {1..10}; do
    (for j in {1..500}; do echo "worker_${i}_${j}" > /dev/null; done) &
done
wait
end_time=$(date +%s%N)
conc_time=$((($end_time - $start_time) / 1000000))
echo "Concurrency: ${conc_time}ms"

# Throughput test
echo "5. Throughput Test"
start_time=$(date +%s%N)
dd if=/dev/zero of=large_file bs=1M count=50 2>/dev/null
if command -v gzip >/dev/null; then
    gzip large_file && gunzip large_file.gz
fi
rm -f large_file
end_time=$(date +%s%N)
throughput_time=$((($end_time - $start_time) / 1000000))
throughput_mbps=$(echo "scale=2; 50 / ($throughput_time / 1000)" | bc -l 2>/dev/null || echo "N/A")
echo "Throughput: ${throughput_time}ms (${throughput_mbps} MB/s)"

# Summary
total_time=$((fs_time + net_time + cpu_time + conc_time + throughput_time))
ops_per_sec=$(echo "scale=2; 5000 / ($total_time / 1000)" | bc -l 2>/dev/null || echo "N/A")

if [ "$total_time" -lt 3000 ]; then grade="A+ (Excellent)"
elif [ "$total_time" -lt 6000 ]; then grade="A (Very Good)"
elif [ "$total_time" -lt 12000 ]; then grade="B (Good)"
elif [ "$total_time" -lt 25000 ]; then grade="C (Fair)"
else grade="D (Needs Improvement)"; fi

echo ""
echo "📊 PERFORMANCE SUMMARY"
echo "File System: ${fs_time}ms | Network: ${net_time}ms"
echo "CPU/Memory: ${cpu_time}ms | Concurrency: ${conc_time}ms"
echo "Throughput: ${throughput_time}ms (${throughput_mbps} MB/s)"
echo "Total: ${total_time}ms | Ops/sec: ${ops_per_sec}"
echo "Grade: $grade"

# Save results
cat > performance_results.json << EOJ
{
    "timestamp": "$(date -Iseconds)",
    "environment": {
        "os": "$(uname -s)",
        "arch": "$(uname -m)",
        "cores": "$(nproc 2>/dev/null || echo 'unknown')"
    },
    "results": {
        "file_system_ms": $fs_time,
        "network_ms": $net_time,
        "cpu_memory_ms": $cpu_time,
        "concurrency_ms": $conc_time,
        "throughput_ms": $throughput_time,
        "throughput_mbps": "$throughput_mbps",
        "total_time_ms": $total_time,
        "ops_per_second": "$ops_per_sec",
        "grade": "$grade"
    }
}
EOJ
