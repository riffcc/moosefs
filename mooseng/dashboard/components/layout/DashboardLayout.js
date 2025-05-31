/**
 * Dashboard Layout Component for MooseNG
 * Manages the overall layout and navigation of the dashboard
 */

class DashboardLayout {
    constructor(containerId, options = {}) {
        this.containerId = containerId;
        this.container = document.getElementById(containerId);
        this.options = {
            theme: 'light',
            sidebarCollapsed: false,
            autoHideNavOnMobile: true,
            ...options
        };
        
        this.currentPage = 'overview';
        this.pages = new Map();
        this.components = new Map();
        
        this.init();
    }

    init() {
        this.createLayout();
        this.setupEventListeners();
        this.applyTheme();
    }

    /**
     * Create the main dashboard layout
     */
    createLayout() {
        if (!this.container) {
            console.error(`Container ${this.containerId} not found`);
            return;
        }

        this.container.innerHTML = `
            <div class="dashboard-layout">
                <!-- Header -->
                <header class="dashboard-header" id="dashboardHeader">
                    <div class="header-content">
                        <div class="header-left">
                            <button class="mobile-menu-toggle" id="mobileMenuToggle">
                                <span class="hamburger"></span>
                            </button>
                            <div class="dashboard-title">
                                <img src="../assets/images/mooseng-logo.svg" alt="MooseNG" class="logo">
                                <h1>MooseNG Dashboard</h1>
                            </div>
                        </div>
                        <div class="header-right">
                            <div class="header-controls" id="headerControls"></div>
                        </div>
                    </div>
                </header>

                <!-- Navigation -->
                <nav class="dashboard-nav" id="dashboardNav">
                    <ul class="nav-tabs" id="navTabs"></ul>
                </nav>

                <!-- Main Content -->
                <main class="dashboard-main" id="dashboardMain">
                    <div class="page-container" id="pageContainer"></div>
                </main>

                <!-- Footer -->
                <footer class="dashboard-footer" id="dashboardFooter">
                    <div class="footer-content">
                        <p>&copy; 2025 MooseNG. Performance Dashboard</p>
                        <div class="footer-links">
                            <a href="#" class="footer-link">Documentation</a>
                            <a href="#" class="footer-link">Support</a>
                            <a href="#" class="footer-link">About</a>
                        </div>
                    </div>
                </footer>

                <!-- Sidebar (for future use) -->
                <aside class="dashboard-sidebar ${this.options.sidebarCollapsed ? 'collapsed' : ''}" id="dashboardSidebar">
                    <div class="sidebar-content" id="sidebarContent"></div>
                </aside>
            </div>

            <!-- Overlays -->
            <div class="loading-overlay" id="loadingOverlay">
                <div class="loading-spinner">
                    <div class="spinner"></div>
                    <p class="loading-text">Loading...</p>
                </div>
            </div>

            <div class="mobile-nav-overlay" id="mobileNavOverlay"></div>
        `;

        this.elements = {
            header: document.getElementById('dashboardHeader'),
            nav: document.getElementById('dashboardNav'),
            main: document.getElementById('dashboardMain'),
            footer: document.getElementById('dashboardFooter'),
            sidebar: document.getElementById('dashboardSidebar'),
            navTabs: document.getElementById('navTabs'),
            pageContainer: document.getElementById('pageContainer'),
            headerControls: document.getElementById('headerControls'),
            sidebarContent: document.getElementById('sidebarContent'),
            loadingOverlay: document.getElementById('loadingOverlay'),
            mobileNavOverlay: document.getElementById('mobileNavOverlay'),
            mobileMenuToggle: document.getElementById('mobileMenuToggle')
        };
    }

    /**
     * Setup event listeners
     */
    setupEventListeners() {
        // Mobile menu toggle
        this.elements.mobileMenuToggle?.addEventListener('click', () => {
            this.toggleMobileNav();
        });

        // Mobile nav overlay
        this.elements.mobileNavOverlay?.addEventListener('click', () => {
            this.hideMobileNav();
        });

        // Window resize
        window.addEventListener('resize', () => {
            this.handleResize();
        });

        // Keyboard shortcuts
        document.addEventListener('keydown', (e) => {
            this.handleKeyboardShortcuts(e);
        });

        // Prevent default link behavior for nav links
        this.elements.navTabs?.addEventListener('click', (e) => {
            if (e.target.classList.contains('nav-link')) {
                e.preventDefault();
            }
        });
    }

    /**
     * Register a page with the layout
     */
    registerPage(id, config) {
        const page = {
            id,
            title: config.title || id,
            icon: config.icon || '',
            component: config.component || null,
            active: config.active || false,
            visible: config.visible !== false,
            order: config.order || 0,
            ...config
        };

        this.pages.set(id, page);
        this.updateNavigation();
        
        if (page.active) {
            this.currentPage = id;
        }

        return page;
    }

    /**
     * Update navigation based on registered pages
     */
    updateNavigation() {
        if (!this.elements.navTabs) return;

        // Sort pages by order
        const sortedPages = Array.from(this.pages.values())
            .filter(page => page.visible)
            .sort((a, b) => a.order - b.order);

        const navHtml = sortedPages.map(page => `
            <li class="nav-item">
                <a href="#${page.id}" 
                   class="nav-link ${page.id === this.currentPage ? 'active' : ''}" 
                   data-page="${page.id}">
                    ${page.icon ? `<span class="nav-icon">${page.icon}</span>` : ''}
                    <span class="nav-text">${page.title}</span>
                </a>
            </li>
        `).join('');

        this.elements.navTabs.innerHTML = navHtml;

        // Add click listeners to nav links
        this.elements.navTabs.querySelectorAll('.nav-link').forEach(link => {
            link.addEventListener('click', (e) => {
                e.preventDefault();
                const pageId = e.currentTarget.dataset.page;
                this.switchToPage(pageId);
            });
        });
    }

    /**
     * Switch to a specific page
     */
    switchToPage(pageId) {
        const page = this.pages.get(pageId);
        if (!page || !page.visible) {
            console.warn(`Page ${pageId} not found or not visible`);
            return;
        }

        // Update current page
        const previousPage = this.currentPage;
        this.currentPage = pageId;

        // Update navigation active state
        this.elements.navTabs?.querySelectorAll('.nav-link').forEach(link => {
            link.classList.remove('active');
            if (link.dataset.page === pageId) {
                link.classList.add('active');
            }
        });

        // Update page content
        this.updatePageContent(pageId);

        // Hide mobile nav if open
        if (this.options.autoHideNavOnMobile && window.innerWidth <= 768) {
            this.hideMobileNav();
        }

        // Emit page change event
        this.emit('pageChanged', { from: previousPage, to: pageId });

        // Update URL hash
        if (history.pushState) {
            history.pushState(null, null, `#${pageId}`);
        } else {
            location.hash = `#${pageId}`;
        }
    }

    /**
     * Update page content
     */
    updatePageContent(pageId) {
        const page = this.pages.get(pageId);
        if (!page || !this.elements.pageContainer) return;

        // Clear existing content
        this.elements.pageContainer.innerHTML = '';

        // Load page component if available
        if (page.component) {
            if (typeof page.component === 'function') {
                // Component is a function/class
                const componentInstance = new page.component(this.elements.pageContainer, page.config);
                this.components.set(pageId, componentInstance);
            } else if (typeof page.component === 'string') {
                // Component is HTML string
                this.elements.pageContainer.innerHTML = page.component;
            } else if (page.component instanceof HTMLElement) {
                // Component is DOM element
                this.elements.pageContainer.appendChild(page.component);
            }
        } else {
            // Default empty page
            this.elements.pageContainer.innerHTML = `
                <div class="page-placeholder">
                    <h2>${page.title}</h2>
                    <p>Page content will be loaded here.</p>
                </div>
            `;
        }
    }

    /**
     * Add control to header
     */
    addHeaderControl(id, element) {
        if (!this.elements.headerControls) return;

        if (typeof element === 'string') {
            const wrapper = document.createElement('div');
            wrapper.id = id;
            wrapper.innerHTML = element;
            this.elements.headerControls.appendChild(wrapper);
        } else if (element instanceof HTMLElement) {
            element.id = id;
            this.elements.headerControls.appendChild(element);
        }
    }

    /**
     * Create status indicator control
     */
    createStatusIndicator() {
        const indicator = document.createElement('div');
        indicator.className = 'status-indicator';
        indicator.innerHTML = `
            <span class="status-dot online"></span>
            <span class="status-text">Online</span>
        `;
        return indicator;
    }

    /**
     * Create refresh button control
     */
    createRefreshButton(onClick) {
        const button = document.createElement('button');
        button.className = 'refresh-btn';
        button.innerHTML = `
            <span class="refresh-icon">⟳</span>
            <span class="refresh-text">Refresh</span>
        `;
        
        button.addEventListener('click', onClick);
        return button;
    }

    /**
     * Toggle mobile navigation
     */
    toggleMobileNav() {
        const isOpen = this.elements.nav?.classList.contains('mobile-open');
        
        if (isOpen) {
            this.hideMobileNav();
        } else {
            this.showMobileNav();
        }
    }

    /**
     * Show mobile navigation
     */
    showMobileNav() {
        this.elements.nav?.classList.add('mobile-open');
        this.elements.mobileNavOverlay?.classList.add('active');
        document.body.classList.add('mobile-nav-open');
    }

    /**
     * Hide mobile navigation
     */
    hideMobileNav() {
        this.elements.nav?.classList.remove('mobile-open');
        this.elements.mobileNavOverlay?.classList.remove('active');
        document.body.classList.remove('mobile-nav-open');
    }

    /**
     * Handle window resize
     */
    handleResize() {
        const width = window.innerWidth;
        
        // Auto-hide mobile nav on desktop
        if (width > 768) {
            this.hideMobileNav();
        }

        // Resize charts in active components
        const activeComponent = this.components.get(this.currentPage);
        if (activeComponent && typeof activeComponent.resize === 'function') {
            activeComponent.resize();
        }
    }

    /**
     * Handle keyboard shortcuts
     */
    handleKeyboardShortcuts(e) {
        // Ctrl/Cmd + number to switch pages
        if ((e.ctrlKey || e.metaKey) && e.key >= '1' && e.key <= '9') {
            e.preventDefault();
            const pageIndex = parseInt(e.key) - 1;
            const pages = Array.from(this.pages.values())
                .filter(page => page.visible)
                .sort((a, b) => a.order - b.order);
            
            if (pages[pageIndex]) {
                this.switchToPage(pages[pageIndex].id);
            }
        }
        
        // Escape to close mobile nav
        if (e.key === 'Escape') {
            this.hideMobileNav();
        }
    }

    /**
     * Show loading overlay
     */
    showLoading(text = 'Loading...') {
        if (this.elements.loadingOverlay) {
            this.elements.loadingOverlay.querySelector('.loading-text').textContent = text;
            this.elements.loadingOverlay.classList.add('active');
        }
    }

    /**
     * Hide loading overlay
     */
    hideLoading() {
        this.elements.loadingOverlay?.classList.remove('active');
    }

    /**
     * Apply theme
     */
    applyTheme() {
        document.documentElement.setAttribute('data-theme', this.options.theme);
    }

    /**
     * Set theme
     */
    setTheme(theme) {
        this.options.theme = theme;
        this.applyTheme();
        this.emit('themeChanged', theme);
    }

    /**
     * Simple event emitter
     */
    emit(event, data) {
        const customEvent = new CustomEvent(`dashboard:${event}`, { detail: data });
        document.dispatchEvent(customEvent);
    }

    /**
     * Listen for dashboard events
     */
    on(event, callback) {
        document.addEventListener(`dashboard:${event}`, callback);
    }

    /**
     * Remove event listener
     */
    off(event, callback) {
        document.removeEventListener(`dashboard:${event}`, callback);
    }

    /**
     * Get current page
     */
    getCurrentPage() {
        return this.currentPage;
    }

    /**
     * Get all registered pages
     */
    getPages() {
        return Array.from(this.pages.values());
    }

    /**
     * Clean up resources
     */
    destroy() {
        // Clear timers and listeners
        this.components.forEach(component => {
            if (typeof component.destroy === 'function') {
                component.destroy();
            }
        });

        this.components.clear();
        this.pages.clear();

        // Clear container
        if (this.container) {
            this.container.innerHTML = '';
        }
    }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
    module.exports = DashboardLayout;
}