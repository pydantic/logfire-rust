//! Example demonstrating how to log custom types using Display, Debug, and Serialize

use serde::Serialize;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone, Serialize)]
struct User {
    id: u64,
    name: String,
    email: String,
    active: bool,
}

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "User #{}: {}", self.id, self.name)
    }
}

#[derive(Debug, Clone, Serialize)]
struct Config {
    timeout_ms: u64,
    max_retries: u32,
    endpoints: Vec<String>,
}

impl std::fmt::Display for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Config(timeout={}ms, retries={}, endpoints={})",
            self.timeout_ms,
            self.max_retries,
            self.endpoints.len()
        )
    }
}

fn main() -> Result<()> {
    let logfire = logfire::configure().finish()?;
    let _guard = logfire.shutdown_guard();

    let user = User {
        id: 123,
        name: "Alice".to_string(),
        email: "alice@example.com".to_string(),
        active: true,
    };

    let config = Config {
        timeout_ms: 5000,
        max_retries: 3,
        endpoints: vec![
            "https://api.example.com".to_string(),
            "https://backup.example.com".to_string(),
        ],
    };

    // ====================================================================
    // Example 1: Using Display (concise, human-readable)
    // ====================================================================
    logfire::info!("User logged in: {user}", user = &user);
    // Logs: "User logged in: User #123: Alice"
    // Good for: Simple, human-readable output

    // ====================================================================
    // Example 2: Using Debug (verbose, developer-friendly)
    // ====================================================================
    // Before: logfire::info!("User details: {user}", user = format!("{user:?}"));
    // Now with as_debug! helper:
    logfire::info!("User details: {user}", user = logfire::as_debug!(user));
    // Logs: "User details: User { id: 123, name: \"Alice\", email: \"alice@example.com\", active: true }"
    // Good for: Debugging, seeing all fields

    // ====================================================================
    // Example 3: Using Serialize (structured JSON)
    // ====================================================================
    // Before: logfire::info!("User profile: {user}", user = serde_json::to_string(&user)?);
    // Now with as_json! helper:
    logfire::info!("User profile: {user}", user = logfire::as_json!(user));
    // Logs: "User profile: {\"id\":123,\"name\":\"Alice\",\"email\":\"alice@example.com\",\"active\":true}"
    // Good for: Structured data, complex objects, machine parsing

    // ====================================================================
    // Example 4: Combining multiple types in a span
    // ====================================================================
    logfire::span!(
        "Processing user {user_id} with config {config_summary}",
        user_id = user.id,
        config_summary = config.to_string()
    )
    .in_scope(|| {
        // Both Display implementations are used
        logfire::info!("Starting operation for {user}", user = &user);
        logfire::info!("Using configuration: {config}", config = &config);

        // Simulating some work
        std::thread::sleep(std::time::Duration::from_millis(100));

        logfire::info!("Operation completed successfully");
        Result::Ok(())
    })?;

    // ====================================================================
    // Example 5: Logging only Debug (no Display implementation needed)
    // ====================================================================
    #[derive(Debug)]
    #[allow(dead_code)]
    struct InternalState {
        request_count: u64,
        last_error: Option<String>,
    }

    let state = InternalState {
        request_count: 42,
        last_error: Some("Connection timeout".to_string()),
    };

    logfire::info!("Current state: {state}", state = logfire::as_debug!(state));
    // Logs: "Current state: InternalState { request_count: 42, last_error: Some(\"Connection timeout\") }"

    // ====================================================================
    // Example 6: Nested structures with Serialize
    // ====================================================================
    #[derive(Serialize)]
    struct ApiResponse {
        status: String,
        user: User,
        config: Config,
    }

    let response = ApiResponse {
        status: "success".to_string(),
        user: user.clone(),
        config,
    };

    // You can still use serde_json directly if you need pretty-printing
    logfire::info!(
        "API response (pretty): {response}",
        response = serde_json::to_string_pretty(&response)?
    );
    // Or use as_json! for compact JSON
    logfire::info!("API response (compact): {response}", response = logfire::as_json!(response));
    // Logs the entire nested structure as JSON

    logfire::info!("Examples completed!");

    Ok(())
}
