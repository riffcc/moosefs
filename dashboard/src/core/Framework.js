/**
 * MooseNG Dashboard Framework - Core Framework
 * 
 * The main framework class that orchestrates all dashboard components,
 * manages state, handles data flow, and provides a unified API for
 * building advanced visualization dashboards.
 * 
 * @version 1.0.0
 * @author MooseNG Team
 */

class MooseNGFramework {
    constructor() {
        this.version = '1.0.0';
        this.components = new Map();
        this.dataConnectors = new Map();
        this.layoutEngine = null;
        this.eventSystem = null;
        this.config = {
            theme: 'light',
            refreshInterval: 30000,
            dataRetention: '24h',
            apiEndpoint: '/api/mooseng',
            wsEndpoint: '/ws/mooseng',
            debug: false,
            performance: {
                maxDataPoints: 1000,
                chartAnimationDuration: 300,
                debounceDelay: 250
            }
        };
        this.state = {
            initialized: false,
            connected: false,
            loading: false,
            error: null,
            activeComponents: new Set(),
            dataCache: new Map(),
            metrics: {
                componentsLoaded: 0,
                dataRequests: 0,
                renderTime: 0,
                lastUpdate: null
            }
        };
        
        this.init();
    }

    /**
     * Initialize the framework
     */
    async init() {
        console.log(`🚀 Initializing MooseNG Dashboard Framework v${this.version}`);
        
        try {
            // Initialize core systems
            await this.initializeCoreSystems();
            
            // Set up data connections
            await this.initializeDataConnections();
            
            // Initialize layout engine
            await this.initializeLayoutEngine();
            
            // Load components
            await this.loadComponents();
            
            // Set up event listeners
            this.setupEventListeners();
            
            // Start the framework
            await this.start();
            
            console.log('✅ MooseNG Dashboard Framework initialized successfully');
            
        } catch (error) {
            console.error('❌ Framework initialization failed:', error);
            this.handleError(error);
        }
    }

    /**
     * Initialize core systems
     */
    async initializeCoreSystems() {
        // Initialize event system
        if (window.EventSystem) {
            this.eventSystem = new EventSystem();
        }
        
        // Initialize component registry
        if (window.ComponentRegistry) {
            this.componentRegistry = new ComponentRegistry();
        }
        
        // Load configuration from localStorage or API
        await this.loadConfiguration();
        
        // Apply theme
        this.applyTheme(this.config.theme);
        
        // Initialize performance monitoring
        this.initializePerformanceMonitoring();
    }

    /**
     * Initialize data connections
     */
    async initializeDataConnections() {
        // Initialize main data connector
        if (window.DataConnector) {
            const mainConnector = new DataConnector({
                apiEndpoint: this.config.apiEndpoint,
                wsEndpoint: this.config.wsEndpoint,
                refreshInterval: this.config.refreshInterval
            });
            
            this.dataConnectors.set('main', mainConnector);
            
            // Set up data event handlers
            mainConnector.on('connected', () => {
                this.state.connected = true;
                this.updateConnectionStatus('connected');
                this.emit('connectionStateChanged', true);
            });
            
            mainConnector.on('disconnected', () => {
                this.state.connected = false;
                this.updateConnectionStatus('disconnected');
                this.emit('connectionStateChanged', false);
            });
            
            mainConnector.on('data', (data) => {
                this.handleDataUpdate(data);
            });
            
            mainConnector.on('error', (error) => {
                this.handleError(error);
            });
        }
        
        // Initialize cache manager
        if (window.CacheManager) {
            this.cacheManager = new CacheManager({
                maxSize: 1000,
                ttl: 300000, // 5 minutes
                retention: this.config.dataRetention
            });
        }
        
        // Initialize realtime client
        if (window.RealtimeClient) {
            this.realtimeClient = new RealtimeClient({
                endpoint: this.config.wsEndpoint,
                reconnectInterval: 5000,
                maxReconnectAttempts: 10
            });
        }
    }

    /**
     * Initialize layout engine
     */
    async initializeLayoutEngine() {
        if (window.LayoutEngine) {
            this.layoutEngine = new LayoutEngine({
                containerSelector: '#dashboardContent',
                responsive: true,
                breakpoints: {
                    xs: 480,
                    sm: 768,
                    md: 1024,
                    lg: 1200,
                    xl: 1400
                },
                gridSystem: {
                    columns: 12,
                    gutter: 16,
                    containerPadding: 24
                }
            });
            
            await this.layoutEngine.initialize();
        }
    }

    /**
     * Load and initialize components
     */
    async loadComponents() {
        const componentTypes = [
            'MetricCard',
            'TimeSeriesChart', 
            'PerformanceChart',
            'TopologyChart',
            'StatusPanel',
            'AlertWidget'
        ];
        
        for (const componentType of componentTypes) {
            if (window[componentType]) {
                this.componentRegistry.register(componentType, window[componentType]);
                console.log(`📦 Registered component: ${componentType}`);
            }
        }
        
        // Create initial demo components
        await this.createDemoComponents();
    }

    /**
     * Create demo components to showcase the framework
     */
    async createDemoComponents() {
        // Create metric cards
        const metricsContainer = document.getElementById('metricsGrid');
        if (metricsContainer && window.MetricCard) {
            const metrics = [
                { label: 'Throughput', value: '1.2K ops/s', trend: 'up', color: 'blue' },
                { label: 'Latency', value: '45ms', trend: 'down', color: 'green' },
                { label: 'Active Nodes', value: '12', trend: 'stable', color: 'orange' },
                { label: 'Storage Used', value: '78%', trend: 'up', color: 'red' },
                { label: 'Network I/O', value: '856 MB/s', trend: 'up', color: 'purple' },
                { label: 'CPU Usage', value: '23%', trend: 'stable', color: 'teal' }
            ];
            
            for (const metric of metrics) {
                const card = new MetricCard(metric);
                metricsContainer.appendChild(card.render());
                this.components.set(`metric-${metric.label}`, card);
            }
        }
        
        // Create performance chart
        if (window.PerformanceChart) {
            const perfChart = new PerformanceChart({
                container: '#performanceChart',
                title: 'Throughput vs Latency',
                datasets: [{
                    label: 'Throughput (ops/s)',
                    data: this.generateMockTimeSeriesData(50),
                    borderColor: 'rgb(75, 192, 192)',
                    tension: 0.1
                }, {
                    label: 'Latency (ms)', 
                    data: this.generateMockTimeSeriesData(50, 20, 80),
                    borderColor: 'rgb(255, 99, 132)',
                    tension: 0.1,
                    yAxisID: 'y1'
                }],
                options: {
                    scales: {
                        y: {
                            type: 'linear',
                            display: true,
                            position: 'left',
                        },
                        y1: {
                            type: 'linear', 
                            display: true,
                            position: 'right',
                            grid: {
                                drawOnChartArea: false,
                            },
                        }
                    }
                }
            });
            
            this.components.set('performance-chart', perfChart);
        }
        
        // Create health distribution chart
        if (window.Chart) {
            const healthCtx = document.getElementById('healthChart');
            if (healthCtx) {
                const healthChart = new Chart(healthCtx, {
                    type: 'doughnut',
                    data: {
                        labels: ['Healthy', 'Warning', 'Critical', 'Offline'],
                        datasets: [{
                            data: [85, 10, 3, 2],
                            backgroundColor: [
                                'rgb(34, 197, 94)',   // green
                                'rgb(234, 179, 8)',   // yellow
                                'rgb(239, 68, 68)',   // red  
                                'rgb(156, 163, 175)'  // gray
                            ],
                            borderWidth: 2,
                            borderColor: '#ffffff'
                        }]
                    },
                    options: {
                        responsive: true,
                        maintainAspectRatio: false,
                        plugins: {
                            legend: {
                                position: 'bottom'
                            }
                        }
                    }
                });
                
                this.components.set('health-chart', healthChart);
            }
        }
        
        // Create topology visualization
        if (window.TopologyChart) {
            const topologyChart = new TopologyChart({
                container: '#topologyChart',
                data: {
                    nodes: [
                        { id: 'master-1', type: 'master', region: 'us-east-1', x: 100, y: 100 },
                        { id: 'master-2', type: 'master', region: 'eu-west-1', x: 300, y: 100 },
                        { id: 'master-3', type: 'master', region: 'ap-south-1', x: 500, y: 100 },
                        { id: 'chunk-1', type: 'chunk', region: 'us-east-1', x: 50, y: 200 },
                        { id: 'chunk-2', type: 'chunk', region: 'us-east-1', x: 150, y: 200 },
                        { id: 'chunk-3', type: 'chunk', region: 'eu-west-1', x: 250, y: 200 },
                        { id: 'chunk-4', type: 'chunk', region: 'eu-west-1', x: 350, y: 200 },
                        { id: 'chunk-5', type: 'chunk', region: 'ap-south-1', x: 450, y: 200 },
                        { id: 'chunk-6', type: 'chunk', region: 'ap-south-1', x: 550, y: 200 }
                    ],
                    links: [
                        { source: 'master-1', target: 'chunk-1', latency: 2 },
                        { source: 'master-1', target: 'chunk-2', latency: 3 },
                        { source: 'master-2', target: 'chunk-3', latency: 2 },
                        { source: 'master-2', target: 'chunk-4', latency: 4 },
                        { source: 'master-3', target: 'chunk-5', latency: 3 },
                        { source: 'master-3', target: 'chunk-6', latency: 2 },
                        { source: 'master-1', target: 'master-2', latency: 45 },
                        { source: 'master-2', target: 'master-3', latency: 180 },
                        { source: 'master-1', target: 'master-3', latency: 220 }
                    ]
                }
            });
            
            this.components.set('topology-chart', topologyChart);
        }
        
        // Create status panels
        const statusContainer = document.getElementById('statusGrid');
        if (statusContainer && window.StatusPanel) {
            const statusPanels = [
                { title: 'Master Servers', status: 'healthy', count: '3/3', details: 'All masters operational' },
                { title: 'Chunk Servers', status: 'warning', count: '47/50', details: '3 servers in maintenance' },
                { title: 'Client Connections', status: 'healthy', count: '1,247', details: 'Active connections' },
                { title: 'Storage Capacity', status: 'healthy', count: '2.4TB', details: 'Available space' },
                { title: 'Replication Health', status: 'healthy', count: '99.9%', details: 'Data integrity OK' },
                { title: 'Network Performance', status: 'warning', count: '856 MB/s', details: 'High utilization detected' }
            ];
            
            for (const panelData of statusPanels) {
                const panel = new StatusPanel(panelData);
                statusContainer.appendChild(panel.render());
                this.components.set(`status-${panelData.title}`, panel);
            }
        }
    }

    /**
     * Set up event listeners for UI interactions
     */
    setupEventListeners() {
        // Refresh button
        const refreshBtn = document.getElementById('refreshBtn');
        if (refreshBtn) {
            refreshBtn.addEventListener('click', () => this.refreshData());
        }
        
        // Settings button
        const settingsBtn = document.getElementById('settingsBtn');
        if (settingsBtn) {
            settingsBtn.addEventListener('click', () => this.openSettings());
        }
        
        // Theme selector
        const themeSelector = document.getElementById('themeSelector');
        if (themeSelector) {
            themeSelector.addEventListener('change', (e) => {
                this.applyTheme(e.target.value);
            });
        }
        
        // Refresh interval selector
        const refreshInterval = document.getElementById('refreshInterval');
        if (refreshInterval) {
            refreshInterval.addEventListener('change', (e) => {
                this.config.refreshInterval = parseInt(e.target.value);
                this.saveConfiguration();
            });
        }
        
        // Window resize handling
        window.addEventListener('resize', this.debounce(() => {
            this.handleResize();
        }, this.config.performance.debounceDelay));
        
        // Keyboard shortcuts
        document.addEventListener('keydown', (e) => {
            if (e.ctrlKey || e.metaKey) {
                switch (e.key) {
                    case 'r':
                        e.preventDefault();
                        this.refreshData();
                        break;
                    case ',':
                        e.preventDefault();
                        this.openSettings();
                        break;
                }
            }
        });
    }

    /**
     * Start the framework
     */
    async start() {
        this.state.initialized = true;
        this.hideLoading();
        
        // Connect to data sources
        if (this.dataConnectors.has('main')) {
            await this.dataConnectors.get('main').connect();
        }
        
        // Start periodic updates
        this.startPeriodicUpdates();
        
        // Emit ready event
        this.emit('ready', {
            version: this.version,
            componentsLoaded: this.components.size,
            timestamp: Date.now()
        });
        
        // Show welcome toast
        this.showToast('MooseNG Dashboard Framework loaded successfully', 'success');
    }

    /**
     * Generate mock time series data for demo purposes
     */
    generateMockTimeSeriesData(points = 50, min = 0, max = 100) {
        const data = [];
        const now = Date.now();
        
        for (let i = 0; i < points; i++) {
            const timestamp = now - (points - i) * 60000; // 1 minute intervals
            const value = min + Math.random() * (max - min);
            data.push({
                x: timestamp,
                y: Math.round(value * 100) / 100
            });
        }
        
        return data;
    }

    /**
     * Handle data updates from connectors
     */
    handleDataUpdate(data) {
        if (this.cacheManager) {
            this.cacheManager.set(data.type, data.payload, data.timestamp);
        }
        
        // Update relevant components
        this.components.forEach((component, id) => {
            if (component.shouldUpdate && component.shouldUpdate(data)) {
                component.update(data);
            }
        });
        
        this.state.metrics.dataRequests++;
        this.state.metrics.lastUpdate = Date.now();
        
        this.emit('dataUpdated', data);
    }

    /**
     * Refresh all data
     */
    async refreshData() {
        this.showLoading();
        
        try {
            // Refresh data from all connectors
            for (const [name, connector] of this.dataConnectors) {
                await connector.refresh();
            }
            
            // Update all components
            this.components.forEach(component => {
                if (component.refresh) {
                    component.refresh();
                }
            });
            
            this.showToast('Data refreshed successfully', 'success');
            
        } catch (error) {
            this.handleError(error);
            this.showToast('Failed to refresh data', 'error');
        } finally {
            this.hideLoading();
        }
    }

    /**
     * Apply theme
     */
    applyTheme(theme) {
        const body = document.body;
        body.classList.remove('theme-light', 'theme-dark');
        
        if (theme === 'auto') {
            const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
            theme = prefersDark ? 'dark' : 'light';
        }
        
        body.classList.add(`theme-${theme}`);
        this.config.theme = theme;
        this.saveConfiguration();
    }

    /**
     * Handle window resize
     */
    handleResize() {
        if (this.layoutEngine) {
            this.layoutEngine.handleResize();
        }
        
        this.components.forEach(component => {
            if (component.resize) {
                component.resize();
            }
        });
    }

    /**
     * Start periodic updates
     */
    startPeriodicUpdates() {
        if (this.updateInterval) {
            clearInterval(this.updateInterval);
        }
        
        this.updateInterval = setInterval(() => {
            if (this.state.connected && !this.state.loading) {
                this.refreshData();
            }
        }, this.config.refreshInterval);
    }

    /**
     * Show/hide loading overlay
     */
    showLoading() {
        const overlay = document.getElementById('loadingOverlay');
        if (overlay) {
            overlay.classList.add('active');
        }
        this.state.loading = true;
    }

    hideLoading() {
        const overlay = document.getElementById('loadingOverlay');
        if (overlay) {
            overlay.classList.remove('active');
        }
        this.state.loading = false;
    }

    /**
     * Update connection status display
     */
    updateConnectionStatus(status) {
        const statusElement = document.getElementById('connectionStatus');
        if (statusElement) {
            const indicator = statusElement.querySelector('.status-indicator');
            const text = statusElement.querySelector('.status-text');
            
            statusElement.className = `connection-status ${status}`;
            
            switch (status) {
                case 'connected':
                    text.textContent = 'Connected';
                    break;
                case 'connecting':
                    text.textContent = 'Connecting...';
                    break;
                case 'disconnected':
                    text.textContent = 'Disconnected';
                    break;
                case 'error':
                    text.textContent = 'Connection Error';
                    break;
            }
        }
    }

    /**
     * Show toast notification
     */
    showToast(message, type = 'info', duration = 3000) {
        const toastContainer = document.getElementById('toastContainer');
        if (!toastContainer) return;
        
        const toast = document.createElement('div');
        toast.className = `toast toast-${type}`;
        toast.innerHTML = `
            <div class="toast-content">
                <span class="toast-message">${message}</span>
                <button class="toast-close" onclick="this.parentElement.parentElement.remove()">×</button>
            </div>
        `;
        
        toastContainer.appendChild(toast);
        
        // Auto remove after duration
        setTimeout(() => {
            if (toast.parentElement) {
                toast.remove();
            }
        }, duration);
    }

    /**
     * Open settings panel
     */
    openSettings() {
        const settingsPanel = document.getElementById('settingsPanel');
        if (settingsPanel) {
            settingsPanel.classList.add('active');
        }
    }

    /**
     * Close settings panel
     */
    closeSettings() {
        const settingsPanel = document.getElementById('settingsPanel');
        if (settingsPanel) {
            settingsPanel.classList.remove('active');
        }
    }

    /**
     * Handle errors
     */
    handleError(error) {
        console.error('Framework Error:', error);
        this.state.error = error;
        this.emit('error', error);
        
        // Show error to user
        this.showToast(`Error: ${error.message}`, 'error', 5000);
    }

    /**
     * Load configuration
     */
    async loadConfiguration() {
        try {
            const saved = localStorage.getItem('mooseng-dashboard-config');
            if (saved) {
                const config = JSON.parse(saved);
                Object.assign(this.config, config);
            }
        } catch (error) {
            console.warn('Failed to load configuration:', error);
        }
    }

    /**
     * Save configuration
     */
    saveConfiguration() {
        try {
            localStorage.setItem('mooseng-dashboard-config', JSON.stringify(this.config));
        } catch (error) {
            console.warn('Failed to save configuration:', error);
        }
    }

    /**
     * Initialize performance monitoring
     */
    initializePerformanceMonitoring() {
        if (window.performance && window.performance.mark) {
            window.performance.mark('framework-init-start');
        }
    }

    /**
     * Export dashboard data
     */
    exportData() {
        const exportData = {
            timestamp: Date.now(),
            version: this.version,
            config: this.config,
            metrics: this.state.metrics,
            components: Array.from(this.components.keys()),
            data: Array.from(this.state.dataCache.entries())
        };
        
        const blob = new Blob([JSON.stringify(exportData, null, 2)], {
            type: 'application/json'
        });
        
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `mooseng-dashboard-export-${new Date().toISOString().split('T')[0]}.json`;
        a.click();
        
        URL.revokeObjectURL(url);
    }

    /**
     * Event system methods
     */
    emit(event, data) {
        if (this.eventSystem) {
            this.eventSystem.emit(event, data);
        }
    }

    on(event, callback) {
        if (this.eventSystem) {
            this.eventSystem.on(event, callback);
        }
    }

    off(event, callback) {
        if (this.eventSystem) {
            this.eventSystem.off(event, callback);
        }
    }

    /**
     * Utility: Debounce function calls
     */
    debounce(func, wait) {
        let timeout;
        return function executedFunction(...args) {
            const later = () => {
                clearTimeout(timeout);
                func(...args);
            };
            clearTimeout(timeout);
            timeout = setTimeout(later, wait);
        };
    }

    /**
     * Get framework information
     */
    getInfo() {
        return {
            version: this.version,
            initialized: this.state.initialized,
            connected: this.state.connected,
            componentsLoaded: this.components.size,
            dataConnectors: this.dataConnectors.size,
            config: { ...this.config },
            metrics: { ...this.state.metrics }
        };
    }
}

// Initialize framework when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    window.Framework = new MooseNGFramework();
});

// Export for module systems
if (typeof module !== 'undefined' && module.exports) {
    module.exports = MooseNGFramework;
}