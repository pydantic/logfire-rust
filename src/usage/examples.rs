//! # Examples of using the `logfire` SDK
//!
//! These are complete code examples which show how to use the `logfire` SDK
//! with various frameworks and libraries.

/// # Example of using `logfire` to instrument an `actix-web` webserver
///
/// ```rust,no_run
#[doc = include_str!("../../examples/actix-web.rs")]
/// ```
pub mod actix_web {}

/// # Example of using `logfire` to instrument an `axum` webserver
///
/// ```rust,no_run
#[doc = include_str!("../../examples/axum.rs")]
/// ```
pub mod axum {}
