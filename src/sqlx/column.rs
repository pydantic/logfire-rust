//! Column metadata for Logfire query results.

use sqlx_core::column::Column;

use crate::sqlx::{Logfire, LogfireTypeInfo};

/// Column metadata from a Logfire query result.
#[derive(Clone, Debug)]
pub struct LogfireColumn {
    /// Zero-based column index.
    pub(crate) ordinal: usize,
    /// Column name.
    pub(crate) name: String,
    /// Column type information.
    pub(crate) type_info: LogfireTypeInfo,
}

impl Column for LogfireColumn {
    type Database = Logfire;

    fn ordinal(&self) -> usize {
        self.ordinal
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn type_info(&self) -> &LogfireTypeInfo {
        &self.type_info
    }
}
