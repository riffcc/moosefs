//! MooseNG Benchmark Dashboard Server
//!
//! This server provides a web interface for viewing benchmark results,
//! historical trends, and real-time monitoring of benchmark sessions.

use anyhow::{Context, Result};
use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Router,
};
use clap::{Parser, Subcommand};
use mooseng_benchmarks::database::{
    BenchmarkDatabase, BenchmarkFilter, DbBenchmarkResult, DbBenchmarkSession, SessionStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "mooseng-dashboard")]
#[command(about = "MooseNG Benchmark Dashboard Server")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// Server bind address
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    bind: SocketAddr,
    
    /// Database path
    #[arg(short, long, default_value = "./benchmark.db")]
    database: PathBuf,
    
    /// Static files directory
    #[arg(long, default_value = "./static")]
    static_dir: PathBuf,
    
    /// Enable development mode (more verbose logging)
    #[arg(long)]
    dev: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the dashboard server
    Serve,
    /// Initialize the database
    Init,
    /// Show database statistics
    Stats,
    /// Clean up old data
    Cleanup {
        /// Number of days to retain
        #[arg(long, default_value = "90")]
        retention_days: i32,
    },
}

/// Application state shared across handlers
#[derive(Clone)]
struct AppState {
    db: Arc<BenchmarkDatabase>,
}

/// API response wrapper
#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now(),
        }
    }
    
    fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: chrono::Utc::now(),
        }
    }
}

/// Query parameters for session listing
#[derive(Deserialize)]
struct SessionQuery {
    limit: Option<i32>,
    offset: Option<i32>,
}

/// Query parameters for result filtering
#[derive(Deserialize)]
struct ResultQuery {
    session_id: Option<String>,
    operation: Option<String>,
    suite: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    limit: Option<i32>,
    offset: Option<i32>,
}

/// Dashboard summary data
#[derive(Serialize)]
struct DashboardSummary {
    total_sessions: i64,
    total_results: i64,
    active_sessions: i64,
    recent_results: Vec<DbBenchmarkResult>,
    performance_trends: Vec<TrendData>,
}

#[derive(Serialize)]
struct TrendData {
    operation: String,
    values: Vec<TrendPoint>,
}

#[derive(Serialize)]
struct TrendPoint {
    timestamp: String,
    value: f64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    let log_level = if cli.dev {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    
    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .init();
    
    // Initialize database
    let db = Arc::new(BenchmarkDatabase::new(&cli.database).await
        .context("Failed to initialize database")?);
    
    match cli.command {
        Some(Commands::Serve) | None => {
            serve_dashboard(cli.bind, db, cli.static_dir).await
        }
        Some(Commands::Init) => {
            info!("Database initialized at: {}", cli.database.display());
            Ok(())
        }
        Some(Commands::Stats) => {
            show_stats(db).await
        }
        Some(Commands::Cleanup { retention_days }) => {
            cleanup_data(db, retention_days).await
        }
    }
}

async fn serve_dashboard(
    bind_addr: SocketAddr,
    db: Arc<BenchmarkDatabase>,
    static_dir: PathBuf,
) -> Result<()> {
    let state = AppState { db };
    
    // Build the router
    let app = Router::new()
        // API routes
        .route("/api/health", get(health_check))
        .route("/api/sessions", get(list_sessions))
        .route("/api/sessions/:id", get(get_session))
        .route("/api/results", get(list_results))
        .route("/api/summary", get(get_dashboard_summary))
        .route("/api/trends/:operation", get(get_operation_trends))
        .route("/api/compare", post(compare_sessions))
        .route("/api/stats", get(get_database_stats))
        // WebSocket for real-time updates
        .route("/ws", get(websocket_handler))
        // Static files and SPA fallback
        .nest_service("/static", ServeDir::new(&static_dir))
        .fallback_service(ServeFile::new(static_dir.join("index.html")))
        // Add middleware
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
        )
        .with_state(state);
    
    info!("Starting benchmark dashboard server on {}", bind_addr);
    info!("Static files served from: {}", static_dir.display());
    
    let listener = TcpListener::bind(&bind_addr).await
        .context("Failed to bind server")?;
    
    axum::serve(listener, app).await
        .context("Server error")?;
    
    Ok(())
}

// API Handlers

async fn health_check() -> Json<ApiResponse<HashMap<String, String>>> {
    let mut status = HashMap::new();
    status.insert("status".to_string(), "healthy".to_string());
    status.insert("service".to_string(), "mooseng-benchmark-dashboard".to_string());
    status.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
    
    Json(ApiResponse::success(status))
}

async fn list_sessions(
    State(state): State<AppState>,
    Query(params): Query<SessionQuery>,
) -> Result<Json<ApiResponse<Vec<DbBenchmarkSession>>>, StatusCode> {
    match state.db.list_sessions(params.limit, params.offset).await {
        Ok(sessions) => Ok(Json(ApiResponse::success(sessions))),
        Err(e) => {
            warn!("Failed to list sessions: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<ApiResponse<Option<DbBenchmarkSession>>>, StatusCode> {
    match state.db.get_session(&session_id).await {
        Ok(session) => Ok(Json(ApiResponse::success(session))),
        Err(e) => {
            warn!("Failed to get session {}: {}", session_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn list_results(
    State(state): State<AppState>,
    Query(params): Query<ResultQuery>,
) -> Result<Json<ApiResponse<Vec<DbBenchmarkResult>>>, StatusCode> {
    let filter = BenchmarkFilter {
        session_ids: params.session_id.map(|id| vec![id]),
        operations: params.operation.map(|op| vec![op]),
        suite_names: params.suite.map(|suite| vec![suite]),
        start_date: params.start_date.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        end_date: params.end_date.and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc)),
        limit: params.limit,
        offset: params.offset,
    };
    
    match state.db.query_results(&filter).await {
        Ok(results) => Ok(Json(ApiResponse::success(results))),
        Err(e) => {
            warn!("Failed to query results: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_dashboard_summary(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<DashboardSummary>>, StatusCode> {
    // Get basic stats
    let stats = match state.db.get_statistics().await {
        Ok(stats) => stats,
        Err(e) => {
            warn!("Failed to get statistics: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    let total_sessions = stats.get("total_sessions")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let total_results = stats.get("total_results")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    
    // Get recent results
    let filter = BenchmarkFilter {
        limit: Some(10),
        ..Default::default()
    };
    
    let recent_results = state.db.query_results(&filter).await
        .unwrap_or_default();
    
    // Get performance trends (simplified)
    let trends = get_simple_trends(&state.db).await
        .unwrap_or_default();
    
    // Count active sessions (running or created)
    let active_sessions = 0; // Would need to implement this query
    
    let summary = DashboardSummary {
        total_sessions,
        total_results,
        active_sessions,
        recent_results,
        performance_trends: trends,
    };
    
    Ok(Json(ApiResponse::success(summary)))
}

async fn get_operation_trends(
    State(state): State<AppState>,
    Path(operation): Path<String>,
) -> Result<Json<ApiResponse<TrendData>>, StatusCode> {
    let filter = BenchmarkFilter {
        operations: Some(vec![operation.clone()]),
        limit: Some(100),
        ..Default::default()
    };
    
    match state.db.query_results(&filter).await {
        Ok(results) => {
            let trend_data = TrendData {
                operation: operation.clone(),
                values: results.into_iter().map(|r| TrendPoint {
                    timestamp: r.timestamp.to_rfc3339(),
                    value: r.mean_time_ns as f64 / 1_000_000.0, // Convert to milliseconds
                }).collect(),
            };
            Ok(Json(ApiResponse::success(trend_data)))
        }
        Err(e) => {
            warn!("Failed to get trends for operation {}: {}", operation, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Deserialize)]
struct CompareRequest {
    baseline_session_id: String,
    current_session_id: String,
}

#[derive(Serialize)]
struct ComparisonResult {
    baseline_session: String,
    current_session: String,
    comparisons: Vec<OperationComparison>,
    summary: ComparisonSummary,
}

#[derive(Serialize)]
struct OperationComparison {
    operation: String,
    baseline_mean_ms: f64,
    current_mean_ms: f64,
    change_percent: f64,
    is_improvement: bool,
}

#[derive(Serialize)]
struct ComparisonSummary {
    total_operations: usize,
    improved_operations: usize,
    degraded_operations: usize,
    unchanged_operations: usize,
}

async fn compare_sessions(
    State(state): State<AppState>,
    Json(request): Json<CompareRequest>,
) -> Result<Json<ApiResponse<ComparisonResult>>, StatusCode> {
    // Get results for baseline session
    let baseline_filter = BenchmarkFilter {
        session_ids: Some(vec![request.baseline_session_id.clone()]),
        ..Default::default()
    };
    
    let baseline_results = match state.db.query_results(&baseline_filter).await {
        Ok(results) => results,
        Err(e) => {
            warn!("Failed to get baseline results: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Get results for current session
    let current_filter = BenchmarkFilter {
        session_ids: Some(vec![request.current_session_id.clone()]),
        ..Default::default()
    };
    
    let current_results = match state.db.query_results(&current_filter).await {
        Ok(results) => results,
        Err(e) => {
            warn!("Failed to get current results: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };
    
    // Create operation-indexed maps
    let baseline_map: HashMap<String, &DbBenchmarkResult> = baseline_results
        .iter()
        .map(|r| (r.operation.clone(), r))
        .collect();
    
    let current_map: HashMap<String, &DbBenchmarkResult> = current_results
        .iter()
        .map(|r| (r.operation.clone(), r))
        .collect();
    
    // Generate comparisons
    let mut comparisons = Vec::new();
    let mut improved = 0;
    let mut degraded = 0;
    let mut unchanged = 0;
    
    for (operation, current_result) in &current_map {
        if let Some(baseline_result) = baseline_map.get(operation) {
            let baseline_ms = baseline_result.mean_time_ns as f64 / 1_000_000.0;
            let current_ms = current_result.mean_time_ns as f64 / 1_000_000.0;
            
            let change_percent = if baseline_ms > 0.0 {
                ((current_ms - baseline_ms) / baseline_ms) * 100.0
            } else {
                0.0
            };
            
            let is_improvement = change_percent < -1.0; // More than 1% improvement
            
            if is_improvement {
                improved += 1;
            } else if change_percent > 1.0 {
                degraded += 1;
            } else {
                unchanged += 1;
            }
            
            comparisons.push(OperationComparison {
                operation: operation.clone(),
                baseline_mean_ms: baseline_ms,
                current_mean_ms: current_ms,
                change_percent,
                is_improvement,
            });
        }
    }
    
    let result = ComparisonResult {
        baseline_session: request.baseline_session_id,
        current_session: request.current_session_id,
        comparisons,
        summary: ComparisonSummary {
            total_operations: current_map.len(),
            improved_operations: improved,
            degraded_operations: degraded,
            unchanged_operations: unchanged,
        },
    };
    
    Ok(Json(ApiResponse::success(result)))
}

async fn get_database_stats(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<HashMap<String, serde_json::Value>>>, StatusCode> {
    match state.db.get_statistics().await {
        Ok(stats) => Ok(Json(ApiResponse::success(stats))),
        Err(e) => {
            warn!("Failed to get database stats: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(_state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(handle_websocket)
}

async fn handle_websocket(mut socket: axum::extract::ws::WebSocket) {
    // Send periodic updates to connected clients
    loop {
        let update = serde_json::json!({
            "type": "heartbeat",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "message": "Dashboard is active"
        });
        
        if socket.send(axum::extract::ws::Message::Text(update.to_string())).await.is_err() {
            break;
        }
        
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}

// Helper functions

async fn get_simple_trends(db: &BenchmarkDatabase) -> Result<Vec<TrendData>> {
    let metrics = db.get_performance_metrics(None, Some(7)).await?;
    
    let mut operation_groups: HashMap<String, Vec<(String, f64)>> = HashMap::new();
    
    for metric in metrics {
        operation_groups
            .entry(metric.operation)
            .or_default()
            .push((metric.time_period, metric.avg_duration_ms));
    }
    
    let trends: Vec<TrendData> = operation_groups
        .into_iter()
        .map(|(operation, mut points)| {
            points.sort_by(|a, b| a.0.cmp(&b.0));
            TrendData {
                operation,
                values: points.into_iter().map(|(timestamp, value)| TrendPoint {
                    timestamp,
                    value,
                }).collect(),
            }
        })
        .collect();
    
    Ok(trends)
}

async fn show_stats(db: Arc<BenchmarkDatabase>) -> Result<()> {
    let stats = db.get_statistics().await?;
    
    println!("Database Statistics:");
    println!("==================");
    
    for (key, value) in &stats {
        match value {
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    println!("{}: {}", key, i);
                } else if let Some(f) = n.as_f64() {
                    println!("{}: {:.2}", key, f);
                }
            }
            serde_json::Value::String(s) => {
                println!("{}: {}", key, s);
            }
            _ => {
                println!("{}: {:?}", key, value);
            }
        }
    }
    
    // Show trend analysis
    println!("\nPerformance Trends (Last 30 days):");
    println!("==================================");
    
    match db.analyze_trends(None, 30).await {
        Ok(trends) => {
            for trend in trends {
                let direction = match trend.trend_direction {
                    mooseng_benchmarks::database::TrendDirection::Improving => "📈 Improving",
                    mooseng_benchmarks::database::TrendDirection::Degrading => "📉 Degrading", 
                    mooseng_benchmarks::database::TrendDirection::Stable => "➡️  Stable",
                    mooseng_benchmarks::database::TrendDirection::Insufficient => "❓ Insufficient Data",
                };
                
                println!("{}: {} ({:.1}% change, {:.0}% confidence, {} data points)",
                         trend.operation, direction, trend.change_percent, 
                         trend.confidence * 100.0, trend.data_points);
            }
        }
        Err(e) => {
            warn!("Failed to analyze trends: {}", e);
        }
    }
    
    Ok(())
}

async fn cleanup_data(db: Arc<BenchmarkDatabase>, retention_days: i32) -> Result<()> {
    info!("Starting data cleanup with {} day retention", retention_days);
    
    let deleted_count = db.cleanup_old_data(retention_days).await?;
    
    if deleted_count > 0 {
        info!("Deleted {} old records", deleted_count);
        
        // Vacuum the database to reclaim space
        db.vacuum().await?;
        info!("Database vacuum completed");
    } else {
        info!("No old data found to clean up");
    }
    
    Ok(())
}