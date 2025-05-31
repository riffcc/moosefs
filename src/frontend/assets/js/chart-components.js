/**
 * MooseNG Chart Components
 * Reusable chart components using Chart.js and D3.js
 */

class ChartComponents {
    constructor() {
        this.defaultColors = {
            primary: '#3498db',
            secondary: '#2ecc71',
            warning: '#f39c12',
            danger: '#e74c3c',
            purple: '#9b59b6',
            info: '#17a2b8',
            dark: '#34495e',
            light: '#ecf0f1'
        };
        
        this.charts = new Map(); // Store chart instances for cleanup
    }

    // Chart.js default configuration
    getDefaultChartOptions() {
        return {
            responsive: true,
            maintainAspectRatio: false,
            plugins: {
                legend: {
                    position: 'bottom',
                    labels: {
                        padding: 20,
                        usePointStyle: true
                    }
                },
                tooltip: {
                    mode: 'index',
                    intersect: false,
                    backgroundColor: 'rgba(0, 0, 0, 0.8)',
                    titleColor: '#ffffff',
                    bodyColor: '#ffffff',
                    borderColor: '#3498db',
                    borderWidth: 1
                }
            },
            interaction: {
                mode: 'nearest',
                axis: 'x',
                intersect: false
            }
        };
    }

    // Performance Breakdown Pie Chart
    createPerformanceBreakdown(canvasId, data) {
        const ctx = document.getElementById(canvasId);
        if (!ctx) return null;

        // Cleanup existing chart
        if (this.charts.has(canvasId)) {
            this.charts.get(canvasId).destroy();
        }

        const chart = new Chart(ctx, {
            type: 'doughnut',
            data: {
                labels: ['File System', 'Network', 'CPU/Memory', 'Concurrency', 'Throughput'],
                datasets: [{
                    data: [
                        data.file_system_ms,
                        data.network_ms,
                        data.cpu_memory_ms,
                        data.concurrency_ms,
                        data.throughput_ms
                    ],
                    backgroundColor: [
                        this.defaultColors.primary,
                        this.defaultColors.secondary,
                        this.defaultColors.warning,
                        this.defaultColors.purple,
                        this.defaultColors.info
                    ],
                    borderWidth: 2,
                    borderColor: '#ffffff'
                }]
            },
            options: {
                ...this.getDefaultChartOptions(),
                plugins: {
                    ...this.getDefaultChartOptions().plugins,
                    legend: {
                        position: 'right'
                    },
                    tooltip: {
                        callbacks: {
                            label: function(context) {
                                const label = context.label || '';
                                const value = context.parsed;
                                const total = context.dataset.data.reduce((a, b) => a + b, 0);
                                const percentage = Math.round((value / total) * 100);
                                return `${label}: ${value}ms (${percentage}%)`;
                            }
                        }
                    }
                }
            }
        });

        this.charts.set(canvasId, chart);
        return chart;
    }

    // Throughput Timeline Chart
    createThroughputTimeline(canvasId, data) {
        const ctx = document.getElementById(canvasId);
        if (!ctx) return null;

        if (this.charts.has(canvasId)) {
            this.charts.get(canvasId).destroy();
        }

        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: data.map(d => new Date(d.timestamp).toLocaleTimeString()),
                datasets: [{
                    label: 'Throughput (MB/s)',
                    data: data.map(d => d.throughput),
                    borderColor: this.defaultColors.primary,
                    backgroundColor: this.defaultColors.primary + '20',
                    fill: true,
                    tension: 0.4,
                    pointRadius: 3,
                    pointHoverRadius: 6
                }]
            },
            options: {
                ...this.getDefaultChartOptions(),
                scales: {
                    y: {
                        beginAtZero: true,
                        title: {
                            display: true,
                            text: 'Throughput (MB/s)'
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

        this.charts.set(canvasId, chart);
        return chart;
    }

    // Response Time Distribution Chart
    createResponseTimeChart(canvasId, data) {
        const ctx = document.getElementById(canvasId);
        if (!ctx) return null;

        if (this.charts.has(canvasId)) {
            this.charts.get(canvasId).destroy();
        }

        const chart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: data.map(d => new Date(d.timestamp).toLocaleTimeString()),
                datasets: [{
                    label: 'Response Time (ms)',
                    data: data.map(d => d.response_time),
                    backgroundColor: this.defaultColors.secondary + '80',
                    borderColor: this.defaultColors.secondary,
                    borderWidth: 1
                }]
            },
            options: {
                ...this.getDefaultChartOptions(),
                scales: {
                    y: {
                        beginAtZero: true,
                        title: {
                            display: true,
                            text: 'Response Time (ms)'
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

        this.charts.set(canvasId, chart);
        return chart;
    }

    // Health Score Gauge Chart
    createHealthScoreChart(canvasId, score) {
        const ctx = document.getElementById(canvasId);
        if (!ctx) return null;

        if (this.charts.has(canvasId)) {
            this.charts.get(canvasId).destroy();
        }

        const chart = new Chart(ctx, {
            type: 'doughnut',
            data: {
                datasets: [{
                    data: [score, 100 - score],
                    backgroundColor: [
                        score >= 90 ? this.defaultColors.secondary : 
                        score >= 70 ? this.defaultColors.warning : this.defaultColors.danger,
                        '#f0f0f0'
                    ],
                    borderWidth: 0,
                    circumference: 180,
                    rotation: 270
                }]
            },
            options: {
                ...this.getDefaultChartOptions(),
                plugins: {
                    legend: {
                        display: false
                    },
                    tooltip: {
                        filter: function(tooltipItem) {
                            return tooltipItem.dataIndex === 0;
                        },
                        callbacks: {
                            label: function(context) {
                                return `Health Score: ${context.parsed}%`;
                            }
                        }
                    }
                },
                elements: {
                    center: {
                        text: `${score}%`,
                        color: '#666',
                        fontStyle: 'bold',
                        sidePadding: 20,
                        minFontSize: 20,
                        lineHeight: 25
                    }
                }
            },
            plugins: [{
                beforeDraw: function(chart) {
                    const width = chart.width;
                    const height = chart.height;
                    const ctx = chart.ctx;
                    
                    ctx.restore();
                    const fontSize = (height / 114).toFixed(2);
                    ctx.font = fontSize + "em sans-serif";
                    ctx.textBaseline = "top";
                    
                    const text = `${score}%`;
                    const textX = Math.round((width - ctx.measureText(text).width) / 2);
                    const textY = height / 1.8;
                    
                    ctx.fillStyle = '#666';
                    ctx.fillText(text, textX, textY);
                    ctx.save();
                }
            }]
        });

        this.charts.set(canvasId, chart);
        return chart;
    }

    // Server Metrics Chart
    createServerMetrics(canvasId, servers, metric) {
        const ctx = document.getElementById(canvasId);
        if (!ctx) return null;

        if (this.charts.has(canvasId)) {
            this.charts.get(canvasId).destroy();
        }

        const chart = new Chart(ctx, {
            type: 'bar',
            data: {
                labels: servers.map(s => s.id),
                datasets: [{
                    label: metric.charAt(0).toUpperCase() + metric.slice(1),
                    data: servers.map(s => s[metric]),
                    backgroundColor: servers.map(s => 
                        s.status === 'offline' ? this.defaultColors.danger + '80' :
                        s[metric] > 80 ? this.defaultColors.warning + '80' :
                        this.defaultColors.primary + '80'
                    ),
                    borderColor: servers.map(s => 
                        s.status === 'offline' ? this.defaultColors.danger :
                        s[metric] > 80 ? this.defaultColors.warning :
                        this.defaultColors.primary
                    ),
                    borderWidth: 1
                }]
            },
            options: {
                ...this.getDefaultChartOptions(),
                scales: {
                    y: {
                        beginAtZero: true,
                        max: 100,
                        title: {
                            display: true,
                            text: `${metric.charAt(0).toUpperCase() + metric.slice(1)} (%)`
                        }
                    },
                    x: {
                        title: {
                            display: true,
                            text: 'Servers'
                        }
                    }
                },
                plugins: {
                    tooltip: {
                        callbacks: {
                            afterLabel: function(context) {
                                const server = servers[context.dataIndex];
                                return `Status: ${server.status}`;
                            }
                        }
                    }
                }
            }
        });

        this.charts.set(canvasId, chart);
        return chart;
    }

    // Multi-Region Latency Matrix using D3.js
    createLatencyMatrix(canvasId, matrixData) {
        const container = document.getElementById(canvasId);
        if (!container) return null;

        // Clear existing content
        container.innerHTML = '';

        const margin = { top: 50, right: 50, bottom: 50, left: 100 };
        const width = 400 - margin.left - margin.right;
        const height = 300 - margin.bottom - margin.top;

        const svg = d3.select(container)
            .append('svg')
            .attr('width', width + margin.left + margin.right)
            .attr('height', height + margin.top + margin.bottom);

        const g = svg.append('g')
            .attr('transform', `translate(${margin.left},${margin.top})`);

        // Create scales
        const regions = matrixData[0].slice(1);
        const x = d3.scaleBand().range([0, width]).domain(regions);
        const y = d3.scaleBand().range([height, 0]).domain(regions);

        // Color scale for latency
        const colorScale = d3.scaleSequential(d3.interpolateRdYlGn)
            .domain([200, 0]); // Reverse scale: high latency = red, low = green

        // Add axes
        g.append('g')
            .attr('transform', `translate(0,${height})`)
            .call(d3.axisBottom(x));

        g.append('g')
            .call(d3.axisLeft(y));

        // Add cells
        for (let i = 1; i < matrixData.length; i++) {
            for (let j = 1; j < matrixData[i].length; j++) {
                const value = matrixData[i][j];
                const sourceRegion = matrixData[i][0];
                const targetRegion = matrixData[0][j];

                g.append('rect')
                    .attr('x', x(targetRegion))
                    .attr('y', y(sourceRegion))
                    .attr('width', x.bandwidth())
                    .attr('height', y.bandwidth())
                    .attr('fill', value === 0 ? '#f0f0f0' : colorScale(value))
                    .attr('stroke', '#fff')
                    .attr('stroke-width', 1);

                if (value > 0) {
                    g.append('text')
                        .attr('x', x(targetRegion) + x.bandwidth() / 2)
                        .attr('y', y(sourceRegion) + y.bandwidth() / 2)
                        .attr('text-anchor', 'middle')
                        .attr('dominant-baseline', 'middle')
                        .attr('fill', value > 100 ? 'white' : 'black')
                        .attr('font-size', '12px')
                        .text(`${value}ms`);
                }
            }
        }

        // Add title
        g.append('text')
            .attr('x', width / 2)
            .attr('y', -20)
            .attr('text-anchor', 'middle')
            .attr('font-size', '14px')
            .attr('font-weight', 'bold')
            .text('Inter-Region Latency (ms)');
    }

    // Multi-Region Trends Chart
    createMultiRegionTrends(canvasId, trendsData) {
        const ctx = document.getElementById(canvasId);
        if (!ctx) return null;

        if (this.charts.has(canvasId)) {
            this.charts.get(canvasId).destroy();
        }

        const regions = Object.keys(trendsData[0]).filter(key => key !== 'timestamp');
        const colors = [this.defaultColors.primary, this.defaultColors.secondary, this.defaultColors.warning];

        const chart = new Chart(ctx, {
            type: 'line',
            data: {
                labels: trendsData.map(d => new Date(d.timestamp).toLocaleTimeString()),
                datasets: regions.map((region, index) => ({
                    label: region,
                    data: trendsData.map(d => d[region]),
                    borderColor: colors[index % colors.length],
                    backgroundColor: colors[index % colors.length] + '20',
                    fill: false,
                    tension: 0.4,
                    pointRadius: 2,
                    pointHoverRadius: 5
                }))
            },
            options: {
                ...this.getDefaultChartOptions(),
                scales: {
                    y: {
                        beginAtZero: true,
                        title: {
                            display: true,
                            text: 'Throughput (MB/s)'
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

        this.charts.set(canvasId, chart);
        return chart;
    }

    // Comparison Chart
    createComparisonChart(canvasId, data) {
        const ctx = document.getElementById(canvasId);
        if (!ctx) return null;

        if (this.charts.has(canvasId)) {
            this.charts.get(canvasId).destroy();
        }

        const chart = new Chart(ctx, {
            type: 'radar',
            data: data.benchmarkData,
            options: {
                ...this.getDefaultChartOptions(),
                scales: {
                    r: {
                        beginAtZero: true,
                        title: {
                            display: true,
                            text: 'Performance Score'
                        }
                    }
                }
            }
        });

        this.charts.set(canvasId, chart);
        return chart;
    }

    // Storage Usage Pie Chart
    createStorageUsage(canvasId, used, total) {
        const ctx = document.getElementById(canvasId);
        if (!ctx) return null;

        if (this.charts.has(canvasId)) {
            this.charts.get(canvasId).destroy();
        }

        const free = total - used;
        const usedPercentage = Math.round((used / total) * 100);

        const chart = new Chart(ctx, {
            type: 'doughnut',
            data: {
                labels: ['Used', 'Free'],
                datasets: [{
                    data: [used, free],
                    backgroundColor: [
                        usedPercentage > 80 ? this.defaultColors.danger : 
                        usedPercentage > 60 ? this.defaultColors.warning : this.defaultColors.primary,
                        '#f0f0f0'
                    ],
                    borderWidth: 2,
                    borderColor: '#ffffff'
                }]
            },
            options: {
                ...this.getDefaultChartOptions(),
                plugins: {
                    ...this.getDefaultChartOptions().plugins,
                    tooltip: {
                        callbacks: {
                            label: function(context) {
                                const label = context.label || '';
                                const value = context.parsed;
                                const percentage = Math.round((value / total) * 100);
                                return `${label}: ${value.toFixed(1)} TB (${percentage}%)`;
                            }
                        }
                    }
                }
            }
        });

        this.charts.set(canvasId, chart);
        return chart;
    }

    // Cleanup method
    destroyChart(canvasId) {
        if (this.charts.has(canvasId)) {
            this.charts.get(canvasId).destroy();
            this.charts.delete(canvasId);
        }
    }

    // Cleanup all charts
    destroyAllCharts() {
        this.charts.forEach((chart, canvasId) => {
            chart.destroy();
        });
        this.charts.clear();
    }
}

// Initialize global chart components
window.ChartComponents = new ChartComponents();