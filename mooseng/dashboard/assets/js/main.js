/**
 * Main JavaScript file for MooseNG Dashboard
 * Integrates all components and manages the application lifecycle
 */

class MooseNGDashboardApp {
    constructor() {
        this.layout = null;
        this.apiClient = null;
        this.benchmarkIntegration = null;
        this.currentPage = null;
        this.pages = new Map();
        
        this.init();
    }

    async init() {
        try {
            // Initialize core components
            this.initializeApiClient();
            await this.initializeBenchmarkIntegration();
            this.initializeLayout();
            this.registerPages();
            this.setupEventListeners();
            
            console.log('MooseNG Dashboard initialized successfully');
        } catch (error) {
            console.error('Failed to initialize dashboard:', error);
            this.showError('Failed to initialize dashboard: ' + error.message);
        }
    }

    /**
     * Initialize API client
     */
    initializeApiClient() {
        this.apiClient = new ApiClient('/api', {
            timeout: 30000,
            retries: 2
        });

        // Enable mock mode for development if backend is not available
        this.apiClient.ping().then(available => {
            if (!available) {
                console.log('Backend not available, enabling mock mode');
                this.apiClient.enableMockMode();
            }
        });
    }

    /**
     * Initialize benchmark integration
     */
    async initializeBenchmarkIntegration() {
        this.benchmarkIntegration = new BenchmarkIntegration(this.apiClient);
        
        // Listen for benchmark data updates
        document.addEventListener('benchmarkDataUpdate', (event) => {
            this.handleBenchmarkUpdate(event.detail);
        });
    }

    /**
     * Initialize dashboard layout
     */
    initializeLayout() {
        this.layout = new DashboardLayout('dashboardContainer', {
            theme: 'light',
            autoHideNavOnMobile: true
        });

        // Add header controls
        this.addHeaderControls();
        
        // Listen for layout events
        this.layout.on('pageChanged', (event) => {
            this.handlePageChange(event.detail);
        });
    }

    /**
     * Add header controls
     */
    addHeaderControls() {
        // Status indicator
        const statusIndicator = this.layout.createStatusIndicator();
        this.layout.addHeaderControl('status', statusIndicator);

        // Refresh button
        const refreshButton = this.layout.createRefreshButton(() => {
            this.refreshCurrentPage();
        });
        this.layout.addHeaderControl('refresh', refreshButton);

        // Theme toggle (future enhancement)
        const themeToggle = this.createThemeToggle();
        this.layout.addHeaderControl('theme', themeToggle);
    }

    /**
     * Create theme toggle button
     */
    createThemeToggle() {
        const button = document.createElement('button');
        button.className = 'theme-toggle-btn';
        button.innerHTML = '<span class="theme-icon">🌙</span>';
        button.title = 'Toggle theme';
        
        button.addEventListener('click', () => {
            const currentTheme = this.layout.options.theme;
            const newTheme = currentTheme === 'light' ? 'dark' : 'light';
            this.layout.setTheme(newTheme);
            button.innerHTML = newTheme === 'light' ? '<span class="theme-icon">🌙</span>' : '<span class="theme-icon">☀️</span>';
        });
        
        return button;
    }

    /**
     * Register all dashboard pages
     */
    registerPages() {
        // Overview page
        this.layout.registerPage('overview', {
            title: 'Overview',
            icon: '📊',
            component: OverviewPage,
            active: true,
            order: 1,
            config: {
                apiClient: this.apiClient,
                benchmarkIntegration: this.benchmarkIntegration
            }
        });

        // Master Server page
        this.layout.registerPage('master-server', {
            title: 'Master Server',
            icon: '🖥️',
            component: this.createPlaceholderPage('Master Server', 'Raft consensus, metadata operations, and cache performance metrics'),
            order: 2,
            config: {
                apiClient: this.apiClient
            }
        });

        // Chunk Server page
        this.layout.registerPage('chunk-server', {
            title: 'Chunk Server',
            icon: '💾',
            component: this.createPlaceholderPage('Chunk Server', 'I/O performance, erasure coding, and storage utilization metrics'),
            order: 3,
            config: {
                apiClient: this.apiClient
            }
        });

        // Client page
        this.layout.registerPage('client', {
            title: 'Client',
            icon: '👥',
            component: this.createPlaceholderPage('Client', 'FUSE performance, cache efficiency, and connection metrics'),
            order: 4,
            config: {
                apiClient: this.apiClient
            }
        });

        // Multi-Region page
        this.layout.registerPage('multiregion', {
            title: 'Multi-Region',
            icon: '🌍',
            component: this.createPlaceholderPage('Multi-Region', 'Cross-region latency, replication efficiency, and consistency metrics'),
            order: 5,
            config: {
                apiClient: this.apiClient
            }
        });

        // Benchmark Results page
        this.layout.registerPage('benchmark-results', {
            title: 'Benchmark Results',
            icon: '📈',
            component: BenchmarkResultsPage,
            order: 6,
            config: {
                apiClient: this.apiClient,
                benchmarkIntegration: this.benchmarkIntegration
            }
        });
    }

    /**
     * Create placeholder page component
     */
    createPlaceholderPage(title, description) {
        return class PlaceholderPage {
            constructor(container, options = {}) {
                this.container = container;
                this.options = options;
                this.render();
            }

            render() {
                this.container.innerHTML = `
                    <div class="page-placeholder">
                        <h2>${title}</h2>
                        <p>${description}</p>
                        <div class="placeholder-content">
                            <div class="grid grid-cols-3 mb-6">
                                <div class="metric-card">
                                    <div class="metric-value">--</div>
                                    <div class="metric-label">Coming Soon</div>
                                </div>
                                <div class="metric-card">
                                    <div class="metric-value">--</div>
                                    <div class="metric-label">Coming Soon</div>
                                </div>
                                <div class="metric-card">
                                    <div class="metric-value">--</div>
                                    <div class="metric-label">Coming Soon</div>
                                </div>
                            </div>
                            <div class="card">
                                <div class="card-header">
                                    <h3 class="card-title">${title} Metrics</h3>
                                    <p class="card-subtitle">Will be implemented in next iteration</p>
                                </div>
                                <div class="chart-container">
                                    <div style="display: flex; align-items: center; justify-content: center; height: 300px; color: #64748b;">
                                        <span>Chart placeholder - ${title} visualization coming soon</span>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                `;
            }

            destroy() {
                // Cleanup if needed
            }
        };
    }

    /**
     * Setup global event listeners
     */
    setupEventListeners() {
        // Handle hash navigation
        window.addEventListener('hashchange', () => {
            this.handleHashNavigation();
        });

        // Handle initial hash
        this.handleHashNavigation();

        // Handle keyboard shortcuts
        document.addEventListener('keydown', (e) => {
            this.handleKeyboardShortcuts(e);
        });

        // Handle window resize
        window.addEventListener('resize', () => {
            this.handleWindowResize();
        });

        // Handle visibility change for auto-refresh
        document.addEventListener('visibilitychange', () => {
            this.handleVisibilityChange();
        });
    }

    /**
     * Handle hash navigation
     */
    handleHashNavigation() {
        const hash = window.location.hash.substring(1);
        if (hash && this.layout.getPages().find(p => p.id === hash)) {
            this.layout.switchToPage(hash);
        } else if (!hash) {
            this.layout.switchToPage('overview');
        }
    }

    /**
     * Handle keyboard shortcuts
     */
    handleKeyboardShortcuts(e) {
        // Ctrl/Cmd + R for refresh
        if ((e.ctrlKey || e.metaKey) && e.key === 'r') {
            e.preventDefault();
            this.refreshCurrentPage();
        }

        // Ctrl/Cmd + number for page navigation
        if ((e.ctrlKey || e.metaKey) && e.key >= '1' && e.key <= '5') {
            e.preventDefault();
            const pageIndex = parseInt(e.key) - 1;
            const pages = this.layout.getPages().sort((a, b) => a.order - b.order);
            if (pages[pageIndex]) {
                this.layout.switchToPage(pages[pageIndex].id);
            }
        }
    }

    /**
     * Handle window resize
     */
    handleWindowResize() {
        // Notify current page component of resize
        const currentPageId = this.layout.getCurrentPage();
        const pageComponent = this.layout.components.get(currentPageId);
        
        if (pageComponent && typeof pageComponent.resize === 'function') {
            pageComponent.resize();
        }
    }

    /**
     * Handle visibility change
     */
    handleVisibilityChange() {
        if (document.visibilityState === 'visible') {
            // Page became visible, refresh data
            setTimeout(() => {
                this.refreshCurrentPage();
            }, 1000);
        }
    }

    /**
     * Handle page changes
     */
    handlePageChange(detail) {
        const { from, to } = detail;
        console.log(`Page changed from ${from} to ${to}`);
        
        this.currentPage = to;
        
        // Update URL without triggering navigation
        if (history.replaceState) {
            history.replaceState(null, null, `#${to}`);
        }
    }

    /**
     * Handle benchmark data updates
     */
    handleBenchmarkUpdate(data) {
        console.log('Benchmark data updated:', data);
        
        // Notify current page component of new data
        const currentPageId = this.layout.getCurrentPage();
        const pageComponent = this.layout.components.get(currentPageId);
        
        if (pageComponent && typeof pageComponent.handleDataUpdate === 'function') {
            pageComponent.handleDataUpdate(data);
        }
    }

    /**
     * Refresh current page
     */
    async refreshCurrentPage() {
        this.layout.showLoading('Refreshing data...');
        
        try {
            const currentPageId = this.layout.getCurrentPage();
            const pageComponent = this.layout.components.get(currentPageId);
            
            if (pageComponent && typeof pageComponent.refresh === 'function') {
                await pageComponent.refresh();
            }
            
            // Update status indicator
            this.updateStatusIndicator('success');
            
        } catch (error) {
            console.error('Error refreshing page:', error);
            this.updateStatusIndicator('error');
            this.showError('Failed to refresh page data');
        } finally {
            this.layout.hideLoading();
        }
    }

    /**
     * Update status indicator
     */
    updateStatusIndicator(status) {
        const indicator = document.querySelector('.status-indicator');
        const dot = indicator?.querySelector('.status-dot');
        const text = indicator?.querySelector('.status-text');
        
        if (dot && text) {
            dot.className = 'status-dot';
            
            switch (status) {
                case 'success':
                    dot.classList.add('online');
                    text.textContent = 'Online';
                    break;
                case 'error':
                    dot.classList.add('error');
                    text.textContent = 'Error';
                    break;
                case 'warning':
                    dot.classList.add('warning');
                    text.textContent = 'Warning';
                    break;
                default:
                    dot.classList.add('offline');
                    text.textContent = 'Offline';
            }
        }
    }

    /**
     * Show error message
     */
    showError(message) {
        // Use browser alert for now, could be enhanced with custom modal
        console.error(message);
        
        // Try to show in error modal if available
        const modal = document.getElementById('errorModal');
        const messageElement = document.getElementById('errorMessage');
        
        if (modal && messageElement) {
            messageElement.textContent = message;
            modal.style.display = 'block';
        } else {
            alert('Error: ' + message);
        }
    }

    /**
     * Get application state for debugging
     */
    getState() {
        return {
            currentPage: this.currentPage,
            layout: this.layout ? {
                theme: this.layout.options.theme,
                pages: this.layout.getPages()
            } : null,
            benchmarkData: this.benchmarkIntegration ? 
                this.benchmarkIntegration.exportData() : null,
            apiClient: this.apiClient ? {
                baseUrl: this.apiClient.baseUrl,
                mockMode: this.apiClient.mockMode
            } : null
        };
    }

    /**
     * Cleanup resources
     */
    destroy() {
        // Stop any intervals or cleanup resources
        if (this.layout) {
            this.layout.destroy();
        }
        
        if (this.benchmarkIntegration) {
            this.benchmarkIntegration.clearCache();
        }
        
        console.log('MooseNG Dashboard destroyed');
    }
}

// Initialize dashboard when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    window.mooseNGDashboard = new MooseNGDashboardApp();
});

// Handle page unload cleanup
window.addEventListener('beforeunload', () => {
    if (window.mooseNGDashboard) {
        window.mooseNGDashboard.destroy();
    }
});

// Make dashboard globally available for debugging
window.getMooseNGDashboardState = () => {
    return window.mooseNGDashboard ? window.mooseNGDashboard.getState() : null;
};