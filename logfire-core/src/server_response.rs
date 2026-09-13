//! Out-of-band signals which the Logfire backend attaches to its API responses.
//!
//! The backend sets the [`WARNING_HEADER_NAME`] header on responses to signal something it wants
//! the user to see (for example the deprecation of an endpoint), without relying on the response
//! body. SDKs surface the header value as a warning on stderr.

use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock, PoisonError},
};

/// The response header used by the Logfire backend to send a warning to the SDK.
pub const WARNING_HEADER_NAME: &str = "x-logfire-warning";

/// Print a warning received from the Logfire backend to stderr.
///
/// Repeats of the same message are suppressed, so that a chatty server only warns once per
/// process (matching the deduplication which Python's `warnings` module does for the Python SDK).
pub fn emit_warning(message: &str) {
    if is_first_occurrence(message) {
        eprintln!("Logfire server warning: {message}");
    }
}

/// Record `message` as seen, returning whether it had not been seen before.
fn is_first_occurrence(message: &str) -> bool {
    static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SEEN.get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::is_first_occurrence;

    #[test]
    fn repeated_warnings_are_deduplicated() {
        let message = "repeated_warnings_are_deduplicated"; // unique to this test
        assert!(is_first_occurrence(message));
        assert!(!is_first_occurrence(message));
        assert!(is_first_occurrence("a different message"));
    }
}
