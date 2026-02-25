//! Statement type for Logfire queries.

use std::{borrow::Cow, sync::Arc};

use sqlx_core::{
    Either, from_row::FromRow, query::Query, query_as::QueryAs, query_scalar::QueryScalar,
    statement::Statement,
};

use crate::sqlx::{Logfire, LogfireArguments, LogfireColumn, LogfireTypeInfo};

/// A prepared statement for Logfire.
#[derive(Clone, Debug)]
pub struct LogfireStatement<'q> {
    /// The SQL query string.
    pub(crate) sql: Cow<'q, str>,
    /// Column metadata (populated after first execution).
    pub(crate) columns: Arc<Vec<LogfireColumn>>,
    /// Parameter type info (empty since Logfire doesn't support bind parameters).
    pub(crate) parameters: Vec<LogfireTypeInfo>,
}

impl<'q> Statement<'q> for LogfireStatement<'q> {
    type Database = Logfire;

    fn to_owned(&self) -> LogfireStatement<'static> {
        LogfireStatement {
            sql: Cow::Owned(self.sql.clone().into_owned()),
            columns: Arc::clone(&self.columns),
            parameters: self.parameters.clone(),
        }
    }

    fn sql(&self) -> &str {
        &self.sql
    }

    fn parameters(&self) -> Option<Either<&[LogfireTypeInfo], usize>> {
        Some(Either::Right(0))
    }

    fn columns(&self) -> &[LogfireColumn] {
        &self.columns
    }

    #[inline]
    fn query(&self) -> Query<'_, Self::Database, LogfireArguments> {
        sqlx_core::query::query_statement(self)
    }

    #[inline]
    fn query_with<'s, A>(&'s self, arguments: A) -> Query<'s, Self::Database, A>
    where
        A: sqlx_core::arguments::IntoArguments<'s, Self::Database>,
    {
        sqlx_core::query::query_statement_with(self, arguments)
    }

    #[inline]
    fn query_as<O>(&self) -> QueryAs<'_, Self::Database, O, LogfireArguments>
    where
        O: for<'r> FromRow<'r, <Self::Database as sqlx_core::database::Database>::Row>,
    {
        sqlx_core::query_as::query_statement_as(self)
    }

    #[inline]
    fn query_as_with<'s, O, A>(&'s self, arguments: A) -> QueryAs<'s, Self::Database, O, A>
    where
        O: for<'r> FromRow<'r, <Self::Database as sqlx_core::database::Database>::Row>,
        A: sqlx_core::arguments::IntoArguments<'s, Self::Database>,
    {
        sqlx_core::query_as::query_statement_as_with(self, arguments)
    }

    #[inline]
    fn query_scalar<O>(&self) -> QueryScalar<'_, Self::Database, O, LogfireArguments>
    where
        (O,): for<'r> FromRow<'r, <Self::Database as sqlx_core::database::Database>::Row>,
    {
        sqlx_core::query_scalar::query_statement_scalar(self)
    }

    #[inline]
    fn query_scalar_with<'s, O, A>(&'s self, arguments: A) -> QueryScalar<'s, Self::Database, O, A>
    where
        (O,): for<'r> FromRow<'r, <Self::Database as sqlx_core::database::Database>::Row>,
        A: sqlx_core::arguments::IntoArguments<'s, Self::Database>,
    {
        sqlx_core::query_scalar::query_statement_scalar_with(self, arguments)
    }
}
