//! Connection and connect options for Logfire.

use std::{str::FromStr, time::Duration};

use futures_core::future::BoxFuture;
use sqlx_core::connection::Connection;

use crate::{builder::LogfireClientBuilder, client::LogfireClient, sqlx::Logfire};

/// Options for connecting to Logfire.
#[derive(Clone, Debug)]
pub struct LogfireConnectOptions {
    /// Authentication token.
    pub(crate) token: String,
    /// Optional base URL override.
    pub(crate) base_url: Option<String>,
    /// Request timeout.
    pub(crate) timeout: Duration,
}

impl LogfireConnectOptions {
    /// Creates connect options from a builder.
    pub fn from_builder(builder: &LogfireClientBuilder) -> Result<Self, sqlx_core::Error> {
        let token = builder
            .token
            .clone()
            .ok_or_else(|| sqlx_core::Error::Configuration("missing token".into()))?;
        Ok(Self {
            token,
            base_url: builder.base_url.clone(),
            timeout: builder.timeout,
        })
    }
}

impl FromStr for LogfireConnectOptions {
    type Err = sqlx_core::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let url = url::Url::parse(s)
            .map_err(|e| sqlx_core::Error::Configuration(format!("invalid URL: {e}").into()))?;

        if url.scheme() != "logfire" {
            return Err(sqlx_core::Error::Configuration(
                format!("expected logfire:// scheme, got {}://", url.scheme()).into(),
            ));
        }

        let token = url.username().to_string();
        if token.is_empty() {
            return Err(sqlx_core::Error::Configuration(
                "missing token in URL (use logfire://TOKEN@host/)".into(),
            ));
        }

        let base_url = url.host_str().map(|host| {
            let region = match host {
                "us" => "https://logfire-us.pydantic.dev",
                "eu" => "https://logfire-eu.pydantic.dev",
                other => return format!("https://{other}"),
            };
            region.to_string()
        });

        Ok(Self {
            token,
            base_url,
            timeout: Duration::from_secs(30),
        })
    }
}

impl sqlx_core::connection::ConnectOptions for LogfireConnectOptions {
    type Connection = LogfireConnection;

    fn from_url(url: &url::Url) -> Result<Self, sqlx_core::Error> {
        url.as_str().parse()
    }

    fn connect(&self) -> BoxFuture<'_, Result<Self::Connection, sqlx_core::Error>> {
        Box::pin(async move {
            let mut builder = LogfireClientBuilder::new();
            builder.token(&self.token).timeout(self.timeout);
            if let Some(ref url) = self.base_url {
                builder.base_url(url);
            }
            let client = builder
                .build_client()
                .map_err(|e| sqlx_core::Error::Configuration(e.into()))?;
            Ok(LogfireConnection { client })
        })
    }

    fn log_statements(self, _level: log::LevelFilter) -> Self {
        self
    }

    fn log_slow_statements(self, _level: log::LevelFilter, _duration: Duration) -> Self {
        self
    }
}

/// A connection to Logfire.
#[derive(Debug)]
pub struct LogfireConnection {
    /// The underlying HTTP client.
    pub(crate) client: LogfireClient,
}

impl Connection for LogfireConnection {
    type Database = Logfire;
    type Options = LogfireConnectOptions;

    fn close(self) -> BoxFuture<'static, Result<(), sqlx_core::Error>> {
        Box::pin(async { Ok(()) })
    }

    fn close_hard(self) -> BoxFuture<'static, Result<(), sqlx_core::Error>> {
        Box::pin(async { Ok(()) })
    }

    fn ping(&mut self) -> BoxFuture<'_, Result<(), sqlx_core::Error>> {
        Box::pin(async {
            self.client.read_token_info().await?;
            Ok(())
        })
    }

    fn begin(
        &mut self,
    ) -> BoxFuture<
        '_,
        Result<sqlx_core::transaction::Transaction<'_, Self::Database>, sqlx_core::Error>,
    >
    where
        Self: Sized,
    {
        sqlx_core::transaction::Transaction::begin(self, None)
    }

    fn shrink_buffers(&mut self) {}

    fn flush(&mut self) -> BoxFuture<'_, Result<(), sqlx_core::Error>> {
        Box::pin(async { Ok(()) })
    }

    fn should_flush(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::LogfireConnectOptions;

    #[test]
    fn parse_us_region() {
        let opts: LogfireConnectOptions = "logfire://mytoken@us/".parse().unwrap();
        assert_eq!(opts.token, "mytoken");
        assert_eq!(
            opts.base_url.as_deref(),
            Some("https://logfire-us.pydantic.dev")
        );
    }

    #[test]
    fn parse_eu_region() {
        let opts: LogfireConnectOptions = "logfire://mytoken@eu/".parse().unwrap();
        assert_eq!(opts.token, "mytoken");
        assert_eq!(
            opts.base_url.as_deref(),
            Some("https://logfire-eu.pydantic.dev")
        );
    }

    #[test]
    fn parse_custom_host() {
        let opts: LogfireConnectOptions = "logfire://mytoken@custom.example.com/".parse().unwrap();
        assert_eq!(opts.token, "mytoken");
        assert_eq!(opts.base_url.as_deref(), Some("https://custom.example.com"));
    }

    #[test]
    fn parse_missing_token() {
        let result: Result<LogfireConnectOptions, _> = "logfire://us/".parse();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("missing token"), "error: {err}");
    }

    #[test]
    fn parse_wrong_scheme() {
        let result: Result<LogfireConnectOptions, _> = "postgres://mytoken@us/".parse();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("logfire://"), "error: {err}");
    }

    #[test]
    fn parse_invalid_url() {
        let result: Result<LogfireConnectOptions, _> = "not a valid url".parse();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("invalid URL"), "error: {err}");
    }
}
