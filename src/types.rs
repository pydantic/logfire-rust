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

/// Information about the read token used for authentication.
#[derive(Clone, Debug, Deserialize)]
pub struct ReadTokenInfo {
    /// Unique identifier for this token.
    pub token_id: String,
    /// Organization this token belongs to.
    pub organization_id: String,
    /// Subscription plan for the organization.
    pub organization_subscription_plan: String,
    /// Project this token has access to.
    pub project_id: String,
    /// Human-readable organization name.
    pub organization_name: String,
    /// Human-readable project name.
    pub project_name: String,
}

/// Schema column metadata from the schemas endpoint.
#[derive(Clone, Debug, Deserialize)]
pub struct SchemaColumn {
    /// SQL data type.
    pub data_type: String,
    /// Whether the column allows null values.
    pub nullable: bool,
    /// Whether the column contains JSON data.
    pub is_json: bool,
    /// Column description.
    pub description: String,
}

/// Schema information for a table in Logfire.
#[derive(Clone, Debug, Deserialize)]
pub struct TableSchema {
    /// Table name.
    pub name: String,
    /// Table description.
    pub description: String,
    /// Columns in this table, keyed by column name.
    pub schema: HashMap<String, SchemaColumn>,
}

/// Response from the schemas endpoint.
#[derive(Clone, Debug, Deserialize)]
pub struct SchemasResponse {
    /// Available tables.
    pub tables: Vec<TableSchema>,
}

/// Results from a row-based JSON query.
#[derive(Clone, Debug, Deserialize)]
pub struct RowQueryResults {
    /// Column metadata for the result set.
    pub columns: Vec<ColumnDetails>,
    /// Rows as key-value maps from column name to value.
    pub rows: Vec<HashMap<String, serde_json::Value>>,
}
