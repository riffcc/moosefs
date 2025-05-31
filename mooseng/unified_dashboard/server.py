#!/usr/bin/env python3
"""
MooseNG Unified Dashboard Backend Server
Provides REST API endpoints and WebSocket support for real-time data
"""

import asyncio
import json
import logging
import sqlite3
import time
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Optional

from aiohttp import web, WSMsgType

# Setup logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class DashboardServer:
    def __init__(self, host='localhost', port=8080, db_path='dashboard.db'):
        self.host = host
        self.port = port
        self.db_path = db_path
        self.connected_clients = set()
        self.app = None
        self.db_connection = None
        self.benchmark_results_cache = {}
        self.live_metrics_cache = {}
        
    async def init_database(self):
        """Initialize SQLite database with tables for benchmark data"""
        self.db_connection = sqlite3.connect(self.db_path, check_same_thread=False)
        cursor = self.db_connection.cursor()
        
        # Create benchmark results table
        cursor.execute('''
            CREATE TABLE IF NOT EXISTS benchmark_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                category TEXT NOT NULL,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                duration_ms INTEGER,
                throughput_mbps REAL,
                latency_ms REAL,
                cpu_usage REAL,
                memory_usage REAL,
                success BOOLEAN DEFAULT TRUE,
                error_message TEXT,
                metadata TEXT
            )
        ''')
        
        # Create live metrics table
        cursor.execute('''
            CREATE TABLE IF NOT EXISTS live_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                metric_name TEXT NOT NULL,
                value REAL NOT NULL,
                unit TEXT,
                source TEXT
            )
        ''')
        
        # Create system status table
        cursor.execute('''
            CREATE TABLE IF NOT EXISTS system_status (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                component TEXT NOT NULL,
                status TEXT NOT NULL,
                details TEXT
            )
        ''')
        
        self.db_connection.commit()
        logger.info("Database initialized successfully")

    async def setup_routes(self):
        """Setup HTTP routes for the dashboard API"""
        self.app = web.Application()
        
        # Static file serving
        self.app.router.add_static('/', Path(__file__).parent / 'public', name='static')
        
        # API routes
        self.app.router.add_get('/api/status', self.get_system_status)
        self.app.router.add_get('/api/results', self.get_benchmark_results)
        self.app.router.add_get('/api/results/detailed', self.get_detailed_results)
        self.app.router.add_get('/api/summary', self.get_summary)
        self.app.router.add_get('/api/benchmarks', self.get_available_benchmarks)
        self.app.router.add_get('/api/metrics/current', self.get_current_metrics)
        self.app.router.add_get('/api/analysis', self.get_analysis_data)
        
        # Control endpoints
        self.app.router.add_post('/api/benchmarks/run', self.run_benchmarks)
        self.app.router.add_post('/api/benchmarks/stop', self.stop_benchmarks)
        
        # WebSocket endpoint
        self.app.router.add_get('/ws', self.websocket_handler)
        
        # Simple CORS middleware
        async def cors_middleware(request, handler):
            response = await handler(request)
            response.headers['Access-Control-Allow-Origin'] = '*'
            response.headers['Access-Control-Allow-Methods'] = 'GET, POST, PUT, DELETE, OPTIONS'
            response.headers['Access-Control-Allow-Headers'] = '*'
            return response
        
        self.app.middlewares.append(cors_middleware)

    async def get_system_status(self, request):
        """Get current system status"""
        try:
            cursor = self.db_connection.cursor()
            cursor.execute('''
                SELECT component, status, details 
                FROM system_status 
                WHERE timestamp > datetime('now', '-5 minutes')
                ORDER BY timestamp DESC
            ''')
            
            results = cursor.fetchall()
            
            # Mock data if no real data available
            if not results:
                status_data = {
                    'healthy': True,
                    'active_services': 4,
                    'total_services': 4,
                    'uptime': '2d 14h 32m',
                    'last_update': datetime.now().isoformat()
                }
            else:
                # Process real status data
                healthy_components = sum(1 for _, status, _ in results if status == 'healthy')
                total_components = len(results)
                
                status_data = {
                    'healthy': healthy_components == total_components,
                    'active_services': healthy_components,
                    'total_services': total_components,
                    'uptime': '2d 14h 32m',  # Would be calculated from actual uptime
                    'last_update': datetime.now().isoformat()
                }
            
            return web.json_response(status_data)
            
        except Exception as e:
            logger.error(f"Error getting system status: {e}")
            return web.json_response({'error': str(e)}, status=500)

    async def get_benchmark_results(self, request):
        """Get recent benchmark results"""
        try:
            limit = int(request.query.get('limit', 100))
            category = request.query.get('category')
            
            cursor = self.db_connection.cursor()
            
            query = '''
                SELECT name, category, timestamp, duration_ms, throughput_mbps, 
                       latency_ms, cpu_usage, memory_usage, success
                FROM benchmark_results 
                WHERE 1=1
            '''
            params = []
            
            if category:
                query += ' AND category = ?'
                params.append(category)
                
            query += ' ORDER BY timestamp DESC LIMIT ?'
            params.append(limit)
            
            cursor.execute(query, params)
            results = cursor.fetchall()
            
            # Convert to list of dictionaries
            benchmark_data = []
            for row in results:
                benchmark_data.append({
                    'name': row[0],
                    'category': row[1],
                    'timestamp': row[2],
                    'duration_ms': row[3],
                    'throughput_mbps': row[4],
                    'latency_ms': row[5],
                    'cpu_usage': row[6],
                    'memory_usage': row[7],
                    'success': bool(row[8])
                })
            
            # Add mock data if no real data available
            if not benchmark_data:
                benchmark_data = self.generate_mock_benchmark_data()
            
            return web.json_response(benchmark_data)
            
        except Exception as e:
            logger.error(f"Error getting benchmark results: {e}")
            return web.json_response({'error': str(e)}, status=500)

    async def get_detailed_results(self, request):
        """Get detailed benchmark results with additional metadata"""
        try:
            results = await self.get_benchmark_results(request)
            # Add additional processing for detailed view
            return results
        except Exception as e:
            logger.error(f"Error getting detailed results: {e}")
            return web.json_response({'error': str(e)}, status=500)

    async def get_summary(self, request):
        """Get summary statistics for the overview page"""
        try:
            cursor = self.db_connection.cursor()
            
            # Get total benchmarks
            cursor.execute('SELECT COUNT(*) FROM benchmark_results')
            total_benchmarks = cursor.fetchone()[0]
            
            # Get average throughput
            cursor.execute('SELECT AVG(throughput_mbps) FROM benchmark_results WHERE success = 1')
            avg_throughput = cursor.fetchone()[0] or 0
            
            # Get average latency
            cursor.execute('SELECT AVG(latency_ms) FROM benchmark_results WHERE success = 1')
            avg_latency = cursor.fetchone()[0] or 0
            
            # Get success rate
            cursor.execute('SELECT AVG(CAST(success AS FLOAT)) FROM benchmark_results')
            success_rate = cursor.fetchone()[0] or 0
            
            # Mock data if no real data
            if total_benchmarks == 0:
                summary_data = {
                    'total_benchmarks': 156,
                    'avg_throughput': 1250.5,
                    'avg_latency': 12.3,
                    'success_rate': 0.987,
                    'recent_performance': self.generate_mock_performance_data()
                }
            else:
                summary_data = {
                    'total_benchmarks': total_benchmarks,
                    'avg_throughput': avg_throughput,
                    'avg_latency': avg_latency,
                    'success_rate': success_rate,
                    'recent_performance': self.generate_mock_performance_data()
                }
            
            return web.json_response(summary_data)
            
        except Exception as e:
            logger.error(f"Error getting summary: {e}")
            return web.json_response({'error': str(e)}, status=500)

    async def get_available_benchmarks(self, request):
        """Get list of available benchmarks"""
        try:
            # This would typically come from the Rust benchmark runner
            benchmarks = [
                {
                    'id': 'file_operations',
                    'name': 'File Operations',
                    'category': 'file_io',
                    'description': 'Tests basic file read/write operations',
                    'estimated_duration': '5 minutes'
                },
                {
                    'id': 'metadata_operations',
                    'name': 'Metadata Operations',
                    'category': 'metadata',
                    'description': 'Tests metadata operations and filesystem calls',
                    'estimated_duration': '3 minutes'
                },
                {
                    'id': 'network_simulation',
                    'name': 'Network Simulation',
                    'category': 'network',
                    'description': 'Simulates network conditions and latency',
                    'estimated_duration': '10 minutes'
                },
                {
                    'id': 'multiregion',
                    'name': 'Multi-region Performance',
                    'category': 'multiregion',
                    'description': 'Tests performance across multiple regions',
                    'estimated_duration': '15 minutes'
                }
            ]
            
            return web.json_response(benchmarks)
            
        except Exception as e:
            logger.error(f"Error getting available benchmarks: {e}")
            return web.json_response({'error': str(e)}, status=500)

    async def get_current_metrics(self, request):
        """Get current live metrics"""
        try:
            cursor = self.db_connection.cursor()
            cursor.execute('''
                SELECT metric_name, value, unit, timestamp
                FROM live_metrics 
                WHERE timestamp > datetime('now', '-1 minute')
                ORDER BY timestamp DESC
            ''')
            
            results = cursor.fetchall()
            
            # Mock data if no real data
            if not results:
                metrics = self.generate_mock_live_metrics()
            else:
                metrics = {}
                for metric_name, value, unit, timestamp in results:
                    if metric_name not in metrics:
                        metrics[metric_name] = []
                    metrics[metric_name].append({
                        'value': value,
                        'unit': unit,
                        'timestamp': timestamp
                    })
            
            return web.json_response(metrics)
            
        except Exception as e:
            logger.error(f"Error getting current metrics: {e}")
            return web.json_response({'error': str(e)}, status=500)

    async def get_analysis_data(self, request):
        """Get analysis and trend data"""
        try:
            # This would perform complex analysis on benchmark data
            analysis_data = {
                'performance_trend': self.generate_mock_trend_data(),
                'regression_analysis': {
                    'detected_regressions': 2,
                    'performance_score': 85.3,
                    'trend_direction': 'improving'
                },
                'resource_analysis': {
                    'cpu_efficiency': 0.87,
                    'memory_efficiency': 0.92,
                    'io_efficiency': 0.78
                }
            }
            
            return web.json_response(analysis_data)
            
        except Exception as e:
            logger.error(f"Error getting analysis data: {e}")
            return web.json_response({'error': str(e)}, status=500)

    async def run_benchmarks(self, request):
        """Start running selected benchmarks"""
        try:
            data = await request.json()
            benchmarks = data.get('benchmarks', [])
            
            # This would integrate with the Rust CLI runner
            logger.info(f"Starting benchmarks: {benchmarks}")
            
            # Broadcast to connected clients
            await self.broadcast_to_clients({
                'type': 'benchmark_update',
                'payload': {
                    'status': 'started',
                    'benchmarks': benchmarks,
                    'timestamp': datetime.now().isoformat()
                }
            })
            
            return web.json_response({'status': 'started', 'benchmarks': benchmarks})
            
        except Exception as e:
            logger.error(f"Error starting benchmarks: {e}")
            return web.json_response({'error': str(e)}, status=500)

    async def stop_benchmarks(self, request):
        """Stop running benchmarks"""
        try:
            logger.info("Stopping benchmarks")
            
            # Broadcast to connected clients
            await self.broadcast_to_clients({
                'type': 'benchmark_update',
                'payload': {
                    'status': 'stopped',
                    'timestamp': datetime.now().isoformat()
                }
            })
            
            return web.json_response({'status': 'stopped'})
            
        except Exception as e:
            logger.error(f"Error stopping benchmarks: {e}")
            return web.json_response({'error': str(e)}, status=500)

    async def websocket_handler(self, request):
        """Handle WebSocket connections for real-time updates"""
        ws = web.WebSocketResponse()
        await ws.prepare(request)
        
        self.connected_clients.add(ws)
        logger.info(f"WebSocket client connected. Total clients: {len(self.connected_clients)}")
        
        try:
            async for msg in ws:
                if msg.type == WSMsgType.TEXT:
                    try:
                        data = json.loads(msg.data)
                        await self.handle_websocket_message(ws, data)
                    except json.JSONDecodeError:
                        logger.error(f"Invalid JSON received: {msg.data}")
                elif msg.type == WSMsgType.ERROR:
                    logger.error(f"WebSocket error: {ws.exception()}")
                    break
        except Exception as e:
            logger.error(f"WebSocket error: {e}")
        finally:
            self.connected_clients.discard(ws)
            logger.info(f"WebSocket client disconnected. Total clients: {len(self.connected_clients)}")
        
        return ws

    async def handle_websocket_message(self, ws, data):
        """Handle incoming WebSocket messages"""
        message_type = data.get('type')
        
        if message_type == 'start_monitoring':
            logger.info("Starting live monitoring for client")
            # Start sending live updates to this client
            
        elif message_type == 'stop_monitoring':
            logger.info("Stopping live monitoring for client")
            # Stop sending live updates to this client
            
        else:
            logger.warning(f"Unknown WebSocket message type: {message_type}")

    async def broadcast_to_clients(self, message):
        """Broadcast message to all connected WebSocket clients"""
        if not self.connected_clients:
            return
            
        message_text = json.dumps(message)
        disconnected_clients = set()
        
        for client in self.connected_clients:
            try:
                await client.send_str(message_text)
            except Exception as e:
                logger.error(f"Error sending to client: {e}")
                disconnected_clients.add(client)
        
        # Remove disconnected clients
        for client in disconnected_clients:
            self.connected_clients.discard(client)

    async def start_live_metrics_generator(self):
        """Generate mock live metrics periodically"""
        while True:
            try:
                metrics = {
                    'type': 'live_metrics',
                    'payload': self.generate_mock_live_metrics()
                }
                
                await self.broadcast_to_clients(metrics)
                await asyncio.sleep(5)  # Send updates every 5 seconds
                
            except Exception as e:
                logger.error(f"Error generating live metrics: {e}")
                await asyncio.sleep(5)

    def generate_mock_benchmark_data(self):
        """Generate mock benchmark data for testing"""
        import random
        
        categories = ['file_io', 'metadata', 'network', 'multiregion']
        benchmarks = []
        
        for i in range(20):
            benchmarks.append({
                'name': f'benchmark_{i:03d}',
                'category': random.choice(categories),
                'timestamp': (datetime.now() - timedelta(hours=random.randint(0, 48))).isoformat(),
                'duration_ms': random.randint(1000, 30000),
                'throughput_mbps': random.uniform(500, 2000),
                'latency_ms': random.uniform(5, 50),
                'cpu_usage': random.uniform(20, 80),
                'memory_usage': random.uniform(30, 90),
                'success': random.choice([True, True, True, False])  # 75% success rate
            })
        
        return benchmarks

    def generate_mock_performance_data(self):
        """Generate mock performance data for charts"""
        import random
        
        data = {
            'labels': [],
            'throughput': [],
            'latency': []
        }
        
        for i in range(24):  # Last 24 hours
            time_point = datetime.now() - timedelta(hours=23-i)
            data['labels'].append(time_point.strftime('%H:%M'))
            data['throughput'].append(random.uniform(800, 1500))
            data['latency'].append(random.uniform(8, 25))
        
        return data

    def generate_mock_live_metrics(self):
        """Generate mock live metrics"""
        import random
        
        return {
            'cpu': random.uniform(20, 80),
            'memory': random.uniform(30, 90),
            'disk_io': random.uniform(50, 200),
            'network_io': random.uniform(100, 300),
            'timestamp': datetime.now().isoformat()
        }

    def generate_mock_trend_data(self):
        """Generate mock trend analysis data"""
        import random
        
        data = {
            'dates': [],
            'performance': [],
            'regression': []
        }
        
        base_performance = 100
        for i in range(30):  # Last 30 days
            date = datetime.now() - timedelta(days=29-i)
            data['dates'].append(date.strftime('%m/%d'))
            
            # Simulate gradual improvement with some variance
            base_performance += random.uniform(-2, 3)
            data['performance'].append(max(50, min(150, base_performance)))
            data['regression'].append(base_performance * 0.95)  # Regression line
        
        return data

    async def start_server(self):
        """Start the dashboard server"""
        try:
            await self.init_database()
            await self.setup_routes()
            
            # Start background tasks
            asyncio.create_task(self.start_live_metrics_generator())
            
            # Start the web server
            runner = web.AppRunner(self.app)
            await runner.setup()
            
            site = web.TCPSite(runner, self.host, self.port)
            await site.start()
            
            logger.info(f"Dashboard server started at http://{self.host}:{self.port}")
            
            # Keep the server running
            try:
                await asyncio.Future()  # Run forever
            except KeyboardInterrupt:
                logger.info("Server shutdown requested")
            finally:
                await runner.cleanup()
                if self.db_connection:
                    self.db_connection.close()
                    
        except Exception as e:
            logger.error(f"Error starting server: {e}")
            raise

async def main():
    """Main entry point"""
    import argparse
    
    parser = argparse.ArgumentParser(description='MooseNG Unified Dashboard Server')
    parser.add_argument('--host', default='localhost', help='Host to bind to')
    parser.add_argument('--port', type=int, default=8080, help='Port to bind to')
    parser.add_argument('--db', default='dashboard.db', help='Database file path')
    
    args = parser.parse_args()
    
    server = DashboardServer(host=args.host, port=args.port, db_path=args.db)
    await server.start_server()

if __name__ == '__main__':
    asyncio.run(main())