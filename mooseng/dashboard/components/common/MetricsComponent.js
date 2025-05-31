/**
 * Metrics Component for MooseNG Dashboard
 * Handles metric data collection, processing, and display
 */

class MetricsComponent {
    constructor(containerId, options = {}) {
        this.containerId = containerId;
        this.container = document.getElementById(containerId);
        this.options = {
            refreshInterval: 30000, // 30 seconds
            maxDataPoints: 100,
            autoRefresh: true,
            ...options
        };
        
        this.data = new Map();
        this.listeners = new Map();
        this.refreshTimer = null;
        
        this.init();
    }

    init() {
        if (this.options.autoRefresh) {
            this.startAutoRefresh();
        }
    }

    /**
     * Register a metric for tracking
     */
    registerMetric(name, config = {}) {
        const metric = {
            name,
            values: [],
            timestamps: [],
            config: {
                unit: '',
                displayName: name,
                color: '#2563eb',
                thresholds: {},
                ...config
            }
        };
        
        this.data.set(name, metric);
        return metric;
    }

    /**
     * Add a data point to a metric
     */
    addDataPoint(metricName, value, timestamp = new Date()) {
        const metric = this.data.get(metricName);
        if (!metric) {
            console.warn(`Metric ${metricName} not found`);
            return;
        }

        metric.values.push(value);
        metric.timestamps.push(timestamp);

        // Keep only the last N data points
        if (metric.values.length > this.options.maxDataPoints) {
            metric.values.shift();
            metric.timestamps.shift();
        }

        // Notify listeners
        this.notifyListeners('dataAdded', metricName, value, timestamp);
    }

    /**
     * Get current value of a metric
     */
    getCurrentValue(metricName) {
        const metric = this.data.get(metricName);
        if (!metric || metric.values.length === 0) {
            return null;
        }
        return metric.values[metric.values.length - 1];
    }

    /**
     * Get historical data for a metric
     */
    getHistoricalData(metricName, count = 10) {
        const metric = this.data.get(metricName);
        if (!metric) {
            return { values: [], timestamps: [] };
        }

        const startIndex = Math.max(0, metric.values.length - count);
        return {
            values: metric.values.slice(startIndex),
            timestamps: metric.timestamps.slice(startIndex)
        };
    }

    /**
     * Calculate statistics for a metric
     */
    getStatistics(metricName) {
        const metric = this.data.get(metricName);
        if (!metric || metric.values.length === 0) {
            return null;
        }

        const values = metric.values;
        const sorted = [...values].sort((a, b) => a - b);
        
        const sum = values.reduce((a, b) => a + b, 0);
        const mean = sum / values.length;
        
        const variance = values.reduce((acc, val) => acc + Math.pow(val - mean, 2), 0) / values.length;
        const stdDev = Math.sqrt(variance);
        
        return {
            count: values.length,
            sum,
            mean,
            median: sorted[Math.floor(sorted.length / 2)],
            min: sorted[0],
            max: sorted[sorted.length - 1],
            stdDev,
            p95: sorted[Math.floor(sorted.length * 0.95)],
            p99: sorted[Math.floor(sorted.length * 0.99)]
        };
    }

    /**
     * Create a metric card
     */
    createMetricCard(metricName, options = {}) {
        const metric = this.data.get(metricName);
        if (!metric) {
            console.warn(`Metric ${metricName} not found`);
            return null;
        }

        const currentValue = this.getCurrentValue(metricName);
        const stats = this.getStatistics(metricName);
        
        const card = document.createElement('div');
        card.className = 'metric-card';
        card.id = `metric-card-${metricName}`;
        
        const changePercent = this.calculateChangePercent(metricName);
        const changeClass = changePercent > 0 ? 'positive' : changePercent < 0 ? 'negative' : 'neutral';
        
        card.innerHTML = `
            <div class="metric-value">${this.formatValue(currentValue, metric.config.unit)}</div>
            <div class="metric-label">${metric.config.displayName}</div>
            ${changePercent !== null ? `<div class="metric-change ${changeClass}">${this.formatChange(changePercent)}</div>` : ''}
            ${stats ? `<div class="metric-stats">
                <small>Min: ${this.formatValue(stats.min, metric.config.unit)} | 
                Max: ${this.formatValue(stats.max, metric.config.unit)} | 
                Avg: ${this.formatValue(stats.mean, metric.config.unit)}</small>
            </div>` : ''}
        `;

        // Add threshold indicators
        if (metric.config.thresholds && currentValue !== null) {
            const indicator = this.createThresholdIndicator(currentValue, metric.config.thresholds);
            if (indicator) {
                card.appendChild(indicator);
            }
        }

        return card;
    }

    /**
     * Create threshold indicator
     */
    createThresholdIndicator(value, thresholds) {
        const indicator = document.createElement('div');
        indicator.className = 'threshold-indicator';
        
        let status = 'normal';
        let color = '#10b981'; // green
        
        if (thresholds.critical !== undefined && value >= thresholds.critical) {
            status = 'critical';
            color = '#ef4444'; // red
        } else if (thresholds.warning !== undefined && value >= thresholds.warning) {
            status = 'warning';
            color = '#f59e0b'; // yellow
        }
        
        indicator.innerHTML = `
            <div class="threshold-bar">
                <div class="threshold-fill" style="background-color: ${color}; width: ${Math.min(100, (value / (thresholds.max || 100)) * 100)}%"></div>
            </div>
            <span class="threshold-status ${status}">${status.toUpperCase()}</span>
        `;
        
        return indicator;
    }

    /**
     * Render all metrics in the container
     */
    render() {
        if (!this.container) {
            console.warn(`Container ${this.containerId} not found`);
            return;
        }

        const fragment = document.createDocumentFragment();
        
        this.data.forEach((metric, name) => {
            const card = this.createMetricCard(name);
            if (card) {
                fragment.appendChild(card);
            }
        });

        this.container.innerHTML = '';
        this.container.appendChild(fragment);
    }

    /**
     * Update a specific metric card
     */
    updateMetricCard(metricName) {
        const cardId = `metric-card-${metricName}`;
        const existingCard = document.getElementById(cardId);
        
        if (existingCard) {
            const newCard = this.createMetricCard(metricName);
            if (newCard) {
                existingCard.replaceWith(newCard);
            }
        }
    }

    /**
     * Calculate percentage change from previous value
     */
    calculateChangePercent(metricName) {
        const metric = this.data.get(metricName);
        if (!metric || metric.values.length < 2) {
            return null;
        }

        const current = metric.values[metric.values.length - 1];
        const previous = metric.values[metric.values.length - 2];
        
        if (previous === 0) {
            return null;
        }
        
        return ((current - previous) / previous) * 100;
    }

    /**
     * Format a value with units
     */
    formatValue(value, unit = '') {
        if (value === null || value === undefined) {
            return 'N/A';
        }

        let formattedValue = value;
        
        // Format large numbers
        if (typeof value === 'number') {
            if (value >= 1000000) {
                formattedValue = (value / 1000000).toFixed(1) + 'M';
            } else if (value >= 1000) {
                formattedValue = (value / 1000).toFixed(1) + 'K';
            } else {
                formattedValue = value.toFixed(1);
            }
        }

        return unit ? `${formattedValue}${unit}` : formattedValue;
    }

    /**
     * Format change percentage
     */
    formatChange(percent) {
        if (percent === null || percent === undefined) {
            return '';
        }
        
        const sign = percent >= 0 ? '+' : '';
        return `${sign}${percent.toFixed(1)}%`;
    }

    /**
     * Add event listener
     */
    addEventListener(event, callback) {
        if (!this.listeners.has(event)) {
            this.listeners.set(event, []);
        }
        this.listeners.get(event).push(callback);
    }

    /**
     * Notify event listeners
     */
    notifyListeners(event, ...args) {
        const callbacks = this.listeners.get(event);
        if (callbacks) {
            callbacks.forEach(callback => {
                try {
                    callback(...args);
                } catch (error) {
                    console.error('Error in metric listener:', error);
                }
            });
        }
    }

    /**
     * Start automatic refresh
     */
    startAutoRefresh() {
        if (this.refreshTimer) {
            clearInterval(this.refreshTimer);
        }
        
        this.refreshTimer = setInterval(() => {
            this.refresh();
        }, this.options.refreshInterval);
    }

    /**
     * Stop automatic refresh
     */
    stopAutoRefresh() {
        if (this.refreshTimer) {
            clearInterval(this.refreshTimer);
            this.refreshTimer = null;
        }
    }

    /**
     * Refresh all metrics
     */
    async refresh() {
        try {
            await this.fetchLatestData();
            this.render();
            this.notifyListeners('refreshed');
        } catch (error) {
            console.error('Error refreshing metrics:', error);
            this.notifyListeners('error', error);
        }
    }

    /**
     * Fetch latest data from the server
     */
    async fetchLatestData() {
        // This would typically fetch from the Rust backend
        // For now, we'll simulate with random data updates
        
        this.data.forEach((metric, name) => {
            // Simulate new data point
            const currentValue = this.getCurrentValue(name) || 100;
            const variation = (Math.random() - 0.5) * 0.2; // ±10% variation
            const newValue = Math.max(0, currentValue * (1 + variation));
            
            this.addDataPoint(name, newValue);
        });
    }

    /**
     * Export metric data
     */
    exportData(format = 'json') {
        const exportData = {};
        
        this.data.forEach((metric, name) => {
            exportData[name] = {
                config: metric.config,
                values: metric.values,
                timestamps: metric.timestamps,
                statistics: this.getStatistics(name)
            };
        });

        switch (format) {
            case 'json':
                return JSON.stringify(exportData, null, 2);
            case 'csv':
                return this.exportToCSV(exportData);
            default:
                return exportData;
        }
    }

    /**
     * Export to CSV format
     */
    exportToCSV(data) {
        const headers = ['timestamp', 'metric', 'value'];
        const rows = [headers.join(',')];
        
        Object.entries(data).forEach(([metricName, metric]) => {
            metric.values.forEach((value, index) => {
                const timestamp = metric.timestamps[index];
                rows.push([timestamp.toISOString(), metricName, value].join(','));
            });
        });
        
        return rows.join('\n');
    }

    /**
     * Clean up resources
     */
    destroy() {
        this.stopAutoRefresh();
        this.data.clear();
        this.listeners.clear();
        
        if (this.container) {
            this.container.innerHTML = '';
        }
    }
}

// Static helper methods
MetricsComponent.createHealthMetric = function(name, value) {
    const colors = {
        healthy: '#10b981',
        warning: '#f59e0b',
        critical: '#ef4444',
        unknown: '#6b7280'
    };
    
    let status = 'unknown';
    if (value >= 90) status = 'healthy';
    else if (value >= 70) status = 'warning';
    else if (value >= 0) status = 'critical';
    
    return {
        name,
        value,
        status,
        color: colors[status],
        config: {
            unit: '%',
            displayName: name,
            color: colors[status],
            thresholds: {
                warning: 70,
                critical: 50,
                max: 100
            }
        }
    };
};

MetricsComponent.createPerformanceMetric = function(name, value, unit = '') {
    return {
        name,
        value,
        config: {
            unit,
            displayName: name,
            color: '#2563eb'
        }
    };
};

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
    module.exports = MetricsComponent;
}