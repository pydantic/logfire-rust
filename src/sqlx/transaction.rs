//! Transaction manager for Logfire.
//!
//! Logfire is a read-only analytics backend, so transactions are no-ops.

use futures_core::future::BoxFuture;
use sqlx_core::transaction::TransactionManager;

use crate::sqlx::{Logfire, LogfireConnection};

/// Transaction manager for Logfire.
///
/// All methods are no-ops since Logfire is read-only.
pub struct LogfireTransactionManager;

impl TransactionManager for LogfireTransactionManager {
    type Database = Logfire;

    fn begin<'conn>(
        _conn: &'conn mut LogfireConnection,
        _statement: Option<std::borrow::Cow<'static, str>>,
    ) -> BoxFuture<'conn, Result<(), sqlx_core::Error>> {
        Box::pin(async { Ok(()) })
    }

    fn commit(_conn: &mut LogfireConnection) -> BoxFuture<'_, Result<(), sqlx_core::Error>> {
        Box::pin(async { Ok(()) })
    }

    fn rollback(_conn: &mut LogfireConnection) -> BoxFuture<'_, Result<(), sqlx_core::Error>> {
        Box::pin(async { Ok(()) })
    }

    fn start_rollback(_conn: &mut LogfireConnection) {}

    fn get_transaction_depth(_conn: &LogfireConnection) -> usize {
        0
    }
}
