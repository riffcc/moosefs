/**
 * Enhanced Benchmark Results Page for MooseNG Dashboard
 * Provides real-time benchmark monitoring and historical analysis
 */

class BenchmarkResultsPage {
    constructor() {
        this.charts = new Map();
        this.currentSession = null;
        this.autoRefresh = true;
        this.refreshInterval = 5000; // 5 seconds
        this.isVisible = false;
        
        this.init();
    }

    async init() {
        await this.loadPage();
        this.setupEventListeners();
        this.setupCharts();
        this.startAutoRefresh();
    }

    async loadPage() {
        const container = document.getElementById('dashboardContainer') || document.body;
        
        container.innerHTML = `
            <div class="benchmark-results-page">
                <!-- Header Section -->
                <div class="page-header">
                    <div class="header-content">
                        <h1>Benchmark Results</h1>
                        <div class="header-controls">
                            <div class="session-selector">
                                <label for="sessionSelect">Session:</label>
                                <select id="sessionSelect">
                                    <option value="live">Live Monitoring</option>
                                    <option value="latest">Latest Session</option>
                                </select>
                            </div>
                            <div class="refresh-controls">
                                <button id="refreshBtn" class="btn btn-primary">
                                    <span class="icon">🔄</span> Refresh
                                </button>
                                <button id="autoRefreshBtn" class="btn btn-secondary">
                                    <span class="icon">⏰</span> Auto: ON
                                </button>
                            </div>
                            <div class="export-controls">
                                <button id="exportBtn" class="btn btn-outline">
                                    <span class="icon">📊</span> Export
                                </button>
                            </div>
                        </div>
                    </div>
                </div>

                <!-- Status Overview -->
                <div class="status-overview">
                    <div class="status-cards">
                        <div class="status-card">
                            <div class="status-icon">⚡</div>
                            <div class="status-content">
                                <div class="status-label">Latest Throughput</div>
                                <div class="status-value" id="latestThroughput">--</div>
                                <div class="status-change" id="throughputChange">--</div>
                            </div>
                        </div>
                        <div class="status-card">
                            <div class="status-icon">🚀</div>
                            <div class="status-content">
                                <div class="status-label">Operations/sec</div>
                                <div class="status-value" id="latestOpsPerSec">--</div>
                                <div class="status-change" id="opsPerSecChange">--</div>
                            </div>
                        </div>
                        <div class="status-card">
                            <div class="status-icon">⏱️</div>
                            <div class="status-content">
                                <div class="status-label">Avg Latency</div>
                                <div class="status-value" id="latestLatency">--</div>
                                <div class="status-change" id="latencyChange">--</div>
                            </div>
                        </div>
                        <div class="status-card">
                            <div class="status-icon">🏆</div>
                            <div class="status-content">
                                <div class="status-label">Grade</div>
                                <div class="status-value" id="latestGrade">--</div>
                                <div class="status-indicator" id="gradeIndicator"></div>
                            </div>
                        </div>
                    </div>
                </div>

                <!-- Main Charts Section -->
                <div class="charts-section">
                    <div class="charts-grid">
                        <!-- Performance Trend Chart -->
                        <div class="chart-container">
                            <div class="chart-header">
                                <h3>Performance Trend</h3>
                                <div class="chart-controls">
                                    <select id="performanceMetric">
                                        <option value="throughput">Throughput</option>
                                        <option value="opsPerSecond">Ops/Second</option>
                                        <option value="latency">Latency</option>
                                    </select>
                                    <select id="timeRange">
                                        <option value="1h">Last Hour</option>
                                        <option value="6h">Last 6 Hours</option>
                                        <option value="24h">Last 24 Hours</option>
                                        <option value="7d">Last 7 Days</option>
                                    </select>
                                </div>
                            </div>
                            <div class="chart-body">
                                <canvas id="performanceTrendChart"></canvas>
                            </div>
                        </div>

                        <!-- Benchmark Comparison Chart -->
                        <div class="chart-container">
                            <div class="chart-header">
                                <h3>Operation Comparison</h3>
                                <div class="chart-controls">
                                    <select id="comparisonType">
                                        <option value="all">All Operations</option>
                                        <option value="file">File Operations</option>
                                        <option value="network">Network Operations</option>
                                        <option value="metadata">Metadata Operations</option>
                                    </select>
                                </div>
                            </div>
                            <div class="chart-body">
                                <canvas id="benchmarkComparisonChart"></canvas>
                            </div>
                        </div>

                        <!-- System Health Distribution -->
                        <div class="chart-container">
                            <div class="chart-header">
                                <h3>System Health Distribution</h3>
                                <div class="health-legend">
                                    <span class="legend-item healthy">● Healthy</span>
                                    <span class="legend-item warning">● Warning</span>
                                    <span class="legend-item error">● Error</span>
                                    <span class="legend-item offline">● Offline</span>
                                </div>
                            </div>
                            <div class="chart-body">
                                <canvas id="healthDistributionChart"></canvas>
                            </div>
                        </div>

                        <!-- Concurrent Operations Performance -->
                        <div class="chart-container">
                            <div class="chart-header">
                                <h3>Concurrent Operations Performance</h3>
                                <div class="chart-controls">
                                    <button id="showConcurrencyDetails" class="btn btn-small">Details</button>
                                </div>
                            </div>
                            <div class="chart-body">
                                <canvas id="concurrencyChart"></canvas>
                            </div>
                        </div>
                    </div>
                </div>

                <!-- Recent Results Table -->
                <div class="results-section">
                    <div class="section-header">
                        <h3>Recent Benchmark Results</h3>
                        <div class="section-controls">
                            <input type="text" id="resultsFilter" placeholder="Filter results...">
                            <select id="resultsLimit">
                                <option value="10">Show 10</option>
                                <option value="25">Show 25</option>
                                <option value="50">Show 50</option>
                                <option value="100">Show 100</option>
                            </select>
                        </div>
                    </div>
                    <div class="results-table-container">
                        <table id="resultsTable" class="results-table">
                            <thead>
                                <tr>
                                    <th>Timestamp</th>
                                    <th>Test Type</th>
                                    <th>Duration</th>
                                    <th>Throughput</th>
                                    <th>Ops/Sec</th>
                                    <th>Grade</th>
                                    <th>Status</th>
                                    <th>Actions</th>
                                </tr>
                            </thead>
                            <tbody id="resultsTableBody">
                                <!-- Results will be populated here -->
                            </tbody>
                        </table>
                    </div>
                </div>

                <!-- Benchmark Control Panel (for live sessions) -->
                <div class="control-panel" id="benchmarkControlPanel" style="display: none;">
                    <div class="panel-header">
                        <h3>Benchmark Control</h3>
                        <button id="toggleControlPanel" class="btn btn-small">Hide</button>
                    </div>
                    <div class="panel-content">
                        <div class="control-groups">
                            <div class="control-group">
                                <label>Quick Benchmarks</label>
                                <div class="control-buttons">
                                    <button id="runQuickBench" class="btn btn-primary">Quick Test</button>
                                    <button id="runFileBench" class="btn btn-secondary">File Ops</button>
                                    <button id="runNetworkBench" class="btn btn-secondary">Network</button>
                                </div>
                            </div>
                            <div class="control-group">
                                <label>Comprehensive Tests</label>
                                <div class="control-buttons">
                                    <button id="runFullBench" class="btn btn-warning">Full Suite</button>
                                    <button id="runStressBench" class="btn btn-danger">Stress Test</button>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>

                <!-- Loading Overlay -->
                <div id="loadingOverlay" class="loading-overlay" style="display: none;">
                    <div class="loading-content">
                        <div class="spinner"></div>
                        <p>Loading benchmark data...</p>
                    </div>
                </div>
            </div>

            <style>
                .benchmark-results-page {
                    padding: 1rem;
                    max-width: 1400px;
                    margin: 0 auto;
                }

                .page-header {
                    background: white;
                    border-radius: 8px;
                    padding: 1.5rem;
                    margin-bottom: 1.5rem;
                    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
                }

                .header-content {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    flex-wrap: wrap;
                    gap: 1rem;
                }

                .header-controls {
                    display: flex;
                    gap: 1rem;
                    align-items: center;
                    flex-wrap: wrap;
                }

                .status-overview {
                    margin-bottom: 1.5rem;
                }

                .status-cards {
                    display: grid;
                    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
                    gap: 1rem;
                }

                .status-card {
                    background: white;
                    border-radius: 8px;
                    padding: 1.5rem;
                    display: flex;
                    align-items: center;
                    gap: 1rem;
                    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
                    transition: transform 0.2s;
                }

                .status-card:hover {
                    transform: translateY(-2px);
                }

                .status-icon {
                    font-size: 2rem;
                    width: 60px;
                    height: 60px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    background: #f1f5f9;
                    border-radius: 50%;
                }

                .status-content {
                    flex: 1;
                }

                .status-label {
                    font-size: 0.875rem;
                    color: #64748b;
                    margin-bottom: 0.25rem;
                }

                .status-value {
                    font-size: 1.5rem;
                    font-weight: bold;
                    color: #1e293b;
                }

                .status-change {
                    font-size: 0.875rem;
                    margin-top: 0.25rem;
                }

                .status-change.positive {
                    color: #059669;
                }

                .status-change.negative {
                    color: #dc2626;
                }

                .charts-section {
                    margin-bottom: 2rem;
                }

                .charts-grid {
                    display: grid;
                    grid-template-columns: repeat(auto-fit, minmax(500px, 1fr));
                    gap: 1.5rem;
                }

                .chart-container {
                    background: white;
                    border-radius: 8px;
                    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
                    overflow: hidden;
                }

                .chart-header {
                    padding: 1rem 1.5rem;
                    background: #f8fafc;
                    border-bottom: 1px solid #e2e8f0;
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                }

                .chart-header h3 {
                    margin: 0;
                    font-size: 1.125rem;
                    color: #1e293b;
                }

                .chart-controls {
                    display: flex;
                    gap: 0.5rem;
                    align-items: center;
                }

                .chart-body {
                    padding: 1.5rem;
                    height: 300px;
                    position: relative;
                }

                .results-section {
                    background: white;
                    border-radius: 8px;
                    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
                    overflow: hidden;
                }

                .section-header {
                    padding: 1rem 1.5rem;
                    background: #f8fafc;
                    border-bottom: 1px solid #e2e8f0;
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                }

                .section-controls {
                    display: flex;
                    gap: 0.5rem;
                    align-items: center;
                }

                .results-table-container {
                    overflow-x: auto;
                }

                .results-table {
                    width: 100%;
                    border-collapse: collapse;
                }

                .results-table th,
                .results-table td {
                    padding: 0.75rem;
                    text-align: left;
                    border-bottom: 1px solid #e2e8f0;
                }

                .results-table th {
                    background: #f8fafc;
                    font-weight: 600;
                    color: #374151;
                }

                .results-table tbody tr:hover {
                    background: #f9fafb;
                }

                .control-panel {
                    background: white;
                    border-radius: 8px;
                    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
                    margin-top: 1.5rem;
                    overflow: hidden;
                }

                .panel-header {
                    padding: 1rem 1.5rem;
                    background: #f8fafc;
                    border-bottom: 1px solid #e2e8f0;
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                }

                .panel-content {
                    padding: 1.5rem;
                }

                .control-groups {
                    display: grid;
                    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
                    gap: 1.5rem;
                }

                .control-group label {
                    display: block;
                    font-weight: 600;
                    margin-bottom: 0.5rem;
                    color: #374151;
                }

                .control-buttons {
                    display: flex;
                    gap: 0.5rem;
                    flex-wrap: wrap;
                }

                .loading-overlay {
                    position: fixed;
                    top: 0;
                    left: 0;
                    right: 0;
                    bottom: 0;
                    background: rgba(255, 255, 255, 0.9);
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    z-index: 1000;
                }

                .loading-content {
                    text-align: center;
                }

                .btn {
                    padding: 0.5rem 1rem;
                    border: none;
                    border-radius: 4px;
                    font-size: 0.875rem;
                    cursor: pointer;
                    transition: all 0.2s;
                    display: inline-flex;
                    align-items: center;
                    gap: 0.25rem;
                }

                .btn-primary {
                    background: #2563eb;
                    color: white;
                }

                .btn-secondary {
                    background: #64748b;
                    color: white;
                }

                .btn-outline {
                    background: transparent;
                    color: #374151;
                    border: 1px solid #d1d5db;
                }

                .btn-small {
                    padding: 0.25rem 0.5rem;
                    font-size: 0.75rem;
                }

                .health-legend {
                    display: flex;
                    gap: 1rem;
                    font-size: 0.875rem;
                }

                .legend-item {
                    display: flex;
                    align-items: center;
                    gap: 0.25rem;
                }

                .legend-item.healthy { color: #059669; }
                .legend-item.warning { color: #d97706; }
                .legend-item.error { color: #dc2626; }
                .legend-item.offline { color: #6b7280; }

                select, input[type="text"] {
                    padding: 0.375rem 0.75rem;
                    border: 1px solid #d1d5db;
                    border-radius: 4px;
                    font-size: 0.875rem;
                }

                .spinner {
                    width: 40px;
                    height: 40px;
                    border: 4px solid #f3f4f6;
                    border-top: 4px solid #2563eb;
                    border-radius: 50%;
                    animation: spin 1s linear infinite;
                    margin: 0 auto 1rem;
                }

                @keyframes spin {
                    0% { transform: rotate(0deg); }
                    100% { transform: rotate(360deg); }
                }

                @media (max-width: 768px) {
                    .charts-grid {
                        grid-template-columns: 1fr;
                    }
                    
                    .status-cards {
                        grid-template-columns: 1fr;
                    }
                    
                    .header-content {
                        flex-direction: column;
                        align-items: flex-start;
                    }
                    
                    .control-groups {
                        grid-template-columns: 1fr;
                    }
                }
            </style>
        `;
    }

    setupEventListeners() {
        // Auto-refresh toggle
        document.getElementById('autoRefreshBtn').addEventListener('click', () => {
            this.toggleAutoRefresh();
        });

        // Manual refresh
        document.getElementById('refreshBtn').addEventListener('click', () => {
            this.refreshData();
        });

        // Session selector
        document.getElementById('sessionSelect').addEventListener('change', (e) => {
            this.changeSession(e.target.value);
        });

        // Chart controls
        document.getElementById('performanceMetric').addEventListener('change', () => {
            this.updatePerformanceChart();
        });

        document.getElementById('timeRange').addEventListener('change', () => {
            this.updatePerformanceChart();
        });

        document.getElementById('comparisonType').addEventListener('change', () => {
            this.updateComparisonChart();
        });

        // Export functionality
        document.getElementById('exportBtn').addEventListener('click', () => {
            this.exportData();
        });

        // Benchmark control buttons
        document.getElementById('runQuickBench')?.addEventListener('click', () => {
            this.runBenchmark('quick');
        });

        document.getElementById('runFileBench')?.addEventListener('click', () => {
            this.runBenchmark('file');
        });

        document.getElementById('runNetworkBench')?.addEventListener('click', () => {
            this.runBenchmark('network');
        });

        document.getElementById('runFullBench')?.addEventListener('click', () => {
            this.runBenchmark('full');
        });

        // Results filtering
        document.getElementById('resultsFilter').addEventListener('input', () => {
            this.filterResults();
        });

        document.getElementById('resultsLimit').addEventListener('change', () => {
            this.updateResultsTable();
        });

        // Listen for benchmark data updates
        document.addEventListener('benchmarkDataUpdate', (event) => {
            this.handleBenchmarkUpdate(event.detail);
        });
    }

    setupCharts() {
        // Performance trend chart
        this.charts.set('performance', new ChartComponent('performanceTrendChart', {
            plugins: {
                title: {
                    display: true,
                    text: 'Performance Over Time'
                }
            }
        }));

        // Benchmark comparison chart
        this.charts.set('comparison', new ChartComponent('benchmarkComparisonChart'));

        // Health distribution chart
        this.charts.set('health', new ChartComponent('healthDistributionChart'));

        // Concurrency chart
        this.charts.set('concurrency', new ChartComponent('concurrencyChart'));

        this.updateAllCharts();
    }

    async refreshData() {
        this.showLoading(true);
        
        try {
            // Get latest benchmark data
            if (window.benchmarkIntegration) {
                await window.benchmarkIntegration.checkForUpdates();
                const metrics = window.benchmarkIntegration.getOverviewMetrics();
                this.updateStatusCards(metrics);
                this.updateAllCharts();
                this.updateResultsTable();
            }
        } catch (error) {
            console.error('Failed to refresh data:', error);
        } finally {
            this.showLoading(false);
        }
    }

    updateStatusCards(metrics) {
        document.getElementById('latestThroughput').textContent = 
            `${metrics.totalThroughputMBps.toFixed(2)} MB/s`;
        document.getElementById('latestOpsPerSec').textContent = 
            metrics.totalOpsPerSecond.toLocaleString();
        document.getElementById('latestLatency').textContent = 
            `${metrics.avgLatencyMs.toFixed(1)}ms`;
        document.getElementById('latestGrade').textContent = metrics.systemGrade;

        // Update change indicators
        this.updateChangeIndicator('throughputChange', metrics.totalThroughputChange);
        this.updateChangeIndicator('opsPerSecChange', metrics.totalOpsPerSecondChange);
        this.updateChangeIndicator('latencyChange', metrics.avgLatencyChange, true);

        // Update grade indicator
        this.updateGradeIndicator(metrics.systemGrade);
    }

    updateChangeIndicator(elementId, change, inverse = false) {
        const element = document.getElementById(elementId);
        const isPositive = inverse ? change < 0 : change > 0;
        const symbol = change > 0 ? '↑' : change < 0 ? '↓' : '—';
        
        element.textContent = `${symbol} ${Math.abs(change).toFixed(1)}%`;
        element.className = `status-change ${isPositive ? 'positive' : 'negative'}`;
    }

    updateGradeIndicator(grade) {
        const indicator = document.getElementById('gradeIndicator');
        const colors = {
            'A': '#059669', 'A-': '#059669',
            'B+': '#d97706', 'B': '#d97706', 'B-': '#d97706',
            'C+': '#dc2626', 'C': '#dc2626', 'C-': '#dc2626',
            'D': '#dc2626', 'F': '#dc2626'
        };
        
        indicator.style.backgroundColor = colors[grade] || '#6b7280';
        indicator.style.width = '12px';
        indicator.style.height = '12px';
        indicator.style.borderRadius = '50%';
        indicator.style.display = 'inline-block';
        indicator.style.marginLeft = '0.5rem';
    }

    updateAllCharts() {
        this.updatePerformanceChart();
        this.updateComparisonChart();
        this.updateHealthChart();
        this.updateConcurrencyChart();
    }

    updatePerformanceChart() {
        const metric = document.getElementById('performanceMetric').value;
        const timeRange = document.getElementById('timeRange').value;
        
        if (window.benchmarkIntegration) {
            const metrics = window.benchmarkIntegration.getOverviewMetrics();
            const trend = metrics.performanceTrend;
            
            let data, label, unit;
            switch (metric) {
                case 'throughput':
                    data = trend.throughput;
                    label = 'Throughput';
                    unit = 'MB/s';
                    break;
                case 'opsPerSecond':
                    data = trend.opsPerSecond;
                    label = 'Operations/Second';
                    unit = 'ops/s';
                    break;
                case 'latency':
                    data = trend.latency;
                    label = 'Latency';
                    unit = 'ms';
                    break;
            }

            const chartData = {
                labels: trend.timestamps,
                data: data,
                label: `${label} (${unit})`
            };

            this.charts.get('performance').createLineChart(chartData);
        }
    }

    updateComparisonChart() {
        const comparisonType = document.getElementById('comparisonType').value;
        
        // Mock data for demonstration
        const data = {
            labels: ['File Create', 'File Read', 'File Write', 'File Delete', 'Metadata Ops'],
            data: [145, 189, 156, 178, 234],
            label: 'Operations/Second'
        };

        this.charts.get('comparison').createBarChart(data);
    }

    updateHealthChart() {
        if (window.benchmarkIntegration) {
            const metrics = window.benchmarkIntegration.getOverviewMetrics();
            const health = metrics.systemHealth;
            
            const data = {
                labels: health.labels,
                data: health.values,
                backgroundColor: ['#059669', '#d97706', '#dc2626', '#6b7280']
            };

            this.charts.get('health').createDoughnutChart(data);
        }
    }

    updateConcurrencyChart() {
        // Mock concurrency data
        const data = {
            labels: ['1 Thread', '5 Threads', '10 Threads', '25 Threads', '50 Threads'],
            data: {
                'Throughput': [120, 450, 780, 1200, 1450],
                'Latency': [2.1, 3.5, 5.2, 8.9, 15.3]
            }
        };

        this.charts.get('concurrency').createMultiAxisChart(data);
    }

    updateResultsTable() {
        if (window.benchmarkIntegration) {
            const metrics = window.benchmarkIntegration.getOverviewMetrics();
            const results = metrics.recentResults || [];
            
            const tbody = document.getElementById('resultsTableBody');
            tbody.innerHTML = '';

            results.forEach(result => {
                const row = document.createElement('tr');
                row.innerHTML = `
                    <td>${new Date(result.timestamp).toLocaleString()}</td>
                    <td>${result.testType}</td>
                    <td>${result.duration}ms</td>
                    <td>${result.throughput} MB/s</td>
                    <td>${result.opsPerSec.toLocaleString()}</td>
                    <td><span class="grade-badge grade-${result.grade.toLowerCase()}">${result.grade}</span></td>
                    <td><span class="status-badge status-${result.status}">${result.status}</span></td>
                    <td>
                        <button class="btn btn-small" onclick="this.viewDetails('${result.timestamp}')">
                            View Details
                        </button>
                    </td>
                `;
                tbody.appendChild(row);
            });
        }
    }

    handleBenchmarkUpdate(data) {
        if (this.isVisible) {
            this.updateStatusCards(data);
            this.updatePerformanceChart();
            this.updateResultsTable();
        }
    }

    changeSession(sessionId) {
        this.currentSession = sessionId;
        
        if (sessionId === 'live') {
            document.getElementById('benchmarkControlPanel').style.display = 'block';
            this.startAutoRefresh();
        } else {
            document.getElementById('benchmarkControlPanel').style.display = 'none';
            this.stopAutoRefresh();
        }
        
        this.refreshData();
    }

    toggleAutoRefresh() {
        this.autoRefresh = !this.autoRefresh;
        const btn = document.getElementById('autoRefreshBtn');
        btn.innerHTML = `<span class="icon">⏰</span> Auto: ${this.autoRefresh ? 'ON' : 'OFF'}`;
        btn.className = `btn ${this.autoRefresh ? 'btn-secondary' : 'btn-outline'}`;
        
        if (this.autoRefresh) {
            this.startAutoRefresh();
        } else {
            this.stopAutoRefresh();
        }
    }

    startAutoRefresh() {
        if (this.refreshTimer) {
            clearInterval(this.refreshTimer);
        }
        
        if (this.autoRefresh) {
            this.refreshTimer = setInterval(() => {
                this.refreshData();
            }, this.refreshInterval);
        }
    }

    stopAutoRefresh() {
        if (this.refreshTimer) {
            clearInterval(this.refreshTimer);
            this.refreshTimer = null;
        }
    }

    async runBenchmark(type) {
        this.showLoading(true);
        
        try {
            // Simulate running benchmark via CLI
            const response = await fetch('/api/benchmark/run', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json'
                },
                body: JSON.stringify({ type, async: true })
            });
            
            if (response.ok) {
                console.log(`Started ${type} benchmark`);
                // Start polling for results
                this.pollForResults();
            } else {
                throw new Error('Failed to start benchmark');
            }
        } catch (error) {
            console.error('Failed to run benchmark:', error);
            alert('Failed to start benchmark. Make sure the backend is running.');
        } finally {
            this.showLoading(false);
        }
    }

    pollForResults() {
        // Poll for new results every 5 seconds
        const pollTimer = setInterval(() => {
            this.refreshData();
        }, 5000);

        // Stop polling after 5 minutes
        setTimeout(() => {
            clearInterval(pollTimer);
        }, 300000);
    }

    exportData() {
        if (window.benchmarkIntegration) {
            const data = window.benchmarkIntegration.exportData();
            const blob = new Blob([JSON.stringify(data, null, 2)], {
                type: 'application/json'
            });
            
            const url = URL.createObjectURL(blob);
            const a = document.createElement('a');
            a.href = url;
            a.download = `mooseng-benchmark-data-${new Date().toISOString().split('T')[0]}.json`;
            document.body.appendChild(a);
            a.click();
            document.body.removeChild(a);
            URL.revokeObjectURL(url);
        }
    }

    filterResults() {
        const filter = document.getElementById('resultsFilter').value.toLowerCase();
        const rows = document.querySelectorAll('#resultsTableBody tr');
        
        rows.forEach(row => {
            const text = row.textContent.toLowerCase();
            row.style.display = text.includes(filter) ? '' : 'none';
        });
    }

    showLoading(show) {
        document.getElementById('loadingOverlay').style.display = show ? 'flex' : 'none';
    }

    show() {
        this.isVisible = true;
        if (this.autoRefresh) {
            this.startAutoRefresh();
        }
        this.refreshData();
    }

    hide() {
        this.isVisible = false;
        this.stopAutoRefresh();
    }

    destroy() {
        this.stopAutoRefresh();
        this.charts.forEach(chart => chart.destroy());
        this.charts.clear();
    }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
    module.exports = BenchmarkResultsPage;
}