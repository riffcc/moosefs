/**
 * Benchmark Integration for MooseNG Dashboard
 * Connects the dashboard to existing benchmark data and Rust backend
 */

class BenchmarkIntegration {
    constructor(apiClient) {
        this.apiClient = apiClient;
        this.benchmarkDataPath = '../benchmark_results';
        this.lastUpdateTime = null;
        this.fileWatcher = null;
        this.dataCache = new Map();
        
        this.init();
    }

    async init() {
        // Try to connect to Rust backend first
        const backendAvailable = await this.checkBackendConnection();
        
        if (!backendAvailable) {
            console.log('Backend not available, using file-based integration');
            this.enableFileWatcher();
        } else {
            console.log('Connected to Rust backend');
        }
    }

    /**
     * Check if Rust backend is available
     */
    async checkBackendConnection() {
        try {
            const response = await fetch('/api/health');
            return response.ok;
        } catch (error) {
            return false;
        }
    }

    /**
     * Enable file watcher for benchmark results
     */
    enableFileWatcher() {
        // Since we can't use Node.js file watching in browser,
        // we'll poll for changes periodically
        this.startPolling();
    }

    /**
     * Start polling for benchmark result changes
     */
    startPolling() {
        setInterval(async () => {
            await this.checkForUpdates();
        }, 10000); // Check every 10 seconds
    }

    /**
     * Check for new benchmark results
     */
    async checkForUpdates() {
        try {
            // Get list of benchmark result directories
            const directories = await this.getBenchmarkDirectories();
            
            for (const dir of directories) {
                const resultFile = `${this.benchmarkDataPath}/${dir}/performance_results.json`;
                await this.loadBenchmarkResult(resultFile, dir);
            }
        } catch (error) {
            console.warn('Error checking for updates:', error);
        }
    }

    /**
     * Get list of benchmark result directories
     */
    async getBenchmarkDirectories() {
        // In a real implementation, this would query the file system
        // For now, we'll return known directories
        return [
            '20250531_155049',
            '20250531_155246', 
            '20250531_155418'
        ];
    }

    /**
     * Load benchmark result from file
     */
    async loadBenchmarkResult(filePath, timestamp) {
        try {
            const response = await fetch(filePath);
            if (response.ok) {
                const data = await response.json();
                this.processBenchmarkResult(data, timestamp);
            }
        } catch (error) {
            console.warn(`Failed to load ${filePath}:`, error);
        }
    }

    /**
     * Process benchmark result data
     */
    processBenchmarkResult(data, timestamp) {
        const processedData = {
            timestamp: timestamp,
            source: 'file',
            metrics: {
                totalOpsPerSecond: this.parseOpsPerSecond(data.results.ops_per_second),
                totalThroughputMBps: parseFloat(data.results.throughput_mbps),
                avgLatencyMs: this.calculateAvgLatency(data.results),
                systemGrade: data.results.grade,
                environment: data.environment,
                individual_results: {
                    file_system_ms: data.results.file_system_ms,
                    network_ms: data.results.network_ms,
                    cpu_memory_ms: data.results.cpu_memory_ms,
                    concurrency_ms: data.results.concurrency_ms,
                    throughput_ms: data.results.throughput_ms
                }
            }
        };

        // Cache the processed data
        this.dataCache.set(timestamp, processedData);
        
        // Emit event for dashboard to update
        this.emitDataUpdate(processedData);
    }

    /**
     * Parse operations per second from string
     */
    parseOpsPerSecond(opsString) {
        if (typeof opsString === 'string') {
            return parseFloat(opsString.replace(/,/g, ''));
        }
        return opsString;
    }

    /**
     * Calculate average latency from results
     */
    calculateAvgLatency(results) {
        const totalTime = results.file_system_ms + results.network_ms + 
                         results.cpu_memory_ms + results.concurrency_ms + 
                         results.throughput_ms;
        return totalTime / 5; // Average of 5 components
    }

    /**
     * Emit data update event
     */
    emitDataUpdate(data) {
        const event = new CustomEvent('benchmarkDataUpdate', { 
            detail: data 
        });
        document.dispatchEvent(event);
    }

    /**
     * Get overview metrics combining recent results
     */
    getOverviewMetrics() {
        const recentResults = Array.from(this.dataCache.values())
            .sort((a, b) => new Date(b.timestamp) - new Date(a.timestamp))
            .slice(0, 10);

        if (recentResults.length === 0) {
            return this.getMockOverviewMetrics();
        }

        const latest = recentResults[0];
        const previous = recentResults[1];

        // Calculate changes
        const opsChange = previous ? 
            ((latest.metrics.totalOpsPerSecond - previous.metrics.totalOpsPerSecond) / previous.metrics.totalOpsPerSecond) * 100 : 0;
        
        const throughputChange = previous ?
            ((latest.metrics.totalThroughputMBps - previous.metrics.totalThroughputMBps) / previous.metrics.totalThroughputMBps) * 100 : 0;

        const latencyChange = previous ?
            ((latest.metrics.avgLatencyMs - previous.metrics.avgLatencyMs) / previous.metrics.avgLatencyMs) * 100 : 0;

        return {
            totalOpsPerSecond: latest.metrics.totalOpsPerSecond,
            totalOpsPerSecondChange: opsChange,
            totalThroughputMBps: latest.metrics.totalThroughputMBps,
            totalThroughputChange: throughputChange,
            avgLatencyMs: latest.metrics.avgLatencyMs,
            avgLatencyChange: Math.abs(latencyChange),
            systemGrade: latest.metrics.systemGrade,
            lastUpdate: latest.timestamp,
            performanceTrend: this.generatePerformanceTrend(recentResults),
            systemHealth: this.generateSystemHealth(),
            components: this.generateComponentStatus(),
            recentResults: this.formatRecentResults(recentResults.slice(0, 5))
        };
    }

    /**
     * Generate performance trend data
     */
    generatePerformanceTrend(results) {
        const timestamps = results.map(r => 
            new Date(r.timestamp).toLocaleTimeString('en-US', { 
                hour: '2-digit', 
                minute: '2-digit' 
            })
        ).reverse();

        return {
            timestamps,
            throughput: results.map(r => r.metrics.totalThroughputMBps).reverse(),
            opsPerSecond: results.map(r => r.metrics.totalOpsPerSecond).reverse(),
            latency: results.map(r => r.metrics.avgLatencyMs).reverse()
        };
    }

    /**
     * Generate system health data
     */
    generateSystemHealth() {
        // Based on recent results, calculate component health
        const recentResults = Array.from(this.dataCache.values()).slice(-5);
        const avgGrade = this.calculateAverageGrade(recentResults);

        let healthy = 85, warning = 10, error = 3, offline = 2;

        if (avgGrade < 70) {
            healthy = 60;
            warning = 25;
            error = 12;
            offline = 3;
        } else if (avgGrade < 85) {
            healthy = 75;
            warning = 15;
            error = 8;
            offline = 2;
        }

        return {
            labels: ['Healthy', 'Warning', 'Error', 'Offline'],
            values: [healthy, warning, error, offline]
        };
    }

    /**
     * Calculate average grade from results
     */
    calculateAverageGrade(results) {
        if (results.length === 0) return 85;

        const gradeValues = {
            'A': 95,
            'A-': 90,
            'B+': 85,
            'B': 80,
            'B-': 75,
            'C+': 70,
            'C': 65,
            'C-': 60,
            'D': 50,
            'F': 30
        };

        const totalGrade = results.reduce((sum, result) => {
            const grade = result.metrics.systemGrade;
            return sum + (gradeValues[grade] || 75);
        }, 0);

        return totalGrade / results.length;
    }

    /**
     * Generate component status
     */
    generateComponentStatus() {
        const avgGrade = this.calculateAverageGrade(Array.from(this.dataCache.values()).slice(-5));
        
        const status = avgGrade >= 85 ? 'healthy' :
                      avgGrade >= 70 ? 'warning' :
                      avgGrade >= 50 ? 'error' : 'offline';

        return {
            master: status,
            chunkserver: status,
            client: status,
            metalogger: status
        };
    }

    /**
     * Format recent results for display
     */
    formatRecentResults(results) {
        return results.map(result => ({
            timestamp: result.timestamp,
            testType: 'Comprehensive Test',
            duration: Object.values(result.metrics.individual_results || {}).reduce((a, b) => a + b, 0),
            throughput: result.metrics.totalThroughputMBps.toFixed(2),
            opsPerSec: Math.round(result.metrics.totalOpsPerSecond),
            grade: result.metrics.systemGrade,
            status: this.getTestStatus(result.metrics.systemGrade)
        }));
    }

    /**
     * Get test status from grade
     */
    getTestStatus(grade) {
        if (['A', 'A-', 'B+'].includes(grade)) return 'passed';
        if (['B', 'B-', 'C+'].includes(grade)) return 'warning';
        return 'failed';
    }

    /**
     * Get mock overview metrics for development
     */
    getMockOverviewMetrics() {
        return {
            totalOpsPerSecond: 1506,
            totalOpsPerSecondChange: 12.5,
            totalThroughputMBps: 135.13,
            totalThroughputChange: 8.3,
            avgLatencyMs: 45.2,
            avgLatencyChange: 15.1,
            systemGrade: 'A',
            performanceTrend: {
                timestamps: this.generateTimeLabels(24),
                throughput: this.generateMockData(24, 100, 150),
                opsPerSecond: this.generateMockData(24, 1200, 1800),
                latency: this.generateMockData(24, 30, 60)
            },
            systemHealth: {
                labels: ['Healthy', 'Warning', 'Error', 'Offline'],
                values: [85, 10, 3, 2]
            },
            components: {
                master: 'healthy',
                chunkserver: 'healthy',
                client: 'healthy',
                metalogger: 'healthy'
            },
            recentResults: [
                {
                    timestamp: new Date().toISOString(),
                    testType: 'Comprehensive Test',
                    duration: 3323,
                    throughput: '135.13',
                    opsPerSec: 1506,
                    grade: 'A',
                    status: 'passed'
                }
            ]
        };
    }

    /**
     * Generate time labels
     */
    generateTimeLabels(hours = 24) {
        const labels = [];
        const now = new Date();
        
        for (let i = hours; i >= 0; i--) {
            const time = new Date(now - i * 60 * 60 * 1000);
            labels.push(time.toLocaleTimeString('en-US', { 
                hour: '2-digit', 
                minute: '2-digit' 
            }));
        }
        
        return labels;
    }

    /**
     * Generate mock data
     */
    generateMockData(points, min, max) {
        const data = [];
        for (let i = 0; i < points; i++) {
            data.push(min + Math.random() * (max - min));
        }
        return data;
    }

    /**
     * Get cached data
     */
    getCachedData() {
        return Array.from(this.dataCache.values());
    }

    /**
     * Clear cache
     */
    clearCache() {
        this.dataCache.clear();
    }

    /**
     * Export data for debugging
     */
    exportData() {
        return {
            cache: Array.from(this.dataCache.entries()),
            lastUpdate: this.lastUpdateTime,
            overview: this.getOverviewMetrics()
        };
    }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
    module.exports = BenchmarkIntegration;
}