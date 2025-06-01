#!/usr/bin/env python3
"""
Script to fix BenchmarkResult struct initialization errors by adding missing fields
"""

import os
import re
import sys

def fix_benchmark_result(file_path):
    """Fix BenchmarkResult initializations in a file"""
    
    with open(file_path, 'r') as f:
        content = f.read()
    
    # Pattern to find BenchmarkResult blocks that are missing fields
    pattern = r'(BenchmarkResult\s*\{[^}]*?samples:\s*[^,\n]*[,\n])(\s*\})'
    
    def replacement(match):
        block = match.group(1)
        end = match.group(2)
        
        # Check if required fields are missing
        has_timestamp = 'timestamp:' in block
        has_metadata = 'metadata:' in block  
        has_percentiles = 'percentiles:' in block
        
        additions = []
        if not has_timestamp:
            additions.append('            timestamp: chrono::Utc::now(),')
        if not has_metadata:
            additions.append('            metadata: None,')
        if not has_percentiles:
            additions.append('            percentiles: None,')
        
        if additions:
            return block + '\n' + '\n'.join(additions) + end
        
        return match.group(0)
    
    fixed_content = re.sub(pattern, replacement, content, flags=re.DOTALL)
    
    if fixed_content != content:
        with open(file_path, 'w') as f:
            f.write(fixed_content)
        print(f"Fixed BenchmarkResult in {file_path}")
        return True
    
    return False

def main():
    benchmark_dir = "/Users/wings/projects/moosefs/mooseng/mooseng-benchmarks/src/benchmarks"
    
    if not os.path.exists(benchmark_dir):
        print(f"Directory not found: {benchmark_dir}")
        return 1
    
    fixed_files = []
    
    for root, dirs, files in os.walk(benchmark_dir):
        for file in files:
            if file.endswith('.rs'):
                file_path = os.path.join(root, file)
                if fix_benchmark_result(file_path):
                    fixed_files.append(file_path)
    
    # Also check the main src files
    src_files = [
        "/Users/wings/projects/moosefs/mooseng/mooseng-benchmarks/src/database.rs"
    ]
    
    for file_path in src_files:
        if os.path.exists(file_path) and fix_benchmark_result(file_path):
            fixed_files.append(file_path)
    
    print(f"Fixed {len(fixed_files)} files:")
    for f in fixed_files:
        print(f"  - {f}")
    
    return 0

if __name__ == "__main__":
    sys.exit(main())