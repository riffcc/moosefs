/**
 * MooseNG Data Manager
 * Handles data fetching, caching, and processing for the dashboard
 */

class DataManager {
    constructor() {
        this.cache = new Map();
        this.cacheTimeout = 30000; // 30 seconds
        this.baseUrl = this.detectBaseUrl();
        this.mockMode = true; // Set to false when real API is available
    }

    detectBaseUrl() {
        // Try to detect if we're running in a development environment
        const hostname = window.location.hostname;
        if (hostname === 'localhost' || hostname === '127.0.0.1') {
            return '/api'; // Development API endpoint
        }
        return '/api'; // Production API endpoint
    }

    // Generic cache management
    getCacheKey(endpoint, params = {}) {
        const paramString = new URLSearchParams(params).toString();
        return `${endpoint}${paramString ? '?' + paramString : ''}`;
    }

    isCacheValid(cacheEntry) {
        return Date.now() - cacheEntry.timestamp < this.cacheTimeout;
    }

    setCache(key, data) {
        this.cache.set(key, {
            data: data,
            timestamp: Date.now()
        });
    }

    getCache(key) {
        const cacheEntry = this.cache.get(key);
        if (cacheEntry && this.isCacheValid(cacheEntry)) {
            return cacheEntry.data;
        }
        return null;
    }

    // Mock data generators for development
    generateMockOverviewData() {
        return {
            overall_grade: "A (Very Good)",
            throughput_mbps: Math.floor(Math.random() * 50 + 100).toString(),
            ops_per_second: Math.floor(Math.random() * 500 + 1200).toString(),
            total_time_ms: Math.floor(Math.random() * 1000 + 2000),
            performance_breakdown: {
                file_system_ms: Math.floor(Math.random() * 100 + 150),
                network_ms: Math.floor(Math.random() * 500 + 1500),
                cpu_memory_ms: Math.floor(Math.random() * 300 + 700),
                concurrency_ms: Math.floor(Math.random() * 20 + 20),
                throughput_ms: Math.floor(Math.random() * 200 + 300)
            },
            historical_data: Array.from({length: 24}, (_, i) => ({
                timestamp: new Date(Date.now() - (23-i) * 3600000).toISOString(),
                throughput: Math.floor(Math.random() * 50 + 100),
                response_time: Math.floor(Math.random() * 100 + 200),
                ops_per_second: Math.floor(Math.random() * 500 + 1200)
            }))
        };
    }

    generateMockMasterServerData() {
        return {
            status: "online",
            uptime: "15d 7h 23m",
            cpu_usage: Math.floor(Math.random() * 30 + 10),
            memory_usage: Math.floor(Math.random() * 40 + 20),
            active_connections: Math.floor(Math.random() * 50 + 100),
            metadata_operations: Math.floor(Math.random() * 1000 + 5000),
            raft_leader: true,
            cluster_health: 98.5,
            servers: Array.from({length: 3}, (_, i) => ({
                id: `master-${i + 1}`,
                status: i === 0 ? "leader" : "follower",
                uptime: `${Math.floor(Math.random() * 20 + 5)}d ${Math.floor(Math.random() * 24)}h`,
                cpu: Math.floor(Math.random() * 30 + 10),
                memory: Math.floor(Math.random() * 40 + 20),
                connections: Math.floor(Math.random() * 50 + 50)
            }))
        };
    }

    generateMockChunkServerData() {
        return {
            total_servers: 8,
            online_servers: 7,
            total_storage: "15.2 TB",
            used_storage: "8.7 TB",
            free_storage: "6.5 TB",
            replication_factor: 3,
            erasure_coding: "8+2",
            servers: Array.from({length: 8}, (_, i) => ({
                id: `chunk-${i + 1}`,
                status: i === 7 ? "offline" : "online",
                storage_used: Math.floor(Math.random() * 70 + 20),
                storage_total: "2.0 TB",
                chunks: Math.floor(Math.random() * 1000 + 2000),
                read_ops: Math.floor(Math.random() * 500 + 100),
                write_ops: Math.floor(Math.random() * 200 + 50),
                network_io: `${Math.floor(Math.random() * 50 + 10)} MB/s`
            }))
        };
    }

    generateMockClientData() {
        return {
            active_mounts: Math.floor(Math.random() * 20 + 30),
            total_operations: Math.floor(Math.random() * 10000 + 50000),
            cache_hit_rate: (Math.random() * 20 + 75).toFixed(1),
            average_latency: Math.floor(Math.random() * 10 + 5),
            clients: Array.from({length: 12}, (_, i) => ({
                id: `client-${i + 1}`,
                status: Math.random() > 0.1 ? "connected" : "disconnected",
                mount_point: `/mnt/mooseng${i + 1}`,
                operations: Math.floor(Math.random() * 1000 + 500),
                cache_size: `${Math.floor(Math.random() * 500 + 100)} MB`,
                bandwidth: `${Math.floor(Math.random() * 100 + 50)} MB/s`
            }))
        };
    }

    generateMockMultiRegionData() {
        return {
            regions: [
                {
                    name: "US-East",
                    status: "online",
                    latency: Math.floor(Math.random() * 10 + 5),
                    throughput: Math.floor(Math.random() * 50 + 100),
                    availability: (Math.random() * 2 + 98).toFixed(1),
                    activeNodes: Math.floor(Math.random() * 5 + 10)
                },
                {
                    name: "EU-West",
                    status: "online",
                    latency: Math.floor(Math.random() * 15 + 15),
                    throughput: Math.floor(Math.random() * 40 + 80),
                    availability: (Math.random() * 2 + 98).toFixed(1),
                    activeNodes: Math.floor(Math.random() * 4 + 8)
                },
                {
                    name: "Asia-Pacific",
                    status: "warning",
                    latency: Math.floor(Math.random() * 20 + 25),
                    throughput: Math.floor(Math.random() * 30 + 60),
                    availability: (Math.random() * 3 + 95).toFixed(1),
                    activeNodes: Math.floor(Math.random() * 3 + 6)
                }
            ],
            latencyMatrix: [
                ["", "US-East", "EU-West", "Asia-Pacific"],
                ["US-East", 0, 85, 180],
                ["EU-West", 85, 0, 165],
                ["Asia-Pacific", 180, 165, 0]
            ],
            trends: Array.from({length: 24}, (_, i) => ({
                timestamp: new Date(Date.now() - (23-i) * 3600000).toISOString(),
                "US-East": Math.floor(Math.random() * 20 + 80),
                "EU-West": Math.floor(Math.random() * 15 + 70),
                "Asia-Pacific": Math.floor(Math.random() * 10 + 60)
            }))
        };
    }

    generateMockComparativeData() {
        return {
            comparisons: [
                {
                    title: "Throughput Comparison",
                    mooseng: "145 MB/s",
                    competitor: "MooseFS 3.0",
                    competitorValue: "87 MB/s",
                    winner: "mooseng"
                },
                {
                    title: "Latency Comparison",
                    mooseng: "12 ms",
                    competitor: "Ceph",
                    competitorValue: "28 ms",
                    winner: "mooseng"
                },
                {
                    title: "CPU Efficiency",
                    mooseng: "23%",
                    competitor: "GlusterFS",
                    competitorValue: "41%",
                    winner: "mooseng"
                },
                {
                    title: "Memory Usage",
                    mooseng: "2.1 GB",
                    competitor: "Previous Version",
                    competitorValue: "3.8 GB",
                    winner: "mooseng"
                }
            ],
            benchmarkData: {
                labels: ["Throughput", "Latency", "IOPS", "CPU Usage", "Memory"],
                datasets: [
                    {
                        label: "MooseNG",
                        data: [145, 12, 8500, 23, 2100],
                        backgroundColor: "rgba(52, 152, 219, 0.6)"
                    },
                    {
                        label: "MooseFS 3.0",
                        data: [87, 18, 6200, 31, 2800],
                        backgroundColor: "rgba(231, 76, 60, 0.6)"
                    }
                ]
            }
        };
    }

    // Real data fetching methods
    async fetchFromApi(endpoint, params = {}) {
        try {
            const url = new URL(endpoint, this.baseUrl);
            Object.keys(params).forEach(key => {
                url.searchParams.append(key, params[key]);
            });

            const response = await fetch(url);
            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }
            
            return await response.json();
        } catch (error) {
            console.warn(`API fetch failed for ${endpoint}, using mock data:`, error);
            return null;
        }
    }

    async loadBenchmarkResults() {
        try {
            // Try to load the latest benchmark results
            const response = await fetch('../../mooseng/benchmark_results/latest/performance_results.json');
            if (response.ok) {
                return await response.json();
            }
        } catch (error) {
            console.warn('Could not load benchmark results:', error);
        }
        return null;
    }

    // Public API methods
    async getOverviewData() {
        const cacheKey = this.getCacheKey('overview');
        const cached = this.getCache(cacheKey);
        if (cached) return cached;

        let data;
        if (this.mockMode) {
            data = this.generateMockOverviewData();
        } else {
            data = await this.fetchFromApi('/overview') || this.generateMockOverviewData();
        }

        // Try to enhance with real benchmark data
        const benchmarkData = await this.loadBenchmarkResults();
        if (benchmarkData && benchmarkData.results) {
            data = {
                ...data,
                overall_grade: benchmarkData.results.grade || data.overall_grade,
                throughput_mbps: benchmarkData.results.throughput_mbps || data.throughput_mbps,
                ops_per_second: benchmarkData.results.ops_per_second || data.ops_per_second,
                total_time_ms: benchmarkData.results.total_time_ms || data.total_time_ms
            };
        }

        this.setCache(cacheKey, data);
        return data;
    }

    async getMasterServerData() {
        const cacheKey = this.getCacheKey('master-server');
        const cached = this.getCache(cacheKey);
        if (cached) return cached;

        let data;
        if (this.mockMode) {
            data = this.generateMockMasterServerData();
        } else {
            data = await this.fetchFromApi('/master-server') || this.generateMockMasterServerData();
        }

        this.setCache(cacheKey, data);
        return data;
    }

    async getChunkServerData() {
        const cacheKey = this.getCacheKey('chunk-server');
        const cached = this.getCache(cacheKey);
        if (cached) return cached;

        let data;
        if (this.mockMode) {
            data = this.generateMockChunkServerData();
        } else {
            data = await this.fetchFromApi('/chunk-server') || this.generateMockChunkServerData();
        }

        this.setCache(cacheKey, data);
        return data;
    }

    async getClientData() {
        const cacheKey = this.getCacheKey('client');
        const cached = this.getCache(cacheKey);
        if (cached) return cached;

        let data;
        if (this.mockMode) {
            data = this.generateMockClientData();
        } else {
            data = await this.fetchFromApi('/client') || this.generateMockClientData();
        }

        this.setCache(cacheKey, data);
        return data;
    }

    async getMultiRegionData() {
        const cacheKey = this.getCacheKey('multi-region');
        const cached = this.getCache(cacheKey);
        if (cached) return cached;

        let data;
        if (this.mockMode) {
            data = this.generateMockMultiRegionData();
        } else {
            data = await this.fetchFromApi('/multi-region') || this.generateMockMultiRegionData();
        }

        this.setCache(cacheKey, data);
        return data;
    }

    async getComparativeData() {
        const cacheKey = this.getCacheKey('comparative');
        const cached = this.getCache(cacheKey);
        if (cached) return cached;

        let data;
        if (this.mockMode) {
            data = this.generateMockComparativeData();
        } else {
            data = await this.fetchFromApi('/comparative') || this.generateMockComparativeData();
        }

        this.setCache(cacheKey, data);
        return data;
    }

    // Utility methods
    clearCache() {
        this.cache.clear();
    }

    setMockMode(enabled) {
        this.mockMode = enabled;
        this.clearCache(); // Clear cache when switching modes
    }

    // Historical data aggregation
    async getHistoricalData(metric, timeRange = '24h') {
        // This would typically fetch from a time-series database
        // For now, generate mock historical data
        const points = this.getTimeRangePoints(timeRange);
        return Array.from({length: points}, (_, i) => ({
            timestamp: new Date(Date.now() - (points-1-i) * this.getTimeRangeInterval(timeRange)).toISOString(),
            value: Math.floor(Math.random() * 100 + 50)
        }));
    }

    getTimeRangePoints(timeRange) {
        const ranges = {
            '1h': 60,
            '6h': 72,
            '24h': 96,
            '7d': 168,
            '30d': 180
        };
        return ranges[timeRange] || 96;
    }

    getTimeRangeInterval(timeRange) {
        const intervals = {
            '1h': 60000,        // 1 minute
            '6h': 300000,       // 5 minutes
            '24h': 900000,      // 15 minutes
            '7d': 3600000,      // 1 hour
            '30d': 14400000     // 4 hours
        };
        return intervals[timeRange] || 900000;
    }
}

// Initialize global data manager
window.DataManager = new DataManager();