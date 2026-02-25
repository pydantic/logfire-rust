//! Row type for Logfire query results.

use std::sync::Arc;

use sqlx_core::{column::ColumnIndex, row::Row};

use crate::sqlx::{Logfire, LogfireColumn, LogfireValueRef};

/// A row from a Logfire query result.
#[derive(Clone, Debug)]
pub struct LogfireRow {
    /// Column metadata shared across all rows in the result set.
    pub(crate) columns: Arc<Vec<LogfireColumn>>,
    /// Values in this row, ordered by column ordinal.
    pub(crate) values: Vec<serde_json::Value>,
}

impl Row for LogfireRow {
    type Database = Logfire;

    fn columns(&self) -> &[LogfireColumn] {
        &self.columns
    }

    fn try_get_raw<I: ColumnIndex<Self>>(
        &self,
        index: I,
    ) -> Result<LogfireValueRef<'_>, sqlx_core::Error> {
        let idx = index.index(self)?;
        Ok(LogfireValueRef(&self.values[idx]))
    }
}

impl ColumnIndex<LogfireRow> for usize {
    fn index(&self, row: &LogfireRow) -> Result<usize, sqlx_core::Error> {
        if *self < row.columns.len() {
            Ok(*self)
        } else {
            Err(sqlx_core::Error::ColumnNotFound(self.to_string()))
        }
    }
}

impl ColumnIndex<LogfireRow> for &str {
    fn index(&self, row: &LogfireRow) -> Result<usize, sqlx_core::Error> {
        row.columns
            .iter()
            .position(|c| c.name == *self)
            .ok_or_else(|| sqlx_core::Error::ColumnNotFound((*self).to_string()))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::sqlx::LogfireTypeInfo;

    fn test_row() -> LogfireRow {
        let columns = Arc::new(vec![
            LogfireColumn {
                ordinal: 0,
                name: "id".to_string(),
                type_info: LogfireTypeInfo::BigInt,
            },
            LogfireColumn {
                ordinal: 1,
                name: "name".to_string(),
                type_info: LogfireTypeInfo::Text,
            },
        ]);
        LogfireRow {
            columns,
            values: vec![json!(42), json!("test")],
        }
    }

    #[test]
    fn index_by_ordinal() {
        let row = test_row();
        assert_eq!(0_usize.index(&row).unwrap(), 0);
        assert_eq!(1_usize.index(&row).unwrap(), 1);
    }

    #[test]
    fn index_by_ordinal_out_of_bounds() {
        let row = test_row();
        assert!(2_usize.index(&row).is_err());
    }

    #[test]
    fn index_by_name() {
        let row = test_row();
        assert_eq!("id".index(&row).unwrap(), 0);
        assert_eq!("name".index(&row).unwrap(), 1);
    }

    #[test]
    fn index_by_name_not_found() {
        let row = test_row();
        assert!("unknown".index(&row).is_err());
    }
}
