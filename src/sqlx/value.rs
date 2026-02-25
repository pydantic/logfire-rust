//! Value types for Logfire query results.

use sqlx_core::value::{Value, ValueRef};

use crate::sqlx::{Logfire, LogfireTypeInfo};

/// An owned value from a Logfire query result.
#[derive(Clone, Debug)]
pub struct LogfireValue(pub(crate) serde_json::Value);

impl Value for LogfireValue {
    type Database = Logfire;

    fn as_ref(&self) -> LogfireValueRef<'_> {
        LogfireValueRef(&self.0)
    }

    fn type_info(&self) -> std::borrow::Cow<'_, LogfireTypeInfo> {
        std::borrow::Cow::Owned(LogfireTypeInfo::from_json(&self.0))
    }

    fn is_null(&self) -> bool {
        self.0.is_null()
    }
}

/// A reference to a value from a Logfire query result.
#[derive(Clone, Debug)]
pub struct LogfireValueRef<'r>(pub(crate) &'r serde_json::Value);

impl<'r> LogfireValueRef<'r> {
    /// Returns `true` if the value is null.
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }
}

impl<'r> ValueRef<'r> for LogfireValueRef<'r> {
    type Database = Logfire;

    fn to_owned(&self) -> LogfireValue {
        LogfireValue(self.0.clone())
    }

    fn type_info(&self) -> std::borrow::Cow<'_, LogfireTypeInfo> {
        std::borrow::Cow::Owned(LogfireTypeInfo::from_json(self.0))
    }

    fn is_null(&self) -> bool {
        self.0.is_null()
    }
}
