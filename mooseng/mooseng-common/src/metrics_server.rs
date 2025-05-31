//! HTTP server for exposing Prometheus metrics

use anyhow::Result;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info};

use crate::metrics::{MetricsRegistry, metrics_handler};

/// HTTP metrics server
pub struct MetricsServer {
    registry: Arc<MetricsRegistry>,
    addr: SocketAddr,
}

impl MetricsServer {
    /// Create a new metrics server
    pub fn new(registry: Arc<MetricsRegistry>, addr: SocketAddr) -> Self {
        Self { registry, addr }
    }

    /// Start the metrics server
    pub async fn start(self) -> Result<()> {
        let registry = Arc::clone(&self.registry);

        let make_svc = make_service_fn(move |_conn| {
            let registry = Arc::clone(&registry);
            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    let registry = Arc::clone(&registry);
                    handle_request(req, registry)
                }))
            }
        });

        let server = Server::bind(&self.addr).serve(make_svc);
        info!("Metrics server listening on http://{}", self.addr);

        if let Err(e) = server.await {
            error!("Metrics server error: {}", e);
            return Err(e.into());
        }

        Ok(())
    }
}

/// Handle HTTP requests
async fn handle_request(
    req: Request<Body>,
    registry: Arc<MetricsRegistry>,
) -> Result<Response<Body>, Infallible> {
    let response = match (req.method(), req.uri().path()) {
        (&Method::GET, "/metrics") => {
            match metrics_handler(&registry).await {
                Ok(metrics) => {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", "text/plain; version=0.0.4; charset=utf-8")
                        .body(Body::from(metrics))
                        .unwrap()
                }
                Err(e) => {
                    error!("Failed to get metrics: {}", e);
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Body::from(format!("Error: {}", e)))
                        .unwrap()
                }
            }
        }
        (&Method::GET, "/health") => {
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"status":"healthy"}"#))
                .unwrap()
        }
        (&Method::GET, "/") => {
            let html = r#"
                <html>
                <head><title>MooseNG Metrics</title></head>
                <body>
                    <h1>MooseNG Metrics</h1>
                    <p>Available endpoints:</p>
                    <ul>
                        <li><a href="/metrics">/metrics</a> - Prometheus metrics</li>
                        <li><a href="/health">/health</a> - Health check</li>
                    </ul>
                </body>
                </html>
            "#;
            Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "text/html")
                .body(Body::from(html))
                .unwrap()
        }
        _ => {
            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap()
        }
    };
    
    Ok(response)
}