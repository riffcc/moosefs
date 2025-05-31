/**
 * Chunk Server Component
 * Handles chunk server performance monitoring and visualization
 */

class ChunkServerComponent {
    constructor() {
        this.data = null;
        this.updateInterval = null;
    }

    async load() {
        try {
            this.data = await window.DataManager.getChunkServerData();
            this.render();
            this.startAutoUpdate();
        } catch (error) {
            console.error('Error loading chunk server data:', error);
            this.renderError();
        }
    }

    render() {
        if (!this.data) return;

        const content = document.getElementById('chunk-server-content');
        if (!content) return;

        content.innerHTML = this.generateHTML();
        this.createCharts();
        this.attachEventListeners();
    }

    generateHTML() {
        const storageUsedPercent = Math.round((parseFloat(this.data.used_storage) / parseFloat(this.data.total_storage)) * 100);
        
        return `
            <!-- Chunk Server Overview -->
            <div class="kpi-grid">
                <div class="kpi-card">
                    <h3>Server Status</h3>
                    <div class="kpi-value">
                        ${this.data.online_servers}/${this.data.total_servers}
                    </div>
                    <div class="kpi-trend">
                        <span class="status-badge ${this.data.online_servers === this.data.total_servers ? 'online' : 'warning'}">
                            ${this.data.online_servers === this.data.total_servers ? 'All Online' : 'Some Offline'}
                        </span>
                    </div>
                </div>
                <div class="kpi-card">
                    <h3>Total Storage</h3>
                    <div class="kpi-value">${this.data.total_storage}</div>
                    <div class="kpi-trend">Available capacity</div>
                </div>
                <div class="kpi-card">
                    <h3>Used Storage</h3>
                    <div class="kpi-value">${this.data.used_storage}</div>
                    <div class="progress-bar">
                        <div class="progress-fill ${this.getStorageUsageClass(storageUsedPercent)}" 
                             style="width: ${storageUsedPercent}%"></div>
                    </div>
                    <div class="kpi-trend">${storageUsedPercent}% utilized</div>
                </div>
                <div class="kpi-card">
                    <h3>Free Storage</h3>
                    <div class="kpi-value">${this.data.free_storage}</div>
                    <div class="kpi-trend">Available space</div>
                </div>
                <div class="kpi-card">
                    <h3>Replication Factor</h3>
                    <div class="kpi-value">${this.data.replication_factor}x</div>
                    <div class="kpi-trend">Data redundancy</div>
                </div>
                <div class="kpi-card">
                    <h3>Erasure Coding</h3>
                    <div class="kpi-value">${this.data.erasure_coding}</div>
                    <div class="kpi-trend">Space efficiency</div>
                </div>
            </div>

            <!-- Storage Distribution -->
            <div class="charts-grid">
                <div class="chart-container">
                    <h3>Storage Usage Distribution</h3>
                    <canvas id="chunk-storage-usage-chart"></canvas>
                </div>
                <div class="chart-container">
                    <h3>Server Performance Overview</h3>
                    <canvas id="chunk-performance-overview-chart"></canvas>
                </div>
            </div>

            <!-- Chunk Server Details Table -->
            <div class="chart-container">
                <h3>Chunk Server Details</h3>
                <div class="filter-section">
                    <div class="filter-group">
                        <label class="filter-label">Filter by Status:</label>
                        <select class="filter-select" id="server-status-filter">
                            <option value="all">All Servers</option>
                            <option value="online">Online Only</option>
                            <option value="offline">Offline Only</option>
                        </select>
                    </div>
                    <div class="filter-group">
                        <label class="filter-label">Sort by:</label>
                        <select class="filter-select" id="server-sort">
                            <option value="id">Server ID</option>
                            <option value="storage">Storage Used</option>
                            <option value="chunks">Chunk Count</option>
                            <option value="ops">Operations</option>
                        </select>
                    </div>
                </div>
                
                <table class="component-table" id="chunk-servers-table">
                    <thead>
                        <tr>
                            <th>Server ID</th>
                            <th>Status</th>
                            <th>Storage Used</th>
                            <th>Total Storage</th>
                            <th>Chunks</th>
                            <th>Read Ops/s</th>
                            <th>Write Ops/s</th>
                            <th>Network I/O</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        ${this.data.servers.map(server => `
                            <tr data-status="${server.status}" data-server="${server.id}">
                                <td><strong>${server.id}</strong></td>
                                <td>
                                    <span class="status-badge ${server.status}">
                                        ${server.status.toUpperCase()}
                                    </span>
                                </td>
                                <td>
                                    <div class="progress-bar">
                                        <div class="progress-fill ${this.getStorageUsageClass(server.storage_used)}" 
                                             style="width: ${server.storage_used}%"></div>
                                    </div>
                                    ${server.storage_used}%
                                </td>
                                <td>${server.storage_total}</td>
                                <td>${this.formatNumber(server.chunks)}</td>
                                <td>${server.read_ops}</td>
                                <td>${server.write_ops}</td>
                                <td>${server.network_io}</td>
                                <td>
                                    <button class="action-btn" onclick="window.ChunkServerComponent.viewServerDetails('${server.id}')">
                                        Details
                                    </button>
                                    ${server.status === 'offline' ? `
                                        <button class="action-btn warning" onclick="window.ChunkServerComponent.restartServer('${server.id}')">
                                            Restart
                                        </button>
                                    ` : `
                                        <button class="action-btn" onclick="window.ChunkServerComponent.maintainServer('${server.id}')">
                                            Maintain
                                        </button>
                                    `}
                                </td>
                            </tr>
                        `).join('')}
                    </tbody>
                </table>
            </div>

            <!-- Performance Charts -->
            <div class="charts-grid">
                <div class="chart-container">
                    <h3>Read Operations by Server</h3>
                    <canvas id="chunk-read-ops-chart"></canvas>
                </div>
                <div class="chart-container">
                    <h3>Write Operations by Server</h3>
                    <canvas id="chunk-write-ops-chart"></canvas>
                </div>
                <div class="chart-container">
                    <h3>Storage Utilization by Server</h3>
                    <canvas id="chunk-storage-utilization-chart"></canvas>
                </div>
                <div class="chart-container">
                    <h3>Chunk Distribution</h3>
                    <canvas id="chunk-distribution-chart"></canvas>
                </div>
            </div>

            <!-- Replication and Erasure Coding Status -->
            <div class="chart-container">
                <h3>Data Protection Status</h3>
                <div class="metrics-grid">
                    <div class="metric-card">
                        <div class="metric-label">Replication Status</div>
                        <div class="metric-value">
                            <span class="status-badge online">HEALTHY</span>
                        </div>
                        <small>All chunks properly replicated</small>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Erasure Coding</div>
                        <div class="metric-value">
                            <span class="status-badge online">ACTIVE</span>
                        </div>
                        <small>8+2 coding scheme active</small>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Under-replicated</div>
                        <div class="metric-value">0</div>
                        <small>Chunks needing attention</small>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Recovery Time</div>
                        <div class="metric-value">< 5min</div>
                        <small>Estimated for full recovery</small>
                    </div>
                </div>
            </div>

            <!-- Storage Analytics -->
            <div class="chart-container">
                <h3>Storage Analytics</h3>
                <div class="charts-grid">
                    <div class="chart-container">
                        <h4>Storage Growth Trend</h4>
                        <canvas id="storage-growth-chart"></canvas>
                    </div>
                    <div class="chart-container">
                        <h4>I/O Performance Timeline</h4>
                        <canvas id="io-performance-chart"></canvas>
                    </div>
                </div>
            </div>

            <!-- Maintenance Operations -->
            <div class="chart-container">
                <h3>Maintenance & Operations</h3>
                <div class="filter-section">
                    <button class="action-btn" onclick="window.ChunkServerComponent.runHealthCheck()">
                        🔍 Run Health Check
                    </button>
                    <button class="action-btn" onclick="window.ChunkServerComponent.balanceStorage()">
                        ⚖️ Balance Storage
                    </button>
                    <button class="action-btn" onclick="window.ChunkServerComponent.runScrubbing()">
                        🔧 Run Scrubbing
                    </button>
                    <button class="action-btn warning" onclick="window.ChunkServerComponent.emergencyReplication()">
                        🚨 Emergency Replication
                    </button>
                </div>
                
                <div class="operation-status" id="operation-status">
                    <div class="status-item">
                        <span class="status-dot status-good"></span>
                        <span>All systems operational</span>
                    </div>
                </div>
            </div>
        `;
    }

    createCharts() {
        // Storage Usage Chart
        this.createStorageUsageChart();

        // Performance Overview
        this.createPerformanceOverviewChart();

        // Read Operations Chart
        this.createReadOpsChart();

        // Write Operations Chart
        this.createWriteOpsChart();

        // Storage Utilization Chart
        this.createStorageUtilizationChart();

        // Chunk Distribution Chart
        this.createChunkDistributionChart();

        // Storage Growth Trend
        this.createStorageGrowthChart();

        // I/O Performance Timeline
        this.createIOPerformanceChart();
    }

    createStorageUsageChart() {
        const usedGB = parseFloat(this.data.used_storage);
        const freeGB = parseFloat(this.data.free_storage);

        window.ChartComponents.createStorageUsage(
            'chunk-storage-usage-chart',
            usedGB,
            usedGB + freeGB
        );
    }

    createPerformanceOverviewChart() {
        const ctx = document.getElementById('chunk-performance-overview-chart');
        if (!ctx) return;

        const onlineServers = this.data.servers.filter(s => s.status === 'online');
        const avgReadOps = onlineServers.reduce((sum, s) => sum + s.read_ops, 0) / onlineServers.length;
        const avgWriteOps = onlineServers.reduce((sum, s) => sum + s.write_ops, 0) / onlineServers.length;
        const avgStorageUsed = onlineServers.reduce((sum, s) => sum + s.storage_used, 0) / onlineServers.length;

        const chart = new Chart(ctx, {
            type: 'radar',
            data: {
                labels: ['Read Performance', 'Write Performance', 'Storage Efficiency', 'Availability', 'Redundancy'],
                datasets: [{
                    label: 'Current Performance',
                    data: [
                        Math.min(avgReadOps / 10, 100), // Scale read ops
                        Math.min(avgWriteOps / 5, 100),  // Scale write ops
                        100 - avgStorageUsed,            // Storage efficiency (inverse of usage)
                        (this.data.online_servers / this.data.total_servers) * 100, // Availability
                        this.data.replication_factor * 20 // Redundancy
                    ],
                    backgroundColor: 'rgba(52, 152, 219, 0.2)',
                    borderColor: '#3498db',
                    borderWidth: 2,
                    pointBackgroundColor: '#3498db',
                    pointBorderColor: '#fff',
                    pointHoverBackgroundColor: '#fff',
                    pointHoverBorderColor: '#3498db'
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

        window.ChartComponents.charts.set('chunk-performance-overview-chart', chart);
    }

    createReadOpsChart() {
        const ctx = document.getElementById('chunk-read-ops-chart');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: this.data.servers.map(s => s.id),
                datasets: [{
                    label: 'Read Operations/s',
                    data: this.data.servers.map(s => s.read_ops),
                    backgroundColor: this.data.servers.map(s => 
                        s.status === 'offline' ? '#e74c3c80' : '#3498db80'
                    ),
                    borderColor: this.data.servers.map(s => 
                        s.status === 'offline' ? '#e74c3c' : '#3498db'
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
                            text: 'Operations per Second'
                        }
                    }
                }
            }
        });

        window.ChartComponents.charts.set('chunk-read-ops-chart', chart);
    }

    createWriteOpsChart() {
        const ctx = document.getElementById('chunk-write-ops-chart');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: this.data.servers.map(s => s.id),
                datasets: [{
                    label: 'Write Operations/s',
                    data: this.data.servers.map(s => s.write_ops),
                    backgroundColor: this.data.servers.map(s => 
                        s.status === 'offline' ? '#e74c3c80' : '#2ecc7180'
                    ),
                    borderColor: this.data.servers.map(s => 
                        s.status === 'offline' ? '#e74c3c' : '#2ecc71'
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
                            text: 'Operations per Second'
                        }
                    }
                }
            }
        });

        window.ChartComponents.charts.set('chunk-write-ops-chart', chart);
    }

    createStorageUtilizationChart() {
        window.ChartComponents.createServerMetrics(
            'chunk-storage-utilization-chart',
            this.data.servers,
            'storage_used'
        );
    }

    createChunkDistributionChart() {
        const ctx = document.getElementById('chunk-distribution-chart');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'doughnut',
            data: {
                labels: this.data.servers.map(s => s.id),
                datasets: [{
                    data: this.data.servers.map(s => s.chunks),
                    backgroundColor: [
                        '#3498db', '#2ecc71', '#f39c12', '#e74c3c', 
                        '#9b59b6', '#17a2b8', '#34495e', '#95a5a6'
                    ].slice(0, this.data.servers.length),
                    borderWidth: 2,
                    borderColor: '#ffffff'
                }]
            },
            options: {
                ...window.ChartComponents.getDefaultChartOptions(),
                plugins: {
                    legend: {
                        position: 'right'
                    },
                    tooltip: {
                        callbacks: {
                            label: function(context) {
                                const total = context.dataset.data.reduce((a, b) => a + b, 0);
                                const percentage = Math.round((context.parsed / total) * 100);
                                return `${context.label}: ${context.parsed.toLocaleString()} chunks (${percentage}%)`;
                            }
                        }
                    }
                }
            }
        });

        window.ChartComponents.charts.set('chunk-distribution-chart', chart);
    }

    createStorageGrowthChart() {
        const ctx = document.getElementById('storage-growth-chart');
        if (!ctx) return;

        // Generate mock growth data
        const growthData = Array.from({length: 30}, (_, i) => ({
            day: i + 1,
            storage: 5.2 + (i * 0.1) + (Math.random() * 0.3 - 0.15)
        }));

        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: growthData.map(d => `Day ${d.day}`),
                datasets: [{
                    label: 'Storage Used (TB)',
                    data: growthData.map(d => d.storage),
                    borderColor: '#f39c12',
                    backgroundColor: '#f39c1220',
                    fill: true,
                    tension: 0.4,
                    pointRadius: 2,
                    pointHoverRadius: 5
                }]
            },
            options: {
                ...window.ChartComponents.getDefaultChartOptions(),
                scales: {
                    y: {
                        beginAtZero: false,
                        title: {
                            display: true,
                            text: 'Storage Used (TB)'
                        }
                    }
                }
            }
        });

        window.ChartComponents.charts.set('storage-growth-chart', chart);
    }

    createIOPerformanceChart() {
        const ctx = document.getElementById('io-performance-chart');
        if (!ctx) return;

        // Generate mock I/O performance data
        const ioData = Array.from({length: 24}, (_, i) => ({
            time: new Date(Date.now() - (23-i) * 3600000),
            read: Math.floor(Math.random() * 1000 + 500),
            write: Math.floor(Math.random() * 400 + 200)
        }));

        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: ioData.map(d => d.time.toLocaleTimeString()),
                datasets: [
                    {
                        label: 'Read IOPS',
                        data: ioData.map(d => d.read),
                        borderColor: '#3498db',
                        backgroundColor: '#3498db20',
                        fill: false,
                        tension: 0.4,
                        pointRadius: 2
                    },
                    {
                        label: 'Write IOPS',
                        data: ioData.map(d => d.write),
                        borderColor: '#2ecc71',
                        backgroundColor: '#2ecc7120',
                        fill: false,
                        tension: 0.4,
                        pointRadius: 2
                    }
                ]
            },
            options: {
                ...window.ChartComponents.getDefaultChartOptions(),
                scales: {
                    y: {
                        beginAtZero: true,
                        title: {
                            display: true,
                            text: 'IOPS'
                        }
                    }
                }
            }
        });

        window.ChartComponents.charts.set('io-performance-chart', chart);
    }

    attachEventListeners() {
        // Status filter
        const statusFilter = document.getElementById('server-status-filter');
        if (statusFilter) {
            statusFilter.addEventListener('change', () => this.filterServers());
        }

        // Sort dropdown
        const sortSelect = document.getElementById('server-sort');
        if (sortSelect) {
            sortSelect.addEventListener('change', () => this.sortServers());
        }
    }

    filterServers() {
        const filter = document.getElementById('server-status-filter')?.value;
        const rows = document.querySelectorAll('#chunk-servers-table tbody tr');
        
        rows.forEach(row => {
            const status = row.getAttribute('data-status');
            if (filter === 'all' || status === filter) {
                row.style.display = '';
            } else {
                row.style.display = 'none';
            }
        });
    }

    sortServers() {
        const sortBy = document.getElementById('server-sort')?.value;
        const tbody = document.querySelector('#chunk-servers-table tbody');
        const rows = Array.from(tbody.querySelectorAll('tr'));
        
        rows.sort((a, b) => {
            const aValue = this.getSortValue(a, sortBy);
            const bValue = this.getSortValue(b, sortBy);
            return bValue - aValue; // Descending order
        });
        
        rows.forEach(row => tbody.appendChild(row));
    }

    getSortValue(row, sortBy) {
        const serverId = row.getAttribute('data-server');
        const server = this.data.servers.find(s => s.id === serverId);
        
        switch (sortBy) {
            case 'storage': return server.storage_used;
            case 'chunks': return server.chunks;
            case 'ops': return server.read_ops + server.write_ops;
            default: return 0;
        }
    }

    getStorageUsageClass(usage) {
        if (usage > 90) return 'danger';
        if (usage > 75) return 'warning';
        return 'good';
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
    viewServerDetails(serverId) {
        const server = this.data.servers.find(s => s.id === serverId);
        if (!server) return;

        const modal = document.createElement('div');
        modal.className = 'modal-overlay';
        modal.innerHTML = `
            <div class="modal-content">
                <div class="modal-header">
                    <h3>Chunk Server Details: ${serverId}</h3>
                    <button class="modal-close" onclick="this.closest('.modal-overlay').remove()">×</button>
                </div>
                <div class="modal-body">
                    <div class="server-details">
                        <div class="detail-row">
                            <label>Status:</label>
                            <span class="status-badge ${server.status}">${server.status.toUpperCase()}</span>
                        </div>
                        <div class="detail-row">
                            <label>Storage Used:</label>
                            <span>${server.storage_used}% of ${server.storage_total}</span>
                        </div>
                        <div class="detail-row">
                            <label>Chunks Stored:</label>
                            <span>${this.formatNumber(server.chunks)}</span>
                        </div>
                        <div class="detail-row">
                            <label>Read Operations/s:</label>
                            <span>${server.read_ops}</span>
                        </div>
                        <div class="detail-row">
                            <label>Write Operations/s:</label>
                            <span>${server.write_ops}</span>
                        </div>
                        <div class="detail-row">
                            <label>Network I/O:</label>
                            <span>${server.network_io}</span>
                        </div>
                    </div>
                </div>
            </div>
        `;

        document.body.appendChild(modal);
    }

    restartServer(serverId) {
        if (confirm(`Are you sure you want to restart ${serverId}?`)) {
            this.showOperationStatus(`Restarting ${serverId}...`, 'warning');
            // Simulate restart operation
            setTimeout(() => {
                this.showOperationStatus(`${serverId} restarted successfully`, 'success');
            }, 3000);
        }
    }

    maintainServer(serverId) {
        if (confirm(`Put ${serverId} into maintenance mode?`)) {
            this.showOperationStatus(`${serverId} entering maintenance mode...`, 'warning');
            setTimeout(() => {
                this.showOperationStatus(`${serverId} is now in maintenance mode`, 'success');
            }, 2000);
        }
    }

    runHealthCheck() {
        this.showOperationStatus('Running comprehensive health check...', 'info');
        setTimeout(() => {
            this.showOperationStatus('Health check completed - all systems healthy', 'success');
        }, 5000);
    }

    balanceStorage() {
        this.showOperationStatus('Starting storage balancing operation...', 'info');
        setTimeout(() => {
            this.showOperationStatus('Storage balancing completed successfully', 'success');
        }, 8000);
    }

    runScrubbing() {
        this.showOperationStatus('Starting data scrubbing operation...', 'info');
        setTimeout(() => {
            this.showOperationStatus('Data scrubbing completed - no errors found', 'success');
        }, 10000);
    }

    emergencyReplication() {
        if (confirm('Start emergency replication? This will prioritize data safety over performance.')) {
            this.showOperationStatus('Emergency replication initiated...', 'warning');
            setTimeout(() => {
                this.showOperationStatus('Emergency replication completed', 'success');
            }, 15000);
        }
    }

    showOperationStatus(message, type) {
        const statusContainer = document.getElementById('operation-status');
        if (statusContainer) {
            const statusClass = type === 'success' ? 'status-good' : 
                             type === 'warning' ? 'status-warning' : 
                             type === 'error' ? 'status-danger' : 'status-good';
            
            statusContainer.innerHTML = `
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
                const newData = await window.DataManager.getChunkServerData();
                if (newData) {
                    this.data = newData;
                    this.render();
                }
            } catch (error) {
                console.error('Error updating chunk server data:', error);
            }
        }, 30000);
    }

    stopAutoUpdate() {
        if (this.updateInterval) {
            clearInterval(this.updateInterval);
            this.updateInterval = null;
        }
    }

    renderError() {
        const content = document.getElementById('chunk-server-content');
        if (content) {
            content.innerHTML = `
                <div class="error-message">
                    <h3>Error Loading Chunk Server Data</h3>
                    <p>Unable to load chunk server performance data. Please check your connection and try again.</p>
                    <button onclick="window.ChunkServerComponent.load()" class="retry-button">
                        Retry
                    </button>
                </div>
            `;
        }
    }

    destroy() {
        this.stopAutoUpdate();
        
        // Destroy chunk server charts
        ['chunk-storage-usage-chart', 'chunk-performance-overview-chart',
         'chunk-read-ops-chart', 'chunk-write-ops-chart', 
         'chunk-storage-utilization-chart', 'chunk-distribution-chart',
         'storage-growth-chart', 'io-performance-chart'].forEach(chartId => {
            window.ChartComponents.destroyChart(chartId);
        });
    }
}

// Initialize global chunk server component
window.ChunkServerComponent = new ChunkServerComponent();