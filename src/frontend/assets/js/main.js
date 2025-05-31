/**
 * MooseNG Dashboard Main Application Entry Point
 * Initializes the dashboard and handles global application logic
 */

class MooseNGDashboard {
    constructor() {
        this.initialized = false;
        this.components = new Map();
        this.globalEventListeners = [];
    }

    async init() {
        if (this.initialized) return;

        try {
            console.log('🚀 Initializing MooseNG Dashboard...');
            
            // Show loading overlay while initializing
            this.showLoadingOverlay();

            // Initialize global error handling
            this.setupGlobalErrorHandling();

            // Add global styles for modals and notifications
            this.addGlobalStyles();

            // Wait for all components to be available
            await this.waitForComponents();

            // Load initial data for the overview section
            await this.loadInitialData();

            // Setup keyboard shortcuts
            this.setupKeyboardShortcuts();

            // Setup window event listeners
            this.setupWindowEventListeners();

            // Hide loading overlay
            this.hideLoadingOverlay();

            // Mark as initialized
            this.initialized = true;

            console.log('✅ MooseNG Dashboard initialized successfully');
            
            // Show welcome notification
            if (window.Dashboard) {
                window.Dashboard.showNotification('MooseNG Dashboard loaded successfully!', 'success');
            }

        } catch (error) {
            console.error('❌ Failed to initialize MooseNG Dashboard:', error);
            this.showInitializationError(error);
        }
    }

    async waitForComponents() {
        const requiredComponents = [
            'Dashboard', 'DataManager', 'ChartComponents', 
            'OverviewSection', 'MasterServerComponent', 
            'ChunkServerComponent', 'ClientComponent'
        ];

        const maxWaitTime = 10000; // 10 seconds
        const startTime = Date.now();

        while (Date.now() - startTime < maxWaitTime) {
            const allLoaded = requiredComponents.every(component => 
                window[component] !== undefined
            );

            if (allLoaded) {
                console.log('✅ All components loaded');
                return;
            }

            await new Promise(resolve => setTimeout(resolve, 100));
        }

        throw new Error('Timeout waiting for components to load');
    }

    async loadInitialData() {
        console.log('📊 Loading initial dashboard data...');
        
        try {
            // Preload data for faster initial rendering
            const dataPromises = [
                window.DataManager.getOverviewData(),
                window.DataManager.getMasterServerData(),
                window.DataManager.getChunkServerData(),
                window.DataManager.getClientData()
            ];

            await Promise.all(dataPromises);
            console.log('✅ Initial data loaded');

        } catch (error) {
            console.warn('⚠️ Some initial data failed to load:', error);
            // Continue anyway - components will handle individual failures
        }
    }

    setupGlobalErrorHandling() {
        // Catch unhandled JavaScript errors
        window.addEventListener('error', (event) => {
            console.error('Global JavaScript Error:', event.error);
            this.showErrorNotification('An unexpected error occurred. Please refresh the page.');
        });

        // Catch unhandled promise rejections
        window.addEventListener('unhandledrejection', (event) => {
            console.error('Unhandled Promise Rejection:', event.reason);
            this.showErrorNotification('A network or data error occurred. Please check your connection.');
        });
    }

    setupKeyboardShortcuts() {
        const shortcuts = [
            { keys: ['r'], action: () => this.refreshDashboard(), description: 'Refresh dashboard' },
            { keys: ['1'], action: () => this.navigateToSection('overview'), description: 'Go to Overview' },
            { keys: ['2'], action: () => this.navigateToSection('master-server'), description: 'Go to Master Server' },
            { keys: ['3'], action: () => this.navigateToSection('chunk-server'), description: 'Go to Chunk Server' },
            { keys: ['4'], action: () => this.navigateToSection('client'), description: 'Go to Client' },
            { keys: ['5'], action: () => this.navigateToSection('multiregion'), description: 'Go to Multi-Region' },
            { keys: ['6'], action: () => this.navigateToSection('comparative'), description: 'Go to Comparative' },
            { keys: ['t'], action: () => this.toggleTheme(), description: 'Toggle theme' },
            { keys: ['h'], action: () => this.showHelpModal(), description: 'Show help' },
            { keys: ['Escape'], action: () => this.closeModals(), description: 'Close modals' }
        ];

        const keydownHandler = (event) => {
            // Don't trigger shortcuts when user is typing in inputs
            if (event.target.tagName === 'INPUT' || event.target.tagName === 'TEXTAREA' || event.target.tagName === 'SELECT') {
                return;
            }

            const shortcut = shortcuts.find(s => s.keys.includes(event.key));
            if (shortcut) {
                event.preventDefault();
                shortcut.action();
            }
        };

        document.addEventListener('keydown', keydownHandler);
        this.globalEventListeners.push({ type: 'keydown', handler: keydownHandler });

        // Store shortcuts for help modal
        this.keyboardShortcuts = shortcuts;
    }

    setupWindowEventListeners() {
        // Handle browser tab visibility change
        const visibilityChangeHandler = () => {
            if (document.hidden) {
                // Pause auto-refresh when tab is not visible
                this.pauseAutoRefresh();
            } else {
                // Resume auto-refresh when tab becomes visible
                this.resumeAutoRefresh();
            }
        };

        document.addEventListener('visibilitychange', visibilityChangeHandler);
        this.globalEventListeners.push({ type: 'visibilitychange', handler: visibilityChangeHandler });

        // Handle window resize for responsive charts
        const resizeHandler = () => {
            // Debounce resize events
            clearTimeout(this.resizeTimeout);
            this.resizeTimeout = setTimeout(() => {
                this.handleWindowResize();
            }, 250);
        };

        window.addEventListener('resize', resizeHandler);
        this.globalEventListeners.push({ type: 'resize', handler: resizeHandler, target: window });

        // Handle online/offline status
        const onlineHandler = () => {
            if (window.Dashboard) {
                window.Dashboard.showNotification('Connection restored', 'success');
            }
            this.resumeAutoRefresh();
        };

        const offlineHandler = () => {
            if (window.Dashboard) {
                window.Dashboard.showNotification('Connection lost - working offline', 'warning');
            }
            this.pauseAutoRefresh();
        };

        window.addEventListener('online', onlineHandler);
        window.addEventListener('offline', offlineHandler);
        this.globalEventListeners.push({ type: 'online', handler: onlineHandler, target: window });
        this.globalEventListeners.push({ type: 'offline', handler: offlineHandler, target: window });
    }

    addGlobalStyles() {
        const styles = `
            /* Modal styles */
            .modal-overlay {
                position: fixed;
                top: 0;
                left: 0;
                right: 0;
                bottom: 0;
                background: rgba(0, 0, 0, 0.7);
                display: flex;
                justify-content: center;
                align-items: center;
                z-index: 1000;
                animation: fadeIn 0.3s ease;
            }

            .modal-content {
                background: var(--bg-primary);
                border-radius: var(--border-radius);
                max-width: 600px;
                max-height: 80vh;
                overflow-y: auto;
                box-shadow: 0 8px 32px var(--shadow-medium);
                animation: slideUp 0.3s ease;
            }

            .modal-header {
                display: flex;
                justify-content: space-between;
                align-items: center;
                padding: 1.5rem;
                border-bottom: 1px solid var(--border-color);
            }

            .modal-header h3 {
                margin: 0;
                color: var(--primary-color);
            }

            .modal-close {
                background: none;
                border: none;
                font-size: 1.5rem;
                cursor: pointer;
                color: var(--text-secondary);
                padding: 0.5rem;
                border-radius: 4px;
                transition: var(--transition);
            }

            .modal-close:hover {
                background: var(--bg-tertiary);
                color: var(--text-primary);
            }

            .modal-body {
                padding: 1.5rem;
            }

            .server-details {
                display: grid;
                gap: 1rem;
            }

            .detail-row {
                display: flex;
                justify-content: space-between;
                align-items: center;
                padding: 0.5rem 0;
                border-bottom: 1px solid var(--border-color);
            }

            .detail-row:last-child {
                border-bottom: none;
            }

            .detail-row label {
                font-weight: 600;
                color: var(--text-secondary);
            }

            /* Action buttons */
            .action-btn {
                padding: 0.4rem 0.8rem;
                border: 1px solid var(--border-color);
                border-radius: 4px;
                background: var(--bg-primary);
                color: var(--text-primary);
                font-size: 0.85rem;
                cursor: pointer;
                transition: var(--transition);
                margin-left: 0.5rem;
            }

            .action-btn:hover {
                background: var(--bg-tertiary);
                transform: translateY(-1px);
            }

            .action-btn.warning {
                background: var(--warning-color);
                color: white;
                border-color: var(--warning-color);
            }

            .action-btn.warning:hover {
                background: #e67e22;
            }

            /* Log entries */
            .log-entries {
                max-height: 300px;
                overflow-y: auto;
                background: var(--bg-secondary);
                border-radius: 4px;
                padding: 1rem;
            }

            .log-entry {
                display: flex;
                align-items: center;
                padding: 0.5rem 0;
                border-bottom: 1px solid var(--border-color);
                font-family: 'Courier New', monospace;
                font-size: 0.85rem;
            }

            .log-entry:last-child {
                border-bottom: none;
            }

            .log-time {
                color: var(--text-muted);
                margin-right: 1rem;
                min-width: 80px;
            }

            .log-level {
                margin-right: 1rem;
                min-width: 80px;
                font-weight: bold;
            }

            .log-info .log-level { color: var(--secondary-color); }
            .log-warning .log-level { color: var(--warning-color); }
            .log-error .log-level { color: var(--danger-color); }

            .log-message {
                color: var(--text-primary);
            }

            /* Error messages */
            .error-message {
                text-align: center;
                padding: 3rem;
                color: var(--text-secondary);
            }

            .error-message h3 {
                color: var(--danger-color);
                margin-bottom: 1rem;
            }

            .retry-button {
                padding: 0.8rem 1.5rem;
                background: var(--secondary-color);
                color: white;
                border: none;
                border-radius: var(--border-radius);
                cursor: pointer;
                font-size: 1rem;
                margin-top: 1rem;
                transition: var(--transition);
            }

            .retry-button:hover {
                background: #2980b9;
                transform: translateY(-1px);
            }

            /* Help modal specific styles */
            .help-content {
                max-width: 500px;
            }

            .shortcut-list {
                display: grid;
                gap: 0.5rem;
                margin-top: 1rem;
            }

            .shortcut-item {
                display: flex;
                justify-content: space-between;
                align-items: center;
                padding: 0.5rem 0;
                border-bottom: 1px solid var(--border-color);
            }

            .shortcut-key {
                background: var(--bg-tertiary);
                padding: 0.25rem 0.5rem;
                border-radius: 4px;
                font-family: monospace;
                font-weight: bold;
            }

            /* Responsive adjustments */
            @media (max-width: 768px) {
                .modal-content {
                    margin: 1rem;
                    max-width: none;
                    width: calc(100% - 2rem);
                }

                .detail-row {
                    flex-direction: column;
                    align-items: flex-start;
                    gap: 0.5rem;
                }

                .action-btn {
                    margin: 0.25rem 0;
                }
            }
        `;

        const styleSheet = document.createElement('style');
        styleSheet.textContent = styles;
        document.head.appendChild(styleSheet);
    }

    // Global action methods
    refreshDashboard() {
        if (window.Dashboard) {
            window.Dashboard.refreshData();
        }
    }

    navigateToSection(sectionId) {
        if (window.Dashboard) {
            window.Dashboard.navigateToSection(sectionId);
        }
    }

    toggleTheme() {
        if (window.Dashboard) {
            window.Dashboard.toggleTheme();
        }
    }

    showHelpModal() {
        const modal = document.createElement('div');
        modal.className = 'modal-overlay';
        modal.innerHTML = `
            <div class="modal-content help-content">
                <div class="modal-header">
                    <h3>MooseNG Dashboard Help</h3>
                    <button class="modal-close" onclick="this.closest('.modal-overlay').remove()">×</button>
                </div>
                <div class="modal-body">
                    <h4>Keyboard Shortcuts</h4>
                    <div class="shortcut-list">
                        ${this.keyboardShortcuts.map(shortcut => `
                            <div class="shortcut-item">
                                <span>${shortcut.description}</span>
                                <span class="shortcut-key">${shortcut.keys[0]}</span>
                            </div>
                        `).join('')}
                    </div>
                    
                    <h4 style="margin-top: 2rem;">About</h4>
                    <p>MooseNG Performance Dashboard provides real-time monitoring and visualization of your MooseNG distributed file system.</p>
                    
                    <p><strong>Version:</strong> 4.57.6<br>
                    <strong>Build:</strong> Production<br>
                    <strong>Last Updated:</strong> ${new Date().toLocaleDateString()}</p>
                </div>
            </div>
        `;

        document.body.appendChild(modal);
    }

    closeModals() {
        const modals = document.querySelectorAll('.modal-overlay');
        modals.forEach(modal => modal.remove());
    }

    handleWindowResize() {
        // Trigger chart resize for all active charts
        if (window.ChartComponents) {
            window.ChartComponents.charts.forEach(chart => {
                chart.resize();
            });
        }
    }

    pauseAutoRefresh() {
        // Pause auto-refresh for all components
        [window.OverviewSection, window.MasterServerComponent, 
         window.ChunkServerComponent, window.ClientComponent].forEach(component => {
            if (component && typeof component.stopAutoUpdate === 'function') {
                component.stopAutoUpdate();
            }
        });
    }

    resumeAutoRefresh() {
        // Resume auto-refresh for all components
        [window.OverviewSection, window.MasterServerComponent, 
         window.ChunkServerComponent, window.ClientComponent].forEach(component => {
            if (component && typeof component.startAutoUpdate === 'function') {
                component.startAutoUpdate();
            }
        });
    }

    showLoadingOverlay() {
        const overlay = document.getElementById('loading-overlay');
        if (overlay) {
            overlay.classList.remove('hidden');
        }
    }

    hideLoadingOverlay() {
        const overlay = document.getElementById('loading-overlay');
        if (overlay) {
            overlay.classList.add('hidden');
        }
    }

    showInitializationError(error) {
        const overlay = document.getElementById('loading-overlay');
        if (overlay) {
            overlay.innerHTML = `
                <div class="error-message">
                    <h3>Failed to Initialize Dashboard</h3>
                    <p>An error occurred while loading the MooseNG Dashboard:</p>
                    <p><code>${error.message}</code></p>
                    <button class="retry-button" onclick="location.reload()">
                        Reload Page
                    </button>
                </div>
            `;
        }
    }

    showErrorNotification(message) {
        if (window.Dashboard && typeof window.Dashboard.showNotification === 'function') {
            window.Dashboard.showNotification(message, 'error');
        } else {
            console.error('Dashboard Error:', message);
        }
    }

    // Cleanup method
    destroy() {
        // Remove global event listeners
        this.globalEventListeners.forEach(({ type, handler, target = document }) => {
            target.removeEventListener(type, handler);
        });

        // Clear timeouts
        if (this.resizeTimeout) {
            clearTimeout(this.resizeTimeout);
        }

        // Destroy all components
        [window.OverviewSection, window.MasterServerComponent, 
         window.ChunkServerComponent, window.ClientComponent].forEach(component => {
            if (component && typeof component.destroy === 'function') {
                component.destroy();
            }
        });

        // Destroy all charts
        if (window.ChartComponents) {
            window.ChartComponents.destroyAllCharts();
        }

        this.initialized = false;
    }
}

// Initialize the application when DOM is ready
document.addEventListener('DOMContentLoaded', async () => {
    window.MooseNGApp = new MooseNGDashboard();
    await window.MooseNGApp.init();
});

// Handle page unload
window.addEventListener('beforeunload', () => {
    if (window.MooseNGApp) {
        window.MooseNGApp.destroy();
    }
});