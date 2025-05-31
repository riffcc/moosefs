/**
 * Dashboard Core Module
 * Provides core functionality for the MooseNG unified benchmark dashboard
 */

class DashboardCore {
    constructor() {
        this.socket = null;
        this.charts = new Map();
        this.currentPage = 'overview';
        this.refreshInterval = null;
        this.benchmarkData = new Map();
        this.liveMetrics = new Map();
        this.config = {
            refreshRate: 5000,
            maxDataPoints: 100,
            wsUrl: 'ws://localhost:8080/ws',
            apiUrl: '/api'
        };
        
        this.init();
    }

    async init() {
        this.setupEventListeners();
        this.setupWebSocket();
        await this.loadInitialData();
        this.startAutoRefresh();
    }

    setupEventListeners() {
        // Navigation
        document.querySelectorAll('.nav-link').forEach(link => {
            link.addEventListener('click', (e) => {
                e.preventDefault();
                const page = e.target.dataset.page;
                if (page) this.navigateTo(page);
            });
        });

        // Theme toggle
        const themeToggle = document.getElementById('themeToggle');
        if (themeToggle) {
            themeToggle.addEventListener('click', () => this.toggleTheme());
        }

        // Refresh button
        const refreshBtn = document.getElementById('refreshBtn');
        if (refreshBtn) {
            refreshBtn.addEventListener('click', () => this.refreshData());
        }

        // Live monitoring toggle
        const liveToggle = document.getElementById('liveToggle');
        if (liveToggle) {
            liveToggle.addEventListener('change', (e) => {
                this.toggleLiveMonitoring(e.target.checked);
            });
        }

        // Benchmark controls
        const runBenchmarkBtn = document.getElementById('runBenchmark');
        if (runBenchmarkBtn) {
            runBenchmarkBtn.addEventListener('click', () => this.runBenchmark());
        }

        const stopBenchmarkBtn = document.getElementById('stopBenchmark');
        if (stopBenchmarkBtn) {
            stopBenchmarkBtn.addEventListener('click', () => this.stopBenchmark());
        }
    }

    setupWebSocket() {
        try {
            this.socket = new WebSocket(this.config.wsUrl);
            
            this.socket.onopen = () => {
                console.log('WebSocket connected');
                this.updateConnectionStatus(true);
            };

            this.socket.onmessage = (event) => {
                this.handleWebSocketMessage(JSON.parse(event.data));
            };

            this.socket.onclose = () => {
                console.log('WebSocket disconnected');
                this.updateConnectionStatus(false);
                // Attempt reconnection after 5 seconds
                setTimeout(() => this.setupWebSocket(), 5000);
            };

            this.socket.onerror = (error) => {
                console.error('WebSocket error:', error);
                this.updateConnectionStatus(false);
            };
        } catch (error) {
            console.error('Failed to setup WebSocket:', error);
            this.updateConnectionStatus(false);
        }
    }

    handleWebSocketMessage(data) {
        switch (data.type) {
            case 'benchmark_update':
                this.handleBenchmarkUpdate(data.payload);
                break;
            case 'live_metrics':
                this.handleLiveMetrics(data.payload);
                break;
            case 'benchmark_complete':
                this.handleBenchmarkComplete(data.payload);
                break;
            case 'error':
                this.showNotification(data.message, 'error');
                break;
            default:
                console.log('Unknown message type:', data.type);
        }
    }

    async loadInitialData() {
        try {
            // Load benchmark results
            const results = await this.fetchData('/api/results');
            this.processBenchmarkResults(results);

            // Load system status
            const status = await this.fetchData('/api/status');
            this.updateSystemStatus(status);

            // Load available benchmarks
            const benchmarks = await this.fetchData('/api/benchmarks');
            this.updateBenchmarkList(benchmarks);

            this.updateUI();
        } catch (error) {
            console.error('Failed to load initial data:', error);
            this.showNotification('Failed to load initial data', 'error');
        }
    }

    async fetchData(endpoint) {
        const response = await fetch(this.config.apiUrl + endpoint);
        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }
        return await response.json();
    }

    navigateTo(page) {
        // Hide all pages
        document.querySelectorAll('.page').forEach(p => {
            p.style.display = 'none';
        });

        // Show target page
        const targetPage = document.getElementById(`${page}Page`);
        if (targetPage) {
            targetPage.style.display = 'block';
            this.currentPage = page;

            // Update navigation
            document.querySelectorAll('.nav-link').forEach(link => {
                link.classList.remove('active');
            });
            
            const activeLink = document.querySelector(`[data-page="${page}"]`);
            if (activeLink) {
                activeLink.classList.add('active');
            }

            // Load page-specific data
            this.loadPageData(page);
        }
    }

    async loadPageData(page) {
        switch (page) {
            case 'overview':
                await this.loadOverviewData();
                break;
            case 'results':
                await this.loadResultsData();
                break;
            case 'monitoring':
                await this.loadMonitoringData();
                break;
            case 'analysis':
                await this.loadAnalysisData();
                break;
        }
    }

    async loadOverviewData() {
        const summaryData = await this.fetchData('/api/summary');
        this.updateOverviewCards(summaryData);
        this.updateOverviewCharts(summaryData);
    }

    async loadResultsData() {
        const results = await this.fetchData('/api/results/detailed');
        this.updateResultsTable(results);
        this.updateResultsCharts(results);
    }

    async loadMonitoringData() {
        const metrics = await this.fetchData('/api/metrics/current');
        this.updateLiveMetrics(metrics);
        this.initializeMonitoringCharts();
    }

    async loadAnalysisData() {
        const analysis = await this.fetchData('/api/analysis');
        this.updateAnalysisData(analysis);
    }

    updateOverviewCards(data) {
        this.updateCard('totalBenchmarks', data.total_benchmarks || 0);
        this.updateCard('avgThroughput', this.formatThroughput(data.avg_throughput || 0));
        this.updateCard('avgLatency', this.formatLatency(data.avg_latency || 0));
        this.updateCard('successRate', this.formatPercentage(data.success_rate || 0));
    }

    updateCard(cardId, value) {
        const element = document.getElementById(cardId);
        if (element) {
            element.textContent = value;
        }
    }

    updateSystemStatus(status) {
        const statusElement = document.getElementById('systemStatus');
        if (statusElement) {
            statusElement.className = `badge ${status.healthy ? 'bg-success' : 'bg-danger'}`;
            statusElement.textContent = status.healthy ? 'Healthy' : 'Issues Detected';
        }

        const servicesElement = document.getElementById('activeServices');
        if (servicesElement) {
            servicesElement.textContent = `${status.active_services || 0}/${status.total_services || 0}`;
        }
    }

    updateConnectionStatus(connected) {
        const statusElement = document.getElementById('connectionStatus');
        if (statusElement) {
            statusElement.className = `badge ${connected ? 'bg-success' : 'bg-danger'}`;
            statusElement.textContent = connected ? 'Connected' : 'Disconnected';
        }
    }

    processBenchmarkResults(results) {
        if (Array.isArray(results)) {
            results.forEach(result => {
                const category = result.category || 'uncategorized';
                if (!this.benchmarkData.has(category)) {
                    this.benchmarkData.set(category, []);
                }
                this.benchmarkData.get(category).push(result);
            });
        }
    }

    handleBenchmarkUpdate(data) {
        this.showNotification(`Benchmark ${data.name} status: ${data.status}`, 'info');
        if (this.currentPage === 'monitoring') {
            this.updateBenchmarkProgress(data);
        }
    }

    handleLiveMetrics(metrics) {
        this.liveMetrics.set(Date.now(), metrics);
        
        // Keep only recent data points
        const cutoff = Date.now() - (this.config.maxDataPoints * this.config.refreshRate);
        for (const [timestamp] of this.liveMetrics) {
            if (timestamp < cutoff) {
                this.liveMetrics.delete(timestamp);
            }
        }

        if (this.currentPage === 'monitoring') {
            this.updateLiveCharts(metrics);
        }
    }

    handleBenchmarkComplete(data) {
        this.showNotification(`Benchmark ${data.name} completed`, 'success');
        this.refreshData();
    }

    async runBenchmark() {
        const selectedBenchmarks = this.getSelectedBenchmarks();
        if (selectedBenchmarks.length === 0) {
            this.showNotification('Please select at least one benchmark', 'warning');
            return;
        }

        try {
            const response = await fetch(`${this.config.apiUrl}/benchmarks/run`, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({ benchmarks: selectedBenchmarks })
            });

            if (response.ok) {
                this.showNotification('Benchmarks started', 'success');
            } else {
                throw new Error(`Failed to start benchmarks: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error running benchmarks:', error);
            this.showNotification('Failed to start benchmarks', 'error');
        }
    }

    async stopBenchmark() {
        try {
            const response = await fetch(`${this.config.apiUrl}/benchmarks/stop`, {
                method: 'POST'
            });

            if (response.ok) {
                this.showNotification('Benchmarks stopped', 'info');
            } else {
                throw new Error(`Failed to stop benchmarks: ${response.statusText}`);
            }
        } catch (error) {
            console.error('Error stopping benchmarks:', error);
            this.showNotification('Failed to stop benchmarks', 'error');
        }
    }

    getSelectedBenchmarks() {
        const checkboxes = document.querySelectorAll('input[name="benchmark"]:checked');
        return Array.from(checkboxes).map(cb => cb.value);
    }

    toggleTheme() {
        const body = document.body;
        body.classList.toggle('dark-theme');
        
        const isDark = body.classList.contains('dark-theme');
        localStorage.setItem('theme', isDark ? 'dark' : 'light');

        // Update chart themes
        this.updateChartThemes(isDark);
    }

    updateChartThemes(isDark) {
        const theme = isDark ? 'dark' : 'light';
        this.charts.forEach((chart, id) => {
            if (chart && typeof chart.updateTheme === 'function') {
                chart.updateTheme(theme);
            }
        });
    }

    toggleLiveMonitoring(enabled) {
        if (enabled) {
            this.startLiveMonitoring();
        } else {
            this.stopLiveMonitoring();
        }
    }

    startLiveMonitoring() {
        if (this.socket && this.socket.readyState === WebSocket.OPEN) {
            this.socket.send(JSON.stringify({ type: 'start_monitoring' }));
        }
    }

    stopLiveMonitoring() {
        if (this.socket && this.socket.readyState === WebSocket.OPEN) {
            this.socket.send(JSON.stringify({ type: 'stop_monitoring' }));
        }
    }

    startAutoRefresh() {
        this.refreshInterval = setInterval(() => {
            this.refreshData();
        }, this.config.refreshRate);
    }

    stopAutoRefresh() {
        if (this.refreshInterval) {
            clearInterval(this.refreshInterval);
            this.refreshInterval = null;
        }
    }

    async refreshData() {
        try {
            await this.loadPageData(this.currentPage);
            this.showNotification('Data refreshed', 'success', 2000);
        } catch (error) {
            console.error('Error refreshing data:', error);
            this.showNotification('Failed to refresh data', 'error');
        }
    }

    showNotification(message, type = 'info', duration = 5000) {
        const container = document.getElementById('notificationContainer');
        if (!container) return;

        const notification = document.createElement('div');
        notification.className = `alert alert-${this.getBootstrapAlertClass(type)} alert-dismissible fade show`;
        notification.innerHTML = `
            ${message}
            <button type="button" class="btn-close" data-bs-dismiss="alert"></button>
        `;

        container.appendChild(notification);

        if (duration > 0) {
            setTimeout(() => {
                if (notification.parentNode) {
                    notification.remove();
                }
            }, duration);
        }
    }

    getBootstrapAlertClass(type) {
        const typeMap = {
            success: 'success',
            error: 'danger',
            warning: 'warning',
            info: 'info'
        };
        return typeMap[type] || 'info';
    }

    updateUI() {
        // Apply saved theme
        const savedTheme = localStorage.getItem('theme');
        if (savedTheme === 'dark') {
            document.body.classList.add('dark-theme');
        }

        // Navigate to default page
        this.navigateTo('overview');
    }

    formatThroughput(value) {
        if (value >= 1e9) return `${(value / 1e9).toFixed(2)} GB/s`;
        if (value >= 1e6) return `${(value / 1e6).toFixed(2)} MB/s`;
        if (value >= 1e3) return `${(value / 1e3).toFixed(2)} KB/s`;
        return `${value.toFixed(2)} B/s`;
    }

    formatLatency(value) {
        if (value >= 1000) return `${(value / 1000).toFixed(2)} s`;
        return `${value.toFixed(2)} ms`;
    }

    formatPercentage(value) {
        return `${(value * 100).toFixed(1)}%`;
    }

    destroy() {
        this.stopAutoRefresh();
        if (this.socket) {
            this.socket.close();
        }
        this.charts.clear();
    }
}

// Chart integration helper
class ChartManager {
    constructor(dashboardCore) {
        this.core = dashboardCore;
        this.defaultOptions = {
            responsive: true,
            maintainAspectRatio: false,
            plugins: {
                legend: {
                    position: 'top',
                },
                tooltip: {
                    mode: 'index',
                    intersect: false,
                }
            },
            scales: {
                x: {
                    display: true,
                    title: {
                        display: true
                    }
                },
                y: {
                    display: true,
                    title: {
                        display: true
                    }
                }
            }
        };
    }

    createChart(canvasId, type, data, options = {}) {
        const canvas = document.getElementById(canvasId);
        if (!canvas) {
            console.error(`Canvas with id '${canvasId}' not found`);
            return null;
        }

        const ctx = canvas.getContext('2d');
        const mergedOptions = this.mergeOptions(this.defaultOptions, options);

        const chart = new Chart(ctx, {
            type: type,
            data: data,
            options: mergedOptions
        });

        this.core.charts.set(canvasId, chart);
        return chart;
    }

    updateChart(canvasId, newData) {
        const chart = this.core.charts.get(canvasId);
        if (chart) {
            chart.data = newData;
            chart.update();
        }
    }

    mergeOptions(defaults, custom) {
        return { ...defaults, ...custom };
    }
}

// Initialize dashboard when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    window.dashboard = new DashboardCore();
    window.chartManager = new ChartManager(window.dashboard);
});

export { DashboardCore, ChartManager };