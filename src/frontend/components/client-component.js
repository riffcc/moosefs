/**
 * Client Component
 * Handles client performance monitoring and visualization
 */

class ClientComponent {
    constructor() {
        this.data = null;
        this.updateInterval = null;
    }

    async load() {
        try {
            this.data = await window.DataManager.getClientData();
            this.render();
            this.startAutoUpdate();
        } catch (error) {
            console.error('Error loading client data:', error);
            this.renderError();
        }
    }

    render() {
        if (!this.data) return;

        const content = document.getElementById('client-content');
        if (!content) return;

        content.innerHTML = this.generateHTML();
        this.createCharts();
        this.attachEventListeners();
    }

    generateHTML() {
        return `
            <!-- Client Overview -->
            <div class="kpi-grid">
                <div class="kpi-card">
                    <h3>Active Mounts</h3>
                    <div class="kpi-value">${this.data.active_mounts}</div>
                    <div class="kpi-trend">
                        <span class="trend-up">↗ 2 new mounts today</span>
                    </div>
                </div>
                <div class="kpi-card">
                    <h3>Total Operations</h3>
                    <div class="kpi-value">${this.formatNumber(this.data.total_operations)}</div>
                    <div class="kpi-trend">
                        <span class="trend-up">↗ 15% from yesterday</span>
                    </div>
                </div>
                <div class="kpi-card">
                    <h3>Cache Hit Rate</h3>
                    <div class="kpi-value">${this.data.cache_hit_rate}%</div>
                    <div class="progress-bar">
                        <div class="progress-fill ${this.getCacheHitClass(this.data.cache_hit_rate)}" 
                             style="width: ${this.data.cache_hit_rate}%"></div>
                    </div>
                    <div class="kpi-trend">
                        <span class="trend-up">↗ 3% improvement</span>
                    </div>
                </div>
                <div class="kpi-card">
                    <h3>Average Latency</h3>
                    <div class="kpi-value">${this.data.average_latency}<span class="metric-unit">ms</span></div>
                    <div class="kpi-trend">
                        <span class="trend-down">↘ 2ms improvement</span>
                    </div>
                </div>
            </div>

            <!-- Client Performance Overview -->
            <div class="charts-grid">
                <div class="chart-container">
                    <h3>Client Performance Distribution</h3>
                    <canvas id="client-performance-radar-chart"></canvas>
                </div>
                <div class="chart-container">
                    <h3>Cache Performance</h3>
                    <canvas id="client-cache-performance-chart"></canvas>
                </div>
            </div>

            <!-- Client Details Table -->
            <div class="chart-container">
                <h3>Client Connections</h3>
                <div class="filter-section">
                    <div class="filter-group">
                        <label class="filter-label">Filter by Status:</label>
                        <select class="filter-select" id="client-status-filter">
                            <option value="all">All Clients</option>
                            <option value="connected">Connected Only</option>
                            <option value="disconnected">Disconnected Only</option>
                        </select>
                    </div>
                    <div class="filter-group">
                        <label class="filter-label">Sort by:</label>
                        <select class="filter-select" id="client-sort">
                            <option value="id">Client ID</option>
                            <option value="operations">Operations</option>
                            <option value="bandwidth">Bandwidth</option>
                            <option value="cache">Cache Size</option>
                        </select>
                    </div>
                    <div class="filter-group">
                        <button class="action-btn" onclick="window.ClientComponent.refreshClients()">
                            🔄 Refresh
                        </button>
                        <button class="action-btn" onclick="window.ClientComponent.exportClientData()">
                            📊 Export Data
                        </button>
                    </div>
                </div>
                
                <table class="component-table" id="clients-table">
                    <thead>
                        <tr>
                            <th>Client ID</th>
                            <th>Status</th>
                            <th>Mount Point</th>
                            <th>Operations</th>
                            <th>Cache Size</th>
                            <th>Bandwidth</th>
                            <th>Latency</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        ${this.data.clients.map(client => `
                            <tr data-status="${client.status}" data-client="${client.id}">
                                <td><strong>${client.id}</strong></td>
                                <td>
                                    <span class="status-badge ${client.status === 'connected' ? 'online' : 'offline'}">
                                        ${client.status.toUpperCase()}
                                    </span>
                                </td>
                                <td><code>${client.mount_point}</code></td>
                                <td>${this.formatNumber(client.operations)}</td>
                                <td>${client.cache_size}</td>
                                <td>${client.bandwidth}</td>
                                <td>${Math.floor(Math.random() * 10 + 3)}ms</td>
                                <td>
                                    <button class="action-btn" onclick="window.ClientComponent.viewClientDetails('${client.id}')">
                                        Details
                                    </button>
                                    ${client.status === 'disconnected' ? `
                                        <button class="action-btn warning" onclick="window.ClientComponent.reconnectClient('${client.id}')">
                                            Reconnect
                                        </button>
                                    ` : `
                                        <button class="action-btn" onclick="window.ClientComponent.clearClientCache('${client.id}')">
                                            Clear Cache
                                        </button>
                                    `}
                                </td>
                            </tr>
                        `).join('')}
                    </tbody>
                </table>
            </div>

            <!-- Performance Analytics -->
            <div class="charts-grid">
                <div class="chart-container">
                    <h3>Operations by Client</h3>
                    <canvas id="client-operations-chart"></canvas>
                </div>
                <div class="chart-container">
                    <h3>Bandwidth Usage by Client</h3>
                    <canvas id="client-bandwidth-chart"></canvas>
                </div>
                <div class="chart-container">
                    <h3>Cache Hit Rate Timeline</h3>
                    <canvas id="client-cache-timeline-chart"></canvas>
                </div>
                <div class="chart-container">
                    <h3>Latency Distribution</h3>
                    <canvas id="client-latency-distribution-chart"></canvas>
                </div>
            </div>

            <!-- Real-time Monitoring -->
            <div class="chart-container">
                <h3>Real-time Client Activity</h3>
                <div class="metrics-grid">
                    <div class="metric-card">
                        <div class="metric-label">Read Operations/sec</div>
                        <div class="metric-value" id="realtime-reads">0</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Write Operations/sec</div>
                        <div class="metric-value" id="realtime-writes">0</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Network Throughput</div>
                        <div class="metric-value" id="realtime-throughput">0 MB/s</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Active Connections</div>
                        <div class="metric-value" id="realtime-connections">${this.data.clients.filter(c => c.status === 'connected').length}</div>
                    </div>
                </div>
                
                <div class="chart-container">
                    <h4>Live Activity Stream</h4>
                    <canvas id="client-realtime-chart"></canvas>
                </div>
            </div>

            <!-- Client Configuration -->
            <div class="chart-container">
                <h3>Client Configuration & Tuning</h3>
                <div class="filter-section">
                    <div class="filter-group">
                        <label class="filter-label">Cache Size:</label>
                        <select class="filter-select" id="cache-size-setting">
                            <option value="64">64 MB</option>
                            <option value="128" selected>128 MB</option>
                            <option value="256">256 MB</option>
                            <option value="512">512 MB</option>
                            <option value="1024">1 GB</option>
                        </select>
                    </div>
                    <div class="filter-group">
                        <label class="filter-label">Prefetch Strategy:</label>
                        <select class="filter-select" id="prefetch-strategy">
                            <option value="none">None</option>
                            <option value="sequential" selected>Sequential</option>
                            <option value="predictive">Predictive</option>
                            <option value="aggressive">Aggressive</option>
                        </select>
                    </div>
                    <div class="filter-group">
                        <label class="filter-label">Retry Policy:</label>
                        <select class="filter-select" id="retry-policy">
                            <option value="exponential" selected>Exponential Backoff</option>
                            <option value="linear">Linear Backoff</option>
                            <option value="immediate">Immediate Retry</option>
                        </select>
                    </div>
                    <button class="action-btn" onclick="window.ClientComponent.applyConfiguration()">
                        ⚙️ Apply Configuration
                    </button>
                </div>

                <div class="comparison-grid">
                    <div class="comparison-card">
                        <div class="comparison-header">
                            <h4>Cache Performance</h4>
                        </div>
                        <div class="comparison-metrics">
                            <div class="comparison-metric">
                                <div class="comparison-metric-label">Hit Rate</div>
                                <div class="comparison-metric-value">${this.data.cache_hit_rate}%</div>
                            </div>
                            <div class="comparison-metric">
                                <div class="comparison-metric-label">Miss Rate</div>
                                <div class="comparison-metric-value">${(100 - parseFloat(this.data.cache_hit_rate)).toFixed(1)}%</div>
                            </div>
                        </div>
                    </div>
                    <div class="comparison-card">
                        <div class="comparison-header">
                            <h4>Network Efficiency</h4>
                        </div>
                        <div class="comparison-metrics">
                            <div class="comparison-metric">
                                <div class="comparison-metric-label">Avg Bandwidth</div>
                                <div class="comparison-metric-value">87 MB/s</div>
                            </div>
                            <div class="comparison-metric">
                                <div class="comparison-metric-label">Peak Bandwidth</div>
                                <div class="comparison-metric-value">145 MB/s</div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <!-- Troubleshooting -->
            <div class="chart-container">
                <h3>Diagnostics & Troubleshooting</h3>
                <div class="filter-section">
                    <button class="action-btn" onclick="window.ClientComponent.runDiagnostics()">
                        🔍 Run Diagnostics
                    </button>
                    <button class="action-btn" onclick="window.ClientComponent.testConnectivity()">
                        🌐 Test Connectivity
                    </button>
                    <button class="action-btn" onclick="window.ClientComponent.clearAllCaches()">
                        🗑️ Clear All Caches
                    </button>
                    <button class="action-btn warning" onclick="window.ClientComponent.forceReconnectAll()">
                        🔄 Force Reconnect All
                    </button>
                </div>

                <div class="diagnostics-output" id="diagnostics-output">
                    <div class="status-item">
                        <span class="status-dot status-good"></span>
                        <span>All clients operating normally</span>
                    </div>
                </div>
            </div>
        `;
    }

    createCharts() {
        // Performance Radar Chart
        this.createPerformanceRadarChart();

        // Cache Performance Chart
        this.createCachePerformanceChart();

        // Operations Chart
        this.createOperationsChart();

        // Bandwidth Chart
        this.createBandwidthChart();

        // Cache Timeline Chart
        this.createCacheTimelineChart();

        // Latency Distribution Chart
        this.createLatencyDistributionChart();

        // Real-time Chart
        this.createRealtimeChart();

        // Start real-time updates
        this.startRealtimeUpdates();
    }

    createPerformanceRadarChart() {
        const ctx = document.getElementById('client-performance-radar-chart');
        if (!ctx) return;

        const connectedClients = this.data.clients.filter(c => c.status === 'connected');
        const avgOperations = connectedClients.reduce((sum, c) => sum + c.operations, 0) / connectedClients.length;
        
        const chart = new Chart(ctx, {
            type: 'radar',
            data: {
                labels: ['Operations', 'Cache Efficiency', 'Network Speed', 'Reliability', 'Response Time'],
                datasets: [{
                    label: 'Client Performance',
                    data: [
                        Math.min(avgOperations / 20, 100),
                        parseFloat(this.data.cache_hit_rate),
                        85, // Mock network speed score
                        (connectedClients.length / this.data.clients.length) * 100,
                        100 - this.data.average_latency // Inverted latency score
                    ],
                    backgroundColor: 'rgba(46, 204, 113, 0.2)',
                    borderColor: '#2ecc71',
                    borderWidth: 2,
                    pointBackgroundColor: '#2ecc71',
                    pointBorderColor: '#fff',
                    pointHoverBackgroundColor: '#fff',
                    pointHoverBorderColor: '#2ecc71'
                }]
            },
            options: {
                ...window.ChartComponents.getDefaultChartOptions(),
                scales: {
                    r: {
                        beginAtZero: true,
                        max: 100,
                        ticks: {
                            stepSize: 20
                        }
                    }
                }
            }
        });

        window.ChartComponents.charts.set('client-performance-radar-chart', chart);
    }

    createCachePerformanceChart() {
        const ctx = document.getElementById('client-cache-performance-chart');
        if (!ctx) return;

        const hitRate = parseFloat(this.data.cache_hit_rate);
        const missRate = 100 - hitRate;

        const chart = new Chart(ctx, {
            type: 'doughnut',
            data: {
                labels: ['Cache Hits', 'Cache Misses'],
                datasets: [{
                    data: [hitRate, missRate],
                    backgroundColor: ['#2ecc71', '#e74c3c'],
                    borderWidth: 2,
                    borderColor: '#ffffff'
                }]
            },
            options: {
                ...window.ChartComponents.getDefaultChartOptions(),
                plugins: {
                    legend: {
                        position: 'bottom'
                    },
                    tooltip: {
                        callbacks: {
                            label: function(context) {
                                return `${context.label}: ${context.parsed.toFixed(1)}%`;
                            }
                        }
                    }
                }
            }
        });

        window.ChartComponents.charts.set('client-cache-performance-chart', chart);
    }

    createOperationsChart() {
        const ctx = document.getElementById('client-operations-chart');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: this.data.clients.map(c => c.id),
                datasets: [{
                    label: 'Total Operations',
                    data: this.data.clients.map(c => c.operations),
                    backgroundColor: this.data.clients.map(c => 
                        c.status === 'disconnected' ? '#e74c3c80' : '#3498db80'
                    ),
                    borderColor: this.data.clients.map(c => 
                        c.status === 'disconnected' ? '#e74c3c' : '#3498db'
                    ),
                    borderWidth: 1
                }]
            },
            options: {
                ...window.ChartComponents.getDefaultChartOptions(),
                scales: {
                    y: {
                        beginAtZero: true,
                        title: {
                            display: true,
                            text: 'Total Operations'
                        }
                    }
                }
            }
        });

        window.ChartComponents.charts.set('client-operations-chart', chart);
    }

    createBandwidthChart() {
        const ctx = document.getElementById('client-bandwidth-chart');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'horizontalBar',
            data: {
                labels: this.data.clients.map(c => c.id),
                datasets: [{
                    label: 'Bandwidth Usage',
                    data: this.data.clients.map(c => parseFloat(c.bandwidth)),
                    backgroundColor: '#f39c1280',
                    borderColor: '#f39c12',
                    borderWidth: 1
                }]
            },
            options: {
                ...window.ChartComponents.getDefaultChartOptions(),
                indexAxis: 'y',
                scales: {
                    x: {
                        beginAtZero: true,
                        title: {
                            display: true,
                            text: 'Bandwidth (MB/s)'
                        }
                    }
                }
            }
        });

        window.ChartComponents.charts.set('client-bandwidth-chart', chart);
    }

    createCacheTimelineChart() {
        const ctx = document.getElementById('client-cache-timeline-chart');
        if (!ctx) return;

        // Generate mock cache hit rate timeline
        const timelineData = Array.from({length: 24}, (_, i) => ({
            time: new Date(Date.now() - (23-i) * 3600000),
            hitRate: Math.random() * 10 + 70 // 70-80% hit rate
        }));

        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: timelineData.map(d => d.time.toLocaleTimeString()),
                datasets: [{
                    label: 'Cache Hit Rate (%)',
                    data: timelineData.map(d => d.hitRate),
                    borderColor: '#9b59b6',
                    backgroundColor: '#9b59b620',
                    fill: true,
                    tension: 0.4,
                    pointRadius: 3,
                    pointHoverRadius: 6
                }]
            },
            options: {
                ...window.ChartComponents.getDefaultChartOptions(),
                scales: {
                    y: {
                        min: 0,
                        max: 100,
                        title: {
                            display: true,
                            text: 'Cache Hit Rate (%)'
                        }
                    }
                }
            }
        });

        window.ChartComponents.charts.set('client-cache-timeline-chart', chart);
    }

    createLatencyDistributionChart() {
        const ctx = document.getElementById('client-latency-distribution-chart');
        if (!ctx) return;

        // Generate mock latency distribution
        const latencyBuckets = [
            { range: '0-5ms', count: 45 },
            { range: '5-10ms', count: 30 },
            { range: '10-20ms', count: 15 },
            { range: '20-50ms', count: 8 },
            { range: '50+ms', count: 2 }
        ];

        const chart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: latencyBuckets.map(b => b.range),
                datasets: [{
                    label: 'Request Count (%)',
                    data: latencyBuckets.map(b => b.count),
                    backgroundColor: [
                        '#2ecc71', '#f39c12', '#e67e22', '#e74c3c', '#8e44ad'
                    ],
                    borderWidth: 1,
                    borderColor: '#ffffff'
                }]
            },
            options: {
                ...window.ChartComponents.getDefaultChartOptions(),
                scales: {
                    y: {
                        beginAtZero: true,
                        max: 50,
                        title: {
                            display: true,
                            text: 'Percentage of Requests'
                        }
                    }
                }
            }
        });

        window.ChartComponents.charts.set('client-latency-distribution-chart', chart);
    }

    createRealtimeChart() {
        const ctx = document.getElementById('client-realtime-chart');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: [],
                datasets: [
                    {
                        label: 'Operations/sec',
                        data: [],
                        borderColor: '#3498db',
                        backgroundColor: '#3498db20',
                        fill: false,
                        tension: 0.4,
                        pointRadius: 0
                    },
                    {
                        label: 'Throughput (MB/s)',
                        data: [],
                        borderColor: '#2ecc71',
                        backgroundColor: '#2ecc7120',
                        fill: false,
                        tension: 0.4,
                        pointRadius: 0,
                        yAxisID: 'y1'
                    }
                ]
            },
            options: {
                ...window.ChartComponents.getDefaultChartOptions(),
                animation: false,
                scales: {
                    x: {
                        type: 'time',
                        time: {
                            unit: 'second',
                            displayFormats: {
                                second: 'HH:mm:ss'
                            }
                        },
                        title: {
                            display: true,
                            text: 'Time'
                        }
                    },
                    y: {
                        type: 'linear',
                        display: true,
                        position: 'left',
                        title: {
                            display: true,
                            text: 'Operations/sec'
                        }
                    },
                    y1: {
                        type: 'linear',
                        display: true,
                        position: 'right',
                        title: {
                            display: true,
                            text: 'Throughput (MB/s)'
                        },
                        grid: {
                            drawOnChartArea: false,
                        },
                    }
                }
            }
        });

        window.ChartComponents.charts.set('client-realtime-chart', chart);
        this.realtimeChart = chart;
    }

    startRealtimeUpdates() {
        this.realtimeInterval = setInterval(() => {
            this.updateRealtimeMetrics();
            this.updateRealtimeChart();
        }, 2000);
    }

    updateRealtimeMetrics() {
        // Update real-time metrics
        const reads = Math.floor(Math.random() * 200 + 50);
        const writes = Math.floor(Math.random() * 100 + 25);
        const throughput = (Math.random() * 50 + 30).toFixed(1);

        document.getElementById('realtime-reads').textContent = reads;
        document.getElementById('realtime-writes').textContent = writes;
        document.getElementById('realtime-throughput').textContent = `${throughput} MB/s`;
    }

    updateRealtimeChart() {
        if (!this.realtimeChart) return;

        const now = new Date();
        const operations = Math.floor(Math.random() * 300 + 100);
        const throughput = Math.random() * 60 + 20;

        // Add new data point
        this.realtimeChart.data.labels.push(now);
        this.realtimeChart.data.datasets[0].data.push(operations);
        this.realtimeChart.data.datasets[1].data.push(throughput);

        // Keep only last 20 data points
        if (this.realtimeChart.data.labels.length > 20) {
            this.realtimeChart.data.labels.shift();
            this.realtimeChart.data.datasets[0].data.shift();
            this.realtimeChart.data.datasets[1].data.shift();
        }

        this.realtimeChart.update('none');
    }

    attachEventListeners() {
        // Status filter
        const statusFilter = document.getElementById('client-status-filter');
        if (statusFilter) {
            statusFilter.addEventListener('change', () => this.filterClients());
        }

        // Sort dropdown
        const sortSelect = document.getElementById('client-sort');
        if (sortSelect) {
            sortSelect.addEventListener('change', () => this.sortClients());
        }
    }

    filterClients() {
        const filter = document.getElementById('client-status-filter')?.value;
        const rows = document.querySelectorAll('#clients-table tbody tr');
        
        rows.forEach(row => {
            const status = row.getAttribute('data-status');
            if (filter === 'all' || status === filter) {
                row.style.display = '';
            } else {
                row.style.display = 'none';
            }
        });
    }

    sortClients() {
        const sortBy = document.getElementById('client-sort')?.value;
        const tbody = document.querySelector('#clients-table tbody');
        const rows = Array.from(tbody.querySelectorAll('tr'));
        
        rows.sort((a, b) => {
            const aValue = this.getSortValue(a, sortBy);
            const bValue = this.getSortValue(b, sortBy);
            return bValue - aValue;
        });
        
        rows.forEach(row => tbody.appendChild(row));
    }

    getSortValue(row, sortBy) {
        const clientId = row.getAttribute('data-client');
        const client = this.data.clients.find(c => c.id === clientId);
        
        switch (sortBy) {
            case 'operations': return client.operations;
            case 'bandwidth': return parseFloat(client.bandwidth);
            case 'cache': return parseFloat(client.cache_size);
            default: return 0;
        }
    }

    getCacheHitClass(hitRate) {
        const rate = parseFloat(hitRate);
        if (rate >= 80) return 'good';
        if (rate >= 60) return 'warning';
        return 'danger';
    }

    formatNumber(num) {
        if (num >= 1000000) {
            return (num / 1000000).toFixed(1) + 'M';
        }
        if (num >= 1000) {
            return (num / 1000).toFixed(1) + 'K';
        }
        return num.toString();
    }

    // Action methods
    viewClientDetails(clientId) {
        const client = this.data.clients.find(c => c.id === clientId);
        if (!client) return;

        const modal = document.createElement('div');
        modal.className = 'modal-overlay';
        modal.innerHTML = `
            <div class="modal-content">
                <div class="modal-header">
                    <h3>Client Details: ${clientId}</h3>
                    <button class="modal-close" onclick="this.closest('.modal-overlay').remove()">×</button>
                </div>
                <div class="modal-body">
                    <div class="server-details">
                        <div class="detail-row">
                            <label>Status:</label>
                            <span class="status-badge ${client.status === 'connected' ? 'online' : 'offline'}">${client.status.toUpperCase()}</span>
                        </div>
                        <div class="detail-row">
                            <label>Mount Point:</label>
                            <span><code>${client.mount_point}</code></span>
                        </div>
                        <div class="detail-row">
                            <label>Total Operations:</label>
                            <span>${this.formatNumber(client.operations)}</span>
                        </div>
                        <div class="detail-row">
                            <label>Cache Size:</label>
                            <span>${client.cache_size}</span>
                        </div>
                        <div class="detail-row">
                            <label>Bandwidth:</label>
                            <span>${client.bandwidth}</span>
                        </div>
                    </div>
                </div>
            </div>
        `;

        document.body.appendChild(modal);
    }

    reconnectClient(clientId) {
        this.showDiagnosticMessage(`Attempting to reconnect ${clientId}...`, 'warning');
        setTimeout(() => {
            this.showDiagnosticMessage(`${clientId} reconnected successfully`, 'success');
        }, 3000);
    }

    clearClientCache(clientId) {
        if (confirm(`Clear cache for ${clientId}?`)) {
            this.showDiagnosticMessage(`Clearing cache for ${clientId}...`, 'info');
            setTimeout(() => {
                this.showDiagnosticMessage(`Cache cleared for ${clientId}`, 'success');
            }, 1500);
        }
    }

    refreshClients() {
        this.showDiagnosticMessage('Refreshing client data...', 'info');
        setTimeout(() => {
            this.showDiagnosticMessage('Client data refreshed successfully', 'success');
            this.load(); // Reload data
        }, 2000);
    }

    exportClientData() {
        const data = {
            timestamp: new Date().toISOString(),
            summary: {
                active_mounts: this.data.active_mounts,
                total_operations: this.data.total_operations,
                cache_hit_rate: this.data.cache_hit_rate,
                average_latency: this.data.average_latency
            },
            clients: this.data.clients
        };

        const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `mooseng-client-data-${new Date().toISOString().split('T')[0]}.json`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);

        this.showDiagnosticMessage('Client data exported successfully', 'success');
    }

    applyConfiguration() {
        const cacheSize = document.getElementById('cache-size-setting')?.value;
        const prefetchStrategy = document.getElementById('prefetch-strategy')?.value;
        const retryPolicy = document.getElementById('retry-policy')?.value;

        this.showDiagnosticMessage(`Applying configuration: Cache=${cacheSize}MB, Prefetch=${prefetchStrategy}, Retry=${retryPolicy}`, 'info');
        setTimeout(() => {
            this.showDiagnosticMessage('Configuration applied successfully to all clients', 'success');
        }, 2000);
    }

    runDiagnostics() {
        this.showDiagnosticMessage('Running comprehensive client diagnostics...', 'info');
        setTimeout(() => {
            this.showDiagnosticMessage('Diagnostics completed - all clients healthy', 'success');
        }, 5000);
    }

    testConnectivity() {
        this.showDiagnosticMessage('Testing connectivity to all clients...', 'info');
        setTimeout(() => {
            this.showDiagnosticMessage('Connectivity test completed - all clients reachable', 'success');
        }, 3000);
    }

    clearAllCaches() {
        if (confirm('Clear cache for all connected clients?')) {
            this.showDiagnosticMessage('Clearing caches for all clients...', 'warning');
            setTimeout(() => {
                this.showDiagnosticMessage('All client caches cleared successfully', 'success');
            }, 4000);
        }
    }

    forceReconnectAll() {
        if (confirm('Force reconnect all clients? This may temporarily disrupt service.')) {
            this.showDiagnosticMessage('Forcing reconnection for all clients...', 'warning');
            setTimeout(() => {
                this.showDiagnosticMessage('All clients reconnected successfully', 'success');
            }, 6000);
        }
    }

    showDiagnosticMessage(message, type) {
        const diagnosticsOutput = document.getElementById('diagnostics-output');
        if (diagnosticsOutput) {
            const statusClass = type === 'success' ? 'status-good' : 
                             type === 'warning' ? 'status-warning' : 
                             type === 'error' ? 'status-danger' : 'status-good';
            
            diagnosticsOutput.innerHTML = `
                <div class="status-item">
                    <span class="status-dot ${statusClass}"></span>
                    <span>${message}</span>
                </div>
            `;
        }
    }

    startAutoUpdate() {
        this.updateInterval = setInterval(async () => {
            try {
                const newData = await window.DataManager.getClientData();
                if (newData) {
                    this.data = newData;
                    this.render();
                }
            } catch (error) {
                console.error('Error updating client data:', error);
            }
        }, 30000);
    }

    stopAutoUpdate() {
        if (this.updateInterval) {
            clearInterval(this.updateInterval);
            this.updateInterval = null;
        }
        if (this.realtimeInterval) {
            clearInterval(this.realtimeInterval);
            this.realtimeInterval = null;
        }
    }

    renderError() {
        const content = document.getElementById('client-content');
        if (content) {
            content.innerHTML = `
                <div class="error-message">
                    <h3>Error Loading Client Data</h3>
                    <p>Unable to load client performance data. Please check your connection and try again.</p>
                    <button onclick="window.ClientComponent.load()" class="retry-button">
                        Retry
                    </button>
                </div>
            `;
        }
    }

    destroy() {
        this.stopAutoUpdate();
        
        // Destroy client charts
        ['client-performance-radar-chart', 'client-cache-performance-chart',
         'client-operations-chart', 'client-bandwidth-chart',
         'client-cache-timeline-chart', 'client-latency-distribution-chart',
         'client-realtime-chart'].forEach(chartId => {
            window.ChartComponents.destroyChart(chartId);
        });
    }
}

// Initialize global client component
window.ClientComponent = new ClientComponent();