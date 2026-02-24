//! Builder for creating Logfire clients.

use crate::Region;

const LOGFIRE_READ_TOKEN_ENV: &str = "LOGFIRE_READ_TOKEN";
const LOGFIRE_BASE_URL_ENV: &str = "LOGFIRE_BASE_URL";

/// Errors that can occur when building a Logfire client.
#[derive(Debug, thiserror::Error)]
pub enum BuilderError {
    /// No token was configured on the builder.
    #[error("no token configured; call token() or use from_env()")]
    MissingToken,
}

/// Builder for creating Logfire clients.
#[derive(Clone, Debug, Default)]
pub struct LogfireClientBuilder {
    /// The authentication token.
    token: Option<String>,
    /// Override for the base URL (auto-detected from token if not set).
    base_url: Option<String>,
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

    /// Resolves the effective base URL.
    #[allow(dead_code)] // Used by build methods in later steps.
    fn resolve_base_url(&self) -> Result<String, BuilderError> {
        let token = self.token.as_ref().ok_or(BuilderError::MissingToken)?;
        if let Some(url) = &self.base_url {
            Ok(url.clone())
        } else {
            Ok(Region::from_token(token).base_url().to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
