/**
 * Main JavaScript Module for MooseNG Unified Dashboard
 * Handles initialization and coordination between components
 */

import { DashboardCore } from '../../src/dashboard-core.js';
import { ChartFactory } from '../../components/charts/ChartFactory.js';

class DashboardApp {
    constructor() {
        this.dashboard = null;
        this.chartFactory = null;
        this.initialized = false;
    }

    async init() {
        if (this.initialized) return;

        try {
            // Initialize core dashboard
            this.dashboard = new DashboardCore();
            
            // Initialize chart factory
            this.chartFactory = new ChartFactory(this.dashboard.chartManager);
            
            // Setup initial charts
            await this.setupInitialCharts();
            
            // Setup page-specific handlers
            this.setupPageHandlers();
            
            // Setup theme management
            this.setupThemeManagement();
            
            // Setup real-time updates
            this.setupRealTimeUpdates();
            
            // Hide loading overlay
            this.hideLoadingOverlay();
            
            this.initialized = true;
            
            console.log('MooseNG Dashboard initialized successfully');
            
        } catch (error) {
            console.error('Failed to initialize dashboard:', error);
            this.showError('Failed to initialize dashboard. Please refresh the page.');
        }
    }

    async setupInitialCharts() {
        // Setup overview page charts
        await this.setupOverviewCharts();
    }

    async setupOverviewCharts() {
        try {
            // Performance timeline chart
            const timelineData = {
                labels: this.generateTimeLabels(24),
                throughput: this.generateMockData(24, 800, 1500),
                latency: this.generateMockData(24, 8, 25)
            };
            
            this.chartFactory.createPerformanceOverviewChart('performanceTimelineChart', timelineData);

            // Category distribution chart
            const categoryData = {
                categories: ['File I/O', 'Metadata', 'Network', 'Multi-region'],
                counts: [45, 23, 18, 14]
            };
            
            this.chartFactory.createCategoryDistributionChart('categoryDistributionChart', categoryData);

            console.log('Overview charts initialized');
            
        } catch (error) {
            console.error('Error setting up overview charts:', error);
        }
    }

    setupPageHandlers() {
        // Override dashboard navigation to include chart updates
        const originalNavigateTo = this.dashboard.navigateTo.bind(this.dashboard);
        
        this.dashboard.navigateTo = async (page) => {
            originalNavigateTo(page);
            await this.onPageChange(page);
        };
    }

    async onPageChange(page) {
        switch (page) {
            case 'overview':
                await this.setupOverviewCharts();
                break;
            case 'results':
                await this.setupResultsCharts();
                break;
            case 'live':
                await this.setupLiveCharts();
                break;
            case 'analysis':
                await this.setupAnalysisCharts();
                break;
        }
    }

    async setupResultsCharts() {
        // Setup results page charts
        console.log('Setting up results charts...');
        
        // Results will be populated by the dashboard core
        const resultsContent = document.getElementById('resultsContent');
        if (resultsContent) {
            resultsContent.innerHTML = `
                <div class="mb-4">
                    <div class="row">
                        <div class="col-md-3">
                            <label for="categoryFilter" class="form-label">Category</label>
                            <select class="form-select" id="categoryFilter">
                                <option value="">All Categories</option>
                                <option value="file_io">File I/O</option>
                                <option value="metadata">Metadata</option>
                                <option value="network">Network</option>
                                <option value="multiregion">Multi-region</option>
                            </select>
                        </div>
                        <div class="col-md-3">
                            <label for="dateFilter" class="form-label">Time Range</label>
                            <select class="form-select" id="dateFilter">
                                <option value="24h">Last 24 Hours</option>
                                <option value="7d">Last 7 Days</option>
                                <option value="30d">Last 30 Days</option>
                                <option value="all">All Time</option>
                            </select>
                        </div>
                        <div class="col-md-3 d-flex align-items-end">
                            <button class="btn btn-primary" id="applyFilters">Apply Filters</button>
                        </div>
                    </div>
                </div>
                <div class="table-responsive">
                    <table class="table table-striped table-hover">
                        <thead>
                            <tr>
                                <th>Benchmark</th>
                                <th>Category</th>
                                <th>Timestamp</th>
                                <th>Duration</th>
                                <th>Throughput</th>
                                <th>Latency</th>
                                <th>Status</th>
                            </tr>
                        </thead>
                        <tbody id="resultsTableBody">
                            <tr>
                                <td colspan="7" class="text-center py-4">
                                    <div class="spinner-border text-primary" role="status">
                                        <span class="visually-hidden">Loading...</span>
                                    </div>
                                </td>
                            </tr>
                        </tbody>
                    </table>
                </div>
            `;
            
            // Setup filter handlers
            document.getElementById('applyFilters')?.addEventListener('click', () => {
                this.applyResultsFilters();
            });
        }
    }

    async setupLiveCharts() {
        // Setup live monitoring charts
        console.log('Setting up live monitoring charts...');
        
        try {
            // Create real-time performance chart
            const liveChart = this.chartFactory.createRealTimeChart('livePerformanceChart', 50);
            
            // Start feeding real-time data
            this.startLiveDataFeed(liveChart);
            
        } catch (error) {
            console.error('Error setting up live charts:', error);
        }
    }

    async setupAnalysisCharts() {
        // Setup analysis page charts
        console.log('Setting up analysis charts...');
        
        const analysisContent = document.getElementById('analysisContent');
        if (analysisContent) {
            analysisContent.innerHTML = `
                <div class="row">
                    <div class="col-lg-8 mb-4">
                        <div class="card shadow">
                            <div class="card-header">
                                <h6 class="m-0 fw-bold text-primary">Performance Trend Analysis</h6>
                            </div>
                            <div class="card-body">
                                <canvas id="trendAnalysisChart" height="100"></canvas>
                            </div>
                        </div>
                    </div>
                    <div class="col-lg-4 mb-4">
                        <div class="card shadow">
                            <div class="card-header">
                                <h6 class="m-0 fw-bold text-primary">Performance Metrics</h6>
                            </div>
                            <div class="card-body">
                                <div class="mb-3">
                                    <div class="d-flex justify-content-between">
                                        <span>CPU Efficiency</span>
                                        <span class="fw-bold text-success">87%</span>
                                    </div>
                                    <div class="progress mt-1">
                                        <div class="progress-bar bg-success" style="width: 87%"></div>
                                    </div>
                                </div>
                                <div class="mb-3">
                                    <div class="d-flex justify-content-between">
                                        <span>Memory Efficiency</span>
                                        <span class="fw-bold text-info">92%</span>
                                    </div>
                                    <div class="progress mt-1">
                                        <div class="progress-bar bg-info" style="width: 92%"></div>
                                    </div>
                                </div>
                                <div class="mb-3">
                                    <div class="d-flex justify-content-between">
                                        <span>I/O Efficiency</span>
                                        <span class="fw-bold text-warning">78%</span>
                                    </div>
                                    <div class="progress mt-1">
                                        <div class="progress-bar bg-warning" style="width: 78%"></div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            `;
            
            // Create trend analysis chart
            const trendData = {
                dates: this.generateDateLabels(30),
                performance: this.generateTrendData(30, 85, 5),
                regression: this.generateTrendData(30, 80, 2)
            };
            
            this.chartFactory.createTrendAnalysisChart('trendAnalysisChart', trendData);
        }
    }

    setupThemeManagement() {
        // Load saved theme
        const savedTheme = localStorage.getItem('theme');
        if (savedTheme === 'dark') {
            document.documentElement.setAttribute('data-theme', 'dark');
            this.updateThemeIcon(true);
        }

        // Theme toggle handler
        const themeToggle = document.getElementById('themeToggle');
        if (themeToggle) {
            themeToggle.addEventListener('click', () => {
                this.toggleTheme();
            });
        }
    }

    toggleTheme() {
        const currentTheme = document.documentElement.getAttribute('data-theme');
        const isDark = currentTheme === 'dark';
        
        document.documentElement.setAttribute('data-theme', isDark ? 'light' : 'dark');
        localStorage.setItem('theme', isDark ? 'light' : 'dark');
        
        this.updateThemeIcon(!isDark);
        
        // Update charts theme
        if (this.chartFactory) {
            this.chartFactory.updateTheme(!isDark);
        }
    }

    updateThemeIcon(isDark) {
        const themeIcon = document.getElementById('themeIcon');
        if (themeIcon) {
            themeIcon.className = isDark ? 'fas fa-sun' : 'fas fa-moon';
        }
    }

    setupRealTimeUpdates() {
        // Listen for WebSocket messages and update UI accordingly
        if (this.dashboard.socket) {
            const originalMessageHandler = this.dashboard.handleWebSocketMessage.bind(this.dashboard);
            
            this.dashboard.handleWebSocketMessage = (data) => {
                originalMessageHandler(data);
                this.handleRealTimeUpdate(data);
            };
        }
    }

    handleRealTimeUpdate(data) {
        switch (data.type) {
            case 'live_metrics':
                this.updateLiveMetrics(data.payload);
                break;
            case 'benchmark_update':
                this.updateBenchmarkStatus(data.payload);
                break;
        }
    }

    updateLiveMetrics(metrics) {
        // Update overview cards
        if (metrics.cpu !== undefined) {
            const cpuElement = document.querySelector('[data-metric="cpu"]');
            if (cpuElement) {
                cpuElement.textContent = `${metrics.cpu.toFixed(1)}%`;
            }
        }
        
        if (metrics.memory !== undefined) {
            const memoryElement = document.querySelector('[data-metric="memory"]');
            if (memoryElement) {
                memoryElement.textContent = `${metrics.memory.toFixed(1)}%`;
            }
        }
    }

    updateBenchmarkStatus(status) {
        const currentBenchmarks = document.getElementById('currentBenchmarks');
        if (currentBenchmarks && status.status === 'started') {
            currentBenchmarks.innerHTML = `
                <div class="d-flex justify-content-between align-items-center mb-2">
                    <span class="fw-bold">${status.name || 'Benchmark'}</span>
                    <span class="badge bg-primary">Running</span>
                </div>
                <div class="progress mb-2">
                    <div class="progress-bar progress-bar-striped progress-bar-animated" 
                         style="width: ${status.progress || 0}%"></div>
                </div>
                <small class="text-muted">Started: ${new Date(status.timestamp).toLocaleTimeString()}</small>
            `;
        }
    }

    startLiveDataFeed(chart) {
        if (!chart || !chart.addDataPoint) return;
        
        setInterval(() => {
            const value = Math.random() * 1000 + 500; // Random throughput value
            const timestamp = new Date().toLocaleTimeString();
            chart.addDataPoint(value, timestamp);
        }, 2000);
    }

    applyResultsFilters() {
        const category = document.getElementById('categoryFilter')?.value;
        const timeRange = document.getElementById('dateFilter')?.value;
        
        console.log('Applying filters:', { category, timeRange });
        
        // This would trigger a new API call with filters
        // For now, we'll just show a loading state
        const tableBody = document.getElementById('resultsTableBody');
        if (tableBody) {
            tableBody.innerHTML = `
                <tr>
                    <td colspan="7" class="text-center py-4">
                        <div class="spinner-border text-primary" role="status">
                            <span class="visually-hidden">Loading filtered results...</span>
                        </div>
                    </td>
                </tr>
            `;
        }
    }

    hideLoadingOverlay() {
        const overlay = document.getElementById('loadingOverlay');
        if (overlay) {
            overlay.classList.add('hidden');
            setTimeout(() => {
                overlay.style.display = 'none';
            }, 300);
        }
    }

    showError(message) {
        const toast = this.createToast('Error', message, 'danger');
        this.showToast(toast);
    }

    createToast(title, message, type = 'info') {
        const toastContainer = document.getElementById('toastContainer');
        if (!toastContainer) return null;

        const toastId = 'toast-' + Date.now();
        const toast = document.createElement('div');
        toast.id = toastId;
        toast.className = `toast align-items-center text-white bg-${type} border-0`;
        toast.setAttribute('role', 'alert');
        toast.innerHTML = `
            <div class="d-flex">
                <div class="toast-body">
                    <strong>${title}</strong><br>${message}
                </div>
                <button type="button" class="btn-close btn-close-white me-2 m-auto" 
                        data-bs-dismiss="toast"></button>
            </div>
        `;

        return toast;
    }

    showToast(toast) {
        if (!toast) return;
        
        const toastContainer = document.getElementById('toastContainer');
        if (toastContainer) {
            toastContainer.appendChild(toast);
            const bsToast = new bootstrap.Toast(toast);
            bsToast.show();
            
            // Remove from DOM after hiding
            toast.addEventListener('hidden.bs.toast', () => {
                toast.remove();
            });
        }
    }

    // Utility methods for generating mock data
    generateTimeLabels(hours) {
        const labels = [];
        const now = new Date();
        for (let i = hours - 1; i >= 0; i--) {
            const time = new Date(now.getTime() - i * 60 * 60 * 1000);
            labels.push(time.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }));
        }
        return labels;
    }

    generateDateLabels(days) {
        const labels = [];
        const now = new Date();
        for (let i = days - 1; i >= 0; i--) {
            const date = new Date(now.getTime() - i * 24 * 60 * 60 * 1000);
            labels.push(date.toLocaleDateString([], { month: 'short', day: 'numeric' }));
        }
        return labels;
    }

    generateMockData(count, min, max) {
        return Array.from({ length: count }, () => 
            Math.random() * (max - min) + min
        );
    }

    generateTrendData(count, base, variance) {
        const data = [];
        let current = base;
        for (let i = 0; i < count; i++) {
            current += (Math.random() - 0.5) * variance;
            data.push(Math.max(0, current));
        }
        return data;
    }
}

// Initialize the application when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    window.mooseNGApp = new DashboardApp();
    window.mooseNGApp.init();
});

export { DashboardApp };