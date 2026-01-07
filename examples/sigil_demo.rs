//! Simple demo showing the simplified syntax

use serde::Serialize;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
struct User {
    id: u64,
    name: String,
}

fn main() -> Result<()> {
    let logfire = logfire::configure().finish()?;
    let _guard = logfire.shutdown_guard();

    let user = User {
        id: 123,
        name: "Alice".to_string(),
    };

    // Option 1: Using the helper macros (works today!)
    logfire::info!("User (debug): {user}", user = logfire::as_debug!(user));
    logfire::info!("User (json): {user}", user = logfire::as_json!(user));

    // Option 2: If we want tracing-style sigils (more complex to implement)
    // logfire::info!("User: {user}", ?user);  // Would use Debug
    // logfire::info!("User: {user}", #user);  // Would use Serialize

    Ok(())
}
