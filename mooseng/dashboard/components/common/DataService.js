/**
 * DataService - Advanced Data Management for MooseNG Dashboard
 * 
 * Handles API communication, caching, real-time updates, and data transformation
 * for the benchmark dashboard with WebSocket support and offline capabilities.
 */

class DataService {
    constructor() {
        this.baseURL = '/api/mooseng';
        this.cache = new Map();
        this.cacheTimeout = 30000; // 30 seconds
        this.subscribers = new Map();
        this.websocket = null;
        this.retryAttempts = 3;
        this.retryDelay = 1000;
        this.isOnline = navigator.onLine;
        
        this.endpoints = {
            overview: '/metrics/overview',
            master: '/metrics/master-server',
            chunkServer: '/metrics/chunk-server',
            client: '/metrics/client',
            multiregion: '/metrics/multiregion',
            benchmarks: '/benchmarks',
            health: '/health',
            realtime: '/ws/metrics'
        };

        this.initializeWebSocket();
        this.setupNetworkMonitoring();
    }

    // Core API methods
    async fetchData(endpoint, options = {}) {
        const cacheKey = this.getCacheKey(endpoint, options);
        const cached = this.getFromCache(cacheKey);
        
        if (cached && !options.forceRefresh) {
            return cached;
        }

        try {
            const response = await this.makeRequest(endpoint, options);
            const data = await this.processResponse(response);
            
            this.setCache(cacheKey, data);
            this.notifySubscribers(endpoint, data);
            
            return data;
        } catch (error) {
            console.error(`Error fetching data from ${endpoint}:`, error);
            
            // Return cached data if available during error
            if (cached) {
                console.warn('Returning cached data due to fetch error');
                return cached;
            }
            
            throw error;
        }
    }

    async makeRequest(endpoint, options = {}) {
        const url = `${this.baseURL}${endpoint}`;
        const requestOptions = {
            method: options.method || 'GET',
            headers: {
                'Content-Type': 'application/json',
                'Accept': 'application/json',
                ...options.headers
            },
            ...options
        };

        if (options.body && typeof options.body === 'object') {
            requestOptions.body = JSON.stringify(options.body);
        }

        let lastError;
        for (let attempt = 0; attempt < this.retryAttempts; attempt++) {
            try {
                const response = await fetch(url, requestOptions);
                
                if (!response.ok) {
                    throw new Error(`HTTP ${response.status}: ${response.statusText}`);
                }
                
                return response;
            } catch (error) {
                lastError = error;
                
                if (attempt < this.retryAttempts - 1) {
                    await this.delay(this.retryDelay * Math.pow(2, attempt));
                }
            }
        }
        
        throw lastError;
    }

    async processResponse(response) {
        const contentType = response.headers.get('content-type');
        
        if (contentType && contentType.includes('application/json')) {
            return await response.json();
        }
        
        return await response.text();
    }

    // Specialized data fetchers
    async getOverviewMetrics(options = {}) {
        const data = await this.fetchData(this.endpoints.overview, options);
        return this.transformOverviewData(data);
    }

    async getMasterServerMetrics(options = {}) {
        const data = await this.fetchData(this.endpoints.master, options);
        return this.transformMasterServerData(data);
    }

    async getChunkServerMetrics(options = {}) {
        const data = await this.fetchData(this.endpoints.chunkServer, options);
        return this.transformChunkServerData(data);
    }

    async getClientMetrics(options = {}) {
        const data = await this.fetchData(this.endpoints.client, options);
        return this.transformClientData(data);
    }

    async getMultiregionMetrics(options = {}) {
        const data = await this.fetchData(this.endpoints.multiregion, options);
        return this.transformMultiregionData(data);
    }

    async getBenchmarkResults(filters = {}) {
        const queryString = new URLSearchParams(filters).toString();
        const endpoint = `${this.endpoints.benchmarks}?${queryString}`;
        return await this.fetchData(endpoint);
    }

    async getHealthStatus() {
        return await this.fetchData(this.endpoints.health);
    }

    // Data transformation methods
    transformOverviewData(rawData) {
        if (!rawData) return this.getDefaultOverviewData();

        return {
            metrics: {
                totalFiles: this.safeNumber(rawData.total_files),
                totalChunks: this.safeNumber(rawData.total_chunks),
                storageUsed: this.safePercent(rawData.storage_used),
                activeSessions: this.safeNumber(rawData.active_sessions),
                throughput: {
                    read: this.safeMBps(rawData.read_throughput_mbps),
                    write: this.safeMBps(rawData.write_throughput_mbps)
                },
                latency: {
                    avg: this.safeMs(rawData.avg_latency_ms),
                    p95: this.safeMs(rawData.p95_latency_ms),
                    p99: this.safeMs(rawData.p99_latency_ms)
                }
            },
            timeSeries: this.transformTimeSeries(rawData.time_series),
            health: rawData.health || 'unknown',
            timestamp: rawData.timestamp || Date.now()
        };
    }

    transformMasterServerData(rawData) {
        if (!rawData) return this.getDefaultMasterServerData();

        return {
            raftMetrics: {
                status: rawData.raft_status || 'unknown',
                leaderElectionTime: this.safeMs(rawData.leader_election_time_ms),
                logReplicationTime: this.safeMs(rawData.log_replication_time_ms),
                consensusLatency: this.safeMs(rawData.consensus_latency_ms)
            },
            metadataMetrics: {
                operationsPerSecond: this.safeNumber(rawData.metadata_ops_per_sec),
                cacheHitRate: this.safePercent(rawData.cache_hit_rate),
                cacheSize: this.safeBytes(rawData.cache_size_bytes)
            },
            multiregionMetrics: {
                syncStatus: rawData.multiregion_sync_status || 'unknown',
                crossRegionLatency: this.safeMs(rawData.cross_region_latency_ms),
                replicationLag: this.safeMs(rawData.replication_lag_ms)
            },
            timeSeries: this.transformTimeSeries(rawData.time_series),
            timestamp: rawData.timestamp || Date.now()
        };
    }

    transformChunkServerData(rawData) {
        if (!rawData) return this.getDefaultChunkServerData();

        return {
            storageMetrics: {
                totalChunks: this.safeNumber(rawData.total_chunks),
                diskUsage: this.safePercent(rawData.disk_usage),
                availableSpace: this.safeBytes(rawData.available_space_bytes),
                usedSpace: this.safeBytes(rawData.used_space_bytes)
            },
            ioMetrics: {
                readThroughput: this.safeMBps(rawData.read_throughput_mbps),
                writeThroughput: this.safeMBps(rawData.write_throughput_mbps),
                iops: this.safeNumber(rawData.iops),
                readLatency: this.safeMs(rawData.read_latency_ms),
                writeLatency: this.safeMs(rawData.write_latency_ms)
            },
            erasureCodingMetrics: {
                efficiency: this.safePercent(rawData.erasure_coding_efficiency),
                repairOperations: this.safeNumber(rawData.repair_operations),
                repairTime: this.safeMs(rawData.avg_repair_time_ms)
            },
            timeSeries: this.transformTimeSeries(rawData.time_series),
            timestamp: rawData.timestamp || Date.now()
        };
    }

    transformClientData(rawData) {
        if (!rawData) return this.getDefaultClientData();

        return {
            connectionMetrics: {
                activeConnections: this.safeNumber(rawData.active_connections),
                totalConnections: this.safeNumber(rawData.total_connections),
                connectionLatency: this.safeMs(rawData.connection_latency_ms)
            },
            cacheMetrics: {
                hitRate: this.safePercent(rawData.cache_hit_rate),
                cacheSize: this.safeBytes(rawData.cache_size_bytes),
                evictions: this.safeNumber(rawData.cache_evictions)
            },
            fileOperations: {
                reads: this.safeNumber(rawData.file_reads_per_sec),
                writes: this.safeNumber(rawData.file_writes_per_sec),
                creates: this.safeNumber(rawData.file_creates_per_sec),
                deletes: this.safeNumber(rawData.file_deletes_per_sec)
            },
            timeSeries: this.transformTimeSeries(rawData.time_series),
            timestamp: rawData.timestamp || Date.now()
        };
    }

    transformMultiregionData(rawData) {
        if (!rawData) return this.getDefaultMultiregionData();

        return {
            regionStatus: rawData.regions?.map(region => ({
                name: region.name,
                status: region.status || 'unknown',
                latency: this.safeMs(region.latency_ms),
                throughput: this.safeMBps(region.throughput_mbps),
                lastSync: region.last_sync_timestamp
            })) || [],
            crossRegionMetrics: {
                averageLatency: this.safeMs(rawData.avg_cross_region_latency_ms),
                replicationLag: this.safeMs(rawData.replication_lag_ms),
                consistencyLevel: rawData.consistency_level || 'unknown'
            },
            networkTopology: rawData.network_topology || {
                nodes: [],
                links: []
            },
            timeSeries: this.transformTimeSeries(rawData.time_series),
            timestamp: rawData.timestamp || Date.now()
        };
    }

    transformTimeSeries(timeSeriesData) {
        if (!Array.isArray(timeSeriesData)) return [];
        
        return timeSeriesData.map(point => ({
            timestamp: point.timestamp || Date.now(),
            value: this.safeNumber(point.value),
            secondary: this.safeNumber(point.secondary),
            tertiary: this.safeNumber(point.tertiary)
        }));
    }

    // Cache management
    getCacheKey(endpoint, options) {
        return `${endpoint}-${JSON.stringify(options)}`;
    }

    getFromCache(key) {
        const cached = this.cache.get(key);
        if (!cached) return null;
        
        if (Date.now() - cached.timestamp > this.cacheTimeout) {
            this.cache.delete(key);
            return null;
        }
        
        return cached.data;
    }

    setCache(key, data) {
        this.cache.set(key, {
            data,
            timestamp: Date.now()
        });
    }

    clearCache() {
        this.cache.clear();
    }

    // WebSocket real-time updates
    initializeWebSocket() {
        if (!window.WebSocket) {
            console.warn('WebSocket not supported, falling back to polling');
            return;
        }

        try {
            const wsUrl = `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}${this.baseURL}${this.endpoints.realtime}`;
            this.websocket = new WebSocket(wsUrl);
            
            this.websocket.onopen = () => {
                console.log('WebSocket connected for real-time updates');
            };
            
            this.websocket.onmessage = (event) => {
                try {
                    const data = JSON.parse(event.data);
                    this.handleRealtimeUpdate(data);
                } catch (error) {
                    console.error('Error parsing WebSocket message:', error);
                }
            };
            
            this.websocket.onclose = () => {
                console.log('WebSocket disconnected, attempting to reconnect...');
                setTimeout(() => this.initializeWebSocket(), 5000);
            };
            
            this.websocket.onerror = (error) => {
                console.error('WebSocket error:', error);
            };
        } catch (error) {
            console.error('Failed to initialize WebSocket:', error);
        }
    }

    handleRealtimeUpdate(data) {
        const { type, payload } = data;
        
        // Update cache with new data
        this.setCache(type, payload);
        
        // Notify subscribers
        this.notifySubscribers(type, payload);
    }

    // Subscription system for real-time updates
    subscribe(endpoint, callback) {
        if (!this.subscribers.has(endpoint)) {
            this.subscribers.set(endpoint, new Set());
        }
        
        this.subscribers.get(endpoint).add(callback);
        
        // Return unsubscribe function
        return () => {
            const callbacks = this.subscribers.get(endpoint);
            if (callbacks) {
                callbacks.delete(callback);
            }
        };
    }

    notifySubscribers(endpoint, data) {
        const callbacks = this.subscribers.get(endpoint);
        if (callbacks) {
            callbacks.forEach(callback => {
                try {
                    callback(data);
                } catch (error) {
                    console.error('Error in subscriber callback:', error);
                }
            });
        }
    }

    // Network monitoring
    setupNetworkMonitoring() {
        window.addEventListener('online', () => {
            this.isOnline = true;
            console.log('Network connection restored');
            this.clearCache(); // Clear cache to get fresh data
        });
        
        window.addEventListener('offline', () => {
            this.isOnline = false;
            console.log('Network connection lost');
        });
    }

    // Utility methods
    safeNumber(value, defaultValue = 0) {
        const num = parseFloat(value);
        return isNaN(num) ? defaultValue : num;
    }

    safePercent(value, defaultValue = 0) {
        const num = this.safeNumber(value, defaultValue);
        return Math.max(0, Math.min(100, num));
    }

    safeMBps(value, defaultValue = 0) {
        return this.safeNumber(value, defaultValue);
    }

    safeMs(value, defaultValue = 0) {
        return this.safeNumber(value, defaultValue);
    }

    safeBytes(value, defaultValue = 0) {
        return this.safeNumber(value, defaultValue);
    }

    formatBytes(bytes) {
        if (bytes === 0) return '0 B';
        
        const k = 1024;
        const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
        const i = Math.floor(Math.log(bytes) / Math.log(k));
        
        return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`;
    }

    formatDuration(ms) {
        if (ms < 1000) return `${ms.toFixed(1)}ms`;
        if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
        if (ms < 3600000) return `${(ms / 60000).toFixed(1)}m`;
        return `${(ms / 3600000).toFixed(1)}h`;
    }

    delay(ms) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }

    // Default data for offline/error scenarios
    getDefaultOverviewData() {
        return {
            metrics: {
                totalFiles: 0,
                totalChunks: 0,
                storageUsed: 0,
                activeSessions: 0,
                throughput: { read: 0, write: 0 },
                latency: { avg: 0, p95: 0, p99: 0 }
            },
            timeSeries: [],
            health: 'unknown',
            timestamp: Date.now()
        };
    }

    getDefaultMasterServerData() {
        return {
            raftMetrics: {
                status: 'unknown',
                leaderElectionTime: 0,
                logReplicationTime: 0,
                consensusLatency: 0
            },
            metadataMetrics: {
                operationsPerSecond: 0,
                cacheHitRate: 0,
                cacheSize: 0
            },
            multiregionMetrics: {
                syncStatus: 'unknown',
                crossRegionLatency: 0,
                replicationLag: 0
            },
            timeSeries: [],
            timestamp: Date.now()
        };
    }

    getDefaultChunkServerData() {
        return {
            storageMetrics: {
                totalChunks: 0,
                diskUsage: 0,
                availableSpace: 0,
                usedSpace: 0
            },
            ioMetrics: {
                readThroughput: 0,
                writeThroughput: 0,
                iops: 0,
                readLatency: 0,
                writeLatency: 0
            },
            erasureCodingMetrics: {
                efficiency: 0,
                repairOperations: 0,
                repairTime: 0
            },
            timeSeries: [],
            timestamp: Date.now()
        };
    }

    getDefaultClientData() {
        return {
            connectionMetrics: {
                activeConnections: 0,
                totalConnections: 0,
                connectionLatency: 0
            },
            cacheMetrics: {
                hitRate: 0,
                cacheSize: 0,
                evictions: 0
            },
            fileOperations: {
                reads: 0,
                writes: 0,
                creates: 0,
                deletes: 0
            },
            timeSeries: [],
            timestamp: Date.now()
        };
    }

    getDefaultMultiregionData() {
        return {
            regionStatus: [],
            crossRegionMetrics: {
                averageLatency: 0,
                replicationLag: 0,
                consistencyLevel: 'unknown'
            },
            networkTopology: {
                nodes: [],
                links: []
            },
            timeSeries: [],
            timestamp: Date.now()
        };
    }
}

// Export service instance
window.DataService = new DataService();

// Export for module systems
if (typeof module !== 'undefined' && module.exports) {
    module.exports = DataService;
}