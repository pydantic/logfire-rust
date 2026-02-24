//! HTTP client for querying the Logfire API.

use reqwest::StatusCode;
use serde::de::DeserializeOwned;

use crate::types::RowQueryResults;

/// Errors that can occur when executing queries.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Failed to construct the HTTP client.
    #[error("failed to build HTTP client")]
    ClientBuild(#[source] reqwest::Error),
    /// HTTP request failed.
    #[error("HTTP request failed")]
    Request(#[source] reqwest::Error),
    /// Server returned an error response.
    #[error("query failed (HTTP {status}): {body}")]
    QueryFailed {
        /// HTTP status code.
        status: StatusCode,
        /// Response body text.
        body: String,
    },
    /// Failed to parse response JSON.
    #[error("failed to deserialize response")]
    Deserialize(#[source] reqwest::Error),
    /// Failed to convert row data to target type.
    #[error("failed to deserialize row")]
    RowDeserialize(#[source] serde_json::Error),
}

/// HTTP client for querying Logfire.
#[derive(Clone, Debug)]
pub struct LogfireClient {
    /// The underlying HTTP client.
    pub(crate) client: reqwest::Client,
    /// Base URL for API requests.
    pub(crate) base_url: String,
}

impl LogfireClient {
    /// Executes a SQL query and deserializes each row to the target type.
    pub async fn query<T: DeserializeOwned>(&self, sql: &str) -> Result<Vec<T>, ClientError> {
        let url = format!("{}/v1/query", self.base_url);
        let response = self
            .client
            .get(&url)
            .header(reqwest::header::ACCEPT, "application/json")
            .query(&[("sql", sql), ("json_rows", "true")])
            .send()
            .await
            .map_err(ClientError::Request)?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ClientError::QueryFailed { status, body });
        }

        let results: RowQueryResults = response.json().await.map_err(ClientError::Deserialize)?;

        results
            .rows
            .into_iter()
            .map(|row| {
                let map: serde_json::Map<String, serde_json::Value> = row.into_iter().collect();
                serde_json::from_value(serde_json::Value::Object(map))
            })
            .collect::<Result<Vec<T>, _>>()
            .map_err(ClientError::RowDeserialize)
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientError, LogfireClient, StatusCode};
    use crate::LogfireClientBuilder;

    #[test]
    fn query_failed_error_format() {
        let err = ClientError::QueryFailed {
            status: StatusCode::BAD_REQUEST,
            body: "invalid SQL".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("400"), "should contain status code");
        assert!(msg.contains("invalid SQL"), "should contain body");
    }

    /// Creates a client configured from environment variables for staging tests.
    fn staging_client() -> LogfireClient {
        LogfireClientBuilder::new()
            .from_env()
            .build_client()
            .expect("LOGFIRE_READ_TOKEN must be set")
    }

    /// Comprehensive connectivity and query test.
    #[tokio::test]
    #[ignore]
    async fn query_happy_path() {
        let client = staging_client();

        #[derive(Debug, serde::Deserialize)]
        struct One {
            one: i64,
        }
        let rows: Vec<One> = client
            .query("SELECT 1 AS one")
            .await
            .expect("basic query failed");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].one, 1);

        #[derive(Debug, serde::Deserialize)]
        struct Record {
            #[serde(rename = "start_timestamp")]
            _start_timestamp: String,
        }
        let rows: Vec<Record> = client
            .query(
                "SELECT start_timestamp FROM records \
                 WHERE start_timestamp > now() - INTERVAL '1 hour' LIMIT 1",
            )
            .await
            .expect("records query failed");
        assert!(rows.len() <= 1);

        #[derive(Debug, serde::Deserialize)]
        struct Metric {
            #[serde(rename = "recorded_timestamp")]
            _recorded_timestamp: String,
        }
        let rows: Vec<Metric> = client
            .query(
                "SELECT recorded_timestamp FROM metrics \
                 WHERE recorded_timestamp > now() - INTERVAL '1 hour' LIMIT 1",
            )
            .await
            .expect("metrics query failed");
        assert!(rows.len() <= 1);

        let rows: Vec<One> = client
            .query("SELECT 1 AS one WHERE false")
            .await
            .expect("empty result query failed");
        assert!(rows.is_empty());
    }

    /// Invalid SQL returns `QueryFailed`.
    #[tokio::test]
    #[ignore]
    async fn query_invalid_sql() {
        let client = staging_client();
        let result: Result<Vec<serde_json::Value>, _> =
            client.query("SELECT * FROM nonexistent_table_xyz").await;
        assert!(
            matches!(result, Err(ClientError::QueryFailed { .. })),
            "expected QueryFailed, got {result:?}"
        );
    }

    /// Type mismatch returns `RowDeserialize`.
    #[tokio::test]
    #[ignore]
    async fn query_type_mismatch() {
        let client = staging_client();
        #[derive(Debug, serde::Deserialize)]
        struct WrongType {
            #[serde(rename = "one")]
            _one: Vec<String>,
        }
        let result: Result<Vec<WrongType>, _> = client.query("SELECT 1 AS one").await;
        assert!(
            matches!(result, Err(ClientError::RowDeserialize(_))),
            "expected RowDeserialize, got {result:?}"
        );
    }
}
