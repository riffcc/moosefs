/**
 * ChartFactory - Advanced Chart Component System for MooseNG Dashboard
 * 
 * Provides reusable, configurable chart components using Chart.js and D3.js
 * with standardized theming, responsive behavior, and real-time updates.
 */

class ChartFactory {
    constructor() {
        this.defaultColors = {
            primary: '#2563eb',
            secondary: '#64748b',
            success: '#10b981',
            warning: '#f59e0b',
            error: '#ef4444',
            info: '#06b6d4',
            purple: '#8b5cf6',
            pink: '#ec4899'
        };
        
        this.gradients = new Map();
        this.charts = new Map();
        this.themes = {
            light: {
                background: '#ffffff',
                text: '#1e293b',
                grid: '#e2e8f0',
                tooltip: '#1e293b'
            },
            dark: {
                background: '#1e293b',
                text: '#f8fafc',
                grid: '#475569',
                tooltip: '#f8fafc'
            }
        };
        
        this.currentTheme = 'light';
    }

    // Core chart creation methods
    createChart(type, containerId, data, options = {}) {
        const container = document.getElementById(containerId);
        if (!container) {
            console.error(`Container ${containerId} not found`);
            return null;
        }

        // Destroy existing chart if it exists
        if (this.charts.has(containerId)) {
            this.charts.get(containerId).destroy();
        }

        const canvas = this.ensureCanvas(container, containerId);
        const ctx = canvas.getContext('2d');
        
        const chartConfig = this.buildChartConfig(type, data, options);
        const chart = new Chart(ctx, chartConfig);
        
        this.charts.set(containerId, chart);
        return chart;
    }

    // Specialized chart creators
    createTimeSeriesChart(containerId, datasets, options = {}) {
        const config = {
            type: 'line',
            responsive: true,
            maintainAspectRatio: false,
            tension: 0.4,
            pointRadius: 2,
            pointHoverRadius: 6,
            ...options
        };

        return this.createChart('timeseries', containerId, datasets, config);
    }

    createPerformanceMetricsChart(containerId, metrics, options = {}) {
        const datasets = metrics.map((metric, index) => ({
            label: metric.name,
            data: metric.values,
            borderColor: this.getColor(index),
            backgroundColor: this.getGradient(containerId, this.getColor(index)),
            fill: false,
            tension: 0.4
        }));

        return this.createTimeSeriesChart(containerId, {
            labels: metrics[0]?.timestamps || [],
            datasets
        }, {
            plugins: {
                title: {
                    display: true,
                    text: options.title || 'Performance Metrics'
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
                        text: options.yAxisTitle || 'Value'
                    },
                    beginAtZero: true
                }
            },
            ...options
        });
    }

    createThroughputLatencyChart(containerId, data, options = {}) {
        const datasets = [
            {
                label: 'Throughput (MB/s)',
                data: data.throughput,
                borderColor: this.defaultColors.primary,
                backgroundColor: this.getGradient(containerId, this.defaultColors.primary, 0.2),
                yAxisID: 'y'
            },
            {
                label: 'Latency (ms)',
                data: data.latency,
                borderColor: this.defaultColors.warning,
                backgroundColor: this.getGradient(containerId, this.defaultColors.warning, 0.2),
                yAxisID: 'y1'
            }
        ];

        return this.createChart('line', containerId, {
            labels: data.timestamps,
            datasets
        }, {
            responsive: true,
            maintainAspectRatio: false,
            plugins: {
                title: {
                    display: true,
                    text: 'Throughput vs Latency'
                }
            },
            scales: {
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
                        drawOnChartArea: false
                    }
                }
            },
            ...options
        });
    }

    createDistributionChart(containerId, data, options = {}) {
        return this.createChart('doughnut', containerId, {
            labels: data.labels,
            datasets: [{
                data: data.values,
                backgroundColor: data.labels.map((_, index) => this.getColor(index)),
                borderWidth: 2,
                borderColor: '#ffffff'
            }]
        }, {
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
                    callbacks: {
                        label: function(context) {
                            const total = context.dataset.data.reduce((a, b) => a + b, 0);
                            const percentage = ((context.parsed / total) * 100).toFixed(1);
                            return `${context.label}: ${context.parsed} (${percentage}%)`;
                        }
                    }
                }
            },
            ...options
        });
    }

    createHeatmapChart(containerId, data, options = {}) {
        // Create D3.js heatmap for more complex visualizations
        const container = document.getElementById(containerId);
        if (!container) return null;

        // Clear existing content
        container.innerHTML = '';

        const margin = { top: 20, right: 20, bottom: 30, left: 40 };
        const width = container.clientWidth - margin.left - margin.right;
        const height = container.clientHeight - margin.top - margin.bottom;

        const svg = d3.select(container)
            .append('svg')
            .attr('width', width + margin.left + margin.right)
            .attr('height', height + margin.bottom + margin.top)
            .append('g')
            .attr('transform', `translate(${margin.left},${margin.top})`);

        // Create scales
        const xScale = d3.scaleBand()
            .range([0, width])
            .domain(data.xLabels)
            .padding(0.1);

        const yScale = d3.scaleBand()
            .range([height, 0])
            .domain(data.yLabels)
            .padding(0.1);

        const colorScale = d3.scaleSequential()
            .interpolator(d3.interpolateBlues)
            .domain(d3.extent(data.values.flat()));

        // Create rectangles
        svg.selectAll('rect')
            .data(data.values.flat())
            .enter()
            .append('rect')
            .attr('x', (d, i) => xScale(data.xLabels[i % data.xLabels.length]))
            .attr('y', (d, i) => yScale(data.yLabels[Math.floor(i / data.xLabels.length)]))
            .attr('width', xScale.bandwidth())
            .attr('height', yScale.bandwidth())
            .style('fill', d => colorScale(d))
            .style('stroke', '#ffffff')
            .style('stroke-width', 1)
            .on('mouseover', function(event, d) {
                d3.select(this).style('opacity', 0.8);
                // Add tooltip
            })
            .on('mouseout', function(d) {
                d3.select(this).style('opacity', 1);
            });

        // Add axes
        svg.append('g')
            .attr('transform', `translate(0,${height})`)
            .call(d3.axisBottom(xScale));

        svg.append('g')
            .call(d3.axisLeft(yScale));

        return svg;
    }

    createMultiRegionTopology(containerId, regionData, options = {}) {
        const container = document.getElementById(containerId);
        if (!container) return null;

        container.innerHTML = '';

        const width = container.clientWidth;
        const height = container.clientHeight;

        const svg = d3.select(container)
            .append('svg')
            .attr('width', width)
            .attr('height', height);

        // Create force simulation for network topology
        const simulation = d3.forceSimulation(regionData.nodes)
            .force('link', d3.forceLink(regionData.links).id(d => d.id).distance(100))
            .force('charge', d3.forceManyBody().strength(-300))
            .force('center', d3.forceCenter(width / 2, height / 2));

        // Add links
        const links = svg.append('g')
            .selectAll('line')
            .data(regionData.links)
            .enter()
            .append('line')
            .attr('stroke', '#999')
            .attr('stroke-opacity', 0.6)
            .attr('stroke-width', d => Math.sqrt(d.value || 1));

        // Add nodes
        const nodes = svg.append('g')
            .selectAll('circle')
            .data(regionData.nodes)
            .enter()
            .append('circle')
            .attr('r', d => d.size || 20)
            .attr('fill', d => this.getRegionColor(d.region))
            .attr('stroke', '#fff')
            .attr('stroke-width', 2)
            .call(d3.drag()
                .on('start', dragstarted)
                .on('drag', dragged)
                .on('end', dragended));

        // Add labels
        const labels = svg.append('g')
            .selectAll('text')
            .data(regionData.nodes)
            .enter()
            .append('text')
            .text(d => d.name)
            .attr('font-size', 12)
            .attr('font-family', 'Arial, sans-serif')
            .attr('text-anchor', 'middle')
            .attr('dy', 5);

        // Update positions on simulation tick
        simulation.on('tick', () => {
            links
                .attr('x1', d => d.source.x)
                .attr('y1', d => d.source.y)
                .attr('x2', d => d.target.x)
                .attr('y2', d => d.target.y);

            nodes
                .attr('cx', d => d.x)
                .attr('cy', d => d.y);

            labels
                .attr('x', d => d.x)
                .attr('y', d => d.y);
        });

        function dragstarted(event, d) {
            if (!event.active) simulation.alphaTarget(0.3).restart();
            d.fx = d.x;
            d.fy = d.y;
        }

        function dragged(event, d) {
            d.fx = event.x;
            d.fy = event.y;
        }

        function dragended(event, d) {
            if (!event.active) simulation.alphaTarget(0);
            d.fx = null;
            d.fy = null;
        }

        return svg;
    }

    // Utility methods
    buildChartConfig(type, data, options) {
        const baseConfig = {
            type,
            data,
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        display: true,
                        position: 'top'
                    },
                    tooltip: {
                        mode: 'index',
                        intersect: false,
                        backgroundColor: this.themes[this.currentTheme].tooltip,
                        titleColor: this.themes[this.currentTheme].background,
                        bodyColor: this.themes[this.currentTheme].background
                    }
                },
                scales: this.getDefaultScales(type),
                animation: {
                    duration: 750,
                    easing: 'easeInOutCubic'
                }
            }
        };

        // Merge with custom options
        return this.deepMerge(baseConfig, { options });
    }

    getDefaultScales(type) {
        if (['pie', 'doughnut', 'radar'].includes(type)) {
            return {};
        }

        return {
            x: {
                grid: {
                    color: this.themes[this.currentTheme].grid,
                    borderColor: this.themes[this.currentTheme].grid
                },
                ticks: {
                    color: this.themes[this.currentTheme].text
                }
            },
            y: {
                grid: {
                    color: this.themes[this.currentTheme].grid,
                    borderColor: this.themes[this.currentTheme].grid
                },
                ticks: {
                    color: this.themes[this.currentTheme].text
                },
                beginAtZero: true
            }
        };
    }

    ensureCanvas(container, id) {
        let canvas = container.querySelector('canvas');
        if (!canvas) {
            canvas = document.createElement('canvas');
            canvas.id = `${id}-canvas`;
            container.appendChild(canvas);
        }
        return canvas;
    }

    getColor(index) {
        const colors = Object.values(this.defaultColors);
        return colors[index % colors.length];
    }

    getRegionColor(region) {
        const regionColors = {
            'us-east': this.defaultColors.primary,
            'us-west': this.defaultColors.info,
            'eu-west': this.defaultColors.success,
            'eu-central': this.defaultColors.warning,
            'ap-southeast': this.defaultColors.error,
            'ap-northeast': this.defaultColors.purple
        };
        return regionColors[region] || this.defaultColors.secondary;
    }

    getGradient(containerId, color, opacity = 0.2) {
        const key = `${containerId}-${color}-${opacity}`;
        if (this.gradients.has(key)) {
            return this.gradients.get(key);
        }

        const canvas = document.createElement('canvas');
        const ctx = canvas.getContext('2d');
        const gradient = ctx.createLinearGradient(0, 0, 0, 400);
        
        gradient.addColorStop(0, this.hexToRgba(color, opacity));
        gradient.addColorStop(1, this.hexToRgba(color, 0));
        
        this.gradients.set(key, gradient);
        return gradient;
    }

    hexToRgba(hex, alpha) {
        const r = parseInt(hex.slice(1, 3), 16);
        const g = parseInt(hex.slice(3, 5), 16);
        const b = parseInt(hex.slice(5, 7), 16);
        return `rgba(${r}, ${g}, ${b}, ${alpha})`;
    }

    // Chart management
    updateChart(containerId, newData) {
        const chart = this.charts.get(containerId);
        if (!chart) return;

        chart.data = newData;
        chart.update('active');
    }

    destroyChart(containerId) {
        const chart = this.charts.get(containerId);
        if (chart) {
            chart.destroy();
            this.charts.delete(containerId);
        }
    }

    resizeCharts() {
        this.charts.forEach(chart => {
            chart.resize();
        });
    }

    setTheme(theme) {
        this.currentTheme = theme;
        // Update all existing charts with new theme
        this.charts.forEach((chart, containerId) => {
            chart.options.scales = this.getDefaultScales(chart.config.type);
            chart.update();
        });
    }

    // Utility method for deep merging objects
    deepMerge(target, source) {
        const result = { ...target };
        for (const key in source) {
            if (source[key] && typeof source[key] === 'object' && !Array.isArray(source[key])) {
                result[key] = this.deepMerge(result[key] || {}, source[key]);
            } else {
                result[key] = source[key];
            }
        }
        return result;
    }

    // Export chart as image
    exportChart(containerId, format = 'png') {
        const chart = this.charts.get(containerId);
        if (!chart) return null;

        const url = chart.toBase64Image();
        const link = document.createElement('a');
        link.download = `${containerId}-chart.${format}`;
        link.href = url;
        link.click();
    }

    // Animation helpers
    animateIn(containerId, duration = 1000) {
        const chart = this.charts.get(containerId);
        if (!chart) return;

        chart.options.animation.duration = duration;
        chart.update();
    }

    // Real-time update helpers
    addDataPoint(containerId, label, data) {
        const chart = this.charts.get(containerId);
        if (!chart) return;

        chart.data.labels.push(label);
        chart.data.datasets.forEach((dataset, index) => {
            dataset.data.push(data[index] || 0);
        });

        // Keep only last 50 data points for performance
        if (chart.data.labels.length > 50) {
            chart.data.labels.shift();
            chart.data.datasets.forEach(dataset => {
                dataset.data.shift();
            });
        }

        chart.update('none');
    }
}

// Export factory instance
window.ChartFactory = new ChartFactory();

// Export for module systems
if (typeof module !== 'undefined' && module.exports) {
    module.exports = ChartFactory;
}