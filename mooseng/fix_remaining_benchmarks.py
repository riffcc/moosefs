#!/usr/bin/env python3
"""Fix remaining BenchmarkResult struct initialization errors"""

import re
import os

def fix_file(file_path):
    """Fix a single file"""
    with open(file_path, 'r') as f:
        content = f.read()
    
    # Pattern for BenchmarkResult { ... samples: ... } without the required fields
    pattern = r'(BenchmarkResult\s*\{[^}]*?samples:\s*[^,}]+)(\s*\})'
    
    def replacement(match):
        block = match.group(1)
        end = match.group(2)
        
        # Check if required fields are missing
        has_timestamp = 'timestamp:' in block
        has_metadata = 'metadata:' in block  
        has_percentiles = 'percentiles:' in block
        
        if not has_timestamp or not has_metadata or not has_percentiles:
            additions = []
            if not has_timestamp:
                additions.append('        timestamp: chrono::Utc::now(),')
            if not has_metadata:
                additions.append('        metadata: None,')
            if not has_percentiles:
                additions.append('        percentiles: None,')
            
            return block + ',\n' + '\n'.join(additions) + end
        
        return match.group(0)
    
    new_content = re.sub(pattern, replacement, content, flags=re.DOTALL)
    
    if new_content != content:
        with open(file_path, 'w') as f:
            f.write(new_content)
        print(f"Fixed {file_path}")
        return True
    return False

def main():
    files_to_fix = [
        "/Users/wings/projects/moosefs/mooseng/mooseng-benchmarks/src/benchmarks/infrastructure.rs",
        "/Users/wings/projects/moosefs/mooseng/mooseng-benchmarks/src/benchmarks/multiregion_infrastructure.rs", 
        "/Users/wings/projects/moosefs/mooseng/mooseng-benchmarks/src/benchmarks/multiregion_performance.rs",
        "/Users/wings/projects/moosefs/mooseng/mooseng-benchmarks/src/benchmarks/real_network.rs"
    ]
    
    for file_path in files_to_fix:
        if os.path.exists(file_path):
            fix_file(file_path)

if __name__ == "__main__":
    main()