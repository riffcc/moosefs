/**
 * API Client for MooseNG Dashboard
 * Handles communication with the Rust backend for benchmark data
 */

class ApiClient {
    constructor(baseUrl = '/api', options = {}) {
        this.baseUrl = baseUrl.replace(/\/$/, ''); // Remove trailing slash
        this.options = {
            timeout: 30000, // 30 seconds
            retries: 3,
            retryDelay: 1000, // 1 second
            ...options
        };
        
        this.cache = new Map();
        this.cacheTimeout = 30000; // 30 seconds cache
    }

    /**
     * Make HTTP request with error handling and retries
     */
    async request(endpoint, options = {}) {
        const url = `${this.baseUrl}${endpoint}`;
        const config = {
            method: 'GET',
            headers: {
                'Content-Type': 'application/json',
                'Accept': 'application/json',
                ...options.headers
            },
            ...options
        };

        let lastError;
        for (let attempt = 0; attempt <= this.options.retries; attempt++) {
            try {
                const controller = new AbortController();
                const timeoutId = setTimeout(() => controller.abort(), this.options.timeout);
                
                const response = await fetch(url, {
                    ...config,
                    signal: controller.signal
                });
                
                clearTimeout(timeoutId);
                
                if (!response.ok) {
                    throw new Error(`HTTP ${response.status}: ${response.statusText}`);
                }
                
                const data = await response.json();
                return data;
                
            } catch (error) {
                lastError = error;
                
                if (attempt < this.options.retries) {
                    console.warn(`Request failed, retrying in ${this.options.retryDelay}ms...`, error);
                    await this.delay(this.options.retryDelay);
                } else {
                    console.error(`Request failed after ${this.options.retries + 1} attempts:`, error);
                }
            }
        }
        
        throw lastError;
    }

    /**
     * GET request with caching
     */
    async get(endpoint, useCache = true) {
        const cacheKey = `GET:${endpoint}`;
        
        if (useCache && this.cache.has(cacheKey)) {
            const cached = this.cache.get(cacheKey);
            if (Date.now() - cached.timestamp < this.cacheTimeout) {
                return cached.data;
            }
        }
        
        try {
            const data = await this.request(endpoint);
            
            if (useCache) {
                this.cache.set(cacheKey, {
                    data,
                    timestamp: Date.now()
                });
            }
            
            return data;
        } catch (error) {
            // Return cached data if available on error
            if (useCache && this.cache.has(cacheKey)) {
                console.warn('Using cached data due to request failure');
                return this.cache.get(cacheKey).data;
            }
            throw error;
        }
    }

    /**
     * POST request
     */
    async post(endpoint, data) {
        return this.request(endpoint, {
            method: 'POST',
            body: JSON.stringify(data)
        });
    }

    /**
     * PUT request
     */
    async put(endpoint, data) {
        return this.request(endpoint, {
            method: 'PUT',
            body: JSON.stringify(data)
        });
    }

    /**
     * DELETE request
     */
    async delete(endpoint) {
        return this.request(endpoint, {
            method: 'DELETE'
        });
    }

    // Benchmark Data API Methods

    /**
     * Get overview metrics
     */
    async getOverviewMetrics() {
        return this.get('/benchmarks/overview');
    }

    /**
     * Get master server metrics
     */
    async getMasterServerMetrics() {
        return this.get('/benchmarks/master-server');
    }

    /**
     * Get chunk server metrics
     */
    async getChunkServerMetrics() {
        return this.get('/benchmarks/chunk-server');
    }

    /**
     * Get client metrics
     */
    async getClientMetrics() {
        return this.get('/benchmarks/client');
    }

    /**
     * Get multi-region metrics
     */
    async getMultiRegionMetrics() {
        return this.get('/benchmarks/multiregion');
    }

    /**
     * Get historical benchmark results
     */
    async getHistoricalResults(timeRange = '24h') {
        return this.get(`/benchmarks/history?range=${timeRange}`);
    }

    /**
     * Get real-time metrics
     */
    async getRealTimeMetrics(component) {
        return this.get(`/metrics/realtime/${component}`, false); // Don't cache real-time data
    }

    /**
     * Get system health status
     */
    async getHealthStatus() {
        return this.get('/health');
    }

    /**
     * Get performance comparison data
     */
    async getPerformanceComparison(baseline, current) {
        return this.get(`/benchmarks/compare?baseline=${baseline}&current=${current}`);
    }

    /**
     * Trigger benchmark run
     */
    async triggerBenchmark(benchmarkType, config = {}) {
        return this.post('/benchmarks/run', {
            type: benchmarkType,
            config
        });
    }

    /**
     * Get benchmark run status
     */
    async getBenchmarkStatus(runId) {
        return this.get(`/benchmarks/runs/${runId}`);
    }

    /**
     * Get available benchmark configurations
     */
    async getBenchmarkConfigs() {
        return this.get('/benchmarks/configs');
    }

    /**
     * Get system configuration
     */
    async getSystemConfig() {
        return this.get('/config');
    }

    /**
     * Update system configuration
     */
    async updateSystemConfig(config) {
        return this.put('/config', config);
    }

    // WebSocket for real-time updates

    /**
     * Connect to real-time updates via WebSocket
     */
    connectRealTime(onMessage, onError = null, onClose = null) {
        const wsProtocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${wsProtocol}//${location.host}/ws/metrics`;
        
        try {
            const ws = new WebSocket(wsUrl);
            
            ws.onopen = () => {
                console.log('WebSocket connected for real-time updates');
            };
            
            ws.onmessage = (event) => {
                try {
                    const data = JSON.parse(event.data);
                    onMessage(data);
                } catch (error) {
                    console.error('Error parsing WebSocket message:', error);
                }
            };
            
            ws.onerror = (error) => {
                console.error('WebSocket error:', error);
                if (onError) onError(error);
            };
            
            ws.onclose = (event) => {
                console.log('WebSocket connection closed:', event.code, event.reason);
                if (onClose) onClose(event);
                
                // Auto-reconnect after 5 seconds
                setTimeout(() => {
                    console.log('Attempting to reconnect WebSocket...');
                    this.connectRealTime(onMessage, onError, onClose);
                }, 5000);
            };
            
            return ws;
        } catch (error) {
            console.error('Failed to create WebSocket connection:', error);
            if (onError) onError(error);
            return null;
        }
    }

    /**
     * Server-Sent Events fallback for real-time updates
     */
    connectSSE(onMessage, onError = null) {
        if (!window.EventSource) {
            console.warn('Server-Sent Events not supported');
            return null;
        }
        
        try {
            const eventSource = new EventSource(`${this.baseUrl}/stream/metrics`);
            
            eventSource.onmessage = (event) => {
                try {
                    const data = JSON.parse(event.data);
                    onMessage(data);
                } catch (error) {
                    console.error('Error parsing SSE message:', error);
                }
            };
            
            eventSource.onerror = (error) => {
                console.error('SSE error:', error);
                if (onError) onError(error);
            };
            
            return eventSource;
        } catch (error) {
            console.error('Failed to create SSE connection:', error);
            if (onError) onError(error);
            return null;
        }
    }

    // Utility Methods

    /**
     * Delay helper for retries
     */
    delay(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }

    /**
     * Clear cache
     */
    clearCache() {
        this.cache.clear();
    }

    /**
     * Check if API is available
     */
    async ping() {
        try {
            await this.get('/ping');
            return true;
        } catch {
            return false;
        }
    }

    /**
     * Get API version
     */
    async getVersion() {
        return this.get('/version');
    }

    /**
     * Mock data methods for development
     */
    async getMockData(endpoint) {
        // Simulate network delay
        await this.delay(100 + Math.random() * 400);
        
        const mockData = {
            '/benchmarks/overview': {
                totalOpsPerSecond: 1506 + Math.random() * 200,
                totalThroughputMBps: 135.13 + Math.random() * 20,
                avgLatencyMs: 45.2 + Math.random() * 10,
                systemGrade: ['A', 'B+', 'A-'][Math.floor(Math.random() * 3)],
                timestamp: new Date().toISOString()
            },
            '/benchmarks/master-server': {
                raftConsensusTime: 12.5 + Math.random() * 5,
                metadataOpsPerSec: 2450 + Math.random() * 300,
                cacheHitRate: 94.2 + Math.random() * 5,
                timestamp: new Date().toISOString()
            },
            '/benchmarks/chunk-server': {
                readThroughputMBps: 450.2 + Math.random() * 50,
                writeThroughputMBps: 380.5 + Math.random() * 40,
                erasureCodingEfficiency: 96.8 + Math.random() * 2,
                storageUtilization: 72.5 + Math.random() * 10,
                timestamp: new Date().toISOString()
            },
            '/benchmarks/client': {
                fuseLatencyMs: 8.4 + Math.random() * 2,
                cacheHitRate: 89.3 + Math.random() * 5,
                concurrentConnections: 125 + Math.floor(Math.random() * 25),
                timestamp: new Date().toISOString()
            },
            '/benchmarks/multiregion': {
                crossRegionLatencyMs: 85.3 + Math.random() * 15,
                replicationEfficiency: 98.2 + Math.random() * 1.5,
                consistencyTime: 125.4 + Math.random() * 20,
                timestamp: new Date().toISOString()
            },
            '/health': {
                status: 'healthy',
                uptime: Math.floor(Date.now() / 1000) - 86400, // 1 day
                version: '1.0.0',
                components: {
                    master: 'healthy',
                    chunkserver: 'healthy',
                    client: 'healthy',
                    metalogger: 'healthy'
                }
            }
        };
        
        return mockData[endpoint] || { error: 'Mock data not found' };
    }

    /**
     * Enable mock mode for development
     */
    enableMockMode() {
        this.mockMode = true;
        console.log('API Client: Mock mode enabled');
    }

    /**
     * Disable mock mode
     */
    disableMockMode() {
        this.mockMode = false;
        console.log('API Client: Mock mode disabled');
    }

    /**
     * Override request method to use mock data in mock mode
     */
    async request(endpoint, options = {}) {
        if (this.mockMode) {
            return this.getMockData(endpoint);
        }
        return super.request ? super.request(endpoint, options) : this.originalRequest(endpoint, options);
    }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
    module.exports = ApiClient;
}