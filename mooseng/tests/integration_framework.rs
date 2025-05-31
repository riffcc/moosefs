use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};
use tokio::time;
use tracing::{error, info, warn};

/// Integration test framework for coordinating parallel development work
pub struct IntegrationTestFramework {
    test_env: TestEnvironment,
    component_tests: HashMap<String, ComponentTest>,
    integration_tests: Vec<IntegrationTest>,
}

/// Test environment configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestEnvironment {
    pub master_endpoints: Vec<String>,
    pub chunkserver_endpoints: Vec<String>,
    pub metalogger_endpoints: Vec<String>,
    pub test_data_path: String,
    pub metrics_endpoint: String,
    pub cli_binary_path: String,
}

/// Component-specific test configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentTest {
    pub component_name: String,
    pub test_modules: Vec<String>,
    pub health_check_endpoint: Option<String>,
    pub metrics_endpoints: Vec<String>,
    pub dependencies: Vec<String>,
    pub timeout_seconds: u64,
}

/// Integration test specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationTest {
    pub test_name: String,
    pub description: String,
    pub required_components: Vec<String>,
    pub test_sequence: Vec<TestStep>,
    pub expected_results: Vec<ExpectedResult>,
    pub cleanup_steps: Vec<TestStep>,
}

/// Individual test step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestStep {
    pub step_name: String,
    pub step_type: TestStepType,
    pub parameters: HashMap<String, String>,
    pub timeout_seconds: u64,
    pub retry_count: u32,
}

/// Test step types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestStepType {
    StartComponent,
    StopComponent,
    CliCommand,
    HealthCheck,
    MetricsCheck,
    FileOperation,
    NetworkTest,
    LoadTest,
    Custom(String),
}

/// Expected test results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedResult {
    pub metric_name: String,
    pub expected_value: ExpectedValue,
    pub tolerance: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpectedValue {
    Exact(String),
    Range(f64, f64),
    Minimum(f64),
    Maximum(f64),
    Boolean(bool),
    Contains(String),
}

/// Test execution result
#[derive(Debug, Serialize, Deserialize)]
pub struct TestResult {
    pub test_name: String,
    pub status: TestStatus,
    pub duration: Duration,
    pub details: String,
    pub metrics: HashMap<String, f64>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Timeout,
    Error,
}

impl IntegrationTestFramework {
    /// Create a new integration test framework
    pub fn new(test_env: TestEnvironment) -> Self {
        Self {
            test_env,
            component_tests: HashMap::new(),
            integration_tests: Vec::new(),
        }
    }

    /// Load test configuration from file
    pub async fn load_config<P: AsRef<Path>>(config_path: P) -> Result<Self> {
        let config_content = tokio::fs::read_to_string(config_path).await?;
        let framework: IntegrationTestFramework = serde_yaml::from_str(&config_content)?;
        Ok(framework)
    }

    /// Register a component test
    pub fn register_component_test(&mut self, test: ComponentTest) {
        self.component_tests.insert(test.component_name.clone(), test);
    }

    /// Add an integration test
    pub fn add_integration_test(&mut self, test: IntegrationTest) {
        self.integration_tests.push(test);
    }

    /// Run all component tests in parallel
    pub async fn run_component_tests(&self) -> Result<HashMap<String, TestResult>> {
        info!("Starting component tests for {} components", self.component_tests.len());
        
        let mut handles = Vec::new();
        
        for (component_name, component_test) in &self.component_tests {
            let test = component_test.clone();
            let env = self.test_env.clone();
            let name = component_name.clone();
            
            let handle = tokio::spawn(async move {
                let result = run_single_component_test(&name, &test, &env).await;
                (name, result)
            });
            
            handles.push(handle);
        }
        
        let mut results = HashMap::new();
        for handle in handles {
            let (component_name, result) = handle.await?;
            results.insert(component_name, result);
        }
        
        Ok(results)
    }

    /// Run integration tests sequentially
    pub async fn run_integration_tests(&self) -> Result<Vec<TestResult>> {
        info!("Starting {} integration tests", self.integration_tests.len());
        
        let mut results = Vec::new();
        
        for test in &self.integration_tests {
            info!("Running integration test: {}", test.test_name);
            let result = run_single_integration_test(test, &self.test_env).await;
            results.push(result);
            
            // Stop on first failure if configured
            if matches!(result.status, TestStatus::Failed | TestStatus::Error) {
                warn!("Integration test failed: {}", result.details);
            }
        }
        
        Ok(results)
    }

    /// Run full test suite
    pub async fn run_full_suite(&self) -> Result<TestSuiteResult> {
        let start_time = Instant::now();
        
        info!("Starting full integration test suite");
        
        // Run component tests first
        let component_results = self.run_component_tests().await?;
        
        // Check if all component tests passed
        let component_failures: Vec<_> = component_results
            .iter()
            .filter(|(_, result)| !matches!(result.status, TestStatus::Passed))
            .map(|(name, _)| name.clone())
            .collect();
        
        if !component_failures.is_empty() {
            warn!("Component test failures: {:?}", component_failures);
        }
        
        // Run integration tests if component tests passed
        let integration_results = if component_failures.is_empty() {
            self.run_integration_tests().await?
        } else {
            warn!("Skipping integration tests due to component failures");
            Vec::new()
        };
        
        let duration = start_time.elapsed();
        
        Ok(TestSuiteResult {
            duration,
            component_results,
            integration_results,
            overall_status: calculate_overall_status(&component_failures),
        })
    }

    /// Generate test report
    pub fn generate_report(&self, results: &TestSuiteResult) -> String {
        let mut report = String::new();
        
        report.push_str("# MooseNG Integration Test Report\n\n");
        report.push_str(&format!("**Test Duration**: {:.2}s\n", results.duration.as_secs_f64()));
        report.push_str(&format!("**Overall Status**: {:?}\n\n", results.overall_status));
        
        // Component test results
        report.push_str("## Component Test Results\n\n");
        for (component, result) in &results.component_results {
            report.push_str(&format!("### {}\n", component));
            report.push_str(&format!("- **Status**: {:?}\n", result.status));
            report.push_str(&format!("- **Duration**: {:.2}s\n", result.duration.as_secs_f64()));
            
            if !result.errors.is_empty() {
                report.push_str("- **Errors**:\n");
                for error in &result.errors {
                    report.push_str(&format!("  - {}\n", error));
                }
            }
            
            if !result.warnings.is_empty() {
                report.push_str("- **Warnings**:\n");
                for warning in &result.warnings {
                    report.push_str(&format!("  - {}\n", warning));
                }
            }
            
            report.push('\n');
        }
        
        // Integration test results
        if !results.integration_results.is_empty() {
            report.push_str("## Integration Test Results\n\n");
            for result in &results.integration_results {
                report.push_str(&format!("### {}\n", result.test_name));
                report.push_str(&format!("- **Status**: {:?}\n", result.status));
                report.push_str(&format!("- **Duration**: {:.2}s\n", result.duration.as_secs_f64()));
                report.push_str(&format!("- **Details**: {}\n\n", result.details));
            }
        }
        
        report
    }
}

/// Test suite execution result
#[derive(Debug)]
pub struct TestSuiteResult {
    pub duration: Duration,
    pub component_results: HashMap<String, TestResult>,
    pub integration_results: Vec<TestResult>,
    pub overall_status: TestStatus,
}

/// Run a single component test
async fn run_single_component_test(
    component_name: &str,
    test: &ComponentTest,
    env: &TestEnvironment,
) -> TestResult {
    let start_time = Instant::now();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut metrics = HashMap::new();
    
    info!("Running component test for: {}", component_name);
    
    // Check health endpoint if available
    if let Some(health_endpoint) = &test.health_check_endpoint {
        match check_health_endpoint(health_endpoint).await {
            Ok(healthy) => {
                if !healthy {
                    errors.push(format!("Health check failed for {}", health_endpoint));
                }
            }
            Err(e) => {
                errors.push(format!("Health check error: {}", e));
            }
        }
    }
    
    // Check metrics endpoints
    for metrics_endpoint in &test.metrics_endpoints {
        match collect_metrics(metrics_endpoint).await {
            Ok(endpoint_metrics) => {
                metrics.extend(endpoint_metrics);
            }
            Err(e) => {
                warnings.push(format!("Metrics collection failed for {}: {}", metrics_endpoint, e));
            }
        }
    }
    
    // Run test modules
    for test_module in &test.test_modules {
        match run_test_module(component_name, test_module).await {
            Ok(_) => {
                info!("Test module {} passed for {}", test_module, component_name);
            }
            Err(e) => {
                errors.push(format!("Test module {} failed: {}", test_module, e));
            }
        }
    }
    
    let duration = start_time.elapsed();
    let status = if errors.is_empty() {
        TestStatus::Passed
    } else {
        TestStatus::Failed
    };
    
    TestResult {
        test_name: component_name.to_string(),
        status,
        duration,
        details: format!("Component test for {}", component_name),
        metrics,
        errors,
        warnings,
    }
}

/// Run a single integration test
async fn run_single_integration_test(
    test: &IntegrationTest,
    env: &TestEnvironment,
) -> TestResult {
    let start_time = Instant::now();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let mut metrics = HashMap::new();
    
    info!("Running integration test: {}", test.test_name);
    
    // Execute test steps
    for step in &test.test_sequence {
        match execute_test_step(step, env).await {
            Ok(step_metrics) => {
                metrics.extend(step_metrics);
            }
            Err(e) => {
                errors.push(format!("Step '{}' failed: {}", step.step_name, e));
                break; // Stop on first failure
            }
        }
    }
    
    // Validate expected results
    for expected in &test.expected_results {
        if let Some(actual_value) = metrics.get(&expected.metric_name) {
            if !validate_expected_result(expected, *actual_value) {
                errors.push(format!(
                    "Metric {} validation failed. Expected: {:?}, Actual: {}",
                    expected.metric_name, expected.expected_value, actual_value
                ));
            }
        } else {
            warnings.push(format!("Metric {} not found in results", expected.metric_name));
        }
    }
    
    // Run cleanup steps
    for cleanup_step in &test.cleanup_steps {
        if let Err(e) = execute_test_step(cleanup_step, env).await {
            warnings.push(format!("Cleanup step '{}' failed: {}", cleanup_step.step_name, e));
        }
    }
    
    let duration = start_time.elapsed();
    let status = if errors.is_empty() {
        TestStatus::Passed
    } else {
        TestStatus::Failed
    };
    
    TestResult {
        test_name: test.test_name.clone(),
        status,
        duration,
        details: test.description.clone(),
        metrics,
        errors,
        warnings,
    }
}

/// Execute a single test step
async fn execute_test_step(
    step: &TestStep,
    env: &TestEnvironment,
) -> Result<HashMap<String, f64>> {
    let timeout = Duration::from_secs(step.timeout_seconds);
    
    let result = time::timeout(timeout, async {
        match &step.step_type {
            TestStepType::CliCommand => {
                execute_cli_command(step, env).await
            }
            TestStepType::HealthCheck => {
                execute_health_check(step, env).await
            }
            TestStepType::MetricsCheck => {
                execute_metrics_check(step, env).await
            }
            TestStepType::FileOperation => {
                execute_file_operation(step, env).await
            }
            TestStepType::NetworkTest => {
                execute_network_test(step, env).await
            }
            TestStepType::LoadTest => {
                execute_load_test(step, env).await
            }
            TestStepType::StartComponent => {
                execute_start_component(step, env).await
            }
            TestStepType::StopComponent => {
                execute_stop_component(step, env).await
            }
            TestStepType::Custom(custom_type) => {
                execute_custom_step(custom_type, step, env).await
            }
        }
    }).await;
    
    match result {
        Ok(Ok(metrics)) => Ok(metrics),
        Ok(Err(e)) => Err(e),
        Err(_) => Err(anyhow::anyhow!("Test step '{}' timed out", step.step_name)),
    }
}

/// Helper functions for different test step types
async fn execute_cli_command(step: &TestStep, env: &TestEnvironment) -> Result<HashMap<String, f64>> {
    let command = step.parameters.get("command").ok_or_else(|| anyhow::anyhow!("Missing command parameter"))?;
    let args: Vec<&str> = step.parameters.get("args").map(|s| s.split_whitespace().collect()).unwrap_or_default();
    
    let output = Command::new(&env.cli_binary_path)
        .args(&[command])
        .args(args)
        .output()?;
    
    if !output.status.success() {
        return Err(anyhow::anyhow!("CLI command failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    
    // Parse metrics from output if needed
    Ok(HashMap::new())
}

async fn execute_health_check(step: &TestStep, _env: &TestEnvironment) -> Result<HashMap<String, f64>> {
    let endpoint = step.parameters.get("endpoint").ok_or_else(|| anyhow::anyhow!("Missing endpoint parameter"))?;
    
    let healthy = check_health_endpoint(endpoint).await?;
    let mut metrics = HashMap::new();
    metrics.insert("health_status".to_string(), if healthy { 1.0 } else { 0.0 });
    
    Ok(metrics)
}

async fn execute_metrics_check(step: &TestStep, _env: &TestEnvironment) -> Result<HashMap<String, f64>> {
    let endpoint = step.parameters.get("endpoint").ok_or_else(|| anyhow::anyhow!("Missing endpoint parameter"))?;
    collect_metrics(endpoint).await
}

async fn execute_file_operation(_step: &TestStep, _env: &TestEnvironment) -> Result<HashMap<String, f64>> {
    // Implement file operation testing
    Ok(HashMap::new())
}

async fn execute_network_test(_step: &TestStep, _env: &TestEnvironment) -> Result<HashMap<String, f64>> {
    // Implement network testing
    Ok(HashMap::new())
}

async fn execute_load_test(_step: &TestStep, _env: &TestEnvironment) -> Result<HashMap<String, f64>> {
    // Implement load testing
    Ok(HashMap::new())
}

async fn execute_start_component(_step: &TestStep, _env: &TestEnvironment) -> Result<HashMap<String, f64>> {
    // Implement component startup
    Ok(HashMap::new())
}

async fn execute_stop_component(_step: &TestStep, _env: &TestEnvironment) -> Result<HashMap<String, f64>> {
    // Implement component shutdown
    Ok(HashMap::new())
}

async fn execute_custom_step(_custom_type: &str, _step: &TestStep, _env: &TestEnvironment) -> Result<HashMap<String, f64>> {
    // Implement custom test steps
    Ok(HashMap::new())
}

/// Check health endpoint
async fn check_health_endpoint(endpoint: &str) -> Result<bool> {
    let client = reqwest::Client::new();
    let response = client.get(endpoint).send().await?;
    Ok(response.status().is_success())
}

/// Collect metrics from endpoint
async fn collect_metrics(endpoint: &str) -> Result<HashMap<String, f64>> {
    let client = reqwest::Client::new();
    let response = client.get(endpoint).send().await?;
    let text = response.text().await?;
    
    // Parse Prometheus metrics format
    let mut metrics = HashMap::new();
    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        
        if let Some((metric_name, value_str)) = line.split_once(' ') {
            if let Ok(value) = value_str.trim().parse::<f64>() {
                metrics.insert(metric_name.to_string(), value);
            }
        }
    }
    
    Ok(metrics)
}

/// Run test module
async fn run_test_module(component_name: &str, test_module: &str) -> Result<()> {
    let output = Command::new("cargo")
        .args(&["test", "--package", component_name, test_module])
        .output()?;
    
    if !output.status.success() {
        return Err(anyhow::anyhow!("Test module failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    
    Ok(())
}

/// Validate expected result
fn validate_expected_result(expected: &ExpectedResult, actual: f64) -> bool {
    match &expected.expected_value {
        ExpectedValue::Exact(s) => {
            if let Ok(expected_val) = s.parse::<f64>() {
                (actual - expected_val).abs() < expected.tolerance.unwrap_or(0.001)
            } else {
                false
            }
        }
        ExpectedValue::Range(min, max) => actual >= *min && actual <= *max,
        ExpectedValue::Minimum(min) => actual >= *min,
        ExpectedValue::Maximum(max) => actual <= *max,
        ExpectedValue::Boolean(expected_bool) => {
            (*expected_bool && actual > 0.5) || (!*expected_bool && actual <= 0.5)
        }
        ExpectedValue::Contains(_) => true, // Not applicable for numeric values
    }
}

/// Calculate overall test status
fn calculate_overall_status(component_failures: &[String]) -> TestStatus {
    if component_failures.is_empty() {
        TestStatus::Passed
    } else {
        TestStatus::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_framework_creation() {
        let env = TestEnvironment {
            master_endpoints: vec!["http://localhost:9421".to_string()],
            chunkserver_endpoints: vec!["http://localhost:9422".to_string()],
            metalogger_endpoints: vec!["http://localhost:9419".to_string()],
            test_data_path: "/tmp/mooseng_test".to_string(),
            metrics_endpoint: "http://localhost:8080/metrics".to_string(),
            cli_binary_path: "./target/debug/mooseng".to_string(),
        };
        
        let framework = IntegrationTestFramework::new(env);
        assert_eq!(framework.component_tests.len(), 0);
        assert_eq!(framework.integration_tests.len(), 0);
    }
    
    #[test]
    fn test_expected_value_validation() {
        let expected = ExpectedResult {
            metric_name: "test_metric".to_string(),
            expected_value: ExpectedValue::Range(10.0, 20.0),
            tolerance: None,
        };
        
        assert!(validate_expected_result(&expected, 15.0));
        assert!(!validate_expected_result(&expected, 5.0));
        assert!(!validate_expected_result(&expected, 25.0));
    }
}