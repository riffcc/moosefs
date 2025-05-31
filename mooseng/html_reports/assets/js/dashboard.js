// MooseNG Dashboard JavaScript
let charts = {};

function initializeCharts(data) {
    // Performance timeline chart
    const perfCtx = document.getElementById('performanceChart');
    if (perfCtx) {
        charts.performance = new Chart(perfCtx, {
            type: 'line',
            data: {
                labels: data.timestamps || [],
                datasets: [{
                    label: 'Total Time (ms)',
                    data: data.totalTimes || [],
                    borderColor: 'rgb(13, 110, 253)',
                    backgroundColor: 'rgba(13, 110, 253, 0.1)',
                    fill: true,
                    tension: 0.4
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: {
                        beginAtZero: true
                    }
                }
            }
        });
    }

    // Throughput chart
    const throughputCtx = document.getElementById('throughputChart');
    if (throughputCtx) {
        charts.throughput = new Chart(throughputCtx, {
            type: 'bar',
            data: {
                labels: data.timestamps || [],
                datasets: [{
                    label: 'Throughput (MB/s)',
                    data: data.throughputs || [],
                    backgroundColor: 'rgba(25, 135, 84, 0.7)',
                    borderColor: 'rgb(25, 135, 84)',
                    borderWidth: 1
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: {
                        beginAtZero: true
                    }
                }
            }
        });
    }

    // Distribution chart
    const distCtx = document.getElementById('distributionChart');
    if (distCtx) {
        charts.distribution = new Chart(distCtx, {
            type: 'pie',
            data: {
                labels: ['File System', 'Network', 'CPU/Memory', 'Concurrency'],
                datasets: [{
                    data: [25, 35, 20, 20],
                    backgroundColor: [
                        'rgba(13, 110, 253, 0.7)',
                        'rgba(25, 135, 84, 0.7)',
                        'rgba(255, 193, 7, 0.7)',
                        'rgba(220, 53, 69, 0.7)'
                    ]
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false
            }
        });
    }

    // Resource chart
    const resourceCtx = document.getElementById('resourceChart');
    if (resourceCtx) {
        charts.resource = new Chart(resourceCtx, {
            type: 'area',
            data: {
                labels: data.timestamps || [],
                datasets: [{
                    label: 'CPU %',
                    data: [45, 52, 38, 47, 43],
                    borderColor: 'rgb(13, 110, 253)',
                    backgroundColor: 'rgba(13, 110, 253, 0.3)',
                    fill: true
                }, {
                    label: 'Memory %',
                    data: [62, 58, 65, 59, 61],
                    borderColor: 'rgb(25, 135, 84)',
                    backgroundColor: 'rgba(25, 135, 84, 0.3)',
                    fill: true
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: {
                        beginAtZero: true,
                        max: 100
                    }
                }
            }
        });
    }
}

function initializeComponentCharts() {
    // Mock data for components
    const componentData = {
        master: [85, 92, 78, 88, 95],
        chunk: [90, 87, 93, 89, 91],
        client: [82, 85, 88, 84, 87]
    };

    ['master', 'chunk', 'client'].forEach(component => {
        const ctx = document.getElementById(component + 'Chart');
        if (ctx) {
            new Chart(ctx, {
                type: 'doughnut',
                data: {
                    labels: ['Performance', 'Reliability', 'Efficiency'],
                    datasets: [{
                        data: componentData[component],
                        backgroundColor: [
                            'rgba(13, 110, 253, 0.7)',
                            'rgba(25, 135, 84, 0.7)',
                            'rgba(255, 193, 7, 0.7)'
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
        }
    });
}

function initializeComparisonCharts() {
    const ctx = document.getElementById('comparisonChart');
    if (ctx) {
        new Chart(ctx, {
            type: 'radar',
            data: {
                labels: ['Read Latency', 'Write Throughput', 'Memory Usage', 'CPU Efficiency', 'Scalability'],
                datasets: [{
                    label: 'MooseNG',
                    data: [85, 92, 88, 90, 95],
                    borderColor: 'rgb(13, 110, 253)',
                    backgroundColor: 'rgba(13, 110, 253, 0.2)',
                    pointBackgroundColor: 'rgb(13, 110, 253)'
                }, {
                    label: 'Baseline',
                    data: [60, 72, 75, 80, 70],
                    borderColor: 'rgb(220, 53, 69)',
                    backgroundColor: 'rgba(220, 53, 69, 0.2)',
                    pointBackgroundColor: 'rgb(220, 53, 69)'
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    r: {
                        beginAtZero: true,
                        max: 100,
                        ticks: {
                            stepSize: 20
                        }
                    }
                },
                plugins: {
                    legend: {
                        position: 'top'
                    }
                }
            }
        });
    }
}

function viewDetails(timestamp) {
    // Create a modal or navigate to detailed view
    const modal = document.createElement('div');
    modal.innerHTML = `
        <div class="modal fade" id="detailModal" tabindex="-1">
            <div class="modal-dialog modal-lg">
                <div class="modal-content">
                    <div class="modal-header">
                        <h5 class="modal-title">Benchmark Details - ${timestamp}</h5>
                        <button type="button" class="btn-close" data-bs-dismiss="modal"></button>
                    </div>
                    <div class="modal-body">
                        <p>Detailed analysis for benchmark run at ${timestamp}</p>
                        <p><em>Feature coming soon: In-depth performance analysis, bottleneck identification, and optimization recommendations.</em></p>
                    </div>
                    <div class="modal-footer">
                        <button type="button" class="btn btn-secondary" data-bs-dismiss="modal">Close</button>
                    </div>
                </div>
            </div>
        </div>
    `;
    
    document.body.appendChild(modal);
    const modalInstance = new bootstrap.Modal(document.getElementById('detailModal'));
    modalInstance.show();
    
    // Clean up modal after closing
    document.getElementById('detailModal').addEventListener('hidden.bs.modal', function() {
        document.body.removeChild(modal);
    });
}

// Theme switching functionality
function switchTheme(theme) {
    document.documentElement.className = `theme-${theme}`;
    localStorage.setItem('theme', theme);
    
    // Update all charts for new theme
    Object.values(charts).forEach(chart => {
        if (chart) chart.update();
    });
}

// Load saved theme on page load
document.addEventListener('DOMContentLoaded', function() {
    const savedTheme = localStorage.getItem('theme') || 'auto';
    if (savedTheme !== 'auto') {
        switchTheme(savedTheme);
    }
    
    // Auto-detect system theme if set to auto
    if (savedTheme === 'auto') {
        const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
        switchTheme(prefersDark ? 'dark' : 'light');
    }
});

// Auto-refresh functionality (if enabled)
let autoRefreshInterval = null;

function toggleAutoRefresh() {
    if (autoRefreshInterval) {
        clearInterval(autoRefreshInterval);
        autoRefreshInterval = null;
        document.getElementById('autoRefreshBtn').innerHTML = '<i class="fas fa-play"></i> Start Auto-refresh';
    } else {
        autoRefreshInterval = setInterval(() => {
            // In a real implementation, this would fetch new data
            console.log('Auto-refreshing data...');
        }, 30000); // 30 seconds
        document.getElementById('autoRefreshBtn').innerHTML = '<i class="fas fa-pause"></i> Stop Auto-refresh';
    }
}

// Export functionality
function exportReport(format) {
    switch (format) {
        case 'pdf':
            window.print();
            break;
        case 'png':
            // Export charts as images
            Object.keys(charts).forEach(chartName => {
                const chart = charts[chartName];
                if (chart) {
                    const link = document.createElement('a');
                    link.download = `${chartName}_chart.png`;
                    link.href = chart.toBase64Image();
                    link.click();
                }
            });
            break;
        case 'json':
            // Export data as JSON
            const data = {
                timestamp: new Date().toISOString(),
                charts: Object.keys(charts).map(name => ({
                    name,
                    data: charts[name]?.data
                }))
            };
            const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
            const link = document.createElement('a');
            link.download = 'benchmark_data.json';
            link.href = URL.createObjectURL(blob);
            link.click();
            break;
        default:
            console.error('Unknown export format:', format);
    }
}