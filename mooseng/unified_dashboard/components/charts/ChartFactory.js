/**
 * Chart Factory Module
 * Creates and manages various chart types for the MooseNG benchmark dashboard
 */

class ChartFactory {
    constructor(chartManager) {
        this.chartManager = chartManager;
        this.colorPalette = {
            primary: '#007bff',
            success: '#28a745',
            danger: '#dc3545',
            warning: '#ffc107',
            info: '#17a2b8',
            light: '#f8f9fa',
            dark: '#343a40',
            secondary: '#6c757d'
        };
        
        this.gradients = new Map();
    }

    // Create performance overview chart
    createPerformanceOverviewChart(canvasId, data) {
        const chartData = {
            labels: data.labels || [],
            datasets: [{
                label: 'Throughput (MB/s)',
                data: data.throughput || [],
                borderColor: this.colorPalette.primary,
                backgroundColor: this.createGradient(canvasId, this.colorPalette.primary, 0.2),
                borderWidth: 2,
                fill: true,
                tension: 0.4,
                yAxisID: 'y'
            }, {
                label: 'Latency (ms)',
                data: data.latency || [],
                borderColor: this.colorPalette.danger,
                backgroundColor: this.createGradient(canvasId, this.colorPalette.danger, 0.2),
                borderWidth: 2,
                fill: true,
                tension: 0.4,
                yAxisID: 'y1'
            }]
        };

        const options = {
            responsive: true,
            interaction: {
                mode: 'index',
                intersect: false,
            },
            plugins: {
                title: {
                    display: true,
                    text: 'Performance Overview'
                },
                legend: {
                    position: 'top'
                }
            },
            scales: {
                x: {
                    display: true,
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

        return this.chartManager.createChart(canvasId, 'line', chartData, options);
    }

    // Create benchmark comparison chart
    createBenchmarkComparisonChart(canvasId, data) {
        const chartData = {
            labels: data.benchmarks || [],
            datasets: [{
                label: 'Current Results',
                data: data.current || [],
                backgroundColor: this.colorPalette.primary,
                borderColor: this.colorPalette.primary,
                borderWidth: 1
            }, {
                label: 'Baseline',
                data: data.baseline || [],
                backgroundColor: this.colorPalette.secondary,
                borderColor: this.colorPalette.secondary,
                borderWidth: 1
            }]
        };

        const options = {
            responsive: true,
            plugins: {
                title: {
                    display: true,
                    text: 'Benchmark Comparison'
                },
                legend: {
                    position: 'top'
                }
            },
            scales: {
                y: {
                    beginAtZero: true,
                    title: {
                        display: true,
                        text: 'Performance Score'
                    }
                }
            }
        };

        return this.chartManager.createChart(canvasId, 'bar', chartData, options);
    }

    // Create resource utilization chart
    createResourceUtilizationChart(canvasId, data) {
        const chartData = {
            labels: data.timestamps || [],
            datasets: [{
                label: 'CPU Usage (%)',
                data: data.cpu || [],
                borderColor: this.colorPalette.danger,
                backgroundColor: this.createGradient(canvasId, this.colorPalette.danger, 0.1),
                borderWidth: 2,
                fill: true,
                tension: 0.3
            }, {
                label: 'Memory Usage (%)',
                data: data.memory || [],
                borderColor: this.colorPalette.warning,
                backgroundColor: this.createGradient(canvasId, this.colorPalette.warning, 0.1),
                borderWidth: 2,
                fill: true,
                tension: 0.3
            }, {
                label: 'Disk I/O (MB/s)',
                data: data.disk_io || [],
                borderColor: this.colorPalette.info,
                backgroundColor: this.createGradient(canvasId, this.colorPalette.info, 0.1),
                borderWidth: 2,
                fill: true,
                tension: 0.3
            }, {
                label: 'Network I/O (MB/s)',
                data: data.network_io || [],
                borderColor: this.colorPalette.success,
                backgroundColor: this.createGradient(canvasId, this.colorPalette.success, 0.1),
                borderWidth: 2,
                fill: true,
                tension: 0.3
            }]
        };

        const options = {
            responsive: true,
            interaction: {
                mode: 'index',
                intersect: false,
            },
            plugins: {
                title: {
                    display: true,
                    text: 'Resource Utilization'
                },
                legend: {
                    position: 'top'
                }
            },
            scales: {
                x: {
                    display: true,
                    title: {
                        display: true,
                        text: 'Time'
                    }
                },
                y: {
                    display: true,
                    title: {
                        display: true,
                        text: 'Utilization'
                    },
                    max: 100
                }
            }
        };

        return this.chartManager.createChart(canvasId, 'line', chartData, options);
    }

    // Create benchmark category distribution pie chart
    createCategoryDistributionChart(canvasId, data) {
        const chartData = {
            labels: data.categories || [],
            datasets: [{
                data: data.counts || [],
                backgroundColor: [
                    this.colorPalette.primary,
                    this.colorPalette.success,
                    this.colorPalette.warning,
                    this.colorPalette.danger,
                    this.colorPalette.info,
                    this.colorPalette.secondary
                ],
                borderWidth: 2,
                borderColor: '#ffffff'
            }]
        };

        const options = {
            responsive: true,
            plugins: {
                title: {
                    display: true,
                    text: 'Benchmark Categories'
                },
                legend: {
                    position: 'right'
                }
            }
        };

        return this.chartManager.createChart(canvasId, 'doughnut', chartData, options);
    }

    // Create latency distribution histogram
    createLatencyHistogramChart(canvasId, data) {
        const chartData = {
            labels: data.bins || [],
            datasets: [{
                label: 'Frequency',
                data: data.frequencies || [],
                backgroundColor: this.colorPalette.info,
                borderColor: this.colorPalette.info,
                borderWidth: 1
            }]
        };

        const options = {
            responsive: true,
            plugins: {
                title: {
                    display: true,
                    text: 'Latency Distribution'
                },
                legend: {
                    display: false
                }
            },
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
                        text: 'Frequency'
                    }
                }
            }
        };

        return this.chartManager.createChart(canvasId, 'bar', chartData, options);
    }

    // Create trend analysis chart
    createTrendAnalysisChart(canvasId, data) {
        const chartData = {
            labels: data.dates || [],
            datasets: [{
                label: 'Performance Trend',
                data: data.performance || [],
                borderColor: this.colorPalette.primary,
                backgroundColor: this.createGradient(canvasId, this.colorPalette.primary, 0.1),
                borderWidth: 2,
                fill: true,
                tension: 0.3
            }, {
                label: 'Regression Line',
                data: data.regression || [],
                borderColor: this.colorPalette.danger,
                borderWidth: 2,
                fill: false,
                borderDash: [5, 5],
                pointRadius: 0
            }]
        };

        const options = {
            responsive: true,
            interaction: {
                mode: 'index',
                intersect: false,
            },
            plugins: {
                title: {
                    display: true,
                    text: 'Performance Trend Analysis'
                },
                legend: {
                    position: 'top'
                }
            },
            scales: {
                x: {
                    title: {
                        display: true,
                        text: 'Date'
                    }
                },
                y: {
                    title: {
                        display: true,
                        text: 'Performance Score'
                    }
                }
            }
        };

        return this.chartManager.createChart(canvasId, 'line', chartData, options);
    }

    // Create real-time monitoring chart
    createRealTimeChart(canvasId, maxDataPoints = 50) {
        const chartData = {
            labels: [],
            datasets: [{
                label: 'Live Throughput',
                data: [],
                borderColor: this.colorPalette.success,
                backgroundColor: this.createGradient(canvasId, this.colorPalette.success, 0.2),
                borderWidth: 2,
                fill: true,
                tension: 0.4
            }]
        };

        const options = {
            responsive: true,
            animation: {
                duration: 0 // Disable animation for real-time updates
            },
            interaction: {
                intersect: false,
            },
            plugins: {
                title: {
                    display: true,
                    text: 'Real-time Performance'
                },
                legend: {
                    position: 'top'
                }
            },
            scales: {
                x: {
                    type: 'realtime',
                    realtime: {
                        duration: 20000,
                        refresh: 1000,
                        delay: 2000,
                        onRefresh: (chart) => {
                            // This will be called by external data updates
                        }
                    }
                },
                y: {
                    title: {
                        display: true,
                        text: 'Throughput (MB/s)'
                    }
                }
            }
        };

        const chart = this.chartManager.createChart(canvasId, 'line', chartData, options);
        
        // Store reference for real-time updates
        if (chart) {
            chart.maxDataPoints = maxDataPoints;
            chart.addDataPoint = (value, label) => {
                const data = chart.data;
                data.labels.push(label || new Date().toLocaleTimeString());
                data.datasets[0].data.push(value);

                // Keep only recent data points
                if (data.labels.length > maxDataPoints) {
                    data.labels.shift();
                    data.datasets[0].data.shift();
                }

                chart.update('none');
            };
        }

        return chart;
    }

    // Create multi-region performance comparison
    createMultiRegionChart(canvasId, data) {
        const datasets = [];
        const regions = data.regions || [];
        const colors = [
            this.colorPalette.primary,
            this.colorPalette.success,
            this.colorPalette.warning,
            this.colorPalette.danger,
            this.colorPalette.info
        ];

        regions.forEach((region, index) => {
            datasets.push({
                label: region.name,
                data: region.performance || [],
                borderColor: colors[index % colors.length],
                backgroundColor: this.createGradient(canvasId, colors[index % colors.length], 0.1),
                borderWidth: 2,
                fill: false,
                tension: 0.3
            });
        });

        const chartData = {
            labels: data.timestamps || [],
            datasets: datasets
        };

        const options = {
            responsive: true,
            interaction: {
                mode: 'index',
                intersect: false,
            },
            plugins: {
                title: {
                    display: true,
                    text: 'Multi-Region Performance'
                },
                legend: {
                    position: 'top'
                }
            },
            scales: {
                x: {
                    title: {
                        display: true,
                        text: 'Time'
                    }
                },
                y: {
                    title: {
                        display: true,
                        text: 'Performance Score'
                    }
                }
            }
        };

        return this.chartManager.createChart(canvasId, 'line', chartData, options);
    }

    // Helper method to create gradients
    createGradient(canvasId, color, alpha = 0.2) {
        const canvas = document.getElementById(canvasId);
        if (!canvas) return color;

        const ctx = canvas.getContext('2d');
        const gradient = ctx.createLinearGradient(0, 0, 0, 400);
        
        // Convert hex to rgba
        const r = parseInt(color.slice(1, 3), 16);
        const g = parseInt(color.slice(3, 5), 16);
        const b = parseInt(color.slice(5, 7), 16);
        
        gradient.addColorStop(0, `rgba(${r}, ${g}, ${b}, ${alpha})`);
        gradient.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0)`);
        
        this.gradients.set(`${canvasId}-${color}`, gradient);
        return gradient;
    }

    // Update chart with new data
    updateChart(canvasId, newData) {
        this.chartManager.updateChart(canvasId, newData);
    }

    // Destroy chart
    destroyChart(canvasId) {
        const chart = this.chartManager.core.charts.get(canvasId);
        if (chart) {
            chart.destroy();
            this.chartManager.core.charts.delete(canvasId);
        }
    }

    // Get chart instance
    getChart(canvasId) {
        return this.chartManager.core.charts.get(canvasId);
    }

    // Update theme for all charts
    updateTheme(isDark) {
        const textColor = isDark ? '#ffffff' : '#000000';
        const gridColor = isDark ? '#404040' : '#e0e0e0';

        this.chartManager.core.charts.forEach((chart, canvasId) => {
            if (chart.options.scales) {
                Object.keys(chart.options.scales).forEach(scaleKey => {
                    if (chart.options.scales[scaleKey].title) {
                        chart.options.scales[scaleKey].title.color = textColor;
                    }
                    if (chart.options.scales[scaleKey].ticks) {
                        chart.options.scales[scaleKey].ticks.color = textColor;
                    }
                    if (chart.options.scales[scaleKey].grid) {
                        chart.options.scales[scaleKey].grid.color = gridColor;
                    }
                });
            }

            if (chart.options.plugins) {
                if (chart.options.plugins.title) {
                    chart.options.plugins.title.color = textColor;
                }
                if (chart.options.plugins.legend) {
                    chart.options.plugins.legend.labels = {
                        ...chart.options.plugins.legend.labels,
                        color: textColor
                    };
                }
            }

            chart.update();
        });
    }
}

export { ChartFactory };