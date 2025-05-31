//! Example demonstrating TLS-enabled networking in MooseNG
//!
//! This example shows how to configure and use TLS encryption
//! for secure communication between MooseNG components.

use anyhow::Result;
use mooseng_common::networking::{ConnectionPool, ConnectionPoolConfig, TlsConfig};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🔐 MooseNG TLS Networking Example");
    println!("================================");

    // Example 1: TLS disabled (for development)
    println!("\n1. Creating connection pool with TLS disabled...");
    let config_no_tls = ConnectionPoolConfig {
        tls_config: TlsConfig::disabled(),
        ..Default::default()
    };
    let pool_no_tls = ConnectionPool::new(config_no_tls)?;
    println!("✅ Connection pool created without TLS");

    // Example 2: TLS enabled with custom configuration
    println!("\n2. Creating connection pool with TLS enabled...");
    let tls_config = TlsConfig::client()
        .with_domain("localhost".to_string())
        .with_ca_cert("/path/to/ca.pem")
        .with_client_cert("/path/to/client.pem", "/path/to/client.key");

    let config_with_tls = ConnectionPoolConfig {
        tls_config,
        ..Default::default()
    };

    match ConnectionPool::new(config_with_tls) {
        Ok(_pool) => println!("✅ TLS-enabled connection pool created successfully"),
        Err(e) => println!("⚠️  TLS configuration failed (expected - no certificates): {}", e),
    }

    // Example 3: Server TLS configuration
    println!("\n3. Demonstrating server TLS configuration...");
    let server_tls_config = TlsConfig::server()
        .with_server_cert("/path/to/server.pem", "/path/to/server.key")
        .with_ca_cert("/path/to/ca.pem")
        .with_domain("mooseng.local".to_string());

    println!("✅ Server TLS configuration created");
    println!("   - Mutual TLS: {}", server_tls_config.require_client_cert);
    println!("   - Domain: {:?}", server_tls_config.domain_name);

    // Show pool statistics
    println!("\n4. Connection pool statistics:");
    let stats = pool_no_tls.get_stats().await;
    println!("   - Total endpoints: {}", stats.len());

    println!("\n🚀 TLS networking is ready for secure MooseNG communication!");
    println!("   - Perfect forward secrecy with ephemeral key exchange");
    println!("   - Mutual TLS authentication support");  
    println!("   - Certificate validation and rotation");
    println!("   - Connection pooling with health monitoring");

    Ok(())
}