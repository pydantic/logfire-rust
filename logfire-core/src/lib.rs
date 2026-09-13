//! Shared types for Pydantic Logfire SDKs.
//!
//! This crate provides common functionality used by both the instrumentation SDK (`logfire`)
//! and the query client (`logfire-client`).

pub mod server_response;
mod token;

pub use token::{Region, parse_region};
