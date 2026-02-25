//! Type information for Logfire columns.

use std::fmt;

/// Type information for a Logfire column, parsed from DataFusion's JSON format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LogfireTypeInfo {
    /// UTF-8 text (`{"Utf8": null}`).
    Text,
    /// 32-bit signed integer (`{"Int32": null}`).
    Integer,
    /// 64-bit signed integer (`{"Int64": null}`).
    BigInt,
    /// 64-bit floating point (`{"Float64": null}`).
    Double,
    /// Boolean (`{"Boolean": null}`).
    Boolean,
    /// Timestamp with timezone (`{"Timestamp": ["Nanosecond", "UTC"]}`).
    TimestampTz,
    /// Date without time (`{"Date32": null}`).
    Date,
    /// Array of text values (`{"List": {"Utf8": null}}`).
    TextArray,
    /// JSON data.
    Json,
    /// Null type (`{"Null": null}`).
    Null,
}

impl LogfireTypeInfo {
    /// Parses type information from DataFusion's JSON representation.
    pub fn from_json(v: &serde_json::Value) -> Self {
        match v {
            serde_json::Value::Object(map) => {
                if map.contains_key("Utf8") {
                    Self::Text
                } else if map.contains_key("Int32") {
                    Self::Integer
                } else if map.contains_key("Int64") {
                    Self::BigInt
                } else if map.contains_key("Float64") {
                    Self::Double
                } else if map.contains_key("Boolean") {
                    Self::Boolean
                } else if map.contains_key("Timestamp") {
                    Self::TimestampTz
                } else if map.contains_key("Date32") {
                    Self::Date
                } else if map.contains_key("List") {
                    // All List types mapped to TextArray; only Vec<String> is supported.
                    Self::TextArray
                } else if map.contains_key("Null") {
                    Self::Null
                } else {
                    Self::Json
                }
            }
            _ => Self::Json,
        }
    }
}

impl fmt::Display for LogfireTypeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => write!(f, "TEXT"),
            Self::Integer => write!(f, "INTEGER"),
            Self::BigInt => write!(f, "BIGINT"),
            Self::Double => write!(f, "DOUBLE"),
            Self::Boolean => write!(f, "BOOLEAN"),
            Self::TimestampTz => write!(f, "TIMESTAMPTZ"),
            Self::Date => write!(f, "DATE"),
            Self::TextArray => write!(f, "TEXT[]"),
            Self::Json => write!(f, "JSON"),
            Self::Null => write!(f, "NULL"),
        }
    }
}

impl sqlx_core::type_info::TypeInfo for LogfireTypeInfo {
    fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    fn name(&self) -> &str {
        match self {
            Self::Text => "TEXT",
            Self::Integer => "INTEGER",
            Self::BigInt => "BIGINT",
            Self::Double => "DOUBLE",
            Self::Boolean => "BOOLEAN",
            Self::TimestampTz => "TIMESTAMPTZ",
            Self::Date => "DATE",
            Self::TextArray => "TEXT[]",
            Self::Json => "JSON",
            Self::Null => "NULL",
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::LogfireTypeInfo;

    #[test]
    fn parse_utf8() {
        let info = LogfireTypeInfo::from_json(&json!({"Utf8": null}));
        assert_eq!(info, LogfireTypeInfo::Text);
    }

    #[test]
    fn parse_int64() {
        let info = LogfireTypeInfo::from_json(&json!({"Int64": null}));
        assert_eq!(info, LogfireTypeInfo::BigInt);
    }

    #[test]
    fn parse_timestamp() {
        let info = LogfireTypeInfo::from_json(&json!({"Timestamp": ["Nanosecond", "UTC"]}));
        assert_eq!(info, LogfireTypeInfo::TimestampTz);
    }

    #[test]
    fn parse_unknown_falls_back_to_json() {
        let info = LogfireTypeInfo::from_json(&json!({"Unknown": null}));
        assert_eq!(info, LogfireTypeInfo::Json);
    }
}
