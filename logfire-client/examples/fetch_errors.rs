//! Fetches recent errors from Logfire and prints them.
//!
//! Requires `LOGFIRE_READ_TOKEN` environment variable to be set.
//! Optionally set `LOGFIRE_BASE_URL` to override the API endpoint.
//!
//! Run with: `cargo run --example fetch_errors`

use logfire_client::builder::LogfireClientBuilder;
use serde::Deserialize;

/// An error record from Logfire.
#[derive(Debug, Deserialize)]
struct ErrorRecord {
    start_timestamp: String,
    message: String,
    service_name: Option<String>,
    exception_type: Option<String>,
    exception_message: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = LogfireClientBuilder::new()
        .from_env()
        .build_client()
        .expect("LOGFIRE_READ_TOKEN must be set");

    let errors: Vec<ErrorRecord> = client
        .query(
            "SELECT start_timestamp, message, service_name, exception_type, exception_message
             FROM records
             WHERE is_exception
               AND start_timestamp > now() - INTERVAL '1 day'
             ORDER BY start_timestamp DESC
             LIMIT 10",
        )
        .await?;

    if errors.is_empty() {
        println!("No errors in the last 24 hours.");
    } else {
        println!("Found {} error(s):\n", errors.len());
        for (i, err) in errors.iter().enumerate() {
            println!("--- Error {} ---", i + 1);
            println!("  Time:    {}", err.start_timestamp);
            println!("  Message: {}", err.message);
            if let Some(svc) = &err.service_name {
                println!("  Service: {}", svc);
            }
            if let Some(exc_type) = &err.exception_type {
                println!("  Type:    {}", exc_type);
            }
            if let Some(exc_msg) = &err.exception_message {
                println!("  Detail:  {}", exc_msg);
            }
            println!();
        }
    }

    Ok(())
}
