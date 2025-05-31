# MooseNG Metadata Cache Prometheus Metrics

This document describes the Prometheus metrics exposed by the MooseNG metadata cache system for monitoring and alerting purposes.

## Overview

The metadata cache system in MooseNG exposes comprehensive metrics that allow monitoring of cache performance, health, and operational characteristics. These metrics are essential for:

- Performance monitoring and optimization
- Capacity planning and resource allocation
- Troubleshooting cache-related issues
- Setting up alerting for cache health problems

## Metrics Categories

### Cache Performance Metrics

#### `master_metadata_cache_hits_total`
- **Type**: Counter
- **Description**: Total number of cache hits for metadata operations
- **Labels**: None
- **Usage**: Monitor cache effectiveness and hit rate trends

#### `master_metadata_cache_misses_total`
- **Type**: Counter
- **Description**: Total number of cache misses for metadata operations
- **Labels**: None
- **Usage**: Monitor cache effectiveness and miss rate trends

#### `master_metadata_cache_size_bytes`
- **Type**: Gauge
- **Description**: Current estimated size of the metadata cache in bytes
- **Labels**: None
- **Usage**: Monitor memory usage and cache capacity utilization

### Operation Metrics

#### `master_metadata_operations_total`
- **Type**: Counter
- **Description**: Total number of metadata operations performed
- **Labels**:
  - `operation`: Type of operation (`get_inode`, `save_inode`, `delete_inode`)
  - `status`: Result status (`success`, `error`, `cache_hit`, `not_found`)
- **Usage**: Monitor operation patterns and success rates

#### `master_metadata_operation_duration_seconds`
- **Type**: Histogram
- **Description**: Duration of metadata operations in seconds
- **Labels**:
  - `operation`: Type of operation (`get_inode`, `save_inode`, `delete_inode`)
- **Buckets**: Standard latency buckets (0.0001s to 60s)
- **Usage**: Monitor operation performance and latency distribution

### System Health Metrics

#### `health_check_status`
- **Type**: Gauge
- **Description**: Health check status (1=healthy, 0=unhealthy)
- **Labels**:
  - `check_name`: Name of the health check (`cache_health`, `component_alive`)
- **Usage**: Monitor overall system health and trigger alerts

#### `last_health_check_timestamp_seconds`
- **Type**: Gauge
- **Description**: Unix timestamp of the last health check
- **Labels**: None
- **Usage**: Ensure health checks are running regularly

### Error and Event Metrics

#### `errors_total`
- **Type**: Counter
- **Description**: Total number of errors encountered
- **Labels**:
  - `type`: Error type (`cache_health`, `operation_error`)
  - `severity`: Error severity (`warning`, `error`, `critical`)
- **Usage**: Monitor error rates and types for troubleshooting

## Metric Collection

### Automatic Collection

The `MetadataCacheMetricsCollector` automatically collects and updates metrics during normal cache operations:

```rust
use mooseng_master::metadata_cache_manager::MetadataCacheMetricsCollector;
use mooseng_common::metrics::MetricsRegistry;

// Create metrics registry
let registry = MetricsRegistry::new("master").unwrap();
let metrics = Arc::new(MasterMetrics::new(&registry).unwrap());

// Create cache manager with metrics
let manager = Arc::new(MetadataCacheManager::new_with_metrics(
    store, 
    config, 
    Some(metrics.clone())
).await.unwrap());

// Create and register metrics collector
let collector = MetadataCacheMetricsCollector::new(manager.clone(), metrics.clone());
registry.register_collector("metadata_cache".to_string(), collector).await.unwrap();
```

### Manual Collection

For custom monitoring, metrics can be collected manually:

```rust
// Collect metrics on demand
collector.collect().await.unwrap();

// Export metrics in Prometheus format
let metrics_text = registry.export_metrics().await.unwrap();
```

## Monitoring Dashboards

### Key Performance Indicators (KPIs)

1. **Cache Hit Rate**
   ```promql
   rate(master_metadata_cache_hits_total[5m]) / 
   (rate(master_metadata_cache_hits_total[5m]) + rate(master_metadata_cache_misses_total[5m]))
   ```

2. **Average Operation Latency**
   ```promql
   rate(master_metadata_operation_duration_seconds_sum[5m]) / 
   rate(master_metadata_operation_duration_seconds_count[5m])
   ```

3. **Cache Memory Usage**
   ```promql
   master_metadata_cache_size_bytes
   ```

4. **Operation Rate**
   ```promql
   rate(master_metadata_operations_total[5m])
   ```

### Example Grafana Dashboard Queries

#### Cache Performance Panel
```promql
# Cache hit rate over time
rate(master_metadata_cache_hits_total[5m]) / 
(rate(master_metadata_cache_hits_total[5m]) + rate(master_metadata_cache_misses_total[5m])) * 100

# Cache miss rate over time
rate(master_metadata_cache_misses_total[5m]) / 
(rate(master_metadata_cache_hits_total[5m]) + rate(master_metadata_cache_misses_total[5m])) * 100
```

#### Operation Latency Panel
```promql
# 95th percentile latency by operation
histogram_quantile(0.95, rate(master_metadata_operation_duration_seconds_bucket[5m]))

# 50th percentile latency by operation
histogram_quantile(0.50, rate(master_metadata_operation_duration_seconds_bucket[5m]))
```

#### Memory Usage Panel
```promql
# Cache size in MB
master_metadata_cache_size_bytes / 1024 / 1024
```

#### Error Rate Panel
```promql
# Error rate by type
rate(errors_total[5m])
```

## Alerting Rules

### Critical Alerts

#### Low Cache Hit Rate
```yaml
- alert: MetadataCacheHitRateLow
  expr: |
    rate(master_metadata_cache_hits_total[5m]) / 
    (rate(master_metadata_cache_hits_total[5m]) + rate(master_metadata_cache_misses_total[5m])) < 0.5
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "Metadata cache hit rate is below 50%"
    description: "Cache hit rate is {{ $value | humanizePercentage }}, indicating potential cache sizing or TTL issues"
```

#### High Operation Latency
```yaml
- alert: MetadataOperationLatencyHigh
  expr: |
    histogram_quantile(0.95, rate(master_metadata_operation_duration_seconds_bucket[5m])) > 0.1
  for: 2m
  labels:
    severity: warning
  annotations:
    summary: "Metadata operation latency is high"
    description: "95th percentile latency is {{ $value }}s, which may impact performance"
```

#### Cache Health Issues
```yaml
- alert: MetadataCacheUnhealthy
  expr: health_check_status{check_name="cache_health"} == 0
  for: 1m
  labels:
    severity: critical
  annotations:
    summary: "Metadata cache health check failed"
    description: "Cache health check indicates unhealthy state - check cache performance metrics"
```

### Warning Alerts

#### High Error Rate
```yaml
- alert: MetadataErrorRateHigh
  expr: rate(errors_total{type="cache_health"}[5m]) > 0.1
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "High error rate in metadata cache operations"
    description: "Error rate is {{ $value }} errors/second"
```

#### Cache Memory Usage High
```yaml
- alert: MetadataCacheMemoryHigh
  expr: master_metadata_cache_size_bytes > 1073741824  # 1GB
  for: 10m
  labels:
    severity: warning
  annotations:
    summary: "Metadata cache memory usage is high"
    description: "Cache is using {{ $value | humanizeBytes }} of memory"
```

## Performance Tuning

### Cache Hit Rate Optimization

Monitor these metrics to optimize cache performance:

1. **Hit Rate Trends**: Track hit rate over different time periods to identify patterns
2. **Operation Patterns**: Analyze which operations benefit most from caching
3. **TTL Effectiveness**: Monitor how TTL settings affect hit rates

### Capacity Planning

Use these metrics for capacity planning:

1. **Memory Growth**: Track cache size growth over time
2. **Operation Volume**: Monitor operation rates during peak usage
3. **Latency Trends**: Identify when additional resources are needed

### Configuration Recommendations

Based on metrics, consider these configuration adjustments:

1. **Low Hit Rate (< 70%)**:
   - Increase cache size (`max_size`)
   - Increase TTL (`ttl_secs`)
   - Enable prefetching (`enable_prefetch`)

2. **High Memory Usage**:
   - Decrease cache size
   - Decrease TTL
   - Increase eviction percentage

3. **High Latency**:
   - Increase cache size
   - Enable LRU eviction
   - Optimize hot threshold

## Troubleshooting

### Common Issues and Metrics to Check

1. **Poor Performance**:
   - Check `master_metadata_operation_duration_seconds` for latency spikes
   - Monitor cache hit rate trends
   - Verify health check status

2. **Memory Issues**:
   - Monitor `master_metadata_cache_size_bytes`
   - Check eviction rates in cache statistics
   - Review cache configuration parameters

3. **High Error Rates**:
   - Check `errors_total` by type and severity
   - Monitor operation success rates
   - Review health check failures

### Debugging Cache Behavior

Use these queries to debug cache issues:

```promql
# Cache operations by status
rate(master_metadata_operations_total[5m])

# Error breakdown by type
rate(errors_total[5m])

# Operation latency percentiles
histogram_quantile(0.99, rate(master_metadata_operation_duration_seconds_bucket[5m]))
```

## Integration Examples

### Prometheus Configuration

```yaml
scrape_configs:
  - job_name: 'mooseng-master'
    static_configs:
      - targets: ['master-server:8080']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

### Grafana Dashboard JSON

Example dashboard configuration for importing:

```json
{
  "dashboard": {
    "title": "MooseNG Metadata Cache",
    "panels": [
      {
        "title": "Cache Hit Rate",
        "type": "stat",
        "targets": [
          {
            "expr": "rate(master_metadata_cache_hits_total[5m]) / (rate(master_metadata_cache_hits_total[5m]) + rate(master_metadata_cache_misses_total[5m])) * 100"
          }
        ]
      },
      {
        "title": "Operation Latency",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(master_metadata_operation_duration_seconds_bucket[5m]))",
            "legendFormat": "95th percentile"
          },
          {
            "expr": "histogram_quantile(0.50, rate(master_metadata_operation_duration_seconds_bucket[5m]))",
            "legendFormat": "50th percentile"
          }
        ]
      }
    ]
  }
}
```

This comprehensive metrics system provides complete observability into the metadata cache performance and health, enabling proactive monitoring and optimization of the MooseNG file system.