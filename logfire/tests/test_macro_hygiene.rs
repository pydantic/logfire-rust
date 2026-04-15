#![no_implicit_prelude]
//! Tests that logfire macros compile without external dependencies.

#[test]
fn test_macros() {
    // If the macros are not hygienic, they will fail to compile due to the
    // use of `no_implicit_prelude` above.
    ::logfire::span!("Test span");

    ::logfire::trace!("Test trace");
    ::logfire::debug!("Test debug");
    ::logfire::info!("Test info");
    ::logfire::warn!("Test warn");
    ::logfire::error!("Test error");
}
