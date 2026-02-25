//! Executor implementation for Logfire.

use std::sync::Arc;

use either::Either;
use futures_core::{future::BoxFuture, stream::BoxStream};
use sqlx_core::{describe::Describe, executor::Executor};

use crate::sqlx::{
    Logfire, LogfireColumn, LogfireConnection, LogfireQueryResult, LogfireRow, LogfireTypeInfo,
};

/// Counts the number of positional parameters in a SQL query.
///
/// Scans for `$N` placeholders and returns the maximum N found. This handles
/// gaps in numbering (e.g., `$1, $3` returns 3).
fn count_placeholders(sql: &str) -> usize {
    let mut max_param = 0;
    let mut chars = sql.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        if c == '$' {
            let start = i + 1;
            let mut end = start;

            while let Some(&(j, d)) = chars.peek() {
                if d.is_ascii_digit() {
                    end = j + 1;
                    chars.next();
                } else {
                    break;
                }
            }

            if end > start {
                if let Ok(n) = sql[start..end].parse::<usize>() {
                    max_param = max_param.max(n);
                }
            }
        }
    }
    max_param
}

/// Substitutes all `$N` placeholders with `NULL` for probe queries.
///
/// Used by `describe` to execute a query and retrieve column metadata without
/// needing actual parameter values.
///
/// Note: This does not account for placeholders inside SQL string literals or
/// comments (e.g., `'$1'` or `-- $1`), which would be incorrectly substituted.
fn substitute_nulls(sql: &str) -> String {
    let mut result = String::with_capacity(sql.len());
    let mut chars = sql.char_indices().peekable();

    while let Some((i, c)) = chars.next() {
        if c == '$' {
            let start = i + 1;
            let mut end = start;

            while let Some(&(j, d)) = chars.peek() {
                if d.is_ascii_digit() {
                    end = j + 1;
                    chars.next();
                } else {
                    break;
                }
            }

            if end > start && sql[start..end].parse::<usize>().is_ok() {
                result.push_str("NULL");
            } else {
                result.push('$');
                result.push_str(&sql[start..end]);
            }
        } else {
            result.push(c);
        }
    }
    result
}

impl<'c> Executor<'c> for &'c mut LogfireConnection {
    type Database = Logfire;

    fn fetch_many<'e, 'q: 'e, E>(
        self,
        mut query: E,
    ) -> BoxStream<'e, Result<Either<LogfireQueryResult, LogfireRow>, sqlx_core::Error>>
    where
        'c: 'e,
        E: sqlx_core::executor::Execute<'q, Self::Database> + 'q,
    {
        let sql_template = query.sql().to_string();
        let arguments = query.take_arguments();

        Box::pin(async_stream::try_stream! {
            let sql = match arguments {
                Ok(Some(args)) => args.interpolate(&sql_template),
                Ok(None) => sql_template,
                Err(e) => {
                    Err(sqlx_core::Error::Encode(e))?;
                    unreachable!()
                }
            };

            let results = self.client.query_raw(&sql).await?;
            let columns = Arc::new(
                results
                    .columns
                    .into_iter()
                    .enumerate()
                    .map(|(i, c)| LogfireColumn {
                        ordinal: i,
                        name: c.name,
                        type_info: LogfireTypeInfo::from_json(&c.data_type),
                    })
                    .collect::<Vec<_>>(),
            );

            for row_map in results.rows {
                let values: Vec<_> = columns
                    .iter()
                    .map(|c| row_map.get(&c.name).cloned().unwrap_or(serde_json::Value::Null))
                    .collect();
                yield Either::Right(LogfireRow {
                    columns: Arc::clone(&columns),
                    values,
                });
            }
        })
    }

    fn fetch_optional<'e, 'q: 'e, E>(
        self,
        mut query: E,
    ) -> BoxFuture<'e, Result<Option<LogfireRow>, sqlx_core::Error>>
    where
        'c: 'e,
        E: sqlx_core::executor::Execute<'q, Self::Database> + 'q,
    {
        let sql_template = query.sql().to_string();
        let arguments = query.take_arguments();

        Box::pin(async move {
            let sql = match arguments {
                Ok(Some(args)) => args.interpolate(&sql_template),
                Ok(None) => sql_template,
                Err(e) => return Err(sqlx_core::Error::Encode(e)),
            };

            let results = self.client.query_raw(&sql).await?;

            if results.rows.is_empty() {
                return Ok(None);
            }

            let columns = Arc::new(
                results
                    .columns
                    .into_iter()
                    .enumerate()
                    .map(|(i, c)| LogfireColumn {
                        ordinal: i,
                        name: c.name,
                        type_info: LogfireTypeInfo::from_json(&c.data_type),
                    })
                    .collect::<Vec<_>>(),
            );

            let row_map = &results.rows[0];
            let values: Vec<_> = columns
                .iter()
                .map(|c| {
                    row_map
                        .get(&c.name)
                        .cloned()
                        .unwrap_or(serde_json::Value::Null)
                })
                .collect();

            Ok(Some(LogfireRow { columns, values }))
        })
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        _sql: &'q str,
        _parameters: &'e [LogfireTypeInfo],
    ) -> BoxFuture<'e, Result<crate::sqlx::LogfireStatement<'q>, sqlx_core::Error>>
    where
        'c: 'e,
    {
        Box::pin(async move {
            Err(sqlx_core::Error::Protocol(
                "Logfire does not support prepared statements".to_string(),
            ))
        })
    }

    fn describe<'e, 'q: 'e>(
        self,
        sql: &'q str,
    ) -> BoxFuture<'e, Result<Describe<Self::Database>, sqlx_core::Error>>
    where
        'c: 'e,
    {
        let sql = sql.to_string();

        Box::pin(async move {
            let param_count = count_placeholders(&sql);

            // Substitute placeholders with NULL and wrap with LIMIT 0 to get column
            // metadata without fetching data.
            let probe_sql = substitute_nulls(&sql);
            let probe_sql = format!("SELECT * FROM ({probe_sql}) AS _logfire_probe LIMIT 0");

            let results = self.client.query_raw(&probe_sql).await?;

            let columns: Vec<LogfireColumn> = results
                .columns
                .iter()
                .enumerate()
                .map(|(i, c)| LogfireColumn {
                    ordinal: i,
                    name: c.name.clone(),
                    type_info: LogfireTypeInfo::from_json(&c.data_type),
                })
                .collect();

            let nullable: Vec<Option<bool>> =
                results.columns.iter().map(|c| Some(c.nullable)).collect();

            Ok(Describe {
                columns,
                parameters: Some(Either::Right(param_count)),
                nullable,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{count_placeholders, substitute_nulls};

    #[test]
    fn count_placeholders_none() {
        assert_eq!(count_placeholders("SELECT 1"), 0);
    }

    #[test]
    fn count_placeholders_sequential() {
        assert_eq!(count_placeholders("SELECT $1, $2, $3"), 3);
    }

    #[test]
    fn count_placeholders_with_gaps() {
        assert_eq!(count_placeholders("SELECT $1, $5"), 5);
    }

    #[test]
    fn count_placeholders_repeated() {
        assert_eq!(count_placeholders("SELECT $1, $1, $2"), 2);
    }

    #[test]
    fn count_placeholders_double_digit() {
        assert_eq!(count_placeholders("SELECT $10, $2"), 10);
    }

    #[test]
    fn substitute_nulls_basic() {
        assert_eq!(
            substitute_nulls("SELECT * FROM t WHERE id = $1"),
            "SELECT * FROM t WHERE id = NULL"
        );
    }

    #[test]
    fn substitute_nulls_multiple() {
        assert_eq!(
            substitute_nulls("SELECT $1, $2, $3"),
            "SELECT NULL, NULL, NULL"
        );
    }

    #[test]
    fn substitute_nulls_preserves_non_placeholder() {
        assert_eq!(substitute_nulls("SELECT $foo"), "SELECT $foo");
    }
}
