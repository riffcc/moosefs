/**
 * MooseNG Dashboard Core Functionality
 * Handles navigation, theme switching, and core dashboard operations
 */

class DashboardCore {
    constructor() {
        this.currentSection = 'overview';
        this.currentTheme = localStorage.getItem('dashboard-theme') || 'light';
        this.refreshInterval = null;
        this.init();
    }

    init() {
        this.setupTheme();
        this.setupNavigation();
        this.setupControls();
        this.setupAutoRefresh();
        this.showLoadingOverlay();
    }

    setupTheme() {
        document.documentElement.setAttribute('data-theme', this.currentTheme);
        const themeToggle = document.getElementById('theme-toggle');
        if (themeToggle) {
            themeToggle.textContent = this.currentTheme === 'dark' ? '☀️' : '🌙';
            themeToggle.addEventListener('click', () => this.toggleTheme());
        }
    }

    toggleTheme() {
        this.currentTheme = this.currentTheme === 'light' ? 'dark' : 'light';
        document.documentElement.setAttribute('data-theme', this.currentTheme);
        localStorage.setItem('dashboard-theme', this.currentTheme);
        
        const themeToggle = document.getElementById('theme-toggle');
        if (themeToggle) {
            themeToggle.textContent = this.currentTheme === 'dark' ? '☀️' : '🌙';
        }
    }

    setupNavigation() {
        const navLinks = document.querySelectorAll('.nav-link');
        navLinks.forEach(link => {
            link.addEventListener('click', (e) => {
                e.preventDefault();
                const section = link.getAttribute('data-section');
                this.navigateToSection(section);
            });
        });

        // Handle browser back/forward buttons
        window.addEventListener('popstate', (e) => {
            if (e.state && e.state.section) {
                this.navigateToSection(e.state.section, false);
            }
        });

        // Set initial state
        const hash = window.location.hash.substring(1);
        if (hash && document.getElementById(hash)) {
            this.navigateToSection(hash, false);
        }
    }

    navigateToSection(sectionId, pushState = true) {
        // Hide all sections
        document.querySelectorAll('.dashboard-section').forEach(section => {
            section.classList.remove('active');
        });

        // Remove active class from all nav links
        document.querySelectorAll('.nav-link').forEach(link => {
            link.classList.remove('active');
        });

        // Show target section
        const targetSection = document.getElementById(sectionId);
        if (targetSection) {
            targetSection.classList.add('active');
            this.currentSection = sectionId;

            // Add active class to corresponding nav link
            const targetNavLink = document.querySelector(`[data-section="${sectionId}"]`);
            if (targetNavLink) {
                targetNavLink.classList.add('active');
            }

            // Update browser history
            if (pushState) {
                window.history.pushState(
                    { section: sectionId },
                    '',
                    `#${sectionId}`
                );
            }

            // Load section-specific content
            this.loadSectionContent(sectionId);
        }
    }

    loadSectionContent(sectionId) {
        switch (sectionId) {
            case 'overview':
                if (window.OverviewSection) {
                    window.OverviewSection.load();
                }
                break;
            case 'master-server':
                if (window.MasterServerComponent) {
                    window.MasterServerComponent.load();
                }
                break;
            case 'chunk-server':
                if (window.ChunkServerComponent) {
                    window.ChunkServerComponent.load();
                }
                break;
            case 'client':
                if (window.ClientComponent) {
                    window.ClientComponent.load();
                }
                break;
            case 'multiregion':
                this.loadMultiRegionContent();
                break;
            case 'comparative':
                this.loadComparativeContent();
                break;
        }
    }

    setupControls() {
        // Time range selector
        const timeRangeSelect = document.getElementById('time-range');
        if (timeRangeSelect) {
            timeRangeSelect.addEventListener('change', (e) => {
                this.updateTimeRange(e.target.value);
            });
        }

        // Refresh button
        const refreshBtn = document.getElementById('refresh-btn');
        if (refreshBtn) {
            refreshBtn.addEventListener('click', () => {
                this.refreshData();
            });
        }
    }

    setupAutoRefresh() {
        // Auto-refresh every 30 seconds
        this.refreshInterval = setInterval(() => {
            this.refreshData();
        }, 30000);
    }

    updateTimeRange(range) {
        this.currentTimeRange = range;
        this.refreshData();
        
        // Store preference
        localStorage.setItem('dashboard-time-range', range);
    }

    async refreshData() {
        try {
            this.showLoadingSpinner();
            
            // Refresh current section data
            await this.loadSectionContent(this.currentSection);
            
            // Update last updated timestamp
            this.updateLastUpdated();
            
        } catch (error) {
            console.error('Error refreshing data:', error);
            this.showNotification('Error refreshing data', 'error');
        } finally {
            this.hideLoadingSpinner();
        }
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

    showLoadingSpinner() {
        // Add a small loading indicator to refresh button
        const refreshBtn = document.getElementById('refresh-btn');
        if (refreshBtn) {
            refreshBtn.textContent = '⟳ Refreshing...';
            refreshBtn.disabled = true;
        }
    }

    hideLoadingSpinner() {
        const refreshBtn = document.getElementById('refresh-btn');
        if (refreshBtn) {
            refreshBtn.textContent = '🔄 Refresh';
            refreshBtn.disabled = false;
        }
    }

    updateLastUpdated() {
        const lastUpdatedElement = document.getElementById('last-updated');
        if (lastUpdatedElement) {
            const now = new Date();
            lastUpdatedElement.textContent = `Last updated: ${now.toLocaleTimeString()}`;
        }
    }

    showNotification(message, type = 'info') {
        // Create notification element
        const notification = document.createElement('div');
        notification.className = `notification notification-${type}`;
        notification.textContent = message;
        
        // Style the notification
        Object.assign(notification.style, {
            position: 'fixed',
            top: '20px',
            right: '20px',
            padding: '1rem 1.5rem',
            borderRadius: '8px',
            color: 'white',
            fontWeight: '500',
            zIndex: '1001',
            animation: 'slideInRight 0.3s ease'
        });

        // Set background color based on type
        const colors = {
            info: '#3498db',
            success: '#27ae60',
            warning: '#f39c12',
            error: '#e74c3c'
        };
        notification.style.backgroundColor = colors[type] || colors.info;

        // Add to page
        document.body.appendChild(notification);

        // Remove after 3 seconds
        setTimeout(() => {
            notification.style.animation = 'slideOutRight 0.3s ease';
            setTimeout(() => {
                if (notification.parentNode) {
                    notification.parentNode.removeChild(notification);
                }
            }, 300);
        }, 3000);
    }

    async loadMultiRegionContent() {
        const content = document.getElementById('multiregion-content');
        if (!content) return;

        try {
            // Load multi-region performance data
            const data = await window.DataManager.getMultiRegionData();
            
            content.innerHTML = `
                <div class="region-grid">
                    ${data.regions.map(region => `
                        <div class="region-card">
                            <div class="region-header">
                                <h3 class="region-name">${region.name}</h3>
                                <span class="status-badge ${region.status}">${region.status}</span>
                            </div>
                            <div class="region-content">
                                <div class="metrics-grid">
                                    <div class="metric-card">
                                        <div class="metric-label">Latency</div>
                                        <div class="metric-value">${region.latency}<span class="metric-unit">ms</span></div>
                                    </div>
                                    <div class="metric-card">
                                        <div class="metric-label">Throughput</div>
                                        <div class="metric-value">${region.throughput}<span class="metric-unit">MB/s</span></div>
                                    </div>
                                    <div class="metric-card">
                                        <div class="metric-label">Availability</div>
                                        <div class="metric-value">${region.availability}<span class="metric-unit">%</span></div>
                                    </div>
                                    <div class="metric-card">
                                        <div class="metric-label">Active Nodes</div>
                                        <div class="metric-value">${region.activeNodes}</div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    `).join('')}
                </div>
                
                <div class="chart-container">
                    <h3>Cross-Region Latency Matrix</h3>
                    <canvas id="latency-matrix-chart"></canvas>
                </div>
                
                <div class="chart-container">
                    <h3>Multi-Region Performance Trends</h3>
                    <canvas id="multiregion-trends-chart"></canvas>
                </div>
            `;

            // Initialize charts
            if (window.ChartComponents) {
                window.ChartComponents.createLatencyMatrix('latency-matrix-chart', data.latencyMatrix);
                window.ChartComponents.createMultiRegionTrends('multiregion-trends-chart', data.trends);
            }

        } catch (error) {
            console.error('Error loading multi-region content:', error);
            content.innerHTML = '<div class="error">Error loading multi-region data</div>';
        }
    }

    async loadComparativeContent() {
        const content = document.getElementById('comparative-content');
        if (!content) return;

        try {
            // Load comparative analysis data
            const data = await window.DataManager.getComparativeData();
            
            content.innerHTML = `
                <div class="filter-section">
                    <div class="filter-group">
                        <label class="filter-label">Compare with:</label>
                        <select class="filter-select" id="comparison-target">
                            <option value="moosefs">MooseFS 3.0</option>
                            <option value="ceph">Ceph</option>
                            <option value="glusterfs">GlusterFS</option>
                            <option value="previous">Previous Version</option>
                        </select>
                    </div>
                    <div class="filter-group">
                        <label class="filter-label">Metric:</label>
                        <select class="filter-select" id="comparison-metric">
                            <option value="throughput">Throughput</option>
                            <option value="latency">Latency</option>
                            <option value="iops">IOPS</option>
                            <option value="cpu">CPU Usage</option>
                            <option value="memory">Memory Usage</option>
                        </select>
                    </div>
                </div>
                
                <div class="comparison-grid">
                    ${data.comparisons.map(comp => `
                        <div class="comparison-card">
                            <div class="comparison-header">
                                <h3 class="comparison-title">${comp.title}</h3>
                                <span class="status-badge ${comp.winner === 'mooseng' ? 'online' : 'warning'}">
                                    ${comp.winner === 'mooseng' ? 'Winner' : 'Competitor'}
                                </span>
                            </div>
                            <div class="comparison-metrics">
                                <div class="comparison-metric">
                                    <div class="comparison-metric-label">MooseNG</div>
                                    <div class="comparison-metric-value">${comp.mooseng}</div>
                                </div>
                                <div class="comparison-metric">
                                    <div class="comparison-metric-label">${comp.competitor}</div>
                                    <div class="comparison-metric-value">${comp.competitorValue}</div>
                                </div>
                            </div>
                        </div>
                    `).join('')}
                </div>
                
                <div class="chart-container">
                    <h3>Performance Comparison</h3>
                    <canvas id="comparison-chart"></canvas>
                </div>
            `;

            // Initialize comparison chart
            if (window.ChartComponents) {
                window.ChartComponents.createComparisonChart('comparison-chart', data);
            }

        } catch (error) {
            console.error('Error loading comparative content:', error);
            content.innerHTML = '<div class="error">Error loading comparative data</div>';
        }
    }

    // Cleanup method
    destroy() {
        if (this.refreshInterval) {
            clearInterval(this.refreshInterval);
        }
    }
}

// Initialize dashboard when DOM is loaded
document.addEventListener('DOMContentLoaded', () => {
    window.Dashboard = new DashboardCore();
});