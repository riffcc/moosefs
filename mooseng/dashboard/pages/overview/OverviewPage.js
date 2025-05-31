/**
 * Overview Page Component for MooseNG Dashboard
 * Displays system-wide performance metrics and health status
 */

class OverviewPage {
    constructor(container, options = {}) {
        this.container = container;
        this.options = {
            refreshInterval: 30000, // 30 seconds
            autoRefresh: true,
            ...options
        };
        
        this.charts = new Map();
        this.metrics = new Map();
        this.apiClient = new ApiClient();
        this.refreshTimer = null;
        
        this.init();
    }

    async init() {
        this.render();
        await this.loadData();
        this.setupCharts();
        
        if (this.options.autoRefresh) {
            this.startAutoRefresh();
        }
    }

    render() {
        this.container.innerHTML = `
            <div class="overview-page">
                <!-- Key Metrics Summary -->
                <section class="metrics-summary mb-6">
                    <h2 class="section-title">System Performance Overview</h2>
                    <div class="grid grid-cols-4" id="keyMetrics">
                        <!-- Metrics will be populated here -->
                    </div>
                </section>

                <!-- Performance Charts -->
                <section class="performance-charts mb-6">
                    <div class="grid grid-cols-2">
                        <div class="card">
                            <div class="card-header">
                                <h3 class="card-title">Performance Trends</h3>
                                <p class="card-subtitle">Last 24 hours</p>
                            </div>
                            <div class="chart-container large">
                                <canvas id="performanceTrendChart"></canvas>
                            </div>
                        </div>
                        
                        <div class="card">
                            <div class="card-header">
                                <h3 class="card-title">System Health</h3>
                                <p class="card-subtitle">Component status</p>
                            </div>
                            <div class="chart-container">
                                <canvas id="systemHealthChart"></canvas>
                            </div>
                        </div>
                    </div>
                </section>

                <!-- Component Status Grid -->
                <section class="component-status mb-6">
                    <h2 class="section-title">Component Status</h2>
                    <div class="grid grid-cols-4" id="componentStatus">
                        <!-- Component status cards will be populated here -->
                    </div>
                </section>

                <!-- Recent Benchmark Results -->
                <section class="recent-results">
                    <div class="card">
                        <div class="card-header">
                            <h3 class="card-title">Recent Benchmark Results</h3>
                            <p class="card-subtitle">Latest test runs</p>
                            <button class="refresh-btn" id="refreshResults">
                                <span class="refresh-icon">⟳</span> Refresh
                            </button>
                        </div>
                        <div class="results-table-container">
                            <table class="results-table" id="resultsTable">
                                <thead>
                                    <tr>
                                        <th>Timestamp</th>
                                        <th>Test Type</th>
                                        <th>Duration</th>
                                        <th>Throughput</th>
                                        <th>Ops/Sec</th>
                                        <th>Grade</th>
                                        <th>Status</th>
                                    </tr>
                                </thead>
                                <tbody id="resultsTableBody">
                                    <!-- Results will be populated here -->
                                </tbody>
                            </table>
                        </div>
                    </div>
                </section>
            </div>
        `;

        this.setupEventListeners();
    }

    setupEventListeners() {
        // Refresh button
        document.getElementById('refreshResults')?.addEventListener('click', () => {
            this.refresh();
        });

        // Chart resize on window resize
        window.addEventListener('resize', () => {
            this.resizeCharts();
        });
    }

    async loadData() {
        try {
            this.showLoading();
            
            // Load overview data from API or use mock data
            const data = await this.apiClient.getOverviewMetrics();
            this.updateMetrics(data);
            this.updateCharts(data);
            this.updateComponentStatus(data);
            this.updateRecentResults(data);
            
        } catch (error) {
            console.error('Error loading overview data:', error);
            this.showError('Failed to load overview data');
            // Use mock data as fallback
            this.loadMockData();
        } finally {
            this.hideLoading();
        }
    }

    updateMetrics(data) {
        const keyMetricsContainer = document.getElementById('keyMetrics');
        if (!keyMetricsContainer) return;

        const metrics = [
            {
                value: data.totalOpsPerSecond || 1506,
                label: 'Total Ops/Sec',
                change: data.totalOpsPerSecondChange || 12.5,
                unit: ''
            },
            {
                value: data.totalThroughputMBps || 135.13,
                label: 'Throughput',
                change: data.totalThroughputChange || 8.3,
                unit: 'MB/s'
            },
            {
                value: data.avgLatencyMs || 45.2,
                label: 'Avg Latency',
                change: -(data.avgLatencyChange || 15.1), // Negative is good for latency
                unit: 'ms'
            },
            {
                value: data.systemGrade || 'A',
                label: 'System Grade',
                change: null,
                unit: ''
            }
        ];

        const metricsHtml = metrics.map(metric => {
            const changeClass = metric.change === null ? 'neutral' : 
                              metric.change > 0 ? 'positive' : 'negative';
            const changeText = metric.change === null ? '' : 
                             `${metric.change >= 0 ? '+' : ''}${metric.change.toFixed(1)}%`;

            return `
                <div class="metric-card">
                    <div class="metric-value">${this.formatValue(metric.value, metric.unit)}</div>
                    <div class="metric-label">${metric.label}</div>
                    ${changeText ? `<div class="metric-change ${changeClass}">${changeText}</div>` : ''}
                </div>
            `;
        }).join('');

        keyMetricsContainer.innerHTML = metricsHtml;
    }

    setupCharts() {
        this.createPerformanceTrendChart();
        this.createSystemHealthChart();
    }

    updateCharts(data) {
        this.updatePerformanceTrendChart(data.performanceTrend);
        this.updateSystemHealthChart(data.systemHealth);
    }

    createPerformanceTrendChart() {
        const chartComponent = new ChartComponent('performanceTrendChart');
        
        const chart = chartComponent.createMultiAxisChart({
            labels: [],
            datasets: [
                {
                    label: 'Throughput (MB/s)',
                    data: [],
                    borderColor: '#2563eb',
                    backgroundColor: 'rgba(37, 99, 235, 0.1)',
                    yAxisID: 'y'
                },
                {
                    label: 'Operations/sec',
                    data: [],
                    borderColor: '#10b981',
                    backgroundColor: 'rgba(16, 185, 129, 0.1)',
                    yAxisID: 'y'
                },
                {
                    label: 'Latency (ms)',
                    data: [],
                    borderColor: '#ef4444',
                    backgroundColor: 'rgba(239, 68, 68, 0.1)',
                    yAxisID: 'y1'
                }
            ]
        }, {
            scales: {
                y: {
                    title: { display: true, text: 'Throughput / Ops' },
                    position: 'left'
                },
                y1: {
                    title: { display: true, text: 'Latency (ms)' },
                    position: 'right'
                }
            }
        });

        this.charts.set('performanceTrend', chartComponent);
    }

    createSystemHealthChart() {
        const chartComponent = new ChartComponent('systemHealthChart');
        
        const chart = chartComponent.createDoughnutChart({
            labels: ['Healthy', 'Warning', 'Error', 'Offline'],
            datasets: [{
                data: [85, 10, 3, 2],
                backgroundColor: [
                    '#10b981', // Green
                    '#f59e0b', // Yellow
                    '#ef4444', // Red
                    '#6b7280'  // Gray
                ]
            }]
        });

        this.charts.set('systemHealth', chartComponent);
    }

    updatePerformanceTrendChart(trendData) {
        const chartComponent = this.charts.get('performanceTrend');
        if (!chartComponent || !trendData) return;

        chartComponent.updateData({
            labels: trendData.timestamps || this.generateTimeLabels(),
            datasets: [
                {
                    label: 'Throughput (MB/s)',
                    data: trendData.throughput || this.generateMockData(24, 100, 150),
                    borderColor: '#2563eb',
                    backgroundColor: 'rgba(37, 99, 235, 0.1)',
                    yAxisID: 'y'
                },
                {
                    label: 'Operations/sec',
                    data: trendData.opsPerSecond || this.generateMockData(24, 1200, 1800),
                    borderColor: '#10b981',
                    backgroundColor: 'rgba(16, 185, 129, 0.1)',
                    yAxisID: 'y'
                },
                {
                    label: 'Latency (ms)',
                    data: trendData.latency || this.generateMockData(24, 30, 60),
                    borderColor: '#ef4444',
                    backgroundColor: 'rgba(239, 68, 68, 0.1)',
                    yAxisID: 'y1'
                }
            ]
        });
    }

    updateSystemHealthChart(healthData) {
        const chartComponent = this.charts.get('systemHealth');
        if (!chartComponent || !healthData) return;

        chartComponent.updateData({
            labels: healthData.labels || ['Healthy', 'Warning', 'Error', 'Offline'],
            datasets: [{
                data: healthData.values || [85, 10, 3, 2],
                backgroundColor: [
                    '#10b981', // Green
                    '#f59e0b', // Yellow  
                    '#ef4444', // Red
                    '#6b7280'  // Gray
                ]
            }]
        });
    }

    updateComponentStatus(data) {
        const statusContainer = document.getElementById('componentStatus');
        if (!statusContainer) return;

        const components = [
            {
                name: 'Master Server',
                status: data.components?.master || 'healthy',
                uptime: '99.9%',
                responseTime: '12ms'
            },
            {
                name: 'Chunk Server',
                status: data.components?.chunkserver || 'healthy',
                uptime: '99.8%',
                responseTime: '8ms'
            },
            {
                name: 'Client',
                status: data.components?.client || 'healthy',
                uptime: '99.7%',
                responseTime: '15ms'
            },
            {
                name: 'Meta Logger',
                status: data.components?.metalogger || 'healthy',
                uptime: '99.9%',
                responseTime: '5ms'
            }
        ];

        const statusHtml = components.map(component => {
            const statusColors = {
                healthy: '#10b981',
                warning: '#f59e0b',
                error: '#ef4444',
                offline: '#6b7280'
            };

            return `
                <div class="component-status-card">
                    <div class="component-header">
                        <h4 class="component-name">${component.name}</h4>
                        <div class="status-indicator">
                            <span class="status-dot" style="background-color: ${statusColors[component.status]}"></span>
                            <span class="status-text">${component.status.toUpperCase()}</span>
                        </div>
                    </div>
                    <div class="component-metrics">
                        <div class="metric">
                            <span class="metric-label">Uptime</span>
                            <span class="metric-value">${component.uptime}</span>
                        </div>
                        <div class="metric">
                            <span class="metric-label">Response</span>
                            <span class="metric-value">${component.responseTime}</span>
                        </div>
                    </div>
                </div>
            `;
        }).join('');

        statusContainer.innerHTML = statusHtml;
    }

    updateRecentResults(data) {
        const tableBody = document.getElementById('resultsTableBody');
        if (!tableBody) return;

        const results = data.recentResults || this.getMockResults();
        
        const resultsHtml = results.map(result => {
            const statusColors = {
                passed: 'success',
                failed: 'error',
                running: 'warning'
            };

            return `
                <tr class="result-row">
                    <td class="timestamp">${new Date(result.timestamp).toLocaleString()}</td>
                    <td class="test-type">${result.testType}</td>
                    <td class="duration">${result.duration}ms</td>
                    <td class="throughput">${result.throughput} MB/s</td>
                    <td class="ops-per-sec">${result.opsPerSec.toLocaleString()}</td>
                    <td class="grade">${result.grade}</td>
                    <td class="status">
                        <span class="status-badge ${statusColors[result.status]}">${result.status.toUpperCase()}</span>
                    </td>
                </tr>
            `;
        }).join('');

        tableBody.innerHTML = resultsHtml;
    }

    // Utility Methods

    formatValue(value, unit = '') {
        if (typeof value === 'number') {
            if (value >= 1000000) {
                return `${(value / 1000000).toFixed(1)}M${unit}`;
            } else if (value >= 1000) {
                return `${(value / 1000).toFixed(1)}K${unit}`;
            } else {
                return `${value.toFixed(1)}${unit}`;
            }
        }
        return `${value}${unit}`;
    }

    generateTimeLabels(hours = 24) {
        const labels = [];
        const now = new Date();
        
        for (let i = hours; i >= 0; i--) {
            const time = new Date(now - i * 60 * 60 * 1000);
            labels.push(time.toLocaleTimeString('en-US', { 
                hour: '2-digit', 
                minute: '2-digit' 
            }));
        }
        
        return labels;
    }

    generateMockData(points, min, max) {
        const data = [];
        for (let i = 0; i < points; i++) {
            data.push(min + Math.random() * (max - min));
        }
        return data;
    }

    getMockResults() {
        return [
            {
                timestamp: new Date().toISOString(),
                testType: 'File Operations',
                duration: 196,
                throughput: 135.13,
                opsPerSec: 1506,
                grade: 'A',
                status: 'passed'
            },
            {
                timestamp: new Date(Date.now() - 300000).toISOString(),
                testType: 'Network Performance',
                duration: 1782,
                throughput: 98.4,
                opsPerSec: 892,
                grade: 'B+',
                status: 'passed'
            },
            {
                timestamp: new Date(Date.now() - 600000).toISOString(),
                testType: 'Multi-Region Sync',
                duration: 3250,
                throughput: 67.8,
                opsPerSec: 623,
                grade: 'B',
                status: 'passed'
            }
        ];
    }

    loadMockData() {
        const mockData = {
            totalOpsPerSecond: 1506,
            totalOpsPerSecondChange: 12.5,
            totalThroughputMBps: 135.13,
            totalThroughputChange: 8.3,
            avgLatencyMs: 45.2,
            avgLatencyChange: 15.1,
            systemGrade: 'A',
            performanceTrend: {
                timestamps: this.generateTimeLabels(),
                throughput: this.generateMockData(24, 100, 150),
                opsPerSecond: this.generateMockData(24, 1200, 1800),
                latency: this.generateMockData(24, 30, 60)
            },
            systemHealth: {
                labels: ['Healthy', 'Warning', 'Error', 'Offline'],
                values: [85, 10, 3, 2]
            },
            components: {
                master: 'healthy',
                chunkserver: 'healthy',
                client: 'healthy',
                metalogger: 'healthy'
            },
            recentResults: this.getMockResults()
        };

        this.updateMetrics(mockData);
        this.updateCharts(mockData);
        this.updateComponentStatus(mockData);
        this.updateRecentResults(mockData);
    }

    startAutoRefresh() {
        this.refreshTimer = setInterval(() => {
            this.refresh();
        }, this.options.refreshInterval);
    }

    stopAutoRefresh() {
        if (this.refreshTimer) {
            clearInterval(this.refreshTimer);
            this.refreshTimer = null;
        }
    }

    async refresh() {
        await this.loadData();
    }

    resizeCharts() {
        this.charts.forEach(chart => {
            chart.resize();
        });
    }

    showLoading() {
        // Implementation depends on loading indicator setup
    }

    hideLoading() {
        // Implementation depends on loading indicator setup
    }

    showError(message) {
        console.error(message);
        // Implementation depends on error display setup
    }

    destroy() {
        this.stopAutoRefresh();
        
        this.charts.forEach(chart => {
            chart.destroy();
        });
        
        this.charts.clear();
        this.metrics.clear();
    }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
    module.exports = OverviewPage;
}