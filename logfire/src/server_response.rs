//! Handling of responses received from the Logfire API.
//!
//! The Logfire backend can attach out-of-band information to the responses of API calls, which
//! this SDK surfaces to the user. Currently the only such signal is a warning header, which is
//! printed to stderr by default (deduplicated, so a chatty server only warns once per process).
//!
//! Use [`AdvancedOptions::with_server_response_hook`][crate::config::AdvancedOptions::with_server_response_hook]
//! to customize or disable this behaviour.
//!
//! # Note
//!
//! Responses are only inspected for exports over HTTP (the default protocol); the `export-grpc`
//! feature has no equivalent support.

use std::sync::Arc;

use logfire_core::server_response::WARNING_HEADER_NAME;

/// A response received from the Logfire API, passed to the
/// [server response hook][crate::config::AdvancedOptions::with_server_response_hook].
///
/// This is experimental and may change in a future release.
#[derive(Debug)]
pub struct ServerResponse<'a> {
    status: http::StatusCode,
    headers: &'a http::HeaderMap,
}

impl<'a> ServerResponse<'a> {
    #[cfg_attr(
        not(any(feature = "export-http-protobuf", feature = "export-http-json")),
        allow(dead_code, reason = "only used by the HTTP exporters")
    )]
    pub(crate) fn new(status: http::StatusCode, headers: &'a http::HeaderMap) -> Self {
        Self { status, headers }
    }

    /// The status code of the response.
    #[must_use]
    pub fn status(&self) -> http::StatusCode {
        self.status
    }

    /// The headers of the response.
    #[must_use]
    pub fn headers(&self) -> &'a http::HeaderMap {
        self.headers
    }

    /// The warning which the Logfire backend sent with this response, if any.
    ///
    /// Returns `None` if the header is absent or is not valid ASCII.
    #[must_use]
    pub fn warning(&self) -> Option<&'a str> {
        self.headers
            .get(WARNING_HEADER_NAME)
            .and_then(|value| value.to_str().ok())
    }

    /// The default handling of a response: print any [`warning`][Self::warning] to stderr.
    ///
    /// Call this from a custom hook to keep the default behaviour alongside your own.
    pub fn default_hook(&self) {
        if let Some(warning) = self.warning() {
            logfire_core::server_response::emit_warning(warning);
        }
    }
}

/// A hook called for every response which the SDK receives from the Logfire API.
///
/// See [`AdvancedOptions::with_server_response_hook`][crate::config::AdvancedOptions::with_server_response_hook].
pub type ServerResponseHook = Arc<dyn Fn(&ServerResponse<'_>) + Send + Sync>;

/// Run the configured hook for a response, or the default behaviour if no hook is configured.
#[cfg_attr(
    not(any(feature = "export-http-protobuf", feature = "export-http-json")),
    allow(dead_code, reason = "only used by the HTTP exporters")
)]
pub(crate) fn handle_response(hook: Option<&ServerResponseHook>, response: &ServerResponse<'_>) {
    match hook {
        Some(hook) => hook(response),
        None => response.default_hook(),
    }
}

/// An [`HttpClient`][opentelemetry_http::HttpClient] which passes every response from the Logfire
/// API to the configured hook before handing it back to the OTLP exporter.
#[cfg(any(feature = "export-http-protobuf", feature = "export-http-json"))]
pub(crate) struct LogfireHttpClient {
    client: reqwest::Client,
    hook: Option<ServerResponseHook>,
}

#[cfg(any(feature = "export-http-protobuf", feature = "export-http-json"))]
impl LogfireHttpClient {
    pub(crate) fn new(client: reqwest::Client, hook: Option<ServerResponseHook>) -> Self {
        Self { client, hook }
    }
}

#[cfg(any(feature = "export-http-protobuf", feature = "export-http-json"))]
impl std::fmt::Debug for LogfireHttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogfireHttpClient")
            .field("client", &self.client)
            .field("hook", &self.hook.as_ref().map(|_| "<hook>"))
            .finish()
    }
}

#[cfg(any(feature = "export-http-protobuf", feature = "export-http-json"))]
#[async_trait::async_trait]
impl opentelemetry_http::HttpClient for LogfireHttpClient {
    async fn send_bytes(
        &self,
        request: opentelemetry_http::Request<opentelemetry_http::Bytes>,
    ) -> Result<
        opentelemetry_http::Response<opentelemetry_http::Bytes>,
        opentelemetry_http::HttpError,
    > {
        // this mirrors the `HttpClient` implementation for `reqwest::Client` in
        // `opentelemetry-http`, with the response passed to the hook before the status is checked
        // (so that warnings sent alongside an error response are not lost).
        let request = request.try_into()?;
        let response = self.client.execute(request).await?;

        handle_response(
            self.hook.as_ref(),
            &ServerResponse::new(response.status(), response.headers()),
        );

        let mut response = response.error_for_status()?;
        let headers = std::mem::take(response.headers_mut());
        let mut http_response = opentelemetry_http::Response::builder()
            .status(response.status())
            .body(response.bytes().await?)?;
        *http_response.headers_mut() = headers;

        Ok(http_response)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{ServerResponse, ServerResponseHook, handle_response};

    fn header_map(pairs: &[(&str, &str)]) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(
                http::HeaderName::try_from(*name).expect("invalid header name"),
                http::HeaderValue::try_from(*value).expect("invalid header value"),
            );
        }
        headers
    }

    #[test]
    fn warning_header_is_read() {
        let headers = header_map(&[("x-logfire-warning", "endpoint is deprecated")]);
        let response = ServerResponse::new(http::StatusCode::OK, &headers);
        assert_eq!(response.warning(), Some("endpoint is deprecated"));
        assert_eq!(response.status(), http::StatusCode::OK);

        let headers = header_map(&[("x-other-header", "not a warning")]);
        assert_eq!(
            ServerResponse::new(http::StatusCode::OK, &headers).warning(),
            None
        );
    }

    #[test]
    fn hook_replaces_default_behaviour() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let hook: ServerResponseHook = {
            let seen = seen.clone();
            Arc::new(move |response: &ServerResponse<'_>| {
                seen.lock()
                    .expect("poisoned")
                    .push(response.warning().map(ToOwned::to_owned));
            })
        };

        let headers = header_map(&[("x-logfire-warning", "hook_replaces_default_behaviour")]);
        handle_response(
            Some(&hook),
            &ServerResponse::new(http::StatusCode::OK, &headers),
        );

        assert_eq!(
            *seen.lock().expect("poisoned"),
            vec![Some("hook_replaces_default_behaviour".to_string())]
        );
    }
}
