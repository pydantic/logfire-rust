//! # Examples of using the `logfire` SDK
//!
//! These are complete code examples which show how to use the `logfire` SDK
//! with various frameworks and libraries, instrumented for Pydantic Logfire.
//!
//! These can also be found in the [`examples/`](https://github.com/pydantic/logfire-rust/tree/main/examples)
//! directory of the repository.

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

/// # Example of using `logfire` to instrument a toy Rust application
///
/// ```rust
#[doc = include_str!("../../examples/basic.rs")]
/// ```
pub mod basic {}
