//! Type, Decode, and Encode implementations for Rust types.

use sqlx_core::{
    decode::Decode,
    encode::{Encode, IsNull},
    error::BoxDynError,
    types::Type,
};

use crate::sqlx::{Logfire, LogfireTypeInfo, LogfireValueRef, arguments::LogfireArgumentBuffer};

/// Escapes a string for use as a SQL string literal.
///
/// Wraps in single quotes and escapes embedded single quotes by doubling them.
fn escape_sql_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

impl Type<Logfire> for String {
    fn type_info() -> LogfireTypeInfo {
        LogfireTypeInfo::Text
    }

    fn compatible(ty: &LogfireTypeInfo) -> bool {
        matches!(ty, LogfireTypeInfo::Text | LogfireTypeInfo::Json)
    }
}

impl Decode<'_, Logfire> for String {
    fn decode(value: LogfireValueRef<'_>) -> Result<Self, BoxDynError> {
        match value.0 {
            serde_json::Value::String(s) => Ok(s.clone()),
            serde_json::Value::Null => Err("unexpected null".into()),
            v => Err(format!("expected string, got {v:?}").into()),
        }
    }
}

impl Encode<'_, Logfire> for String {
    fn encode_by_ref(&self, buf: &mut LogfireArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.push(escape_sql_string(self));
        Ok(IsNull::No)
    }
}

impl Encode<'_, Logfire> for &str {
    fn encode_by_ref(&self, buf: &mut LogfireArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.push(escape_sql_string(self));
        Ok(IsNull::No)
    }
}

impl Type<Logfire> for i32 {
    fn type_info() -> LogfireTypeInfo {
        LogfireTypeInfo::Integer
    }

    fn compatible(ty: &LogfireTypeInfo) -> bool {
        matches!(
            ty,
            LogfireTypeInfo::Integer | LogfireTypeInfo::BigInt | LogfireTypeInfo::Json
        )
    }
}

impl Decode<'_, Logfire> for i32 {
    fn decode(value: LogfireValueRef<'_>) -> Result<Self, BoxDynError> {
        match value.0 {
            serde_json::Value::Number(n) => n
                .as_i64()
                .and_then(|v| i32::try_from(v).ok())
                .ok_or_else(|| format!("number {n} out of range for i32").into()),
            serde_json::Value::Null => Err("unexpected null".into()),
            v => Err(format!("expected number, got {v:?}").into()),
        }
    }
}

impl Encode<'_, Logfire> for i32 {
    fn encode_by_ref(&self, buf: &mut LogfireArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.push(self.to_string());
        Ok(IsNull::No)
    }
}

impl Type<Logfire> for i64 {
    fn type_info() -> LogfireTypeInfo {
        LogfireTypeInfo::BigInt
    }

    fn compatible(ty: &LogfireTypeInfo) -> bool {
        matches!(
            ty,
            LogfireTypeInfo::Integer | LogfireTypeInfo::BigInt | LogfireTypeInfo::Json
        )
    }
}

impl Decode<'_, Logfire> for i64 {
    fn decode(value: LogfireValueRef<'_>) -> Result<Self, BoxDynError> {
        match value.0 {
            serde_json::Value::Number(n) => n
                .as_i64()
                .ok_or_else(|| format!("number {n} is not i64").into()),
            serde_json::Value::Null => Err("unexpected null".into()),
            v => Err(format!("expected number, got {v:?}").into()),
        }
    }
}

impl Encode<'_, Logfire> for i64 {
    fn encode_by_ref(&self, buf: &mut LogfireArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.push(self.to_string());
        Ok(IsNull::No)
    }
}

impl Type<Logfire> for f64 {
    fn type_info() -> LogfireTypeInfo {
        LogfireTypeInfo::Double
    }

    fn compatible(ty: &LogfireTypeInfo) -> bool {
        matches!(
            ty,
            LogfireTypeInfo::Double
                | LogfireTypeInfo::Integer
                | LogfireTypeInfo::BigInt
                | LogfireTypeInfo::Json
        )
    }
}

impl Decode<'_, Logfire> for f64 {
    fn decode(value: LogfireValueRef<'_>) -> Result<Self, BoxDynError> {
        match value.0 {
            serde_json::Value::Number(n) => n
                .as_f64()
                .ok_or_else(|| format!("number {n} is not f64").into()),
            serde_json::Value::Null => Err("unexpected null".into()),
            v => Err(format!("expected number, got {v:?}").into()),
        }
    }
}

impl Encode<'_, Logfire> for f64 {
    fn encode_by_ref(&self, buf: &mut LogfireArgumentBuffer) -> Result<IsNull, BoxDynError> {
        let literal = if self.is_nan() {
            "'NaN'::float8".to_string()
        } else if self.is_infinite() {
            if self.is_sign_positive() {
                "'Infinity'::float8".to_string()
            } else {
                "'-Infinity'::float8".to_string()
            }
        } else {
            self.to_string()
        };
        buf.push(literal);
        Ok(IsNull::No)
    }
}

impl Type<Logfire> for bool {
    fn type_info() -> LogfireTypeInfo {
        LogfireTypeInfo::Boolean
    }

    fn compatible(ty: &LogfireTypeInfo) -> bool {
        matches!(ty, LogfireTypeInfo::Boolean | LogfireTypeInfo::Json)
    }
}

impl Decode<'_, Logfire> for bool {
    fn decode(value: LogfireValueRef<'_>) -> Result<Self, BoxDynError> {
        match value.0 {
            serde_json::Value::Bool(b) => Ok(*b),
            serde_json::Value::Null => Err("unexpected null".into()),
            v => Err(format!("expected boolean, got {v:?}").into()),
        }
    }
}

impl Encode<'_, Logfire> for bool {
    fn encode_by_ref(&self, buf: &mut LogfireArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.push(if *self { "TRUE" } else { "FALSE" }.to_string());
        Ok(IsNull::No)
    }
}

impl Type<Logfire> for chrono::DateTime<chrono::Utc> {
    fn type_info() -> LogfireTypeInfo {
        LogfireTypeInfo::TimestampTz
    }

    fn compatible(ty: &LogfireTypeInfo) -> bool {
        matches!(
            ty,
            LogfireTypeInfo::TimestampTz | LogfireTypeInfo::Text | LogfireTypeInfo::Json
        )
    }
}

impl Decode<'_, Logfire> for chrono::DateTime<chrono::Utc> {
    fn decode(value: LogfireValueRef<'_>) -> Result<Self, BoxDynError> {
        match value.0 {
            serde_json::Value::String(s) => chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| format!("failed to parse timestamp '{s}': {e}").into()),
            serde_json::Value::Null => Err("unexpected null".into()),
            v => Err(format!("expected timestamp string, got {v:?}").into()),
        }
    }
}

impl Encode<'_, Logfire> for chrono::DateTime<chrono::Utc> {
    fn encode_by_ref(&self, buf: &mut LogfireArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.push(format!("TIMESTAMP WITH TIME ZONE '{}'", self.to_rfc3339()));
        Ok(IsNull::No)
    }
}

impl Type<Logfire> for chrono::NaiveDate {
    fn type_info() -> LogfireTypeInfo {
        LogfireTypeInfo::Date
    }

    fn compatible(ty: &LogfireTypeInfo) -> bool {
        matches!(
            ty,
            LogfireTypeInfo::Date | LogfireTypeInfo::Text | LogfireTypeInfo::Json
        )
    }
}

impl Decode<'_, Logfire> for chrono::NaiveDate {
    fn decode(value: LogfireValueRef<'_>) -> Result<Self, BoxDynError> {
        match value.0 {
            serde_json::Value::String(s) => chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|e| format!("failed to parse date '{s}': {e}").into()),
            serde_json::Value::Null => Err("unexpected null".into()),
            v => Err(format!("expected date string, got {v:?}").into()),
        }
    }
}

impl Encode<'_, Logfire> for chrono::NaiveDate {
    fn encode_by_ref(&self, buf: &mut LogfireArgumentBuffer) -> Result<IsNull, BoxDynError> {
        buf.push(format!("DATE '{}'", self.format("%Y-%m-%d")));
        Ok(IsNull::No)
    }
}

impl Type<Logfire> for Vec<String> {
    fn type_info() -> LogfireTypeInfo {
        LogfireTypeInfo::TextArray
    }

    fn compatible(ty: &LogfireTypeInfo) -> bool {
        matches!(ty, LogfireTypeInfo::TextArray | LogfireTypeInfo::Json)
    }
}

impl Decode<'_, Logfire> for Vec<String> {
    fn decode(value: LogfireValueRef<'_>) -> Result<Self, BoxDynError> {
        match value.0 {
            serde_json::Value::Array(arr) => arr
                .iter()
                .map(|v| match v {
                    serde_json::Value::String(s) => Ok(s.clone()),
                    serde_json::Value::Null => Err("unexpected null in array".into()),
                    other => Err(format!("expected string in array, got {other:?}").into()),
                })
                .collect(),
            serde_json::Value::Null => Err("unexpected null".into()),
            v => Err(format!("expected array, got {v:?}").into()),
        }
    }
}

impl Encode<'_, Logfire> for Vec<String> {
    fn encode_by_ref(&self, buf: &mut LogfireArgumentBuffer) -> Result<IsNull, BoxDynError> {
        let elements: Vec<String> = self.iter().map(|s| escape_sql_string(s)).collect();
        buf.push(format!("ARRAY[{}]", elements.join(",")));
        Ok(IsNull::No)
    }
}

impl Type<Logfire> for serde_json::Value {
    fn type_info() -> LogfireTypeInfo {
        LogfireTypeInfo::Json
    }

    fn compatible(_ty: &LogfireTypeInfo) -> bool {
        true
    }
}

impl Decode<'_, Logfire> for serde_json::Value {
    fn decode(value: LogfireValueRef<'_>) -> Result<Self, BoxDynError> {
        Ok(value.0.clone())
    }
}

impl Encode<'_, Logfire> for serde_json::Value {
    fn encode_by_ref(&self, buf: &mut LogfireArgumentBuffer) -> Result<IsNull, BoxDynError> {
        let json_str =
            serde_json::to_string(self).map_err(|e| format!("failed to serialize JSON: {e}"))?;
        buf.push(format!("{}::jsonb", escape_sql_string(&json_str)));
        Ok(IsNull::No)
    }
}

impl<'q, T> Encode<'q, Logfire> for Option<T>
where
    T: Encode<'q, Logfire>,
{
    fn encode_by_ref(&self, buf: &mut LogfireArgumentBuffer) -> Result<IsNull, BoxDynError> {
        match self {
            Some(value) => value.encode_by_ref(buf),
            None => Ok(IsNull::Yes),
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Datelike;
    use serde_json::json;

    use super::*;

    #[test]
    fn decode_string() {
        let val = json!("hello");
        let result = String::decode(LogfireValueRef(&val)).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn decode_i64() {
        let val = json!(42);
        let result = i64::decode(LogfireValueRef(&val)).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn decode_option_some() {
        let val = json!("hello");
        let result = Option::<String>::decode(LogfireValueRef(&val)).unwrap();
        assert_eq!(result, Some("hello".to_string()));
    }

    #[test]
    fn decode_option_none() {
        let val = json!(null);
        let result = Option::<String>::decode(LogfireValueRef(&val)).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn decode_datetime() {
        let val = json!("2024-01-15T10:30:00Z");
        let result = chrono::DateTime::<chrono::Utc>::decode(LogfireValueRef(&val)).unwrap();
        assert_eq!(result.year(), 2024);
        assert_eq!(result.month(), 1);
        assert_eq!(result.day(), 15);
    }

    #[test]
    fn escape_sql_string_basic() {
        assert_eq!(escape_sql_string("hello"), "'hello'");
    }

    #[test]
    fn escape_sql_string_with_quotes() {
        assert_eq!(escape_sql_string("it's"), "'it''s'");
        assert_eq!(escape_sql_string("a'b'c"), "'a''b''c'");
    }

    #[test]
    fn escape_sql_string_empty() {
        assert_eq!(escape_sql_string(""), "''");
    }

    #[test]
    fn escape_sql_string_unicode() {
        assert_eq!(escape_sql_string("日本語"), "'日本語'");
        assert_eq!(escape_sql_string("emoji: 🎉"), "'emoji: 🎉'");
    }

    #[test]
    fn escape_sql_string_backslash() {
        assert_eq!(escape_sql_string(r"a\b"), r"'a\b'");
    }

    #[test]
    fn encode_string() {
        let mut buf = LogfireArgumentBuffer::default();
        let _ = "hello".encode_by_ref(&mut buf).unwrap();
        assert_eq!(buf.values(), &["'hello'"]);
    }

    #[test]
    fn encode_i32() {
        let mut buf = LogfireArgumentBuffer::default();
        let _ = 42i32.encode_by_ref(&mut buf).unwrap();
        assert_eq!(buf.values(), &["42"]);
    }

    #[test]
    fn encode_i64() {
        let mut buf = LogfireArgumentBuffer::default();
        let _ = (-9999i64).encode_by_ref(&mut buf).unwrap();
        assert_eq!(buf.values(), &["-9999"]);
    }

    #[test]
    fn encode_f64_normal() {
        let mut buf = LogfireArgumentBuffer::default();
        let _ = 3.14f64.encode_by_ref(&mut buf).unwrap();
        assert_eq!(buf.values(), &["3.14"]);
    }

    #[test]
    fn encode_f64_special() {
        let mut buf = LogfireArgumentBuffer::default();
        let _ = f64::NAN.encode_by_ref(&mut buf).unwrap();
        assert_eq!(buf.values(), &["'NaN'::float8"]);

        let mut buf = LogfireArgumentBuffer::default();
        let _ = f64::INFINITY.encode_by_ref(&mut buf).unwrap();
        assert_eq!(buf.values(), &["'Infinity'::float8"]);

        let mut buf = LogfireArgumentBuffer::default();
        let _ = f64::NEG_INFINITY.encode_by_ref(&mut buf).unwrap();
        assert_eq!(buf.values(), &["'-Infinity'::float8"]);
    }

    #[test]
    fn encode_bool() {
        let mut buf = LogfireArgumentBuffer::default();
        let _ = true.encode_by_ref(&mut buf).unwrap();
        assert_eq!(buf.values(), &["TRUE"]);

        let mut buf = LogfireArgumentBuffer::default();
        let _ = false.encode_by_ref(&mut buf).unwrap();
        assert_eq!(buf.values(), &["FALSE"]);
    }

    #[test]
    fn encode_datetime() {
        let mut buf = LogfireArgumentBuffer::default();
        let dt = chrono::DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let _ = dt.encode_by_ref(&mut buf).unwrap();
        assert_eq!(
            buf.values(),
            &["TIMESTAMP WITH TIME ZONE '2024-01-15T10:30:00+00:00'"]
        );
    }

    #[test]
    fn encode_naive_date() {
        let mut buf = LogfireArgumentBuffer::default();
        let date = chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let _ = date.encode_by_ref(&mut buf).unwrap();
        assert_eq!(buf.values(), &["DATE '2024-01-15'"]);
    }

    #[test]
    fn encode_vec_string() {
        let mut buf = LogfireArgumentBuffer::default();
        let arr = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let _ = arr.encode_by_ref(&mut buf).unwrap();
        assert_eq!(buf.values(), &["ARRAY['a','b','c']"]);
    }

    #[test]
    fn encode_vec_string_with_quotes() {
        let mut buf = LogfireArgumentBuffer::default();
        let arr = vec!["it's".to_string()];
        let _ = arr.encode_by_ref(&mut buf).unwrap();
        assert_eq!(buf.values(), &["ARRAY['it''s']"]);
    }

    #[test]
    fn encode_json() {
        let mut buf = LogfireArgumentBuffer::default();
        let json = json!({"key": "value"});
        let _ = json.encode_by_ref(&mut buf).unwrap();
        assert_eq!(buf.values(), &["'{\"key\":\"value\"}'::jsonb"]);
    }

    #[test]
    fn encode_option_some() {
        let mut buf = LogfireArgumentBuffer::default();
        let opt: Option<i32> = Some(42);
        let is_null = opt.encode_by_ref(&mut buf).unwrap();
        assert!(matches!(is_null, IsNull::No));
        assert_eq!(buf.values(), &["42"]);
    }

    #[test]
    fn encode_option_none() {
        let mut buf = LogfireArgumentBuffer::default();
        let opt: Option<i32> = None;
        let is_null = opt.encode_by_ref(&mut buf).unwrap();
        assert!(matches!(is_null, IsNull::Yes));
        assert!(buf.is_empty());
    }
}
