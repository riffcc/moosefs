# MooseNG Unified Benchmark Dashboard

A comprehensive web-based dashboard for visualizing MooseNG benchmark results, providing real-time monitoring, historical analysis, and performance insights.

## Features

### 🚀 Real-time Monitoring
- Live benchmark execution tracking
- WebSocket-based real-time updates
- Performance metric streaming
- System resource monitoring

### 📊 Benchmark Results Visualization
- Interactive charts and graphs
- Performance trend analysis
- Comparison between benchmark runs
- Detailed operation breakdowns

### 📈 Historical Analysis
- Long-term performance trends
- Regression detection
- Performance grade tracking
- Baseline comparisons

### 🎯 Multi-format Support
- JSON, CSV, HTML, and Markdown reports
- Prometheus metrics export
- Grafana dashboard integration
- Custom report templates

## Architecture

### Frontend Components
```
unified_dashboard/
├── components/
│   ├── charts/           # Chart components (Chart.js, D3.js)
│   ├── layout/           # Layout and navigation components
│   └── common/           # Shared UI components
├── pages/
│   ├── overview/         # Dashboard overview page
│   ├── benchmark-results/# Historical results browser
│   └── live-monitoring/  # Real-time monitoring interface
├── assets/
│   ├── css/             # Stylesheets
│   ├── js/              # JavaScript modules
│   └── images/          # Static assets
└── public/              # Served static files
```

### Backend Integration
- Rust-based backend server (Axum framework)
- SQLite/PostgreSQL database support
- RESTful API endpoints
- WebSocket real-time streaming
- Prometheus metrics export

## Getting Started

### Prerequisites
- Node.js 14+ (for frontend development)
- Rust 1.70+ (for backend)
- Modern web browser

### Installation
```bash
# Install frontend dependencies
cd unified_dashboard
npm install

# Start development server
npm run dev

# Build for production
npm run build
```

### Backend Server
```bash
# Run unified benchmark runner with dashboard
cd ../mooseng-benchmarks
cargo run --bin mooseng-bench dashboard --port 8080 --realtime

# Or run dashboard server directly
cargo run --bin mooseng-dashboard
```

## Usage

### Starting the Dashboard
1. Launch the backend server:
   ```bash
   mooseng-bench dashboard --port 8080 --realtime
   ```

2. Open your browser to `http://localhost:8080`

### Running Benchmarks with Live Monitoring
```bash
# Run benchmarks with live dashboard updates
mooseng-bench all --output ./results --format json

# Monitor specific benchmark in real-time
mooseng-bench monitor --interval 5 --alerts
```

### Viewing Historical Results
1. Navigate to the "Benchmark Results" page
2. Select time range or specific benchmark runs
3. Compare performance across different configurations
4. Export results in various formats

## Configuration

### Dashboard Settings
```toml
[dashboard]
enabled = true
port = 8080
host = "0.0.0.0"
realtime_updates = true
websocket_port = 8081

[database]
url = "sqlite:benchmark_results.db"
max_connections = 10
timeout_seconds = 30
```

### Benchmark Integration
The dashboard automatically integrates with the unified benchmark runner:
- Results are stored in the configured database
- Real-time updates stream via WebSocket
- Charts and visualizations update automatically
- Performance alerts can be configured

## API Reference

### REST Endpoints
- `GET /api/results` - Get benchmark results
- `GET /api/results/{id}` - Get specific result
- `POST /api/benchmarks/start` - Start benchmark run
- `GET /api/metrics` - Get Prometheus metrics
- `GET /api/status` - Get system status

### WebSocket Events
- `benchmark:started` - Benchmark execution started
- `benchmark:progress` - Progress update
- `benchmark:completed` - Benchmark finished
- `metrics:update` - Real-time metrics update

## Customization

### Adding Custom Charts
```javascript
// components/charts/CustomChart.js
import { ChartComponent } from './ChartComponent.js';

export class CustomChart extends ChartComponent {
    constructor(container, data) {
        super(container, data);
        this.chartType = 'custom';
    }
    
    render() {
        // Custom chart implementation
    }
}
```

### Custom Metrics
```rust
// Custom metrics in backend
pub struct CustomMetric {
    pub name: String,
    pub value: f64,
    pub timestamp: DateTime<Utc>,
}
```

## Performance

### Optimization Features
- Client-side caching
- Lazy loading of large datasets
- Efficient WebSocket compression
- Database query optimization
- Progressive chart rendering

### Scalability
- Supports thousands of benchmark results
- Efficient real-time streaming
- Horizontal scaling support
- Database sharding capabilities

## Contributing

### Development Workflow
1. Fork the repository
2. Create feature branch
3. Make changes with tests
4. Submit pull request

### Code Style
- Frontend: ESLint + Prettier
- Backend: rustfmt + clippy
- Documentation: Markdown + JSDoc

## License

This project is part of the MooseNG distributed file system and follows the same licensing terms.

## Support

For issues and questions:
- GitHub Issues: [MooseNG Repository](https://github.com/mooseng/mooseng)
- Documentation: [MooseNG Docs](https://docs.mooseng.io)
- Community: [MooseNG Discord](https://discord.gg/mooseng)