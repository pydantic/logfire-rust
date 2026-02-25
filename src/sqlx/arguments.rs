//! Arguments for Logfire queries.
//!
//! Logfire's API accepts raw SQL strings, so this module implements client-side SQL
//! interpolation: `$1`, `$2`, etc. placeholders are replaced with properly escaped
//! literal values before sending.

use sqlx_core::{arguments::Arguments, encode::Encode, error::BoxDynError};

use crate::sqlx::Logfire;

/// Buffer for storing encoded SQL literal values.
///
/// Values are stored as SQL literal strings (e.g., `'text'`, `42`, `TRUE`) ready for
/// direct interpolation into SQL.
#[derive(Clone, Debug, Default)]
pub struct LogfireArgumentBuffer {
    /// Encoded SQL literals in order.
    values: Vec<String>,
}

impl LogfireArgumentBuffer {
    /// Pushes an encoded SQL literal value.
    pub fn push(&mut self, value: String) {
        self.values.push(value);
    }

    /// Pushes a SQL `NULL` literal.
    pub fn push_null(&mut self) {
        self.values.push("NULL".to_string());
    }

    /// Returns the number of stored values.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the stored values as a slice.
    pub fn values(&self) -> &[String] {
        &self.values
    }
}

/// Arguments for a Logfire query.
///
/// Stores encoded SQL literals and provides interpolation into SQL templates.
#[derive(Clone, Debug, Default)]
pub struct LogfireArguments {
    /// Buffer of encoded values.
    buffer: LogfireArgumentBuffer,
}

impl LogfireArguments {
    /// Interpolates stored arguments into a SQL template.
    ///
    /// Replaces `$N` placeholders with the corresponding encoded values. Uses a
    /// single-pass approach to avoid incorrectly replacing placeholder patterns
    /// that appear within interpolated values.
    pub fn interpolate(&self, sql: &str) -> String {
        let values = self.buffer.values();
        if values.is_empty() {
            return sql.to_string();
        }

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

                if end > start {
                    if let Ok(idx) = sql[start..end].parse::<usize>() {
                        if idx > 0 && idx <= values.len() {
                            result.push_str(&values[idx - 1]);
                            continue;
                        }
                    }
                    result.push('$');
                    result.push_str(&sql[start..end]);
                } else {
                    result.push('$');
                }
            } else {
                result.push(c);
            }
        }
        result
    }
}

impl<'q> Arguments<'q> for LogfireArguments {
    type Database = Logfire;

    fn reserve(&mut self, additional: usize, _size: usize) {
        self.buffer.values.reserve(additional);
    }

    fn add<T: Encode<'q, Logfire>>(&mut self, value: T) -> Result<(), BoxDynError> {
        let is_null = value.encode(&mut self.buffer)?;
        if matches!(is_null, sqlx_core::encode::IsNull::Yes) {
            self.buffer.push_null();
        }
        Ok(())
    }

    fn len(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolate_single_placeholder() {
        let mut args = LogfireArguments::default();
        args.buffer.push("42".to_string());
        assert_eq!(
            args.interpolate("SELECT * FROM t WHERE id = $1"),
            "SELECT * FROM t WHERE id = 42"
        );
    }

    #[test]
    fn interpolate_multiple_placeholders() {
        let mut args = LogfireArguments::default();
        args.buffer.push("'alice'".to_string());
        args.buffer.push("30".to_string());
        assert_eq!(
            args.interpolate("SELECT * FROM users WHERE name = $1 AND age > $2"),
            "SELECT * FROM users WHERE name = 'alice' AND age > 30"
        );
    }

    #[test]
    fn interpolate_double_digit_placeholders() {
        let mut args = LogfireArguments::default();
        for i in 1..=12 {
            args.buffer.push(i.to_string());
        }
        let sql = "SELECT $1, $2, $10, $11, $12";
        let result = args.interpolate(sql);
        assert_eq!(result, "SELECT 1, 2, 10, 11, 12");
    }

    #[test]
    fn interpolate_no_placeholders() {
        let args = LogfireArguments::default();
        assert_eq!(args.interpolate("SELECT 1"), "SELECT 1");
    }

    #[test]
    fn interpolate_value_containing_placeholder_pattern() {
        let mut args = LogfireArguments::default();
        args.buffer.push("'foo'".to_string());
        args.buffer.push("'$1'".to_string());
        let result = args.interpolate("SELECT $1, $2");
        assert_eq!(result, "SELECT 'foo', '$1'");
    }

    #[test]
    fn interpolate_preserves_unmatched_placeholder() {
        let mut args = LogfireArguments::default();
        args.buffer.push("42".to_string());
        assert_eq!(args.interpolate("SELECT $1, $2"), "SELECT 42, $2");
    }
}
