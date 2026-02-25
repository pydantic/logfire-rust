//! Type and Decode implementations for Rust types.

use sqlx_core::{decode::Decode, error::BoxDynError, types::Type};

use crate::sqlx::{Logfire, LogfireTypeInfo, LogfireValueRef};

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
}
