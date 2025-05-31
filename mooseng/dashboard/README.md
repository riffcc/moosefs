# MooseNG Performance Dashboard

A modern, responsive web dashboard for monitoring and visualizing MooseNG distributed file system performance metrics and benchmarks.

## Features

### Core Visualization Framework
- **Responsive Design**: Mobile-first responsive layout that works on all devices
- **Modern UI Components**: Clean, professional interface with customizable themes
- **Real-time Updates**: Live data streaming via WebSocket connections
- **Interactive Charts**: Built with Chart.js and D3.js for rich visualizations
- **Modular Architecture**: Component-based structure for easy maintenance and extension

### Performance Monitoring
- **Master Server Metrics**: Raft consensus performance, metadata operations, cache efficiency
- **Chunk Server Metrics**: I/O throughput, erasure coding performance, storage utilization
- **Client Metrics**: FUSE latency, cache hit rates, concurrent connections
- **Multi-Region Analysis**: Cross-region latency, replication efficiency, consistency timing
- **Historical Trends**: Time-series data with configurable retention periods

### Dashboard Pages
1. **Overview**: System-wide performance summary and health status
2. **Master Server**: Detailed metrics for metadata operations and Raft consensus
3. **Chunk Server**: Storage performance, erasure coding, and data integrity metrics
4. **Client**: Client-side performance and network connectivity metrics
5. **Multi-Region**: Geographic distribution and cross-region performance analysis

## Technology Stack

- **Frontend**: Vanilla JavaScript (ES6+), HTML5, CSS3
- **Charts**: Chart.js for standard charts, D3.js for custom visualizations
- **Communication**: RESTful API with WebSocket for real-time updates
- **Backend Integration**: Connects to Rust-based MooseNG benchmarking system
- **Build Tools**: Simple HTTP server setup, no complex build pipeline required

## Quick Start

### Prerequisites
- Python 3.6+ (for development server)
- Modern web browser (Chrome, Firefox, Safari, Edge)
- MooseNG backend running (for live data)

### Development Setup

1. **Navigate to dashboard directory**:
   ```bash
   cd /path/to/moosefs/mooseng/dashboard
   ```

2. **Start development server**:
   ```bash
   npm run dev
   # or
   python3 -m http.server 8080 --directory public
   ```

3. **Open browser**:
   ```
   http://localhost:8080
   ```

### Production Deployment

1. **Static File Hosting**: Deploy the `public/` directory to any static web server
2. **API Configuration**: Update `src/ApiClient.js` with production API endpoints
3. **HTTPS Setup**: Ensure HTTPS is configured for WebSocket connections

## Project Structure

```
dashboard/
├── public/                 # Static files served to browser
│   └── index.html         # Main HTML page
├── src/                   # Core application logic
│   └── ApiClient.js       # Backend communication layer
├── components/            # Reusable UI components
│   ├── charts/           # Chart components
│   ├── layout/           # Layout management
│   └── common/           # Shared utilities
├── pages/                # Page-specific components
│   ├── overview/         # Overview dashboard
│   ├── master-server/    # Master server metrics
│   ├── chunk-server/     # Chunk server metrics
│   ├── client/           # Client metrics
│   └── multiregion/      # Multi-region analysis
├── assets/               # Static assets
│   ├── css/             # Stylesheets
│   ├── js/              # JavaScript libraries
│   └── images/          # Images and icons
└── README.md            # This file
```

## Configuration

### API Endpoints
The dashboard expects the following API endpoints from the MooseNG backend:

- `GET /api/benchmarks/overview` - System overview metrics
- `GET /api/benchmarks/master-server` - Master server performance
- `GET /api/benchmarks/chunk-server` - Chunk server performance  
- `GET /api/benchmarks/client` - Client performance
- `GET /api/benchmarks/multiregion` - Multi-region metrics
- `WebSocket /ws/metrics` - Real-time metric updates

### Environment Variables
- `MOOSENG_API_BASE_URL`: Base URL for the MooseNG API (default: `/api`)
- `MOOSENG_WS_URL`: WebSocket URL for real-time updates (default: `/ws/metrics`)

## Development

### Adding New Components

1. **Create component file** in appropriate directory:
   ```javascript
   class MyComponent {
       constructor(container, options) {
           this.container = container;
           this.options = options;
           this.init();
       }
       
       init() {
           // Component initialization
       }
       
       render() {
           // Render component UI
       }
       
       destroy() {
           // Cleanup resources
       }
   }
   ```

2. **Register with dashboard** in main application:
   ```javascript
   dashboard.registerPage('my-page', {
       title: 'My Page',
       component: MyComponent,
       order: 5
   });
   ```

### Adding New Charts

1. **Use ChartComponent** for standardized charts:
   ```javascript
   const chart = new ChartComponent('myChart');
   chart.createLineChart(data, options);
   ```

2. **Custom D3.js visualizations** for advanced charts:
   ```javascript
   const svg = d3.select('#myChart')
       .append('svg')
       .attr('width', width)
       .attr('height', height);
   ```

### Mock Data Development

For development without backend:
```javascript
const apiClient = new ApiClient();
apiClient.enableMockMode();
```

## Integration with MooseNG

### Data Flow
1. **MooseNG Benchmarks** → Generate performance metrics
2. **Rust Backend** → Serves metrics via REST API and WebSocket
3. **Dashboard** → Fetches and visualizes data in real-time

### Metric Types
- **Throughput**: Operations per second, data transfer rates
- **Latency**: Response times, consensus delays
- **Utilization**: CPU, memory, storage, network usage
- **Health**: Component status, error rates, uptime

## Browser Support

- Chrome 60+
- Firefox 55+
- Safari 12+
- Edge 79+

## Performance Considerations

- **Efficient Rendering**: Charts update incrementally for real-time data
- **Memory Management**: Automatic cleanup of old data points
- **Network Optimization**: Request caching and batch updates
- **Mobile Optimization**: Touch-friendly interface and reduced data usage

## Contributing

1. Follow existing code style and patterns
2. Add tests for new functionality
3. Update documentation for API changes
4. Ensure responsive design principles
5. Test across supported browsers

## License

This project is part of the MooseNG distributed file system and follows the same licensing terms.

## Support

For issues and questions:
- Create an issue in the MooseNG repository
- Check existing documentation
- Review API integration examples