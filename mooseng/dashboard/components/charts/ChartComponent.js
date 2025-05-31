/**
 * Reusable Chart Component for MooseNG Dashboard
 * Provides a standardized interface for creating various chart types
 */

class ChartComponent {
    constructor(canvasId, options = {}) {
        this.canvasId = canvasId;
        this.canvas = document.getElementById(canvasId);
        this.ctx = this.canvas?.getContext('2d');
        this.chart = null;
        this.options = {
            responsive: true,
            maintainAspectRatio: false,
            animation: {
                duration: 750,
                easing: 'easeInOutQuart'
            },
            plugins: {
                legend: {
                    display: true,
                    position: 'top',
                    labels: {
                        usePointStyle: true,
                        padding: 20
                    }
                },
                tooltip: {
                    mode: 'index',
                    intersect: false,
                    backgroundColor: 'rgba(0, 0, 0, 0.8)',
                    titleColor: 'white',
                    bodyColor: 'white',
                    borderColor: '#2563eb',
                    borderWidth: 1
                }
            },
            ...options
        };
    }

    /**
     * Create a line chart
     */
    createLineChart(data, customOptions = {}) {
        if (!this.ctx) return null;

        const config = {
            type: 'line',
            data: this.formatData(data),
            options: {
                ...this.options,
                scales: {
                    x: {
                        grid: {
                            color: '#e2e8f0'
                        },
                        ticks: {
                            color: '#64748b'
                        }
                    },
                    y: {
                        grid: {
                            color: '#e2e8f0'
                        },
                        ticks: {
                            color: '#64748b'
                        }
                    }
                },
                elements: {
                    line: {
                        tension: 0.4,
                        borderWidth: 2
                    },
                    point: {
                        radius: 4,
                        hoverRadius: 6
                    }
                },
                ...customOptions
            }
        };

        this.destroy();
        this.chart = new Chart(this.ctx, config);
        return this.chart;
    }

    /**
     * Create a bar chart
     */
    createBarChart(data, customOptions = {}) {
        if (!this.ctx) return null;

        const config = {
            type: 'bar',
            data: this.formatData(data),
            options: {
                ...this.options,
                scales: {
                    x: {
                        grid: {
                            display: false
                        },
                        ticks: {
                            color: '#64748b'
                        }
                    },
                    y: {
                        grid: {
                            color: '#e2e8f0'
                        },
                        ticks: {
                            color: '#64748b'
                        }
                    }
                },
                ...customOptions
            }
        };

        this.destroy();
        this.chart = new Chart(this.ctx, config);
        return this.chart;
    }

    /**
     * Create a doughnut chart
     */
    createDoughnutChart(data, customOptions = {}) {
        if (!this.ctx) return null;

        const config = {
            type: 'doughnut',
            data: this.formatData(data),
            options: {
                ...this.options,
                cutout: '70%',
                plugins: {
                    ...this.options.plugins,
                    legend: {
                        position: 'bottom',
                        labels: {
                            usePointStyle: true,
                            padding: 20
                        }
                    }
                },
                ...customOptions
            }
        };

        this.destroy();
        this.chart = new Chart(this.ctx, config);
        return this.chart;
    }

    /**
     * Create a scatter plot
     */
    createScatterChart(data, customOptions = {}) {
        if (!this.ctx) return null;

        const config = {
            type: 'scatter',
            data: this.formatData(data),
            options: {
                ...this.options,
                scales: {
                    x: {
                        type: 'linear',
                        position: 'bottom',
                        grid: {
                            color: '#e2e8f0'
                        },
                        ticks: {
                            color: '#64748b'
                        }
                    },
                    y: {
                        grid: {
                            color: '#e2e8f0'
                        },
                        ticks: {
                            color: '#64748b'
                        }
                    }
                },
                ...customOptions
            }
        };

        this.destroy();
        this.chart = new Chart(this.ctx, config);
        return this.chart;
    }

    /**
     * Create a multi-axis chart
     */
    createMultiAxisChart(data, customOptions = {}) {
        if (!this.ctx) return null;

        const config = {
            type: 'line',
            data: this.formatData(data),
            options: {
                ...this.options,
                scales: {
                    x: {
                        grid: {
                            color: '#e2e8f0'
                        },
                        ticks: {
                            color: '#64748b'
                        }
                    },
                    y: {
                        type: 'linear',
                        display: true,
                        position: 'left',
                        grid: {
                            color: '#e2e8f0'
                        },
                        ticks: {
                            color: '#64748b'
                        }
                    },
                    y1: {
                        type: 'linear',
                        display: true,
                        position: 'right',
                        grid: {
                            drawOnChartArea: false,
                        },
                        ticks: {
                            color: '#64748b'
                        }
                    }
                },
                ...customOptions
            }
        };

        this.destroy();
        this.chart = new Chart(this.ctx, config);
        return this.chart;
    }

    /**
     * Format data for Chart.js
     */
    formatData(data) {
        if (data.datasets) {
            // Data is already in Chart.js format
            return data;
        }

        // Convert simple data format to Chart.js format
        const colors = [
            '#2563eb', '#ef4444', '#10b981', '#f59e0b', 
            '#8b5cf6', '#06b6d4', '#84cc16', '#f97316'
        ];

        if (Array.isArray(data.data)) {
            // Single dataset
            return {
                labels: data.labels || [],
                datasets: [{
                    label: data.label || 'Data',
                    data: data.data,
                    backgroundColor: data.backgroundColor || colors[0],
                    borderColor: data.borderColor || colors[0],
                    ...data.options
                }]
            };
        } else {
            // Multiple datasets
            const datasets = Object.keys(data.data).map((key, index) => ({
                label: key,
                data: data.data[key],
                backgroundColor: data.backgroundColor?.[index] || colors[index % colors.length],
                borderColor: data.borderColor?.[index] || colors[index % colors.length],
                ...data.options
            }));

            return {
                labels: data.labels || [],
                datasets
            };
        }
    }

    /**
     * Update chart data
     */
    updateData(newData) {
        if (!this.chart) return;

        const formattedData = this.formatData(newData);
        this.chart.data = formattedData;
        this.chart.update();
    }

    /**
     * Update chart options
     */
    updateOptions(newOptions) {
        if (!this.chart) return;

        this.chart.options = {
            ...this.chart.options,
            ...newOptions
        };
        this.chart.update();
    }

    /**
     * Destroy the chart
     */
    destroy() {
        if (this.chart) {
            this.chart.destroy();
            this.chart = null;
        }
    }

    /**
     * Resize the chart
     */
    resize() {
        if (this.chart) {
            this.chart.resize();
        }
    }

    /**
     * Get chart instance
     */
    getChart() {
        return this.chart;
    }

    /**
     * Export chart as image
     */
    exportAsImage(type = 'png') {
        if (!this.chart) return null;
        return this.chart.toBase64Image(type);
    }

    /**
     * Add real-time data point
     */
    addDataPoint(datasetIndex, value, label = null) {
        if (!this.chart) return;

        const chart = this.chart;
        
        if (label) {
            chart.data.labels.push(label);
        }
        
        chart.data.datasets[datasetIndex].data.push(value);
        
        // Keep only last 20 data points for performance
        if (chart.data.labels.length > 20) {
            chart.data.labels.shift();
            chart.data.datasets.forEach(dataset => {
                dataset.data.shift();
            });
        }
        
        chart.update('none'); // No animation for real-time updates
    }

    /**
     * Set up real-time updates
     */
    setupRealTimeUpdates(updateFunction, interval = 1000) {
        if (this.updateInterval) {
            clearInterval(this.updateInterval);
        }

        this.updateInterval = setInterval(() => {
            updateFunction(this);
        }, interval);
    }

    /**
     * Stop real-time updates
     */
    stopRealTimeUpdates() {
        if (this.updateInterval) {
            clearInterval(this.updateInterval);
            this.updateInterval = null;
        }
    }
}

// Static method to create performance chart preset
ChartComponent.createPerformanceChart = function(canvasId, data) {
    const chart = new ChartComponent(canvasId, {
        plugins: {
            title: {
                display: true,
                text: 'Performance Metrics'
            }
        }
    });
    
    return chart.createLineChart(data, {
        scales: {
            y: {
                beginAtZero: true,
                title: {
                    display: true,
                    text: 'Value'
                }
            }
        }
    });
};

// Static method to create health status chart preset
ChartComponent.createHealthChart = function(canvasId, data) {
    const chart = new ChartComponent(canvasId);
    
    return chart.createDoughnutChart(data, {
        plugins: {
            legend: {
                position: 'bottom'
            }
        }
    });
};

// Static method to create throughput comparison chart
ChartComponent.createThroughputChart = function(canvasId, data) {
    const chart = new ChartComponent(canvasId);
    
    return chart.createBarChart(data, {
        scales: {
            y: {
                beginAtZero: true,
                title: {
                    display: true,
                    text: 'Throughput (MB/s)'
                }
            }
        }
    });
};

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
    module.exports = ChartComponent;
}