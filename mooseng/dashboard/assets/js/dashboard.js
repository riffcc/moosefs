/**
 * MooseNG Dashboard JavaScript
 * Main application logic for the benchmark dashboard
 */

class MooseNGDashboard {
    constructor() {
        this.currentPage = 'overview';
        this.refreshInterval = null;
        this.charts = new Map();
        this.metrics = new Map();
        this.dataCache = new Map();
        
        this.init();
    }

    init() {
        this.setupEventListeners();
        this.loadInitialData();
        this.startAutoRefresh();
        this.hideLoading();
    }

    setupEventListeners() {
        // Navigation
        document.querySelectorAll('.nav-link').forEach(link => {
            link.addEventListener('click', (e) => {
                e.preventDefault();
                const page = e.target.dataset.page;
                this.switchPage(page);
            });
        });

        // Refresh button
        document.getElementById('refreshBtn')?.addEventListener('click', () => {
            this.refreshData();
        });

        // Modal close
        document.querySelector('.close')?.addEventListener('click', () => {
            this.hideModal();
        });

        // Window resize for chart responsiveness
        window.addEventListener('resize', () => {
            this.resizeCharts();
        });

        // Keyboard shortcuts
        document.addEventListener('keydown', (e) => {
            if (e.key === 'r' && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                this.refreshData();
            }
        });
    }

    switchPage(page) {
        // Update navigation
        document.querySelectorAll('.nav-link').forEach(link => {
            link.classList.remove('active');
        });
        document.querySelector(`[data-page="${page}"]`)?.classList.add('active');

        // Update page content
        document.querySelectorAll('.page-content').forEach(content => {
            content.classList.remove('active');
        });
        document.getElementById(`${page}-page`)?.classList.add('active');

        this.currentPage = page;
        this.loadPageData(page);
    }

    async loadInitialData() {
        this.showLoading();
        
        try {
            await Promise.all([
                this.loadOverviewData(),
                this.loadMasterServerData(),
                this.loadChunkServerData(),
                this.loadClientData(),
                this.loadMultiRegionData()
            ]);
            
            this.renderCurrentPage();
        } catch (error) {
            this.showError('Failed to load initial data: ' + error.message);
        } finally {
            this.hideLoading();
        }
    }

    async loadPageData(page) {
        this.showLoading();
        
        try {
            switch (page) {
                case 'overview':
                    await this.loadOverviewData();
                    this.renderOverviewPage();
                    break;
                case 'master-server':
                    await this.loadMasterServerData();
                    this.renderMasterServerPage();
                    break;
                case 'chunk-server':
                    await this.loadChunkServerData();
                    this.renderChunkServerPage();
                    break;
                case 'client':
                    await this.loadClientData();
                    this.renderClientPage();
                    break;
                case 'multiregion':
                    await this.loadMultiRegionData();
                    this.renderMultiRegionPage();
                    break;
            }
        } catch (error) {
            this.showError(`Failed to load ${page} data: ` + error.message);
        } finally {
            this.hideLoading();
        }
    }

    renderCurrentPage() {
        switch (this.currentPage) {
            case 'overview':
                this.renderOverviewPage();
                break;
            case 'master-server':
                this.renderMasterServerPage();
                break;
            case 'chunk-server':
                this.renderChunkServerPage();
                break;
            case 'client':
                this.renderClientPage();
                break;
            case 'multiregion':
                this.renderMultiRegionPage();
                break;
        }
    }

    // Data Loading Methods
    async loadOverviewData() {
        try {
            const response = await this.fetchBenchmarkData('overview');
            this.dataCache.set('overview', response);
            return response;
        } catch (error) {
            console.error('Error loading overview data:', error);
            // Return mock data for development
            return this.getMockOverviewData();
        }
    }

    async loadMasterServerData() {
        try {
            const response = await this.fetchBenchmarkData('master-server');
            this.dataCache.set('master-server', response);
            return response;
        } catch (error) {
            console.error('Error loading master server data:', error);
            return this.getMockMasterServerData();
        }
    }

    async loadChunkServerData() {
        try {
            const response = await this.fetchBenchmarkData('chunk-server');
            this.dataCache.set('chunk-server', response);
            return response;
        } catch (error) {
            console.error('Error loading chunk server data:', error);
            return this.getMockChunkServerData();
        }
    }

    async loadClientData() {
        try {
            const response = await this.fetchBenchmarkData('client');
            this.dataCache.set('client', response);
            return response;
        } catch (error) {
            console.error('Error loading client data:', error);
            return this.getMockClientData();
        }
    }

    async loadMultiRegionData() {
        try {
            const response = await this.fetchBenchmarkData('multiregion');
            this.dataCache.set('multiregion', response);
            return response;
        } catch (error) {
            console.error('Error loading multi-region data:', error);
            return this.getMockMultiRegionData();
        }
    }

    async fetchBenchmarkData(endpoint) {
        // In a real implementation, this would fetch from the Rust backend
        const response = await fetch(`/api/benchmarks/${endpoint}`);
        
        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }
        
        return await response.json();
    }

    // Page Rendering Methods
    renderOverviewPage() {
        const container = document.getElementById('overview-page');
        const data = this.dataCache.get('overview');
        
        container.innerHTML = `
            <div class="grid grid-cols-4 mb-6">
                <div class="metric-card">
                    <div class="metric-value">${data.totalOpsPerSecond.toLocaleString()}</div>
                    <div class="metric-label">Total Ops/Sec</div>
                    <div class="metric-change positive">+${data.totalOpsPerSecondChange}%</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">${data.totalThroughputMBps.toFixed(1)}</div>
                    <div class="metric-label">Throughput (MB/s)</div>
                    <div class="metric-change positive">+${data.totalThroughputChange}%</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">${data.avgLatencyMs.toFixed(1)}ms</div>
                    <div class="metric-label">Avg Latency</div>
                    <div class="metric-change negative">-${data.avgLatencyChange}%</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">${data.systemGrade}</div>
                    <div class="metric-label">Overall Grade</div>
                    <div class="metric-change neutral">-</div>
                </div>
            </div>
            
            <div class="grid grid-cols-2 mb-6">
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Performance Trends</h3>
                        <p class="card-subtitle">Last 24 hours</p>
                    </div>
                    <div class="chart-container">
                        <canvas id="performanceTrendChart"></canvas>
                    </div>
                </div>
                
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Component Health</h3>
                        <p class="card-subtitle">Current status</p>
                    </div>
                    <div class="chart-container">
                        <canvas id="componentHealthChart"></canvas>
                    </div>
                </div>
            </div>
            
            <div class="card">
                <div class="card-header">
                    <h3 class="card-title">Recent Benchmark Results</h3>
                    <p class="card-subtitle">Latest test runs</p>
                </div>
                <div id="recentResults"></div>
            </div>
        `;
        
        // Create charts
        this.createPerformanceTrendChart(data.performanceTrend);
        this.createComponentHealthChart(data.componentHealth);
        this.renderRecentResults(data.recentResults);
    }

    renderMasterServerPage() {
        const container = document.getElementById('master-server-page');
        const data = this.dataCache.get('master-server');
        
        container.innerHTML = `
            <div class="grid grid-cols-3 mb-6">
                <div class="metric-card">
                    <div class="metric-value">${data.raftConsensusTime.toFixed(1)}ms</div>
                    <div class="metric-label">Raft Consensus Time</div>
                    <div class="metric-change positive">-${data.raftTimeChange}%</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">${data.metadataOpsPerSec.toLocaleString()}</div>
                    <div class="metric-label">Metadata Ops/Sec</div>
                    <div class="metric-change positive">+${data.metadataOpsChange}%</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">${data.cacheHitRate.toFixed(1)}%</div>
                    <div class="metric-label">Cache Hit Rate</div>
                    <div class="metric-change positive">+${data.cacheHitRateChange}%</div>
                </div>
            </div>
            
            <div class="grid grid-cols-2 mb-6">
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Raft Performance</h3>
                    </div>
                    <div class="chart-container">
                        <canvas id="raftPerformanceChart"></canvas>
                    </div>
                </div>
                
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Metadata Operations</h3>
                    </div>
                    <div class="chart-container">
                        <canvas id="metadataOpsChart"></canvas>
                    </div>
                </div>
            </div>
        `;
        
        this.createRaftPerformanceChart(data.raftMetrics);
        this.createMetadataOpsChart(data.metadataMetrics);
    }

    renderChunkServerPage() {
        const container = document.getElementById('chunk-server-page');
        const data = this.dataCache.get('chunk-server');
        
        container.innerHTML = `
            <div class="grid grid-cols-4 mb-6">
                <div class="metric-card">
                    <div class="metric-value">${data.readThroughputMBps.toFixed(1)}</div>
                    <div class="metric-label">Read Throughput (MB/s)</div>
                    <div class="metric-change positive">+${data.readThroughputChange}%</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">${data.writeThroughputMBps.toFixed(1)}</div>
                    <div class="metric-label">Write Throughput (MB/s)</div>
                    <div class="metric-change positive">+${data.writeThroughputChange}%</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">${data.erasureCodingEfficiency.toFixed(1)}%</div>
                    <div class="metric-label">Erasure Coding Efficiency</div>
                    <div class="metric-change positive">+${data.erasureEfficiencyChange}%</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">${data.storageUtilization.toFixed(1)}%</div>
                    <div class="metric-label">Storage Utilization</div>
                    <div class="metric-change neutral">${data.storageUtilizationChange}%</div>
                </div>
            </div>
            
            <div class="grid grid-cols-2 mb-6">
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">I/O Performance</h3>
                    </div>
                    <div class="chart-container">
                        <canvas id="ioPerformanceChart"></canvas>
                    </div>
                </div>
                
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Erasure Coding Performance</h3>
                    </div>
                    <div class="chart-container">
                        <canvas id="erasureCodingChart"></canvas>
                    </div>
                </div>
            </div>
        `;
        
        this.createIOPerformanceChart(data.ioMetrics);
        this.createErasureCodingChart(data.erasureMetrics);
    }

    renderClientPage() {
        const container = document.getElementById('client-page');
        const data = this.dataCache.get('client');
        
        container.innerHTML = `
            <div class="grid grid-cols-3 mb-6">
                <div class="metric-card">
                    <div class="metric-value">${data.fuseLatencyMs.toFixed(1)}ms</div>
                    <div class="metric-label">FUSE Latency</div>
                    <div class="metric-change negative">-${data.fuseLatencyChange}%</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">${data.cacheHitRate.toFixed(1)}%</div>
                    <div class="metric-label">Client Cache Hit Rate</div>
                    <div class="metric-change positive">+${data.cacheHitRateChange}%</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">${data.concurrentConnections}</div>
                    <div class="metric-label">Concurrent Connections</div>
                    <div class="metric-change positive">+${data.connectionsChange}%</div>
                </div>
            </div>
            
            <div class="grid grid-cols-2 mb-6">
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Client Operations</h3>
                    </div>
                    <div class="chart-container">
                        <canvas id="clientOpsChart"></canvas>
                    </div>
                </div>
                
                <div class="card">
                    <div class="card-header">
                        <h3 class="card-title">Network Performance</h3>
                    </div>
                    <div class="chart-container">
                        <canvas id="networkPerformanceChart"></canvas>
                    </div>
                </div>
            </div>
        `;
        
        this.createClientOpsChart(data.clientMetrics);
        this.createNetworkPerformanceChart(data.networkMetrics);
    }

    renderMultiRegionPage() {
        const container = document.getElementById('multiregion-page');
        const data = this.dataCache.get('multiregion');
        
        container.innerHTML = `
            <div class="grid grid-cols-3 mb-6">
                <div class="metric-card">
                    <div class="metric-value">${data.crossRegionLatencyMs.toFixed(1)}ms</div>
                    <div class="metric-label">Cross-Region Latency</div>
                    <div class="metric-change negative">-${data.crossRegionLatencyChange}%</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">${data.replicationEfficiency.toFixed(1)}%</div>
                    <div class="metric-label">Replication Efficiency</div>
                    <div class="metric-change positive">+${data.replicationEfficiencyChange}%</div>
                </div>
                <div class="metric-card">
                    <div class="metric-value">${data.consistencyTime.toFixed(1)}ms</div>
                    <div class="metric-label">Consistency Time</div>
                    <div class="metric-change positive">-${data.consistencyTimeChange}%</div>
                </div>
            </div>
            
            <div class="card mb-6">
                <div class="card-header">
                    <h3 class="card-title">Multi-Region Performance</h3>
                </div>
                <div class="chart-container large">
                    <canvas id="multiRegionChart"></canvas>
                </div>
            </div>
        `;
        
        this.createMultiRegionChart(data.regionMetrics);
    }

    // Chart Creation Methods
    createPerformanceTrendChart(data) {
        const ctx = document.getElementById('performanceTrendChart')?.getContext('2d');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: data.timestamps,
                datasets: [
                    {
                        label: 'Throughput (MB/s)',
                        data: data.throughput,
                        borderColor: '#2563eb',
                        backgroundColor: 'rgba(37, 99, 235, 0.1)',
                        tension: 0.4
                    },
                    {
                        label: 'Latency (ms)',
                        data: data.latency,
                        borderColor: '#ef4444',
                        backgroundColor: 'rgba(239, 68, 68, 0.1)',
                        tension: 0.4,
                        yAxisID: 'y1'
                    }
                ]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
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
                    },
                }
            }
        });

        this.charts.set('performanceTrend', chart);
    }

    createComponentHealthChart(data) {
        const ctx = document.getElementById('componentHealthChart')?.getContext('2d');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'doughnut',
            data: {
                labels: data.labels,
                datasets: [{
                    data: data.values,
                    backgroundColor: [
                        '#10b981',
                        '#f59e0b',
                        '#ef4444',
                        '#6b7280'
                    ]
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

        this.charts.set('componentHealth', chart);
    }

    createRaftPerformanceChart(data) {
        const ctx = document.getElementById('raftPerformanceChart')?.getContext('2d');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: data.operations,
                datasets: [{
                    label: 'Response Time (ms)',
                    data: data.responseTimes,
                    backgroundColor: '#2563eb'
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false
            }
        });

        this.charts.set('raftPerformance', chart);
    }

    createMetadataOpsChart(data) {
        const ctx = document.getElementById('metadataOpsChart')?.getContext('2d');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: data.timestamps,
                datasets: [{
                    label: 'Operations/sec',
                    data: data.opsPerSecond,
                    borderColor: '#10b981',
                    backgroundColor: 'rgba(16, 185, 129, 0.1)',
                    tension: 0.4
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false
            }
        });

        this.charts.set('metadataOps', chart);
    }

    createIOPerformanceChart(data) {
        const ctx = document.getElementById('ioPerformanceChart')?.getContext('2d');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: data.timestamps,
                datasets: [
                    {
                        label: 'Read (MB/s)',
                        data: data.readThroughput,
                        borderColor: '#2563eb',
                        backgroundColor: 'rgba(37, 99, 235, 0.1)'
                    },
                    {
                        label: 'Write (MB/s)',
                        data: data.writeThroughput,
                        borderColor: '#ef4444',
                        backgroundColor: 'rgba(239, 68, 68, 0.1)'
                    }
                ]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false
            }
        });

        this.charts.set('ioPerformance', chart);
    }

    createErasureCodingChart(data) {
        const ctx = document.getElementById('erasureCodingChart')?.getContext('2d');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: data.configurations,
                datasets: [{
                    label: 'Efficiency (%)',
                    data: data.efficiencies,
                    backgroundColor: '#10b981'
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false
            }
        });

        this.charts.set('erasureCoding', chart);
    }

    createClientOpsChart(data) {
        const ctx = document.getElementById('clientOpsChart')?.getContext('2d');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: data.operations,
                datasets: [{
                    label: 'Operations/sec',
                    data: data.opsPerSecond,
                    backgroundColor: '#f59e0b'
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false
            }
        });

        this.charts.set('clientOps', chart);
    }

    createNetworkPerformanceChart(data) {
        const ctx = document.getElementById('networkPerformanceChart')?.getContext('2d');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: data.timestamps,
                datasets: [{
                    label: 'Network Throughput (MB/s)',
                    data: data.throughput,
                    borderColor: '#8b5cf6',
                    backgroundColor: 'rgba(139, 92, 246, 0.1)',
                    tension: 0.4
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false
            }
        });

        this.charts.set('networkPerformance', chart);
    }

    createMultiRegionChart(data) {
        const ctx = document.getElementById('multiRegionChart')?.getContext('2d');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'scatter',
            data: {
                datasets: data.regions.map((region, index) => ({
                    label: region.name,
                    data: region.coordinates,
                    backgroundColor: [
                        '#2563eb',
                        '#ef4444',
                        '#10b981'
                    ][index]
                }))
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    x: {
                        title: {
                            display: true,
                            text: 'Latency (ms)'
                        }
                    },
                    y: {
                        title: {
                            display: true,
                            text: 'Throughput (MB/s)'
                        }
                    }
                }
            }
        });

        this.charts.set('multiRegion', chart);
    }

    renderRecentResults(results) {
        const container = document.getElementById('recentResults');
        
        const html = results.map(result => `
            <div class="grid grid-cols-6 p-3 border-b border-gray-200">
                <div class="text-sm font-medium">${new Date(result.timestamp).toLocaleString()}</div>
                <div class="text-sm">${result.test}</div>
                <div class="text-sm">${result.duration}ms</div>
                <div class="text-sm">${result.throughput} MB/s</div>
                <div class="text-sm">${result.opsPerSec} ops/s</div>
                <div class="text-sm">
                    <span class="inline-flex px-2 py-1 text-xs font-semibold rounded-full ${
                        result.status === 'passed' ? 'bg-green-100 text-green-800' : 
                        result.status === 'failed' ? 'bg-red-100 text-red-800' : 
                        'bg-yellow-100 text-yellow-800'
                    }">
                        ${result.status}
                    </span>
                </div>
            </div>
        `).join('');
        
        container.innerHTML = `
            <div class="grid grid-cols-6 p-3 bg-gray-50 text-sm font-semibold">
                <div>Timestamp</div>
                <div>Test</div>
                <div>Duration</div>
                <div>Throughput</div>
                <div>Ops/Sec</div>
                <div>Status</div>
            </div>
            ${html}
        `;
    }

    // Utility Methods
    resizeCharts() {
        this.charts.forEach(chart => {
            chart.resize();
        });
    }

    async refreshData() {
        await this.loadPageData(this.currentPage);
        this.updateStatusIndicator();
    }

    startAutoRefresh() {
        this.refreshInterval = setInterval(() => {
            this.refreshData();
        }, 30000); // Refresh every 30 seconds
    }

    stopAutoRefresh() {
        if (this.refreshInterval) {
            clearInterval(this.refreshInterval);
            this.refreshInterval = null;
        }
    }

    updateStatusIndicator() {
        const indicator = document.getElementById('statusIndicator');
        const dot = indicator?.querySelector('.status-dot');
        const text = indicator?.querySelector('.status-text');
        
        if (dot && text) {
            dot.style.backgroundColor = '#10b981';
            text.textContent = 'Live';
        }
    }

    showLoading() {
        document.getElementById('loadingIndicator')?.classList.remove('hidden');
    }

    hideLoading() {
        document.getElementById('loadingIndicator')?.classList.add('hidden');
    }

    showError(message) {
        const modal = document.getElementById('errorModal');
        const messageElement = document.getElementById('errorMessage');
        
        if (modal && messageElement) {
            messageElement.textContent = message;
            modal.style.display = 'block';
        }
    }

    hideModal() {
        const modal = document.getElementById('errorModal');
        if (modal) {
            modal.style.display = 'none';
        }
    }

    // Mock Data Methods (for development)
    getMockOverviewData() {
        return {
            totalOpsPerSecond: 1506,
            totalOpsPerSecondChange: 12.5,
            totalThroughputMBps: 135.13,
            totalThroughputChange: 8.3,
            avgLatencyMs: 45.2,
            avgLatencyChange: 15.1,
            systemGrade: 'A',
            performanceTrend: {
                timestamps: ['12:00', '12:05', '12:10', '12:15', '12:20', '12:25'],
                throughput: [120, 125, 130, 135, 138, 135],
                latency: [50, 48, 45, 43, 45, 45]
            },
            componentHealth: {
                labels: ['Healthy', 'Warning', 'Error', 'Offline'],
                values: [85, 10, 3, 2]
            },
            recentResults: [
                {
                    timestamp: new Date().toISOString(),
                    test: 'File Operations',
                    duration: 196,
                    throughput: 135.13,
                    opsPerSec: 1506,
                    status: 'passed'
                },
                {
                    timestamp: new Date(Date.now() - 60000).toISOString(),
                    test: 'Network Test',
                    duration: 1782,
                    throughput: 98.4,
                    opsPerSec: 892,
                    status: 'passed'
                }
            ]
        };
    }

    getMockMasterServerData() {
        return {
            raftConsensusTime: 12.5,
            raftTimeChange: 8.2,
            metadataOpsPerSec: 2450,
            metadataOpsChange: 15.6,
            cacheHitRate: 94.2,
            cacheHitRateChange: 2.1,
            raftMetrics: {
                operations: ['Leader Election', 'Log Replication', 'Heartbeat', 'Snapshot'],
                responseTimes: [45, 12, 5, 250]
            },
            metadataMetrics: {
                timestamps: ['12:00', '12:05', '12:10', '12:15', '12:20', '12:25'],
                opsPerSecond: [2200, 2300, 2400, 2450, 2500, 2450]
            }
        };
    }

    getMockChunkServerData() {
        return {
            readThroughputMBps: 450.2,
            readThroughputChange: 12.3,
            writeThroughputMBps: 380.5,
            writeThroughputChange: 8.7,
            erasureCodingEfficiency: 96.8,
            erasureEfficiencyChange: 1.2,
            storageUtilization: 72.5,
            storageUtilizationChange: 0,
            ioMetrics: {
                timestamps: ['12:00', '12:05', '12:10', '12:15', '12:20', '12:25'],
                readThroughput: [420, 435, 445, 450, 455, 450],
                writeThroughput: [360, 370, 375, 380, 385, 380]
            },
            erasureMetrics: {
                configurations: ['4+2', '8+4', '16+4'],
                efficiencies: [94.2, 96.8, 98.1]
            }
        };
    }

    getMockClientData() {
        return {
            fuseLatencyMs: 8.4,
            fuseLatencyChange: 12.1,
            cacheHitRate: 89.3,
            cacheHitRateChange: 4.2,
            concurrentConnections: 125,
            connectionsChange: 8.5,
            clientMetrics: {
                operations: ['Read', 'Write', 'Stat', 'Create', 'Delete'],
                opsPerSecond: [1200, 800, 450, 200, 150]
            },
            networkMetrics: {
                timestamps: ['12:00', '12:05', '12:10', '12:15', '12:20', '12:25'],
                throughput: [100, 105, 110, 115, 112, 108]
            }
        };
    }

    getMockMultiRegionData() {
        return {
            crossRegionLatencyMs: 85.3,
            crossRegionLatencyChange: 8.1,
            replicationEfficiency: 98.2,
            replicationEfficiencyChange: 1.5,
            consistencyTime: 125.4,
            consistencyTimeChange: 12.3,
            regionMetrics: {
                regions: [
                    {
                        name: 'US-East',
                        coordinates: [{x: 10, y: 120}, {x: 15, y: 115}, {x: 12, y: 125}]
                    },
                    {
                        name: 'EU-West',
                        coordinates: [{x: 85, y: 95}, {x: 90, y: 100}, {x: 88, y: 92}]
                    },
                    {
                        name: 'AP-South',
                        coordinates: [{x: 150, y: 80}, {x: 155, y: 85}, {x: 148, y: 78}]
                    }
                ]
            }
        };
    }
}

// Initialize dashboard when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    window.dashboard = new MooseNGDashboard();
});

// Export for testing
if (typeof module !== 'undefined' && module.exports) {
    module.exports = MooseNGDashboard;
}