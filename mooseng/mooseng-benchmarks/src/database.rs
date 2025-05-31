//! Database schema and operations for MooseNG benchmark results
//!
//! This module provides persistent storage for benchmark results, enabling
//! historical analysis, trend detection, and performance regression tracking.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{migrate::MigrateDatabase, Pool, Row, Sqlite, SqlitePool};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::{BenchmarkResult, Percentiles};

/// Database connection and operations manager
pub struct BenchmarkDatabase {
    pool: SqlitePool,
    db_path: PathBuf,
}

/// Represents a benchmark session in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbBenchmarkSession {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub status: SessionStatus,
    pub config: serde_json::Value,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Status of a benchmark session
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[repr(i32)]
pub enum SessionStatus {
    Created = 0,
    Running = 1,
    Completed = 2,
    Failed = 3,
    Cancelled = 4,
}

/// Individual benchmark result stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbBenchmarkResult {
    pub id: String,
    pub session_id: String,
    pub suite_name: String,
    pub benchmark_name: String,
    pub operation: String,
    pub mean_time_ns: i64,
    pub std_dev_ns: i64,
    pub min_time_ns: i64,
    pub max_time_ns: i64,
    pub throughput: Option<f64>,
    pub samples: i32,
    pub percentiles: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub timestamp: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Performance metrics aggregated by time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub operation: String,
    pub time_period: String,
    pub avg_duration_ms: f64,
    pub min_duration_ms: f64,
    pub max_duration_ms: f64,
    pub avg_throughput: Option<f64>,
    pub sample_count: i32,
    pub regression_score: Option<f64>,
}

/// Trend analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    pub operation: String,
    pub trend_direction: TrendDirection,
    pub change_percent: f64,
    pub confidence: f64,
    pub period_days: i32,
    pub data_points: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrendDirection {
    Improving,
    Degrading,
    Stable,
    Insufficient,
}

/// Database query filters
#[derive(Debug, Clone, Default)]
pub struct BenchmarkFilter {
    pub session_ids: Option<Vec<String>>,
    pub suite_names: Option<Vec<String>>,
    pub operations: Option<Vec<String>>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

impl BenchmarkDatabase {
    /// Create a new database connection
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let db_url = format!("sqlite://{}", db_path.display());
        
        // Create database if it doesn't exist
        if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
            info!("Creating new benchmark database at: {}", db_path.display());
            Sqlite::create_database(&db_url).await
                .context("Failed to create database")?;
        }
        
        let pool = SqlitePool::connect(&db_url).await
            .context("Failed to connect to database")?;
        
        let mut db = Self { pool, db_path };
        db.run_migrations().await?;
        
        Ok(db)
    }
    
    /// Run database migrations
    async fn run_migrations(&mut self) -> Result<()> {
        info!("Running database migrations");
        
        // Create sessions table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS benchmark_sessions (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                start_time TEXT NOT NULL,
                end_time TEXT,
                status INTEGER NOT NULL DEFAULT 0,
                config TEXT NOT NULL,
                metadata TEXT,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create sessions table")?;
        
        // Create results table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS benchmark_results (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                suite_name TEXT NOT NULL,
                benchmark_name TEXT NOT NULL,
                operation TEXT NOT NULL,
                mean_time_ns INTEGER NOT NULL,
                std_dev_ns INTEGER NOT NULL,
                min_time_ns INTEGER NOT NULL,
                max_time_ns INTEGER NOT NULL,
                throughput REAL,
                samples INTEGER NOT NULL,
                percentiles TEXT,
                metadata TEXT,
                timestamp TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (session_id) REFERENCES benchmark_sessions (id)
            )
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create results table")?;
        
        // Create indexes for better query performance
        let indexes = [
            "CREATE INDEX IF NOT EXISTS idx_results_session_id ON benchmark_results(session_id)",
            "CREATE INDEX IF NOT EXISTS idx_results_operation ON benchmark_results(operation)",
            "CREATE INDEX IF NOT EXISTS idx_results_timestamp ON benchmark_results(timestamp)",
            "CREATE INDEX IF NOT EXISTS idx_results_suite_name ON benchmark_results(suite_name)",
            "CREATE INDEX IF NOT EXISTS idx_sessions_start_time ON benchmark_sessions(start_time)",
            "CREATE INDEX IF NOT EXISTS idx_sessions_status ON benchmark_sessions(status)",
        ];
        
        for index_sql in &indexes {
            sqlx::query(index_sql)
                .execute(&self.pool)
                .await
                .context("Failed to create index")?;
        }
        
        // Create view for performance metrics
        sqlx::query(
            r#"
            CREATE VIEW IF NOT EXISTS performance_metrics AS
            SELECT 
                operation,
                DATE(timestamp) as date,
                AVG(CAST(mean_time_ns AS REAL) / 1000000.0) as avg_duration_ms,
                MIN(CAST(min_time_ns AS REAL) / 1000000.0) as min_duration_ms,
                MAX(CAST(max_time_ns AS REAL) / 1000000.0) as max_duration_ms,
                AVG(throughput) as avg_throughput,
                SUM(samples) as total_samples,
                COUNT(*) as measurement_count
            FROM benchmark_results
            GROUP BY operation, DATE(timestamp)
            ORDER BY date DESC
            "#,
        )
        .execute(&self.pool)
        .await
        .context("Failed to create performance metrics view")?;
        
        info!("Database migrations completed successfully");
        Ok(())
    }
    
    /// Create a new benchmark session
    pub async fn create_session(
        &self,
        name: String,
        description: Option<String>,
        config: serde_json::Value,
        metadata: Option<serde_json::Value>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        
        sqlx::query(
            r#"
            INSERT INTO benchmark_sessions 
            (id, name, description, start_time, status, config, metadata, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&name)
        .bind(&description)
        .bind(now.to_rfc3339())
        .bind(SessionStatus::Created as i32)
        .bind(config.to_string())
        .bind(metadata.map(|m| m.to_string()))
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await
        .context("Failed to create session")?;
        
        info!("Created benchmark session: {} ({})", name, id);
        Ok(id)
    }
    
    /// Update session status
    pub async fn update_session_status(
        &self,
        session_id: &str,
        status: SessionStatus,
        end_time: Option<DateTime<Utc>>,
    ) -> Result<()> {
        let now = Utc::now();
        
        sqlx::query(
            r#"
            UPDATE benchmark_sessions 
            SET status = ?, end_time = ?, updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(status as i32)
        .bind(end_time.map(|t| t.to_rfc3339()))
        .bind(now.to_rfc3339())
        .bind(session_id)
        .execute(&self.pool)
        .await
        .context("Failed to update session status")?;
        
        debug!("Updated session {} status to {:?}", session_id, status);
        Ok(())
    }
    
    /// Store benchmark results
    pub async fn store_results(
        &self,
        session_id: &str,
        suite_name: &str,
        results: &[BenchmarkResult],
    ) -> Result<()> {
        for result in results {
            let id = Uuid::new_v4().to_string();
            let percentiles_json = result.percentiles.as_ref()
                .map(|p| serde_json::to_string(p).unwrap_or_default());
            let metadata_json = result.metadata.as_ref()
                .map(|m| m.to_string());
            
            sqlx::query(
                r#"
                INSERT INTO benchmark_results 
                (id, session_id, suite_name, benchmark_name, operation, 
                 mean_time_ns, std_dev_ns, min_time_ns, max_time_ns, 
                 throughput, samples, percentiles, metadata, timestamp, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&id)
            .bind(session_id)
            .bind(suite_name)
            .bind(&result.operation)
            .bind(&result.operation)
            .bind(result.mean_time.as_nanos() as i64)
            .bind(result.std_dev.as_nanos() as i64)
            .bind(result.min_time.as_nanos() as i64)
            .bind(result.max_time.as_nanos() as i64)
            .bind(result.throughput)
            .bind(result.samples as i32)
            .bind(percentiles_json)
            .bind(metadata_json)
            .bind(result.timestamp.to_rfc3339())
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await
            .context("Failed to store benchmark result")?;
        }
        
        info!("Stored {} benchmark results for session {}", results.len(), session_id);
        Ok(())
    }
    
    /// Get session information
    pub async fn get_session(&self, session_id: &str) -> Result<Option<DbBenchmarkSession>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, description, start_time, end_time, status, 
                   config, metadata, created_at, updated_at
            FROM benchmark_sessions 
            WHERE id = ?
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to fetch session")?;
        
        if let Some(row) = row {
            let session = DbBenchmarkSession {
                id: row.get("id"),
                name: row.get("name"),
                description: row.get("description"),
                start_time: DateTime::parse_from_rfc3339(&row.get::<String, _>("start_time"))
                    .unwrap().with_timezone(&Utc),
                end_time: row.get::<Option<String>, _>("end_time")
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                status: match row.get::<i32, _>("status") {
                    0 => SessionStatus::Created,
                    1 => SessionStatus::Running,
                    2 => SessionStatus::Completed,
                    3 => SessionStatus::Failed,
                    4 => SessionStatus::Cancelled,
                    _ => SessionStatus::Created,
                },
                config: serde_json::from_str(&row.get::<String, _>("config")).unwrap_or_default(),
                metadata: row.get::<Option<String>, _>("metadata")
                    .map(|s| serde_json::from_str(&s).unwrap_or_default()),
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                    .unwrap().with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))
                    .unwrap().with_timezone(&Utc),
            };
            Ok(Some(session))
        } else {
            Ok(None)
        }
    }
    
    /// List benchmark sessions
    pub async fn list_sessions(
        &self,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> Result<Vec<DbBenchmarkSession>> {
        let limit = limit.unwrap_or(50);
        let offset = offset.unwrap_or(0);
        
        let rows = sqlx::query(
            r#"
            SELECT id, name, description, start_time, end_time, status, 
                   config, metadata, created_at, updated_at
            FROM benchmark_sessions 
            ORDER BY start_time DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list sessions")?;
        
        let mut sessions = Vec::new();
        for row in rows {
            let session = DbBenchmarkSession {
                id: row.get("id"),
                name: row.get("name"),
                description: row.get("description"),
                start_time: DateTime::parse_from_rfc3339(&row.get::<String, _>("start_time"))
                    .unwrap().with_timezone(&Utc),
                end_time: row.get::<Option<String>, _>("end_time")
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                status: match row.get::<i32, _>("status") {
                    0 => SessionStatus::Created,
                    1 => SessionStatus::Running,
                    2 => SessionStatus::Completed,
                    3 => SessionStatus::Failed,
                    4 => SessionStatus::Cancelled,
                    _ => SessionStatus::Created,
                },
                config: serde_json::from_str(&row.get::<String, _>("config")).unwrap_or_default(),
                metadata: row.get::<Option<String>, _>("metadata")
                    .map(|s| serde_json::from_str(&s).unwrap_or_default()),
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                    .unwrap().with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))
                    .unwrap().with_timezone(&Utc),
            };
            sessions.push(session);
        }
        
        Ok(sessions)
    }
    
    /// Query benchmark results with filters
    pub async fn query_results(&self, filter: &BenchmarkFilter) -> Result<Vec<DbBenchmarkResult>> {
        let mut query = String::from(
            r#"
            SELECT id, session_id, suite_name, benchmark_name, operation,
                   mean_time_ns, std_dev_ns, min_time_ns, max_time_ns,
                   throughput, samples, percentiles, metadata, timestamp, created_at
            FROM benchmark_results
            WHERE 1=1
            "#,
        );
        
        let mut bind_values: Vec<Box<dyn sqlx::Encode<'_, Sqlite> + Send>> = Vec::new();
        
        if let Some(ref session_ids) = filter.session_ids {
            let placeholders = session_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            query.push_str(&format!(" AND session_id IN ({})", placeholders));
            for id in session_ids {
                bind_values.push(Box::new(id.clone()));
            }
        }
        
        if let Some(ref operations) = filter.operations {
            let placeholders = operations.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            query.push_str(&format!(" AND operation IN ({})", placeholders));
            for op in operations {
                bind_values.push(Box::new(op.clone()));
            }
        }
        
        if let Some(ref start_date) = filter.start_date {
            query.push_str(" AND timestamp >= ?");
            bind_values.push(Box::new(start_date.to_rfc3339()));
        }
        
        if let Some(ref end_date) = filter.end_date {
            query.push_str(" AND timestamp <= ?");
            bind_values.push(Box::new(end_date.to_rfc3339()));
        }
        
        query.push_str(" ORDER BY timestamp DESC");
        
        if let Some(limit) = filter.limit {
            query.push_str(" LIMIT ?");
            bind_values.push(Box::new(limit));
        }
        
        if let Some(offset) = filter.offset {
            query.push_str(" OFFSET ?");
            bind_values.push(Box::new(offset));
        }
        
        // For now, use a simpler approach due to dynamic query complexity
        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .context("Failed to query results")?;
        
        let mut results = Vec::new();
        for row in rows {
            let result = DbBenchmarkResult {
                id: row.get("id"),
                session_id: row.get("session_id"),
                suite_name: row.get("suite_name"),
                benchmark_name: row.get("benchmark_name"),
                operation: row.get("operation"),
                mean_time_ns: row.get("mean_time_ns"),
                std_dev_ns: row.get("std_dev_ns"),
                min_time_ns: row.get("min_time_ns"),
                max_time_ns: row.get("max_time_ns"),
                throughput: row.get("throughput"),
                samples: row.get("samples"),
                percentiles: row.get::<Option<String>, _>("percentiles")
                    .map(|s| serde_json::from_str(&s).unwrap_or_default()),
                metadata: row.get::<Option<String>, _>("metadata")
                    .map(|s| serde_json::from_str(&s).unwrap_or_default()),
                timestamp: DateTime::parse_from_rfc3339(&row.get::<String, _>("timestamp"))
                    .unwrap().with_timezone(&Utc),
                created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                    .unwrap().with_timezone(&Utc),
            };
            results.push(result);
        }
        
        Ok(results)
    }
    
    /// Get performance metrics for operations
    pub async fn get_performance_metrics(
        &self,
        operations: Option<Vec<String>>,
        days: Option<i32>,
    ) -> Result<Vec<PerformanceMetrics>> {
        let days = days.unwrap_or(30);
        let cutoff_date = Utc::now() - chrono::Duration::days(days as i64);
        
        let mut query = String::from(
            r#"
            SELECT operation, date, avg_duration_ms, min_duration_ms, max_duration_ms,
                   avg_throughput, total_samples as sample_count
            FROM performance_metrics
            WHERE date >= ?
            "#,
        );
        
        if let Some(ref ops) = operations {
            let placeholders = ops.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            query.push_str(&format!(" AND operation IN ({})", placeholders));
        }
        
        query.push_str(" ORDER BY operation, date DESC");
        
        let rows = sqlx::query(&query)
            .bind(cutoff_date.format("%Y-%m-%d").to_string())
            .fetch_all(&self.pool)
            .await
            .context("Failed to get performance metrics")?;
        
        let mut metrics = Vec::new();
        for row in rows {
            let metric = PerformanceMetrics {
                operation: row.get("operation"),
                time_period: row.get("date"),
                avg_duration_ms: row.get("avg_duration_ms"),
                min_duration_ms: row.get("min_duration_ms"),
                max_duration_ms: row.get("max_duration_ms"),
                avg_throughput: row.get("avg_throughput"),
                sample_count: row.get("sample_count"),
                regression_score: None, // Calculate separately if needed
            };
            metrics.push(metric);
        }
        
        Ok(metrics)
    }
    
    /// Analyze performance trends
    pub async fn analyze_trends(
        &self,
        operations: Option<Vec<String>>,
        period_days: i32,
    ) -> Result<Vec<TrendAnalysis>> {
        let cutoff_date = Utc::now() - chrono::Duration::days(period_days as i64);
        
        let mut query = String::from(
            r#"
            SELECT operation,
                   COUNT(*) as data_points,
                   AVG(avg_duration_ms) as avg_duration,
                   MIN(avg_duration_ms) as min_duration,
                   MAX(avg_duration_ms) as max_duration,
                   STDDEV(avg_duration_ms) as std_dev
            FROM performance_metrics
            WHERE date >= ?
            "#,
        );
        
        if let Some(ref ops) = operations {
            let placeholders = ops.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            query.push_str(&format!(" AND operation IN ({})", placeholders));
        }
        
        query.push_str(" GROUP BY operation HAVING COUNT(*) >= 3");
        
        let rows = sqlx::query(&query)
            .bind(cutoff_date.format("%Y-%m-%d").to_string())
            .fetch_all(&self.pool)
            .await
            .context("Failed to analyze trends")?;
        
        let mut trends = Vec::new();
        for row in rows {
            let operation: String = row.get("operation");
            let data_points: i32 = row.get("data_points");
            let avg_duration: f64 = row.get("avg_duration");
            let min_duration: f64 = row.get("min_duration");
            let max_duration: f64 = row.get("max_duration");
            let std_dev: Option<f64> = row.get("std_dev");
            
            // Simple trend analysis based on variation
            let variation = if avg_duration > 0.0 {
                ((max_duration - min_duration) / avg_duration) * 100.0
            } else {
                0.0
            };
            
            let (trend_direction, change_percent, confidence) = if data_points < 5 {
                (TrendDirection::Insufficient, 0.0, 0.0)
            } else if variation < 5.0 {
                (TrendDirection::Stable, variation, 0.8)
            } else if max_duration > avg_duration * 1.1 {
                (TrendDirection::Degrading, variation, 0.7)
            } else if min_duration < avg_duration * 0.9 {
                (TrendDirection::Improving, -variation, 0.7)
            } else {
                (TrendDirection::Stable, variation, 0.6)
            };
            
            trends.push(TrendAnalysis {
                operation,
                trend_direction,
                change_percent,
                confidence,
                period_days,
                data_points,
            });
        }
        
        Ok(trends)
    }
    
    /// Get database statistics
    pub async fn get_statistics(&self) -> Result<HashMap<String, serde_json::Value>> {
        let mut stats = HashMap::new();
        
        // Total sessions
        let total_sessions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM benchmark_sessions")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);
        stats.insert("total_sessions".to_string(), serde_json::json!(total_sessions));
        
        // Total results
        let total_results: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM benchmark_results")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);
        stats.insert("total_results".to_string(), serde_json::json!(total_results));
        
        // Unique operations
        let unique_operations: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT operation) FROM benchmark_results")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);
        stats.insert("unique_operations".to_string(), serde_json::json!(unique_operations));
        
        // Date range
        if let Ok(Some(earliest)) = sqlx::query_scalar::<_, Option<String>>(
            "SELECT MIN(timestamp) FROM benchmark_results"
        ).fetch_one(&self.pool).await {
            stats.insert("earliest_result".to_string(), serde_json::json!(earliest));
        }
        
        if let Ok(Some(latest)) = sqlx::query_scalar::<_, Option<String>>(
            "SELECT MAX(timestamp) FROM benchmark_results"
        ).fetch_one(&self.pool).await {
            stats.insert("latest_result".to_string(), serde_json::json!(latest));
        }
        
        // Database file size
        if let Ok(metadata) = tokio::fs::metadata(&self.db_path).await {
            stats.insert("database_size_bytes".to_string(), serde_json::json!(metadata.len()));
        }
        
        Ok(stats)
    }
    
    /// Clean up old data
    pub async fn cleanup_old_data(&self, retention_days: i32) -> Result<u64> {
        let cutoff_date = Utc::now() - chrono::Duration::days(retention_days as i64);
        
        // Delete old results first (due to foreign key constraints)
        let deleted_results = sqlx::query(
            "DELETE FROM benchmark_results WHERE timestamp < ?"
        )
        .bind(cutoff_date.to_rfc3339())
        .execute(&self.pool)
        .await
        .context("Failed to delete old results")?
        .rows_affected();
        
        // Delete old sessions
        let deleted_sessions = sqlx::query(
            "DELETE FROM benchmark_sessions WHERE start_time < ? AND status IN (2, 3, 4)"
        )
        .bind(cutoff_date.to_rfc3339())
        .execute(&self.pool)
        .await
        .context("Failed to delete old sessions")?
        .rows_affected();
        
        info!("Cleanup completed: {} results, {} sessions deleted", 
              deleted_results, deleted_sessions);
        
        Ok(deleted_results + deleted_sessions)
    }
    
    /// Vacuum database to reclaim space
    pub async fn vacuum(&self) -> Result<()> {
        sqlx::query("VACUUM")
            .execute(&self.pool)
            .await
            .context("Failed to vacuum database")?;
        
        info!("Database vacuum completed");
        Ok(())
    }
}

impl Drop for BenchmarkDatabase {
    fn drop(&mut self) {
        // Close the connection pool
        self.pool.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio;
    
    #[tokio::test]
    async fn test_database_creation() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        let db = BenchmarkDatabase::new(&db_path).await.unwrap();
        assert!(db_path.exists());
    }
    
    #[tokio::test]
    async fn test_session_lifecycle() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = BenchmarkDatabase::new(&db_path).await.unwrap();
        
        // Create session
        let session_id = db.create_session(
            "Test Session".to_string(),
            Some("Test description".to_string()),
            serde_json::json!({"test": true}),
            None,
        ).await.unwrap();
        
        // Get session
        let session = db.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(session.name, "Test Session");
        assert_eq!(session.status as i32, SessionStatus::Created as i32);
        
        // Update status
        db.update_session_status(&session_id, SessionStatus::Completed, Some(Utc::now()))
            .await.unwrap();
        
        let updated_session = db.get_session(&session_id).await.unwrap().unwrap();
        assert_eq!(updated_session.status as i32, SessionStatus::Completed as i32);
        assert!(updated_session.end_time.is_some());
    }
    
    #[tokio::test]
    async fn test_result_storage() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db = BenchmarkDatabase::new(&db_path).await.unwrap();
        
        let session_id = db.create_session(
            "Test Session".to_string(),
            None,
            serde_json::json!({}),
            None,
        ).await.unwrap();
        
        let results = vec![
            BenchmarkResult {
                operation: "test_op".to_string(),
                mean_time: std::time::Duration::from_millis(100),
                std_dev: std::time::Duration::from_millis(10),
                min_time: std::time::Duration::from_millis(90),
                max_time: std::time::Duration::from_millis(110),
                throughput: Some(1000.0),
                samples: 50,
                timestamp: Utc::now(),
                metadata: None,
                percentiles: None,
            }
        ];
        
        db.store_results(&session_id, "test_suite", &results).await.unwrap();
        
        let filter = BenchmarkFilter {
            session_ids: Some(vec![session_id]),
            ..Default::default()
        };
        
        let stored_results = db.query_results(&filter).await.unwrap();
        assert_eq!(stored_results.len(), 1);
        assert_eq!(stored_results[0].operation, "test_op");
    }
}