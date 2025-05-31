/**
 * Overview Section Component
 * Handles the main dashboard overview with KPIs and performance charts
 */

class OverviewSection {
    constructor() {
        this.data = null;
        this.updateInterval = null;
    }

    async load() {
        try {
            this.data = await window.DataManager.getOverviewData();
            this.render();
            this.startAutoUpdate();
        } catch (error) {
            console.error('Error loading overview data:', error);
            this.renderError();
        }
    }

    render() {
        if (!this.data) return;

        this.updateKPIs();
        this.createCharts();
    }

    updateKPIs() {
        // Update KPI values
        const elements = {
            'overall-grade': this.data.overall_grade,
            'throughput-value': `${this.data.throughput_mbps} MB/s`,
            'ops-value': `${this.data.ops_per_second} ops/s`,
            'response-time-value': `${this.data.total_time_ms} ms`
        };

        Object.entries(elements).forEach(([id, value]) => {
            const element = document.getElementById(id);
            if (element) {
                element.textContent = value;
                element.classList.add('fade-in');
            }
        });

        // Update trends (simulate trend calculations)
        this.updateTrends();
    }

    updateTrends() {
        const trends = {
            'overall-trend': { direction: 'up', value: '5%' },
            'throughput-trend': { direction: 'up', value: '12%' },
            'ops-trend': { direction: 'up', value: '8%' },
            'response-time-trend': { direction: 'down', value: '3%' }
        };

        Object.entries(trends).forEach(([id, trend]) => {
            const element = document.getElementById(id);
            if (element) {
                const arrow = trend.direction === 'up' ? '↗' : '↘';
                const className = trend.direction === 'up' ? 'trend-up' : 
                                 trend.direction === 'down' ? 'trend-down' : 'trend-neutral';
                
                element.innerHTML = `<span class="${className}">${arrow} ${trend.value}</span>`;
            }
        });
    }

    createCharts() {
        // Performance Breakdown Chart
        if (this.data.performance_breakdown) {
            window.ChartComponents.createPerformanceBreakdown(
                'performance-breakdown-chart',
                this.data.performance_breakdown
            );
        }

        // Throughput Timeline Chart
        if (this.data.historical_data) {
            window.ChartComponents.createThroughputTimeline(
                'throughput-timeline-chart',
                this.data.historical_data
            );

            // Response Time Chart
            window.ChartComponents.createResponseTimeChart(
                'response-time-chart',
                this.data.historical_data
            );
        }

        // Health Score Chart (calculate based on performance)
        const healthScore = this.calculateHealthScore();
        window.ChartComponents.createHealthScoreChart(
            'health-score-chart',
            healthScore
        );
    }

    calculateHealthScore() {
        if (!this.data) return 0;

        // Simple health score calculation based on performance metrics
        let score = 100;
        
        // Throughput factor (lower is worse)
        const throughput = parseFloat(this.data.throughput_mbps);
        if (throughput < 50) score -= 20;
        else if (throughput < 100) score -= 10;
        
        // Response time factor (higher is worse)
        const responseTime = this.data.total_time_ms;
        if (responseTime > 5000) score -= 20;
        else if (responseTime > 3000) score -= 10;
        
        // Operations per second factor
        const ops = parseFloat(this.data.ops_per_second);
        if (ops < 500) score -= 15;
        else if (ops < 1000) score -= 5;
        
        // Grade factor
        if (this.data.overall_grade.includes('C') || this.data.overall_grade.includes('D')) {
            score -= 25;
        } else if (this.data.overall_grade.includes('B')) {
            score -= 10;
        }

        return Math.max(0, Math.min(100, Math.round(score)));
    }

    startAutoUpdate() {
        // Update every 30 seconds
        this.updateInterval = setInterval(async () => {
            try {
                const newData = await window.DataManager.getOverviewData();
                if (newData) {
                    this.data = newData;
                    this.render();
                }
            } catch (error) {
                console.error('Error updating overview data:', error);
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
        const sections = ['overview'];
        sections.forEach(sectionId => {
            const section = document.getElementById(sectionId);
            if (section) {
                section.innerHTML = `
                    <div class="error-message">
                        <h3>Error Loading Data</h3>
                        <p>Unable to load performance data. Please check your connection and try again.</p>
                        <button onclick="window.OverviewSection.load()" class="retry-button">
                            Retry
                        </button>
                    </div>
                `;
            }
        });
    }

    // Export data for reports
    exportData() {
        if (!this.data) return null;

        return {
            timestamp: new Date().toISOString(),
            summary: {
                grade: this.data.overall_grade,
                throughput: this.data.throughput_mbps,
                ops_per_second: this.data.ops_per_second,
                response_time: this.data.total_time_ms,
                health_score: this.calculateHealthScore()
            },
            breakdown: this.data.performance_breakdown,
            historical: this.data.historical_data
        };
    }

    // Cleanup method
    destroy() {
        this.stopAutoUpdate();
        
        // Destroy overview charts
        ['performance-breakdown-chart', 'throughput-timeline-chart', 
         'response-time-chart', 'health-score-chart'].forEach(chartId => {
            window.ChartComponents.destroyChart(chartId);
        });
    }
}

// Initialize global overview section
window.OverviewSection = new OverviewSection();