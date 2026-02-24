//! Logfire client SDK for querying Pydantic Logfire logs and metrics using SQL.

mod builder;
mod token;

pub use builder::{BuilderError, LogfireClientBuilder};
pub use token::Region;
