# MooseNG Performance Dashboard

A comprehensive HTML-based benchmark reporting system for MooseNG distributed file system.

## Features

### 🎯 Core Visualization Framework
- **Modular Dashboard Architecture**: Built with vanilla JavaScript and Web Components
- **Modern Responsive Design**: CSS Grid and Flexbox layouts with dark/light themes
- **Interactive Charts**: Chart.js and D3.js integration for rich visualizations
- **Real-time Updates**: Auto-refreshing data with WebSocket-ready architecture
- **Component-based**: Reusable chart components and data managers

### 📊 Performance Monitoring Sections

#### 1. Overview Dashboard
- System-wide performance metrics and KPIs
- Performance breakdown charts (file system, network, CPU/memory, concurrency)
- Historical trend analysis with time-series charts
- Health score visualization and status indicators

#### 2. Master Server Monitoring
- Raft consensus status and cluster health
- CPU/memory usage by server instance
- Active connections and metadata operations
- Real-time logs and configuration management

#### 3. Chunk Server Analytics
- Storage utilization and distribution across servers
- Read/write operations performance metrics
- Erasure coding and replication status
- I/O performance timelines and health monitoring

#### 4. Client Performance
- Active mount points and connection status
- Cache hit rates and performance optimization
- Bandwidth usage and latency distribution
- Real-time activity monitoring and diagnostics

#### 5. Multi-Region Analysis
- Cross-region latency matrices with heat maps
- Regional performance comparisons
- Failover and consistency monitoring
- Geographic distribution of data and operations

#### 6. Comparative Benchmarks
- Performance comparison with other file systems
- Version-to-version performance analysis
- Benchmark result visualization and reporting
- Export capabilities for performance data

## 🚀 Quick Start

### Option 1: Using the Built-in Server (Recommended)

```bash
# Navigate to the frontend directory
cd /home/wings/projects/moosefs/src/frontend

# Start the development server
python3 server.py --port 8080 --host localhost

# Open your browser to:
# http://localhost:8080/index.html
```

### Option 2: Using Any Web Server

```bash
# Using Python's built-in server
cd /home/wings/projects/moosefs/src/frontend
python3 -m http.server 8080

# Using Node.js live-server (if available)
npx live-server --port=8080

# Using nginx or apache - serve the frontend directory
```

### Option 3: Integration with MooseNG

The dashboard can be integrated directly into the MooseNG build system by:

1. Adding the frontend files to the existing `mfsscripts/` directory
2. Modifying the CGI server to serve the new dashboard
3. Integrating with the existing MooseFS web interface

## 📁 Project Structure

```
src/frontend/
├── index.html                 # Main dashboard HTML
├── assets/
│   ├── css/
│   │   ├── dashboard.css      # Main dashboard styles
│   │   └── components.css     # Component-specific styles
│   └── js/
│       ├── dashboard-core.js  # Core dashboard functionality
│       ├── data-manager.js    # Data fetching and caching
│       ├── chart-components.js # Reusable chart components
│       └── main.js           # Application entry point
├── components/
│   ├── overview-section.js    # Overview dashboard component
│   ├── master-server-component.js  # Master server monitoring
│   ├── chunk-server-component.js   # Chunk server analytics
│   └── client-component.js    # Client performance monitoring
├── server.py                  # Development server with mock API
└── README.md                 # This file
```

## 🔧 Configuration

### Data Sources

The dashboard supports multiple data source modes:

1. **Mock Mode** (default): Uses generated mock data for development
2. **API Mode**: Connects to real MooseNG API endpoints
3. **File Mode**: Reads benchmark results from JSON files

Configure in `assets/js/data-manager.js`:

```javascript
// Enable/disable mock mode
window.DataManager.setMockMode(false);

// Set custom API endpoints
window.DataManager.baseUrl = 'http://your-mooseng-api:port';
```

### Theme Customization

Themes can be customized in `assets/css/dashboard.css`:

```css
:root {
    --primary-color: #2c3e50;
    --secondary-color: #3498db;
    /* ... other variables ... */
}

[data-theme="dark"] {
    --primary-color: #ecf0f1;
    /* ... dark theme overrides ... */
}
```

### Chart Configuration

Charts are configured in `assets/js/chart-components.js`:

```javascript
// Customize default chart options
getDefaultChartOptions() {
    return {
        responsive: true,
        // ... other options
    };
}
```

## 🔌 API Integration

### Real Data Integration

To connect to real MooseNG data sources:

1. **Update Data Manager**: Modify `data-manager.js` to point to your API endpoints
2. **Add Authentication**: Include API keys or authentication tokens
3. **Handle Real-time Updates**: Implement WebSocket connections for live data

Example API endpoint configuration:

```javascript
// In data-manager.js
this.apiEndpoints = {
    overview: '/api/v1/overview',
    masterServer: '/api/v1/master-server/stats',
    chunkServer: '/api/v1/chunk-servers/metrics',
    client: '/api/v1/clients/performance'
};
```

### Expected API Response Format

```json
{
    "timestamp": "2025-05-31T15:54:21+01:00",
    "data": {
        "throughput_mbps": "135.13",
        "ops_per_second": "1506.02",
        "total_time_ms": 3323,
        "performance_breakdown": {
            "file_system_ms": 196,
            "network_ms": 1782,
            "cpu_memory_ms": 937
        }
    }
}
```

## 🎨 Customization

### Adding New Components

1. Create a new component file in `components/`
2. Follow the existing component pattern:

```javascript
class NewComponent {
    constructor() {
        this.data = null;
        this.updateInterval = null;
    }

    async load() {
        // Load data and render
    }

    render() {
        // Generate HTML and create charts
    }

    destroy() {
        // Cleanup
    }
}

window.NewComponent = new NewComponent();
```

3. Add navigation link in `index.html`
4. Update `dashboard-core.js` to handle the new section

### Adding New Chart Types

1. Add chart creation method to `chart-components.js`
2. Use Chart.js or D3.js for visualization
3. Follow the existing pattern for chart management and cleanup

### Styling Customization

The dashboard uses CSS custom properties for easy theming:

- Modify `dashboard.css` for layout changes
- Update `components.css` for component-specific styles
- Use CSS media queries for responsive behavior

## 🚀 Deployment

### Production Deployment

1. **Static File Serving**: Serve files through nginx, apache, or CDN
2. **API Integration**: Connect to production MooseNG API endpoints
3. **Performance Optimization**: Minify CSS/JS, enable gzip compression
4. **Security**: Implement proper CORS policies and authentication

### Integration with MooseNG

The dashboard can be integrated with the existing MooseFS web interface:

1. Copy files to `mfsscripts/` directory
2. Update CGI scripts to serve the new interface
3. Modify existing charts and monitoring to use new components

## 🔍 Development

### Local Development

```bash
# Start development server with hot reload
python3 server.py --port 8080

# For development with real data
python3 server.py --port 8080 --api-mode
```

### Debugging

- Open browser developer tools for console logs
- Check Network tab for API request/response debugging
- Use the built-in error handling and notifications

### Adding Mock Data

Modify `data-manager.js` mock data generators for testing:

```javascript
generateMockOverviewData() {
    return {
        // Your mock data structure
    };
}
```

## 📈 Performance

The dashboard is optimized for performance:

- **Lazy Loading**: Components load data only when visible
- **Caching**: Data manager includes intelligent caching
- **Debounced Updates**: Chart updates are debounced to prevent excessive redraws
- **Memory Management**: Proper cleanup of charts and event listeners

## 🤝 Contributing

1. Follow the existing code structure and patterns
2. Add proper error handling and loading states
3. Include responsive design considerations
4. Test with both mock and real data sources
5. Update documentation for new features

## 📄 License

This dashboard is part of the MooseNG project and follows the same licensing terms.