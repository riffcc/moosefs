#!/usr/bin/env python3
"""
Mock Master Server for MooseNG Demo
Simulates basic master server functionality with HTTP endpoints
"""
import os
import json
import time
import random
from flask import Flask, jsonify, request
from datetime import datetime

app = Flask(__name__)

# Configuration from environment
NODE_ID = int(os.environ.get('MOOSENG_NODE_ID', 1))
CLUSTER_NAME = os.environ.get('MOOSENG_CLUSTER_NAME', 'mooseng-demo')
CLIENT_PORT = int(os.environ.get('MOOSENG_CLIENT_PORT', 9421))

# State
state = {
    'node_id': NODE_ID,
    'cluster_name': CLUSTER_NAME,
    'role': 'follower',
    'term': 0,
    'leader_id': None,
    'health': 'healthy',
    'start_time': time.time(),
    'stats': {
        'files': 0,
        'directories': 0,
        'chunks': 0,
        'metadata_version': 1
    }
}

# Simulate leader election on startup
if NODE_ID == 1:
    state['role'] = 'leader'
    state['leader_id'] = NODE_ID
    state['term'] = 1

@app.route('/health', methods=['GET'])
def health():
    """Health check endpoint"""
    uptime = int(time.time() - state['start_time'])
    return jsonify({
        'status': state['health'],
        'node_id': NODE_ID,
        'role': state['role'],
        'uptime_seconds': uptime,
        'timestamp': datetime.utcnow().isoformat()
    })

@app.route('/status', methods=['GET'])
def status():
    """Master status endpoint"""
    return jsonify({
        'cluster_name': CLUSTER_NAME,
        'node_id': NODE_ID,
        'role': state['role'],
        'term': state['term'],
        'leader_id': state['leader_id'],
        'stats': state['stats'],
        'version': '0.1.0-mock'
    })

@app.route('/metrics', methods=['GET'])
def metrics():
    """Prometheus metrics endpoint"""
    metrics_text = f"""# HELP mooseng_master_up Master server up status
# TYPE mooseng_master_up gauge
mooseng_master_up{{node_id="{NODE_ID}",cluster="{CLUSTER_NAME}"}} 1

# HELP mooseng_master_role Master server role (1=leader, 0=follower)
# TYPE mooseng_master_role gauge
mooseng_master_role{{node_id="{NODE_ID}"}} {1 if state['role'] == 'leader' else 0}

# HELP mooseng_master_term Current Raft term
# TYPE mooseng_master_term counter
mooseng_master_term{{node_id="{NODE_ID}"}} {state['term']}

# HELP mooseng_master_files Total number of files
# TYPE mooseng_master_files gauge
mooseng_master_files{{node_id="{NODE_ID}"}} {state['stats']['files']}

# HELP mooseng_master_directories Total number of directories
# TYPE mooseng_master_directories gauge
mooseng_master_directories{{node_id="{NODE_ID}"}} {state['stats']['directories']}

# HELP mooseng_master_chunks Total number of chunks
# TYPE mooseng_master_chunks gauge
mooseng_master_chunks{{node_id="{NODE_ID}"}} {state['stats']['chunks']}
"""
    return metrics_text, 200, {'Content-Type': 'text/plain'}

@app.route('/api/v1/filesystem/stat', methods=['POST'])
def filesystem_stat():
    """Mock filesystem stat operation"""
    path = request.json.get('path', '/')
    
    # Mock response
    if path == '/':
        return jsonify({
            'inode': 1,
            'type': 'directory',
            'mode': 0o755,
            'uid': 0,
            'gid': 0,
            'size': 4096,
            'mtime': int(time.time()),
            'atime': int(time.time()),
            'ctime': int(time.time()),
            'nlink': 2
        })
    else:
        return jsonify({'error': 'Not found'}), 404

@app.route('/api/v1/filesystem/create', methods=['POST'])
def filesystem_create():
    """Mock file creation"""
    state['stats']['files'] += 1
    state['stats']['chunks'] += random.randint(1, 10)
    
    return jsonify({
        'inode': random.randint(1000, 9999),
        'status': 'created',
        'chunks': state['stats']['chunks']
    })

@app.route('/api/v1/chunkservers', methods=['GET'])
def list_chunkservers():
    """List connected chunk servers"""
    return jsonify({
        'chunkservers': [
            {
                'id': 1,
                'hostname': 'chunkserver-1',
                'ip': '172.20.0.10',
                'port': 9420,
                'status': 'active',
                'capacity_bytes': 107374182400,  # 100GB
                'used_bytes': random.randint(0, 50000000000),
                'chunks': random.randint(100, 1000)
            },
            {
                'id': 2,
                'hostname': 'chunkserver-2',
                'ip': '172.20.0.11',
                'port': 9420,
                'status': 'active',
                'capacity_bytes': 107374182400,
                'used_bytes': random.randint(0, 50000000000),
                'chunks': random.randint(100, 1000)
            },
            {
                'id': 3,
                'hostname': 'chunkserver-3',
                'ip': '172.20.0.12',
                'port': 9420,
                'status': 'active',
                'capacity_bytes': 107374182400,
                'used_bytes': random.randint(0, 50000000000),
                'chunks': random.randint(100, 1000)
            }
        ]
    })

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=CLIENT_PORT)