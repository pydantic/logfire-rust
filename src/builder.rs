//! Builder for creating Logfire clients.

use std::time::Duration;

use reqwest::header;

use crate::{
    client::{ClientError, LogfireClient},
    token::Region,
};

const LOGFIRE_READ_TOKEN_ENV: &str = "LOGFIRE_READ_TOKEN";
const LOGFIRE_BASE_URL_ENV: &str = "LOGFIRE_BASE_URL";

/// Errors that can occur when building a Logfire client.
#[derive(Debug, thiserror::Error)]
pub enum BuilderError {
    /// Failed to construct the HTTP client.
    #[error("failed to build client")]
    Client(#[source] ClientError),
    /// Token contains invalid characters for HTTP headers.
    #[error("token contains invalid header characters")]
    InvalidToken,
    /// No token was configured on the builder.
    #[error("no token configured; call token() or use from_env()")]
    MissingToken,
}

/// Default request timeout (30 seconds).
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Builder for creating Logfire clients.
#[derive(Clone, Debug)]
pub struct LogfireClientBuilder {
    /// The authentication token.
    pub(crate) token: Option<String>,
    /// Override for the base URL (auto-detected from token if not set).
    pub(crate) base_url: Option<String>,
    /// Request timeout.
    pub(crate) timeout: Duration,
}

impl Default for LogfireClientBuilder {
    fn default() -> Self {
        Self {
            token: None,
            base_url: None,
            timeout: DEFAULT_TIMEOUT,
        }
    }
}

impl LogfireClientBuilder {
    /// Creates a new builder with no configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads configuration from environment variables.
    ///
    /// Reads `LOGFIRE_READ_TOKEN` and `LOGFIRE_BASE_URL`.
    pub fn from_env(&mut self) -> &mut Self {
        self.token = std::env::var(LOGFIRE_READ_TOKEN_ENV).ok();
        self.base_url = std::env::var(LOGFIRE_BASE_URL_ENV).ok();
        self
    }

    /// Sets the authentication token.
    pub fn token(&mut self, token: impl Into<String>) -> &mut Self {
        self.token = Some(token.into());
        self
    }

    /// Overrides the base URL.
    pub fn base_url(&mut self, url: impl Into<String>) -> &mut Self {
        self.base_url = Some(url.into());
        self
    }

    /// Sets the request timeout.
    pub fn timeout(&mut self, timeout: Duration) -> &mut Self {
        self.timeout = timeout;
        self
    }

    /// Builds a [`LogfireClient`] from the current configuration.
    pub fn build_client(&self) -> Result<LogfireClient, BuilderError> {
        let token = self.token.as_ref().ok_or(BuilderError::MissingToken)?;
        let base_url = self.resolve_base_url()?;

        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            header::HeaderValue::from_str(token).map_err(|_| BuilderError::InvalidToken)?,
        );
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_static(concat!(
                "logfire-client-rust/",
                env!("CARGO_PKG_VERSION")
            )),
        );

        let client = reqwest::Client::builder()
            .timeout(self.timeout)
            .default_headers(headers)
            .build()
            .map_err(ClientError::ClientBuild)
            .map_err(BuilderError::Client)?;

        Ok(LogfireClient { client, base_url })
    }

    /// Resolves the effective base URL.
    fn resolve_base_url(&self) -> Result<String, BuilderError> {
        let token = self.token.as_ref().ok_or(BuilderError::MissingToken)?;
        if let Some(url) = &self.base_url {
            Ok(url.clone())
        } else {
            Ok(Region::from_token(token).base_url().to_owned())
        }
    }

    /// Builds sqlx-compatible connect options.
    #[cfg(feature = "sqlx")]
    pub fn build_sqlx_options(
        &self,
    ) -> Result<crate::sqlx::LogfireConnectOptions, sqlx_core::Error> {
        crate::sqlx::LogfireConnectOptions::from_builder(self)
    }

    /// Builds a sqlx-compatible connection.
    #[cfg(feature = "sqlx")]
    pub async fn build_sqlx_connection(
        &self,
    ) -> Result<crate::sqlx::LogfireConnection, sqlx_core::Error> {
        use sqlx_core::connection::ConnectOptions;
        self.build_sqlx_options()?.connect().await
    }
}

#[cfg(test)]
mod tests {
    use super::{BuilderError, LogfireClientBuilder};
    use crate::token::Region;

    #[test]
    fn token_with_region_detection() {
        let mut builder = LogfireClientBuilder::new();
        builder.token("pylf_v1_eu_abc123");
        assert_eq!(builder.resolve_base_url().unwrap(), Region::EU_URL);
    }

    #[test]
    fn base_url_override() {
        let mut builder = LogfireClientBuilder::new();
        builder
            .token("pylf_v1_us_abc123")
            .base_url("https://custom.example.com");
        assert_eq!(
            builder.resolve_base_url().unwrap(),
            "https://custom.example.com"
        );
    }

    #[test]
    fn missing_token_error() {
        let builder = LogfireClientBuilder::new();
        assert!(matches!(
            builder.resolve_base_url(),
            Err(BuilderError::MissingToken)
        ));
    }

    #[test]
    fn build_client_requires_token() {
        let builder = LogfireClientBuilder::new();
        assert!(matches!(
            builder.build_client(),
            Err(BuilderError::MissingToken)
        ));
    }

    #[test]
    fn build_client_with_token() {
        let mut builder = LogfireClientBuilder::new();
        builder.token("pylf_v1_us_abc123");
        assert!(builder.build_client().is_ok());
    }

    #[test]
    fn build_client_rejects_invalid_token() {
        let mut builder = LogfireClientBuilder::new();
        builder.token("token\nwith\nnewlines");
        assert!(matches!(
            builder.build_client(),
            Err(BuilderError::InvalidToken)
        ));
    }
}
