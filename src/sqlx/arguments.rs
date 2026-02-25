//! Arguments for Logfire queries.
//!
//! Logfire's API does not support bind parameters, so this implementation is a no-op.

use sqlx_core::{arguments::Arguments, encode::Encode, error::BoxDynError};

use crate::sqlx::Logfire;

/// Arguments for a Logfire query.
///
/// This is a no-op implementation since Logfire does not support bind parameters.
/// Queries must be complete SQL strings.
#[derive(Clone, Debug, Default)]
pub struct LogfireArguments;

impl<'q> Arguments<'q> for LogfireArguments {
    type Database = Logfire;

    fn reserve(&mut self, _additional: usize, _size: usize) {}

    fn add<T: Encode<'q, Logfire>>(&mut self, _value: T) -> Result<(), BoxDynError> {
        Ok(())
    }

    fn len(&self) -> usize {
        0
    }
}
