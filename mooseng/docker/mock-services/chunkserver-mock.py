#!/usr/bin/env python3
"""
Mock Chunk Server for MooseNG Demo
Simulates basic chunk server functionality with HTTP endpoints
"""
import os
import json
import time
import random
from flask import Flask, jsonify, request
from datetime import datetime

app = Flask(__name__)

# Configuration from environment
SERVER_ID = int(os.environ.get('MOOSENG_SERVER_ID', 1))
MASTER_ENDPOINTS = os.environ.get('MOOSENG_MASTER_ENDPOINTS', 'master-1:9422').split(',')
PORT = int(os.environ.get('MOOSENG_PORT', 9420))

# State
state = {
    'server_id': SERVER_ID,
    'health': 'healthy',
    'start_time': time.time(),
    'stats': {
        'chunks': random.randint(100, 500),
        'capacity_bytes': 107374182400,  # 100GB
        'used_bytes': random.randint(10000000000, 50000000000),
        'io_operations': 0,
        'bytes_read': 0,
        'bytes_written': 0
    },
    'data_dirs': ['/data1', '/data2', '/data3', '/data4']
}

@app.route('/health', methods=['GET'])
def health():
    """Health check endpoint"""
    uptime = int(time.time() - state['start_time'])
    return jsonify({
        'status': state['health'],
        'server_id': SERVER_ID,
        'uptime_seconds': uptime,
        'timestamp': datetime.utcnow().isoformat()
    })

@app.route('/status', methods=['GET'])
def status():
    """Chunk server status endpoint"""
    return jsonify({
        'server_id': SERVER_ID,
        'stats': state['stats'],
        'data_dirs': state['data_dirs'],
        'master_connections': len(MASTER_ENDPOINTS),
        'version': '0.1.0-mock'
    })

@app.route('/metrics', methods=['GET'])
def metrics():
    """Prometheus metrics endpoint"""
    metrics_text = f"""# HELP mooseng_chunkserver_up Chunk server up status
# TYPE mooseng_chunkserver_up gauge
mooseng_chunkserver_up{{server_id="{SERVER_ID}"}} 1

# HELP mooseng_chunkserver_chunks Total number of chunks
# TYPE mooseng_chunkserver_chunks gauge
mooseng_chunkserver_chunks{{server_id="{SERVER_ID}"}} {state['stats']['chunks']}

# HELP mooseng_chunkserver_capacity_bytes Total capacity in bytes
# TYPE mooseng_chunkserver_capacity_bytes gauge
mooseng_chunkserver_capacity_bytes{{server_id="{SERVER_ID}"}} {state['stats']['capacity_bytes']}

# HELP mooseng_chunkserver_used_bytes Used space in bytes
# TYPE mooseng_chunkserver_used_bytes gauge
mooseng_chunkserver_used_bytes{{server_id="{SERVER_ID}"}} {state['stats']['used_bytes']}

# HELP mooseng_chunkserver_io_operations Total I/O operations
# TYPE mooseng_chunkserver_io_operations counter
mooseng_chunkserver_io_operations{{server_id="{SERVER_ID}"}} {state['stats']['io_operations']}

# HELP mooseng_chunkserver_bytes_read Total bytes read
# TYPE mooseng_chunkserver_bytes_read counter
mooseng_chunkserver_bytes_read{{server_id="{SERVER_ID}"}} {state['stats']['bytes_read']}

# HELP mooseng_chunkserver_bytes_written Total bytes written
# TYPE mooseng_chunkserver_bytes_written counter
mooseng_chunkserver_bytes_written{{server_id="{SERVER_ID}"}} {state['stats']['bytes_written']}
"""
    return metrics_text, 200, {'Content-Type': 'text/plain'}

@app.route('/api/v1/chunk/<chunk_id>', methods=['GET'])
def get_chunk(chunk_id):
    """Mock chunk read operation"""
    state['stats']['io_operations'] += 1
    state['stats']['bytes_read'] += 65536  # 64KB
    
    return jsonify({
        'chunk_id': chunk_id,
        'size': 67108864,  # 64MB
        'checksum': 'mock-checksum-' + chunk_id,
        'version': 1
    })

@app.route('/api/v1/chunk/<chunk_id>', methods=['PUT'])
def put_chunk(chunk_id):
    """Mock chunk write operation"""
    state['stats']['io_operations'] += 1
    state['stats']['bytes_written'] += 65536  # 64KB
    state['stats']['chunks'] += 1
    state['stats']['used_bytes'] += 67108864  # 64MB
    
    return jsonify({
        'chunk_id': chunk_id,
        'status': 'stored',
        'replicas': 2
    })

@app.route('/api/v1/chunk/<chunk_id>', methods=['DELETE'])
def delete_chunk(chunk_id):
    """Mock chunk delete operation"""
    if state['stats']['chunks'] > 0:
        state['stats']['chunks'] -= 1
        state['stats']['used_bytes'] -= 67108864  # 64MB
    
    return jsonify({
        'chunk_id': chunk_id,
        'status': 'deleted'
    })

@app.route('/api/v1/replicate', methods=['POST'])
def replicate_chunk():
    """Mock chunk replication"""
    source_chunk = request.json.get('chunk_id')
    target_server = request.json.get('target_server')
    
    state['stats']['io_operations'] += 2
    state['stats']['bytes_read'] += 67108864
    state['stats']['bytes_written'] += 67108864
    
    return jsonify({
        'chunk_id': source_chunk,
        'target_server': target_server,
        'status': 'replicated',
        'duration_ms': random.randint(100, 500)
    })

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=PORT)