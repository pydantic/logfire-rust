//! sqlx integration for Logfire.
//!
//! This module implements sqlx's `Database` trait family over HTTP/JSON, enabling
//! familiar sqlx patterns for querying Logfire. The integration wraps the existing
//! [`LogfireClient`](crate::client::LogfireClient) infrastructure.
//!
//! # Constraints
//!
//! - **Runtime only**: Compile-time `sqlx::query!` macros are not supported.
//! - **Read-only**: Transactions are no-ops.

pub mod arguments;
mod column;
mod connection;
mod error;
mod executor;
mod row;
mod statement;
mod transaction;
mod type_info;
mod types;
mod value;

pub use arguments::{LogfireArgumentBuffer, LogfireArguments};
pub use column::LogfireColumn;
pub use connection::{LogfireConnectOptions, LogfireConnection};
pub use row::LogfireRow;
use sqlx_core::database::Database;
pub use statement::LogfireStatement;
pub use transaction::LogfireTransactionManager;
pub use type_info::LogfireTypeInfo;
pub use value::{LogfireValue, LogfireValueRef};

/// Logfire database marker type for sqlx integration.
#[derive(Debug)]
pub struct Logfire;

impl Database for Logfire {
    type Connection = LogfireConnection;
    type TransactionManager = LogfireTransactionManager;
    type Row = LogfireRow;
    type QueryResult = LogfireQueryResult;
    type Column = LogfireColumn;
    type TypeInfo = LogfireTypeInfo;
    type Value = LogfireValue;
    type ValueRef<'r> = LogfireValueRef<'r>;
    type Arguments<'q> = LogfireArguments;
    type ArgumentBuffer<'q> = LogfireArgumentBuffer;
    type Statement<'q> = LogfireStatement<'q>;

    const NAME: &'static str = "Logfire";
    const URL_SCHEMES: &'static [&'static str] = &["logfire"];
}

/// Result of a query execution.
#[derive(Clone, Debug, Default)]
pub struct LogfireQueryResult {
    /// Number of rows affected (or returned for SELECT queries).
    pub(crate) rows_affected: u64,
}

impl LogfireQueryResult {
    /// Returns the number of rows affected by the query.
    pub fn rows_affected(&self) -> u64 {
        self.rows_affected
    }
}

impl Extend<LogfireQueryResult> for LogfireQueryResult {
    fn extend<T: IntoIterator<Item = LogfireQueryResult>>(&mut self, iter: T) {
        for result in iter {
            self.rows_affected += result.rows_affected;
        }
    }
}
