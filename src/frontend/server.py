#!/usr/bin/env python3
"""
Simple HTTP server to serve the MooseNG Dashboard
"""

import http.server
import socketserver
import os
import sys
import argparse
import json
import mimetypes
from pathlib import Path

class MooseNGDashboardHandler(http.server.SimpleHTTPRequestHandler):
    """Custom HTTP handler for the MooseNG Dashboard"""
    
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(Path(__file__).parent), **kwargs)
    
    def end_headers(self):
        # Add CORS headers for development
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type')
        super().end_headers()
    
    def do_GET(self):
        """Handle GET requests"""
        
        # API endpoints for mock data
        if self.path.startswith('/api/'):
            self.handle_api_request()
            return
        
        # Serve benchmark data
        if self.path.startswith('/benchmark-data/'):
            self.serve_benchmark_data()
            return
        
        # Default to serving static files
        super().do_GET()
    
    def handle_api_request(self):
        """Handle API requests for dashboard data"""
        endpoint = self.path[5:]  # Remove '/api/' prefix
        
        # Mock API responses
        api_responses = {
            'overview': {
                "overall_grade": "A (Very Good)",
                "throughput_mbps": "135.13",
                "ops_per_second": "1506.02",
                "total_time_ms": 3323,
                "performance_breakdown": {
                    "file_system_ms": 196,
                    "network_ms": 1782,
                    "cpu_memory_ms": 937,
                    "concurrency_ms": 29,
                    "throughput_ms": 379
                },
                "historical_data": self.generate_historical_data()
            },
            'master-server': {
                "status": "online",
                "uptime": "15d 7h 23m",
                "cpu_usage": 23,
                "memory_usage": 34,
                "active_connections": 127,
                "metadata_operations": 5847,
                "raft_leader": True,
                "cluster_health": 98.5,
                "servers": [
                    {"id": "master-1", "status": "leader", "uptime": "15d 7h", "cpu": 23, "memory": 34, "connections": 67},
                    {"id": "master-2", "status": "follower", "uptime": "15d 6h", "cpu": 19, "memory": 28, "connections": 34},
                    {"id": "master-3", "status": "follower", "uptime": "15d 5h", "cpu": 21, "memory": 31, "connections": 26}
                ]
            },
            'chunk-server': {
                "total_servers": 8,
                "online_servers": 7,
                "total_storage": "15.2",
                "used_storage": "8.7",
                "free_storage": "6.5",
                "replication_factor": 3,
                "erasure_coding": "8+2",
                "servers": [
                    {"id": f"chunk-{i+1}", "status": "offline" if i == 7 else "online", 
                     "storage_used": 45 + i*8, "storage_total": "2.0 TB", 
                     "chunks": 2000 + i*300, "read_ops": 150 + i*20, 
                     "write_ops": 50 + i*10, "network_io": f"{20 + i*5} MB/s"} 
                    for i in range(8)
                ]
            },
            'client': {
                "active_mounts": 42,
                "total_operations": 67834,
                "cache_hit_rate": "78.3",
                "average_latency": 7,
                "clients": [
                    {"id": f"client-{i+1}", "status": "disconnected" if i == 11 else "connected",
                     "mount_point": f"/mnt/mooseng{i+1}", "operations": 500 + i*150,
                     "cache_size": f"{100 + i*50} MB", "bandwidth": f"{50 + i*10} MB/s"}
                    for i in range(12)
                ]
            }
        }
        
        response_data = api_responses.get(endpoint)
        if response_data:
            self.send_response(200)
            self.send_header('Content-type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps(response_data).encode())
        else:
            self.send_response(404)
            self.end_headers()
    
    def serve_benchmark_data(self):
        """Serve benchmark data from the mooseng directory"""
        # Try to find and serve actual benchmark data
        benchmark_path = Path(__file__).parent.parent.parent / "mooseng" / "benchmark_results"
        
        if benchmark_path.exists():
            # Find the latest benchmark result
            result_dirs = [d for d in benchmark_path.iterdir() if d.is_dir()]
            if result_dirs:
                latest_dir = max(result_dirs, key=lambda x: x.stat().st_mtime)
                json_file = latest_dir / "performance_results.json"
                
                if json_file.exists():
                    self.send_response(200)
                    self.send_header('Content-type', 'application/json')
                    self.end_headers()
                    with open(json_file, 'rb') as f:
                        self.wfile.write(f.read())
                    return
        
        # Fallback to mock data
        mock_data = {
            "timestamp": "2025-05-31T15:54:21+01:00",
            "environment": {
                "os": "Linux",
                "arch": "x86_64", 
                "cores": "16"
            },
            "results": {
                "file_system_ms": 196,
                "network_ms": 1782,
                "cpu_memory_ms": 937,
                "concurrency_ms": 29,
                "throughput_ms": 379,
                "throughput_mbps": "135.13",
                "total_time_ms": 3323,
                "ops_per_second": "1506.02",
                "grade": "A (Very Good)"
            }
        }
        
        self.send_response(200)
        self.send_header('Content-type', 'application/json')
        self.end_headers()
        self.wfile.write(json.dumps(mock_data).encode())
    
    def generate_historical_data(self):
        """Generate mock historical data for charts"""
        import datetime
        import random
        
        data = []
        now = datetime.datetime.now()
        
        for i in range(24):
            timestamp = now - datetime.timedelta(hours=23-i)
            data.append({
                "timestamp": timestamp.isoformat(),
                "throughput": random.randint(100, 150),
                "response_time": random.randint(200, 400),
                "ops_per_second": random.randint(1200, 1600)
            })
        
        return data
    
    def log_message(self, format, *args):
        """Override to provide cleaner logging"""
        print(f"[{self.date_time_string()}] {format % args}")

def main():
    parser = argparse.ArgumentParser(description='MooseNG Dashboard Server')
    parser.add_argument('--port', type=int, default=8080, help='Port to serve on (default: 8080)')
    parser.add_argument('--host', default='localhost', help='Host to bind to (default: localhost)')
    args = parser.parse_args()
    
    # Change to the frontend directory
    frontend_dir = Path(__file__).parent
    os.chdir(frontend_dir)
    
    print(f"🚀 Starting MooseNG Dashboard Server...")
    print(f"📍 Serving from: {frontend_dir}")
    print(f"🌐 Server address: http://{args.host}:{args.port}")
    print(f"📊 Dashboard URL: http://{args.host}:{args.port}/index.html")
    print(f"🔧 API endpoints available at: http://{args.host}:{args.port}/api/")
    print()
    print("Available API endpoints:")
    print("  - /api/overview")
    print("  - /api/master-server") 
    print("  - /api/chunk-server")
    print("  - /api/client")
    print("  - /benchmark-data/latest")
    print()
    print("Press Ctrl+C to stop the server")
    print()
    
    try:
        with socketserver.TCPServer((args.host, args.port), MooseNGDashboardHandler) as httpd:
            httpd.serve_forever()
    except KeyboardInterrupt:
        print("\n🛑 Server stopped by user")
    except Exception as e:
        print(f"❌ Server error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()