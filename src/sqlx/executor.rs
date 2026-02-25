//! Executor implementation for Logfire.

use std::sync::Arc;

use either::Either;
use futures_core::{future::BoxFuture, stream::BoxStream};
use sqlx_core::{describe::Describe, executor::Executor};

use crate::sqlx::{
    Logfire, LogfireColumn, LogfireConnection, LogfireQueryResult, LogfireRow, LogfireTypeInfo,
};

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
        _sql: &'q str,
    ) -> BoxFuture<'e, Result<Describe<Self::Database>, sqlx_core::Error>>
    where
        'c: 'e,
    {
        Box::pin(async move {
            Err(sqlx_core::Error::Protocol(
                "Logfire does not support describe".to_string(),
            ))
        })
    }
}
