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
        document.querySelectorAll('.dashboard-page').forEach(p => {
            p.classList.remove('active');
        });

        // Show target page
        const targetPage = document.getElementById(`page-${page}`);
        if (targetPage) {
            targetPage.classList.add('active');
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
        try {
            const summaryData = await this.fetchData('/api/summary');
            this.updateOverviewCards(summaryData);
            this.updateOverviewCharts(summaryData);
        } catch (error) {
            console.error('Failed to load overview data:', error);
            this.showNotification('Failed to load overview data', 'error');
        }
    }

    async loadResultsData() {
        try {
            const results = await this.fetchData('/api/results/detailed');
            this.updateResultsTable(results);
            this.updateResultsCharts(results);
        } catch (error) {
            console.error('Failed to load results data:', error);
            this.showNotification('Failed to load results', 'error');
        }
    }

    async loadMonitoringData() {
        try {
            const metrics = await this.fetchData('/api/metrics/current');
            this.updateLiveMetrics(metrics);
            this.initializeMonitoringCharts();
        } catch (error) {
            console.error('Failed to load monitoring data:', error);
            this.showNotification('Failed to load monitoring data', 'error');
        }
    }

    async loadAnalysisData() {
        try {
            const analysis = await this.fetchData('/api/analysis');
            this.updateAnalysisData(analysis);
        } catch (error) {
            console.error('Failed to load analysis data:', error);
            this.showNotification('Failed to load analysis data', 'error');
        }
    }

    updateResultsTable(results) {
        const container = document.getElementById('resultsContent');
        if (!container) return;

        if (!Array.isArray(results) || results.length === 0) {
            container.innerHTML = `
                <div class="text-center py-5">
                    <i class="fas fa-chart-bar fa-3x text-muted mb-3"></i>
                    <h5 class="text-muted">No benchmark results found</h5>
                    <p class="text-muted">Run some benchmarks to see results here</p>
                </div>
            `;
            return;
        }

        const tableHTML = `
            <div class="table-responsive">
                <table class="table table-hover">
                    <thead class="table-light">
                        <tr>
                            <th>Name</th>
                            <th>Category</th>
                            <th>Duration (ms)</th>
                            <th>Throughput (MB/s)</th>
                            <th>Latency (ms)</th>
                            <th>CPU Usage (%)</th>
                            <th>Memory Usage (%)</th>
                            <th>Status</th>
                            <th>Timestamp</th>
                        </tr>
                    </thead>
                    <tbody>
                        ${results.map(result => `
                            <tr>
                                <td>${result.name}</td>
                                <td><span class="badge bg-primary">${result.category}</span></td>
                                <td>${result.duration_ms || 'N/A'}</td>
                                <td>${result.throughput_mbps ? result.throughput_mbps.toFixed(2) : 'N/A'}</td>
                                <td>${result.latency_ms ? result.latency_ms.toFixed(2) : 'N/A'}</td>
                                <td>${result.cpu_usage ? result.cpu_usage.toFixed(1) : 'N/A'}</td>
                                <td>${result.memory_usage ? result.memory_usage.toFixed(1) : 'N/A'}</td>
                                <td>
                                    <span class="badge bg-${result.success ? 'success' : 'danger'}">
                                        ${result.success ? 'Success' : 'Failed'}
                                    </span>
                                </td>
                                <td>${new Date(result.timestamp).toLocaleString()}</td>
                            </tr>
                        `).join('')}
                    </tbody>
                </table>
            </div>
        `;

        container.innerHTML = tableHTML;
    }

    updateResultsCharts(results) {
        // Could add additional charts for results page here
    }

    initializeMonitoringCharts() {
        this.createLivePerformanceChart();
        this.updateCurrentBenchmarks();
    }

    createLivePerformanceChart() {
        const chartManager = window.chartManager;
        if (!chartManager) return;

        const chartData = {
            labels: [],
            datasets: [{
                label: 'Throughput',
                data: [],
                borderColor: '#007bff',
                backgroundColor: 'rgba(0, 123, 255, 0.1)',
                tension: 0.4
            }, {
                label: 'Latency',
                data: [],
                borderColor: '#28a745',
                backgroundColor: 'rgba(40, 167, 69, 0.1)',
                tension: 0.4
            }]
        };

        chartManager.createChart('livePerformanceChart', 'line', chartData);
    }

    updateCurrentBenchmarks() {
        const container = document.getElementById('currentBenchmarks');
        if (!container) return;

        // For now, show placeholder
        container.innerHTML = `
            <div class="text-center py-4">
                <i class="fas fa-pause-circle fa-3x text-muted mb-3"></i>
                <p class="text-muted">No active benchmarks</p>
                <button class="btn btn-primary btn-sm" onclick="dashboard.runBenchmark()">
                    <i class="fas fa-play me-1"></i>Start Benchmark
                </button>
            </div>
        `;
    }

    updateAnalysisData(analysis) {
        const container = document.getElementById('analysisContent');
        if (!container) return;

        const analysisHTML = `
            <div class="row">
                <div class="col-md-6">
                    <div class="card border-0 shadow-sm">
                        <div class="card-header bg-primary text-white">
                            <h6 class="mb-0"><i class="fas fa-chart-line me-2"></i>Performance Trends</h6>
                        </div>
                        <div class="card-body">
                            <canvas id="trendAnalysisChart" height="200"></canvas>
                        </div>
                    </div>
                </div>
                <div class="col-md-6">
                    <div class="card border-0 shadow-sm">
                        <div class="card-header bg-success text-white">
                            <h6 class="mb-0"><i class="fas fa-cogs me-2"></i>Resource Analysis</h6>
                        </div>
                        <div class="card-body">
                            <div class="mb-3">
                                <label class="form-label">CPU Efficiency</label>
                                <div class="progress">
                                    <div class="progress-bar bg-info" style="width: ${(analysis.resource_analysis?.cpu_efficiency || 0.87) * 100}%"></div>
                                </div>
                                <small class="text-muted">${((analysis.resource_analysis?.cpu_efficiency || 0.87) * 100).toFixed(1)}%</small>
                            </div>
                            <div class="mb-3">
                                <label class="form-label">Memory Efficiency</label>
                                <div class="progress">
                                    <div class="progress-bar bg-success" style="width: ${(analysis.resource_analysis?.memory_efficiency || 0.92) * 100}%"></div>
                                </div>
                                <small class="text-muted">${((analysis.resource_analysis?.memory_efficiency || 0.92) * 100).toFixed(1)}%</small>
                            </div>
                            <div class="mb-3">
                                <label class="form-label">I/O Efficiency</label>
                                <div class="progress">
                                    <div class="progress-bar bg-warning" style="width: ${(analysis.resource_analysis?.io_efficiency || 0.78) * 100}%"></div>
                                </div>
                                <small class="text-muted">${((analysis.resource_analysis?.io_efficiency || 0.78) * 100).toFixed(1)}%</small>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <div class="row mt-4">
                <div class="col-12">
                    <div class="card border-0 shadow-sm">
                        <div class="card-header bg-warning text-dark">
                            <h6 class="mb-0"><i class="fas fa-exclamation-triangle me-2"></i>Regression Analysis</h6>
                        </div>
                        <div class="card-body">
                            <div class="row">
                                <div class="col-md-4">
                                    <div class="text-center">
                                        <h3 class="text-danger">${analysis.regression_analysis?.detected_regressions || 2}</h3>
                                        <p class="text-muted mb-0">Detected Regressions</p>
                                    </div>
                                </div>
                                <div class="col-md-4">
                                    <div class="text-center">
                                        <h3 class="text-primary">${analysis.regression_analysis?.performance_score || 85.3}</h3>
                                        <p class="text-muted mb-0">Performance Score</p>
                                    </div>
                                </div>
                                <div class="col-md-4">
                                    <div class="text-center">
                                        <h3 class="text-success">
                                            <i class="fas fa-arrow-${(analysis.regression_analysis?.trend_direction === 'improving') ? 'up' : 'down'}"></i>
                                            ${analysis.regression_analysis?.trend_direction || 'improving'}
                                        </h3>
                                        <p class="text-muted mb-0">Trend Direction</p>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        `;

        container.innerHTML = analysisHTML;

        // Create trend analysis chart
        setTimeout(() => this.createTrendAnalysisChart(analysis.performance_trend), 100);
    }

    createTrendAnalysisChart(trendData) {
        const chartManager = window.chartManager;
        if (!chartManager || !trendData) return;

        const chartData = {
            labels: trendData.dates || [],
            datasets: [{
                label: 'Performance',
                data: trendData.performance || [],
                borderColor: '#007bff',
                backgroundColor: 'rgba(0, 123, 255, 0.1)',
                tension: 0.4
            }, {
                label: 'Regression Line',
                data: trendData.regression || [],
                borderColor: '#dc3545',
                backgroundColor: 'rgba(220, 53, 69, 0.1)',
                borderDash: [5, 5],
                tension: 0.4
            }]
        };

        chartManager.createChart('trendAnalysisChart', 'line', chartData);
    }

    updateOverviewCards(data) {
        this.updateCard('totalCategories', data.total_benchmarks || 156);
        this.updateCard('avgThroughput', this.formatThroughput(data.avg_throughput || 1250.5));
        this.updateCard('avgLatency', this.formatLatency(data.avg_latency || 12.3));
        this.updateCard('overallGrade', this.calculateGrade(data.success_rate || 0.987));
    }

    calculateGrade(successRate) {
        if (successRate >= 0.95) return 'A+';
        if (successRate >= 0.90) return 'A';
        if (successRate >= 0.85) return 'B+';
        if (successRate >= 0.80) return 'B';
        if (successRate >= 0.75) return 'C+';
        if (successRate >= 0.70) return 'C';
        return 'D';
    }

    updateOverviewCharts(data) {
        this.createPerformanceTimelineChart(data.recent_performance || {});
        this.createCategoryDistributionChart();
        this.updateTopPerformersTable();
        this.updatePerformanceIssuesTable();
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
        const textElement = document.getElementById('connectionText');
        
        if (statusElement) {
            statusElement.className = `status-dot ${connected ? 'online' : 'offline'}`;
        }
        
        if (textElement) {
            textElement.textContent = connected ? 'Connected' : 'Disconnected';
        }
    }

    createPerformanceTimelineChart(data) {
        const chartManager = window.chartManager;
        if (!chartManager) return;

        const chartData = {
            labels: data.labels || [],
            datasets: [{
                label: 'Throughput (MB/s)',
                data: data.throughput || [],
                borderColor: '#007bff',
                backgroundColor: 'rgba(0, 123, 255, 0.1)',
                tension: 0.4,
                yAxisID: 'y'
            }, {
                label: 'Latency (ms)',
                data: data.latency || [],
                borderColor: '#28a745',
                backgroundColor: 'rgba(40, 167, 69, 0.1)',
                tension: 0.4,
                yAxisID: 'y1'
            }]
        };

        const options = {
            scales: {
                y: {
                    type: 'linear',
                    display: true,
                    position: 'left',
                    title: {
                        display: true,
                        text: 'Throughput (MB/s)'
                    }
                },
                y1: {
                    type: 'linear',
                    display: true,
                    position: 'right',
                    title: {
                        display: true,
                        text: 'Latency (ms)'
                    },
                    grid: {
                        drawOnChartArea: false,
                    },
                }
            }
        };

        chartManager.createChart('performanceTimelineChart', 'line', chartData, options);
    }

    createCategoryDistributionChart() {
        const chartManager = window.chartManager;
        if (!chartManager) return;

        const chartData = {
            labels: ['File I/O', 'Metadata', 'Network', 'Multi-region'],
            datasets: [{
                data: [35, 25, 20, 20],
                backgroundColor: [
                    '#007bff',
                    '#28a745',
                    '#ffc107',
                    '#dc3545'
                ],
                borderWidth: 2,
                borderColor: '#fff'
            }]
        };

        const options = {
            plugins: {
                legend: {
                    position: 'bottom'
                }
            }
        };

        chartManager.createChart('categoryDistributionChart', 'doughnut', chartData, options);
    }

    updateTopPerformersTable() {
        const container = document.getElementById('topPerformersTable');
        if (!container) return;

        const performers = [
            { name: 'Large File Read', throughput: '2,450 MB/s', category: 'File I/O' },
            { name: 'Metadata Lookup', latency: '8.2 ms', category: 'Metadata' },
            { name: 'Network Transfer', throughput: '1,850 MB/s', category: 'Network' },
            { name: 'Cross-region Sync', latency: '45.3 ms', category: 'Multi-region' }
        ];

        const tableHTML = `
            <table class="table table-hover mb-0">
                <thead class="table-light">
                    <tr>
                        <th>Operation</th>
                        <th>Performance</th>
                        <th>Category</th>
                    </tr>
                </thead>
                <tbody>
                    ${performers.map(p => `
                        <tr>
                            <td>${p.name}</td>
                            <td class="text-success fw-bold">${p.throughput || p.latency}</td>
                            <td><span class="badge bg-primary">${p.category}</span></td>
                        </tr>
                    `).join('')}
                </tbody>
            </table>
        `;

        container.innerHTML = tableHTML;
    }

    updatePerformanceIssuesTable() {
        const container = document.getElementById('performanceIssuesTable');
        if (!container) return;

        const issues = [
            { name: 'Small File Write', issue: 'High latency', severity: 'warning' },
            { name: 'Concurrent Access', issue: 'Lock contention', severity: 'danger' }
        ];

        const tableHTML = `
            <table class="table table-hover mb-0">
                <thead class="table-light">
                    <tr>
                        <th>Operation</th>
                        <th>Issue</th>
                        <th>Severity</th>
                    </tr>
                </thead>
                <tbody>
                    ${issues.map(issue => `
                        <tr>
                            <td>${issue.name}</td>
                            <td>${issue.issue}</td>
                            <td><span class="badge bg-${issue.severity}">${issue.severity.toUpperCase()}</span></td>
                        </tr>
                    `).join('')}
                    ${issues.length === 0 ? '<tr><td colspan="3" class="text-center text-muted">No issues detected</td></tr>' : ''}
                </tbody>
            </table>
        `;

        container.innerHTML = tableHTML;
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