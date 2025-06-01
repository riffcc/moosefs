#!/usr/bin/env python3
"""
Mock Client for MooseNG Demo
Simulates FUSE client behavior with monitoring endpoints
"""
import os
import json
import time
import random
from flask import Flask, jsonify
from datetime import datetime

app = Flask(__name__)

# Configuration from environment
CLIENT_ID = os.environ.get('MOOSENG_CLIENT_ID', 'client-1')
MASTER_ENDPOINTS = os.environ.get('MOOSENG_MASTER_ENDPOINTS', 'master-1:9421').split(',')
METRICS_PORT = int(os.environ.get('MOOSENG_METRICS_PORT', 9427))

# State
state = {
    'client_id': CLIENT_ID,
    'health': 'healthy',
    'mount_point': '/mnt/mooseng',
    'mounted': True,
    'start_time': time.time(),
    'cache': {
        'size_mb': 512,
        'used_mb': random.randint(50, 400),
        'entries': random.randint(100, 1000),
        'hit_rate': random.uniform(0.7, 0.95)
    },
    'stats': {
        'operations': 0,
        'bytes_read': 0,
        'bytes_written': 0,
        'open_files': random.randint(0, 10)
    }
}

@app.route('/health', methods=['GET'])
def health():
    """Health check endpoint"""
    uptime = int(time.time() - state['start_time'])
    return jsonify({
        'status': state['health'],
        'client_id': CLIENT_ID,
        'mounted': state['mounted'],
        'mount_point': state['mount_point'],
        'uptime_seconds': uptime,
        'timestamp': datetime.utcnow().isoformat()
    })

@app.route('/status', methods=['GET'])
def status():
    """Client status endpoint"""
    return jsonify({
        'client_id': CLIENT_ID,
        'mount_point': state['mount_point'],
        'mounted': state['mounted'],
        'cache': state['cache'],
        'stats': state['stats'],
        'master_connections': len(MASTER_ENDPOINTS),
        'version': '0.1.0-mock'
    })

@app.route('/metrics', methods=['GET'])
def metrics():
    """Prometheus metrics endpoint"""
    metrics_text = f"""# HELP mooseng_client_up Client up status
# TYPE mooseng_client_up gauge
mooseng_client_up{{client_id="{CLIENT_ID}"}} 1

# HELP mooseng_client_mounted Mount status (1=mounted, 0=unmounted)
# TYPE mooseng_client_mounted gauge
mooseng_client_mounted{{client_id="{CLIENT_ID}"}} {1 if state['mounted'] else 0}

# HELP mooseng_client_cache_size_mb Cache size in MB
# TYPE mooseng_client_cache_size_mb gauge
mooseng_client_cache_size_mb{{client_id="{CLIENT_ID}"}} {state['cache']['size_mb']}

# HELP mooseng_client_cache_used_mb Cache used in MB
# TYPE mooseng_client_cache_used_mb gauge
mooseng_client_cache_used_mb{{client_id="{CLIENT_ID}"}} {state['cache']['used_mb']}

# HELP mooseng_client_cache_hit_rate Cache hit rate
# TYPE mooseng_client_cache_hit_rate gauge
mooseng_client_cache_hit_rate{{client_id="{CLIENT_ID}"}} {state['cache']['hit_rate']}

# HELP mooseng_client_operations Total operations
# TYPE mooseng_client_operations counter
mooseng_client_operations{{client_id="{CLIENT_ID}"}} {state['stats']['operations']}

# HELP mooseng_client_bytes_read Total bytes read
# TYPE mooseng_client_bytes_read counter
mooseng_client_bytes_read{{client_id="{CLIENT_ID}"}} {state['stats']['bytes_read']}

# HELP mooseng_client_bytes_written Total bytes written
# TYPE mooseng_client_bytes_written counter
mooseng_client_bytes_written{{client_id="{CLIENT_ID}"}} {state['stats']['bytes_written']}

# HELP mooseng_client_open_files Number of open files
# TYPE mooseng_client_open_files gauge
mooseng_client_open_files{{client_id="{CLIENT_ID}"}} {state['stats']['open_files']}
"""
    return metrics_text, 200, {'Content-Type': 'text/plain'}

@app.route('/cache/stats', methods=['GET'])
def cache_stats():
    """Get detailed cache statistics"""
    # Simulate some activity
    state['stats']['operations'] += random.randint(1, 10)
    state['stats']['bytes_read'] += random.randint(1000, 100000)
    state['stats']['bytes_written'] += random.randint(1000, 50000)
    state['cache']['used_mb'] = min(state['cache']['size_mb'], 
                                    state['cache']['used_mb'] + random.randint(-10, 10))
    
    return jsonify({
        'client_id': CLIENT_ID,
        'cache': state['cache'],
        'performance': {
            'avg_read_latency_ms': random.uniform(0.1, 2.0),
            'avg_write_latency_ms': random.uniform(0.5, 5.0),
            'throughput_mbps': random.uniform(50, 200)
        }
    })

if __name__ == '__main__':
    # Create mount point directory
    os.makedirs(state['mount_point'], exist_ok=True)
    
    # Write a marker file to indicate mount
    with open(os.path.join(state['mount_point'], '.mooseng_mounted'), 'w') as f:
        f.write(f'{CLIENT_ID} mounted at {datetime.utcnow().isoformat()}\n')
    
    app.run(host='0.0.0.0', port=METRICS_PORT)