//! Test the new sigil syntax

use serde::Serialize;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
struct User {
    id: u64,
    name: String,
}

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "User #{}: {}", self.id, self.name)
    }
}

fn main() -> Result<()> {
    let logfire = logfire::configure().finish()?;
    let _guard = logfire.shutdown_guard();

    let user = User {
        id: 123,
        name: "Alice".to_string(),
    };

    // Test Display (default, no sigil)
    logfire::info!("User (display): {user}", user = &user);

    // Test Display (explicit % sigil)
    logfire::info!("User (display explicit): {user}", %user);

    // Test Debug (? sigil)
    logfire::info!("User (debug): {user}", ?user);

    // Test Serialize (# sigil)
    logfire::info!("User (json): {user}", #user);

    // Test with explicit values
    logfire::info!("User display: {u}", u = % user);
    logfire::info!("User debug: {u}", u = ? user);
    logfire::info!("User json: {u}", u = # user);

    logfire::info!("All sigil tests completed!");

    Ok(())
}
