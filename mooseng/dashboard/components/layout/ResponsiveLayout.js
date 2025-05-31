/**
 * ResponsiveLayout - Advanced Layout Management for MooseNG Dashboard
 * 
 * Provides responsive grid layouts, adaptive component sizing, and
 * intelligent layout reorganization based on screen size and content.
 */

class ResponsiveLayout {
    constructor() {
        this.breakpoints = {
            xs: 480,
            sm: 768,
            md: 1024,
            lg: 1200,
            xl: 1400
        };
        
        this.currentBreakpoint = this.getCurrentBreakpoint();
        this.layouts = new Map();
        this.components = new Map();
        this.observers = new Map();
        
        this.setupResizeObserver();
        this.setupIntersectionObserver();
        this.initializeLayoutEngine();
    }

    // Layout Engine
    initializeLayoutEngine() {
        this.registerLayouts();
        this.applyCurrentLayout();
        
        window.addEventListener('resize', this.debounce(() => {
            this.handleResize();
        }, 250));
    }

    registerLayouts() {
        // Overview page layouts
        this.layouts.set('overview', {
            xl: {
                metrics: { cols: 4, rows: 1 },
                charts: { cols: 2, rows: 2 },
                summary: { cols: 1, rows: 1 }
            },
            lg: {
                metrics: { cols: 4, rows: 1 },
                charts: { cols: 2, rows: 2 },
                summary: { cols: 1, rows: 1 }
            },
            md: {
                metrics: { cols: 2, rows: 2 },
                charts: { cols: 1, rows: 2 },
                summary: { cols: 1, rows: 1 }
            },
            sm: {
                metrics: { cols: 2, rows: 2 },
                charts: { cols: 1, rows: 2 },
                summary: { cols: 1, rows: 1 }
            },
            xs: {
                metrics: { cols: 1, rows: 4 },
                charts: { cols: 1, rows: 2 },
                summary: { cols: 1, rows: 1 }
            }
        });

        // Master server page layouts
        this.layouts.set('master-server', {
            xl: {
                metrics: { cols: 3, rows: 1 },
                raftCharts: { cols: 2, rows: 1 },
                metadataCharts: { cols: 1, rows: 2 }
            },
            lg: {
                metrics: { cols: 3, rows: 1 },
                raftCharts: { cols: 2, rows: 1 },
                metadataCharts: { cols: 1, rows: 2 }
            },
            md: {
                metrics: { cols: 3, rows: 1 },
                raftCharts: { cols: 1, rows: 1 },
                metadataCharts: { cols: 1, rows: 1 }
            },
            sm: {
                metrics: { cols: 1, rows: 3 },
                raftCharts: { cols: 1, rows: 1 },
                metadataCharts: { cols: 1, rows: 1 }
            },
            xs: {
                metrics: { cols: 1, rows: 3 },
                raftCharts: { cols: 1, rows: 1 },
                metadataCharts: { cols: 1, rows: 1 }
            }
        });

        // Chunk server page layouts
        this.layouts.set('chunk-server', {
            xl: {
                metrics: { cols: 4, rows: 1 },
                performance: { cols: 2, rows: 1 },
                storage: { cols: 2, rows: 1 }
            },
            lg: {
                metrics: { cols: 4, rows: 1 },
                performance: { cols: 2, rows: 1 },
                storage: { cols: 1, rows: 2 }
            },
            md: {
                metrics: { cols: 2, rows: 2 },
                performance: { cols: 1, rows: 1 },
                storage: { cols: 1, rows: 1 }
            },
            sm: {
                metrics: { cols: 2, rows: 2 },
                performance: { cols: 1, rows: 1 },
                storage: { cols: 1, rows: 1 }
            },
            xs: {
                metrics: { cols: 1, rows: 4 },
                performance: { cols: 1, rows: 1 },
                storage: { cols: 1, rows: 1 }
            }
        });

        // Client page layouts
        this.layouts.set('client', {
            xl: {
                metrics: { cols: 3, rows: 1 },
                operations: { cols: 2, rows: 1 },
                network: { cols: 1, rows: 2 }
            },
            lg: {
                metrics: { cols: 3, rows: 1 },
                operations: { cols: 2, rows: 1 },
                network: { cols: 1, rows: 1 }
            },
            md: {
                metrics: { cols: 3, rows: 1 },
                operations: { cols: 1, rows: 1 },
                network: { cols: 1, rows: 1 }
            },
            sm: {
                metrics: { cols: 1, rows: 3 },
                operations: { cols: 1, rows: 1 },
                network: { cols: 1, rows: 1 }
            },
            xs: {
                metrics: { cols: 1, rows: 3 },
                operations: { cols: 1, rows: 1 },
                network: { cols: 1, rows: 1 }
            }
        });

        // Multiregion page layouts
        this.layouts.set('multiregion', {
            xl: {
                metrics: { cols: 3, rows: 1 },
                topology: { cols: 1, rows: 1 },
                performance: { cols: 1, rows: 1 }
            },
            lg: {
                metrics: { cols: 3, rows: 1 },
                topology: { cols: 1, rows: 1 },
                performance: { cols: 1, rows: 1 }
            },
            md: {
                metrics: { cols: 3, rows: 1 },
                topology: { cols: 1, rows: 1 },
                performance: { cols: 1, rows: 1 }
            },
            sm: {
                metrics: { cols: 1, rows: 3 },
                topology: { cols: 1, rows: 1 },
                performance: { cols: 1, rows: 1 }
            },
            xs: {
                metrics: { cols: 1, rows: 3 },
                topology: { cols: 1, rows: 1 },
                performance: { cols: 1, rows: 1 }
            }
        });
    }

    // Layout Application
    applyCurrentLayout() {
        const pages = ['overview', 'master-server', 'chunk-server', 'client', 'multiregion'];
        
        pages.forEach(page => {
            this.applyPageLayout(page);
        });
    }

    applyPageLayout(pageName) {
        const pageLayout = this.layouts.get(pageName);
        if (!pageLayout) return;

        const layout = pageLayout[this.currentBreakpoint] || pageLayout.md;
        const pageElement = document.getElementById(`${pageName}-page`);
        
        if (!pageElement) return;

        // Apply layout to all grid containers in the page
        const gridContainers = pageElement.querySelectorAll('.grid, .dashboard-grid');
        
        gridContainers.forEach((container, index) => {
            this.applyGridLayout(container, layout, index);
        });

        // Apply component-specific layouts
        this.applyComponentLayouts(pageElement, layout);
    }

    applyGridLayout(container, layout, index) {
        // Clear existing grid classes
        container.classList.forEach(className => {
            if (className.startsWith('grid-cols-') || className.startsWith('grid-rows-')) {
                container.classList.remove(className);
            }
        });

        // Apply new grid layout based on container context
        const containerType = this.getContainerType(container);
        const layoutConfig = layout[containerType];
        
        if (layoutConfig) {
            container.classList.add(`grid-cols-${layoutConfig.cols}`);
            if (layoutConfig.rows > 1) {
                container.classList.add(`grid-rows-${layoutConfig.rows}`);
            }
        }
    }

    getContainerType(container) {
        // Determine container type based on its content or classes
        if (container.querySelector('.metric-card')) {
            return 'metrics';
        }
        if (container.querySelector('.chart-container')) {
            return 'charts';
        }
        if (container.querySelector('[id*="chart"]')) {
            return 'performance';
        }
        return 'default';
    }

    applyComponentLayouts(pageElement, layout) {
        // Apply specific component layouts
        const chartContainers = pageElement.querySelectorAll('.chart-container');
        
        chartContainers.forEach(container => {
            this.adjustChartContainer(container);
        });

        // Adjust metric cards
        const metricCards = pageElement.querySelectorAll('.metric-card');
        metricCards.forEach(card => {
            this.adjustMetricCard(card);
        });
    }

    adjustChartContainer(container) {
        const minHeight = this.getMinChartHeight();
        const maxHeight = this.getMaxChartHeight();
        
        container.style.minHeight = `${minHeight}px`;
        container.style.maxHeight = `${maxHeight}px`;
        
        // Adjust chart size based on breakpoint
        if (this.currentBreakpoint === 'xs' || this.currentBreakpoint === 'sm') {
            container.classList.add('chart-small');
            container.classList.remove('chart-large');
        } else if (this.currentBreakpoint === 'xl') {
            container.classList.add('chart-large');
            container.classList.remove('chart-small');
        } else {
            container.classList.remove('chart-small', 'chart-large');
        }
    }

    adjustMetricCard(card) {
        // Adjust metric card padding and font sizes based on breakpoint
        const valueElement = card.querySelector('.metric-value');
        const labelElement = card.querySelector('.metric-label');
        
        if (valueElement) {
            if (this.currentBreakpoint === 'xs') {
                valueElement.style.fontSize = '1.5rem';
            } else if (this.currentBreakpoint === 'sm') {
                valueElement.style.fontSize = '1.75rem';
            } else {
                valueElement.style.fontSize = '2rem';
            }
        }
        
        if (labelElement) {
            if (this.currentBreakpoint === 'xs' || this.currentBreakpoint === 'sm') {
                labelElement.style.fontSize = '0.75rem';
            } else {
                labelElement.style.fontSize = '0.875rem';
            }
        }
    }

    // Responsive Utilities
    getCurrentBreakpoint() {
        const width = window.innerWidth;
        
        if (width >= this.breakpoints.xl) return 'xl';
        if (width >= this.breakpoints.lg) return 'lg';
        if (width >= this.breakpoints.md) return 'md';
        if (width >= this.breakpoints.sm) return 'sm';
        return 'xs';
    }

    getMinChartHeight() {
        switch (this.currentBreakpoint) {
            case 'xs': return 200;
            case 'sm': return 250;
            case 'md': return 300;
            case 'lg': return 350;
            case 'xl': return 400;
            default: return 300;
        }
    }

    getMaxChartHeight() {
        switch (this.currentBreakpoint) {
            case 'xs': return 300;
            case 'sm': return 400;
            case 'md': return 500;
            case 'lg': return 600;
            case 'xl': return 700;
            default: return 500;
        }
    }

    // Event Handlers
    handleResize() {
        const newBreakpoint = this.getCurrentBreakpoint();
        
        if (newBreakpoint !== this.currentBreakpoint) {
            this.currentBreakpoint = newBreakpoint;
            this.applyCurrentLayout();
            this.notifyComponents('breakpointChange', newBreakpoint);
            
            // Trigger chart resize
            if (window.ChartFactory) {
                window.ChartFactory.resizeCharts();
            }
            if (window.dashboard && window.dashboard.resizeCharts) {
                window.dashboard.resizeCharts();
            }
        }
    }

    // Component Registration and Management
    registerComponent(id, component, options = {}) {
        this.components.set(id, {
            component,
            options,
            lastUpdate: Date.now()
        });

        // Set up intersection observer for lazy loading
        if (options.lazyLoad && component.element) {
            this.setupLazyLoading(component.element, id);
        }
    }

    unregisterComponent(id) {
        this.components.delete(id);
        
        if (this.observers.has(id)) {
            this.observers.get(id).disconnect();
            this.observers.delete(id);
        }
    }

    notifyComponents(event, data) {
        this.components.forEach((componentData, id) => {
            if (componentData.component && typeof componentData.component.handleLayoutEvent === 'function') {
                try {
                    componentData.component.handleLayoutEvent(event, data);
                } catch (error) {
                    console.error(`Error notifying component ${id}:`, error);
                }
            }
        });
    }

    // Lazy Loading and Performance
    setupResizeObserver() {
        if (!window.ResizeObserver) return;

        this.resizeObserver = new ResizeObserver(entries => {
            entries.forEach(entry => {
                const element = entry.target;
                const componentId = element.dataset.componentId;
                
                if (componentId && this.components.has(componentId)) {
                    this.handleComponentResize(componentId, entry);
                }
            });
        });
    }

    setupIntersectionObserver() {
        if (!window.IntersectionObserver) return;

        this.intersectionObserver = new IntersectionObserver(entries => {
            entries.forEach(entry => {
                const element = entry.target;
                const componentId = element.dataset.componentId;
                
                if (componentId && this.components.has(componentId)) {
                    this.handleComponentVisibility(componentId, entry.isIntersecting);
                }
            });
        }, {
            root: null,
            rootMargin: '50px',
            threshold: 0.1
        });
    }

    setupLazyLoading(element, componentId) {
        element.dataset.componentId = componentId;
        this.intersectionObserver.observe(element);
    }

    handleComponentResize(componentId, entry) {
        const componentData = this.components.get(componentId);
        if (!componentData) return;

        const { width, height } = entry.contentRect;
        
        if (componentData.component.handleResize) {
            componentData.component.handleResize(width, height);
        }
    }

    handleComponentVisibility(componentId, isVisible) {
        const componentData = this.components.get(componentId);
        if (!componentData) return;

        if (isVisible && componentData.options.lazyLoad) {
            // Load component data when it becomes visible
            if (componentData.component.loadData && !componentData.loaded) {
                componentData.component.loadData();
                componentData.loaded = true;
            }
        }

        if (componentData.component.handleVisibilityChange) {
            componentData.component.handleVisibilityChange(isVisible);
        }
    }

    // Layout Presets
    createResponsiveGrid(container, items, options = {}) {
        const {
            minItemWidth = 300,
            maxItemWidth = 500,
            gap = '1.5rem',
            aspectRatio = null
        } = options;

        container.style.display = 'grid';
        container.style.gap = gap;
        container.style.gridTemplateColumns = `repeat(auto-fit, minmax(${minItemWidth}px, ${maxItemWidth}px))`;
        
        if (aspectRatio) {
            container.style.gridAutoRows = `minmax(${minItemWidth / aspectRatio}px, 1fr)`;
        }

        return container;
    }

    createDashboard(containerId, config) {
        const container = document.getElementById(containerId);
        if (!container) return null;

        const {
            sections = [],
            responsive = true,
            theme = 'light'
        } = config;

        container.classList.add('dashboard-container');
        if (responsive) {
            container.classList.add('responsive-dashboard');
        }

        sections.forEach(section => {
            const sectionElement = this.createSection(section);
            container.appendChild(sectionElement);
        });

        return container;
    }

    createSection(config) {
        const {
            id,
            title,
            components = [],
            layout = 'grid',
            className = ''
        } = config;

        const section = document.createElement('section');
        section.id = id;
        section.className = `dashboard-section ${className}`;

        if (title) {
            const header = document.createElement('h2');
            header.textContent = title;
            header.className = 'section-title';
            section.appendChild(header);
        }

        const content = document.createElement('div');
        content.className = `section-content ${layout}`;
        
        components.forEach(component => {
            const componentElement = this.createComponent(component);
            content.appendChild(componentElement);
        });

        section.appendChild(content);
        return section;
    }

    createComponent(config) {
        const {
            type,
            id,
            className = '',
            data = {},
            options = {}
        } = config;

        const component = document.createElement('div');
        component.id = id;
        component.className = `dashboard-component ${type} ${className}`;
        component.dataset.componentId = id;

        // Register component for layout management
        this.registerComponent(id, { element: component }, options);

        return component;
    }

    // Utility Methods
    debounce(func, wait) {
        let timeout;
        return function executedFunction(...args) {
            const later = () => {
                clearTimeout(timeout);
                func(...args);
            };
            clearTimeout(timeout);
            timeout = setTimeout(later, wait);
        };
    }

    throttle(func, limit) {
        let inThrottle;
        return function() {
            const args = arguments;
            const context = this;
            if (!inThrottle) {
                func.apply(context, args);
                inThrottle = true;
                setTimeout(() => inThrottle = false, limit);
            }
        };
    }

    // Public API
    setBreakpoint(breakpoint) {
        if (this.breakpoints[breakpoint]) {
            this.currentBreakpoint = breakpoint;
            this.applyCurrentLayout();
        }
    }

    getBreakpoint() {
        return this.currentBreakpoint;
    }

    isBreakpoint(breakpoint) {
        return this.currentBreakpoint === breakpoint;
    }

    isMobile() {
        return this.currentBreakpoint === 'xs' || this.currentBreakpoint === 'sm';
    }

    isTablet() {
        return this.currentBreakpoint === 'md';
    }

    isDesktop() {
        return this.currentBreakpoint === 'lg' || this.currentBreakpoint === 'xl';
    }

    // Cleanup
    destroy() {
        if (this.resizeObserver) {
            this.resizeObserver.disconnect();
        }
        
        if (this.intersectionObserver) {
            this.intersectionObserver.disconnect();
        }
        
        this.observers.forEach(observer => observer.disconnect());
        this.observers.clear();
        this.components.clear();
        this.layouts.clear();
    }
}

// Export layout instance
window.ResponsiveLayout = new ResponsiveLayout();

// Export for module systems
if (typeof module !== 'undefined' && module.exports) {
    module.exports = ResponsiveLayout;
}