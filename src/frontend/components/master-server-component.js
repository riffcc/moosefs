/**
 * Master Server Component
 * Handles master server performance monitoring and visualization
 */

class MasterServerComponent {
    constructor() {
        this.data = null;
        this.updateInterval = null;
    }

    async load() {
        try {
            this.data = await window.DataManager.getMasterServerData();
            this.render();
            this.startAutoUpdate();
        } catch (error) {
            console.error('Error loading master server data:', error);
            this.renderError();
        }
    }

    render() {
        if (!this.data) return;

        const content = document.getElementById('master-server-content');
        if (!content) return;

        content.innerHTML = this.generateHTML();
        this.createCharts();
        this.attachEventListeners();
    }

    generateHTML() {
        return `
            <!-- Master Server Status Overview -->
            <div class="kpi-grid">
                <div class="kpi-card">
                    <h3>Server Status</h3>
                    <div class="kpi-value">
                        <span class="status-badge ${this.data.status}">${this.data.status.toUpperCase()}</span>
                    </div>
                    <div class="kpi-trend">Uptime: ${this.data.uptime}</div>
                </div>
                <div class="kpi-card">
                    <h3>CPU Usage</h3>
                    <div class="kpi-value">${this.data.cpu_usage}%</div>
                    <div class="progress-bar">
                        <div class="progress-fill ${this.getUsageClass(this.data.cpu_usage)}" 
                             style="width: ${this.data.cpu_usage}%"></div>
                    </div>
                </div>
                <div class="kpi-card">
                    <h3>Memory Usage</h3>
                    <div class="kpi-value">${this.data.memory_usage}%</div>
                    <div class="progress-bar">
                        <div class="progress-fill ${this.getUsageClass(this.data.memory_usage)}" 
                             style="width: ${this.data.memory_usage}%"></div>
                    </div>
                </div>
                <div class="kpi-card">
                    <h3>Active Connections</h3>
                    <div class="kpi-value">${this.data.active_connections}</div>
                    <div class="kpi-trend">
                        <span class="trend-up">↗ 5% from last hour</span>
                    </div>
                </div>
                <div class="kpi-card">
                    <h3>Metadata Operations</h3>
                    <div class="kpi-value">${this.formatNumber(this.data.metadata_operations)}</div>
                    <div class="kpi-trend">
                        <span class="trend-up">↗ 12% from last hour</span>
                    </div>
                </div>
                <div class="kpi-card">
                    <h3>Cluster Health</h3>
                    <div class="kpi-value">${this.data.cluster_health}%</div>
                    <div class="kpi-trend">
                        <span class="status-badge ${this.data.cluster_health > 95 ? 'online' : 'warning'}">
                            ${this.data.cluster_health > 95 ? 'Excellent' : 'Good'}
                        </span>
                    </div>
                </div>
            </div>

            <!-- Raft Consensus Status -->
            <div class="chart-container">
                <h3>Raft Consensus Status</h3>
                <div class="metrics-grid">
                    <div class="metric-card">
                        <div class="metric-label">Role</div>
                        <div class="metric-value">
                            <span class="status-badge ${this.data.raft_leader ? 'online' : 'warning'}">
                                ${this.data.raft_leader ? 'LEADER' : 'FOLLOWER'}
                            </span>
                        </div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Cluster Size</div>
                        <div class="metric-value">${this.data.servers.length}</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Healthy Nodes</div>
                        <div class="metric-value">${this.data.servers.filter(s => s.status !== 'offline').length}</div>
                    </div>
                    <div class="metric-card">
                        <div class="metric-label">Consensus Health</div>
                        <div class="metric-value">${this.data.cluster_health}%</div>
                    </div>
                </div>
            </div>

            <!-- Master Server Cluster Table -->
            <div class="chart-container">
                <h3>Master Server Cluster</h3>
                <table class="component-table">
                    <thead>
                        <tr>
                            <th>Server ID</th>
                            <th>Role</th>
                            <th>Status</th>
                            <th>Uptime</th>
                            <th>CPU Usage</th>
                            <th>Memory Usage</th>
                            <th>Connections</th>
                            <th>Actions</th>
                        </tr>
                    </thead>
                    <tbody>
                        ${this.data.servers.map(server => `
                            <tr>
                                <td><strong>${server.id}</strong></td>
                                <td>
                                    <span class="status-badge ${server.status === 'leader' ? 'online' : 'warning'}">
                                        ${server.status.toUpperCase()}
                                    </span>
                                </td>
                                <td>
                                    <span class="status-badge online">ONLINE</span>
                                </td>
                                <td>${server.uptime}</td>
                                <td>
                                    <div class="progress-bar">
                                        <div class="progress-fill ${this.getUsageClass(server.cpu)}" 
                                             style="width: ${server.cpu}%"></div>
                                    </div>
                                    ${server.cpu}%
                                </td>
                                <td>
                                    <div class="progress-bar">
                                        <div class="progress-fill ${this.getUsageClass(server.memory)}" 
                                             style="width: ${server.memory}%"></div>
                                    </div>
                                    ${server.memory}%
                                </td>
                                <td>${server.connections}</td>
                                <td>
                                    <button class="action-btn" onclick="window.MasterServerComponent.viewServerDetails('${server.id}')">
                                        View Details
                                    </button>
                                </td>
                            </tr>
                        `).join('')}
                    </tbody>
                </table>
            </div>

            <!-- Performance Charts -->
            <div class="charts-grid">
                <div class="chart-container">
                    <h3>CPU Usage by Server</h3>
                    <canvas id="master-cpu-chart"></canvas>
                </div>
                <div class="chart-container">
                    <h3>Memory Usage by Server</h3>
                    <canvas id="master-memory-chart"></canvas>
                </div>
                <div class="chart-container">
                    <h3>Connection Distribution</h3>
                    <canvas id="master-connections-chart"></canvas>
                </div>
                <div class="chart-container">
                    <h3>Metadata Operations Timeline</h3>
                    <canvas id="master-operations-chart"></canvas>
                </div>
            </div>

            <!-- Configuration and Logs -->
            <div class="chart-container">
                <h3>Configuration & Monitoring</h3>
                <div class="filter-section">
                    <div class="filter-group">
                        <label class="filter-label">Log Level:</label>
                        <select class="filter-select" id="log-level">
                            <option value="info">Info</option>
                            <option value="warning">Warning</option>
                            <option value="error">Error</option>
                            <option value="debug">Debug</option>
                        </select>
                    </div>
                    <div class="filter-group">
                        <label class="filter-label">Time Range:</label>
                        <select class="filter-select" id="log-time-range">
                            <option value="1h">Last Hour</option>
                            <option value="6h">Last 6 Hours</option>
                            <option value="24h" selected>Last 24 Hours</option>
                        </select>
                    </div>
                    <button class="refresh-button" onclick="window.MasterServerComponent.refreshLogs()">
                        🔄 Refresh Logs
                    </button>
                </div>
                
                <div class="log-container" id="master-logs">
                    ${this.generateMockLogs()}
                </div>
            </div>
        `;
    }

    createCharts() {
        // CPU Usage Chart
        window.ChartComponents.createServerMetrics(
            'master-cpu-chart',
            this.data.servers,
            'cpu'
        );

        // Memory Usage Chart
        window.ChartComponents.createServerMetrics(
            'master-memory-chart',
            this.data.servers,
            'memory'
        );

        // Connection Distribution Chart
        this.createConnectionsChart();

        // Operations Timeline Chart
        this.createOperationsTimelineChart();
    }

    createConnectionsChart() {
        const ctx = document.getElementById('master-connections-chart');
        if (!ctx) return;

        const chart = new Chart(ctx, {
            type: 'doughnut',
            data: {
                labels: this.data.servers.map(s => s.id),
                datasets: [{
                    data: this.data.servers.map(s => s.connections),
                    backgroundColor: [
                        '#3498db', '#2ecc71', '#f39c12', '#e74c3c', 
                        '#9b59b6', '#17a2b8', '#34495e'
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
                                return `${context.label}: ${context.parsed} connections (${percentage}%)`;
                            }
                        }
                    }
                }
            }
        });

        window.ChartComponents.charts.set('master-connections-chart', chart);
    }

    createOperationsTimelineChart() {
        const ctx = document.getElementById('master-operations-chart');
        if (!ctx) return;

        // Generate mock timeline data
        const timelineData = Array.from({length: 24}, (_, i) => ({
            timestamp: new Date(Date.now() - (23-i) * 3600000),
            operations: Math.floor(Math.random() * 2000 + 4000)
        }));

        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: timelineData.map(d => d.timestamp.toLocaleTimeString()),
                datasets: [{
                    label: 'Metadata Operations',
                    data: timelineData.map(d => d.operations),
                    borderColor: '#3498db',
                    backgroundColor: '#3498db20',
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
                        beginAtZero: true,
                        title: {
                            display: true,
                            text: 'Operations per Hour'
                        }
                    },
                    x: {
                        title: {
                            display: true,
                            text: 'Time'
                        }
                    }
                }
            }
        });

        window.ChartComponents.charts.set('master-operations-chart', chart);
    }

    generateMockLogs() {
        const logEntries = [
            { level: 'info', time: '14:23:15', message: 'Raft leader election completed successfully' },
            { level: 'info', time: '14:22:45', message: 'Metadata snapshot created (size: 2.3MB)' },
            { level: 'warning', time: '14:20:12', message: 'High CPU usage detected on master-2 (89%)' },
            { level: 'info', time: '14:18:33', message: 'Client connection established from 192.168.1.100' },
            { level: 'info', time: '14:15:22', message: 'Chunk replication completed for chunk ID 0x4f2a9b8c' },
            { level: 'error', time: '14:12:01', message: 'Failed to connect to chunk server chunk-8' },
            { level: 'info', time: '14:10:45', message: 'Periodic health check completed - all systems nominal' },
            { level: 'info', time: '14:08:17', message: 'Metadata operation processed: CREATE /data/file123.txt' }
        ];

        return `
            <div class="log-entries">
                ${logEntries.map(entry => `
                    <div class="log-entry log-${entry.level}">
                        <span class="log-time">${entry.time}</span>
                        <span class="log-level">[${entry.level.toUpperCase()}]</span>
                        <span class="log-message">${entry.message}</span>
                    </div>
                `).join('')}
            </div>
        `;
    }

    attachEventListeners() {
        // Add any specific event listeners for the master server component
        const logLevelSelect = document.getElementById('log-level');
        if (logLevelSelect) {
            logLevelSelect.addEventListener('change', () => this.filterLogs());
        }

        const timeRangeSelect = document.getElementById('log-time-range');
        if (timeRangeSelect) {
            timeRangeSelect.addEventListener('change', () => this.refreshLogs());
        }
    }

    getUsageClass(usage) {
        if (usage > 80) return 'danger';
        if (usage > 60) return 'warning';
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

    viewServerDetails(serverId) {
        // Find server data
        const server = this.data.servers.find(s => s.id === serverId);
        if (!server) return;

        // Create modal or detailed view
        const modal = document.createElement('div');
        modal.className = 'modal-overlay';
        modal.innerHTML = `
            <div class="modal-content">
                <div class="modal-header">
                    <h3>Server Details: ${serverId}</h3>
                    <button class="modal-close" onclick="this.closest('.modal-overlay').remove()">×</button>
                </div>
                <div class="modal-body">
                    <div class="server-details">
                        <div class="detail-row">
                            <label>Status:</label>
                            <span class="status-badge ${server.status === 'leader' ? 'online' : 'warning'}">
                                ${server.status.toUpperCase()}
                            </span>
                        </div>
                        <div class="detail-row">
                            <label>Uptime:</label>
                            <span>${server.uptime}</span>
                        </div>
                        <div class="detail-row">
                            <label>CPU Usage:</label>
                            <span>${server.cpu}%</span>
                        </div>
                        <div class="detail-row">
                            <label>Memory Usage:</label>
                            <span>${server.memory}%</span>
                        </div>
                        <div class="detail-row">
                            <label>Active Connections:</label>
                            <span>${server.connections}</span>
                        </div>
                    </div>
                </div>
            </div>
        `;

        document.body.appendChild(modal);
    }

    filterLogs() {
        const level = document.getElementById('log-level')?.value;
        const logEntries = document.querySelectorAll('.log-entry');
        
        logEntries.forEach(entry => {
            if (level === 'all' || entry.classList.contains(`log-${level}`)) {
                entry.style.display = 'flex';
            } else {
                entry.style.display = 'none';
            }
        });
    }

    refreshLogs() {
        const logsContainer = document.getElementById('master-logs');
        if (logsContainer) {
            logsContainer.innerHTML = this.generateMockLogs();
        }
    }

    startAutoUpdate() {
        this.updateInterval = setInterval(async () => {
            try {
                const newData = await window.DataManager.getMasterServerData();
                if (newData) {
                    this.data = newData;
                    this.render();
                }
            } catch (error) {
                console.error('Error updating master server data:', error);
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
        const content = document.getElementById('master-server-content');
        if (content) {
            content.innerHTML = `
                <div class="error-message">
                    <h3>Error Loading Master Server Data</h3>
                    <p>Unable to load master server performance data. Please check your connection and try again.</p>
                    <button onclick="window.MasterServerComponent.load()" class="retry-button">
                        Retry
                    </button>
                </div>
            `;
        }
    }

    destroy() {
        this.stopAutoUpdate();
        
        // Destroy master server charts
        ['master-cpu-chart', 'master-memory-chart', 
         'master-connections-chart', 'master-operations-chart'].forEach(chartId => {
            window.ChartComponents.destroyChart(chartId);
        });
    }
}

// Initialize global master server component
window.MasterServerComponent = new MasterServerComponent();