//! Response types for Logfire API queries.

use std::collections::HashMap;

use serde::Deserialize;

/// Metadata about a column in query results.
#[derive(Clone, Debug, Deserialize)]
pub struct ColumnDetails {
    /// Column name.
    pub name: String,
    /// Data type information (kept as JSON since the format varies by backend).
    #[serde(rename = "datatype")]
    pub data_type: serde_json::Value,
    /// Whether the column allows null values.
    pub nullable: bool,
}

/// Results from a row-based JSON query.
#[derive(Clone, Debug, Deserialize)]
pub struct RowQueryResults {
    /// Column metadata for the result set.
    pub columns: Vec<ColumnDetails>,
    /// Rows as key-value maps from column name to value.
    pub rows: Vec<HashMap<String, serde_json::Value>>,
}
