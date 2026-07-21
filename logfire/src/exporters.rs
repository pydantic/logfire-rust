#![cfg_attr(
    not(any(
        feature = "export-grpc",
        feature = "export-http-protobuf",
        feature = "export-http-json"
    )),
    expect(unreachable_code, unused_variables)
)]
//! Helper functions to configure exporters via OTLP.
use std::collections::HashMap;

use opentelemetry_otlp::Protocol;
use opentelemetry_sdk::{
    logs::LogExporter, metrics::exporter::PushMetricExporter, trace::SpanExporter,
};

use crate::{ConfigureError, internal::env::get_optional_env};

/// The `User-Agent` sent with OTLP exports, e.g. `logfire-rust/0.11.0`.
///
/// This identifies the Logfire SDK as the sender of the telemetry (as opposed to the resource
/// attributes, which identify what generated it). The OTLP exporter specification permits
/// exporters to add their own details to the `User-Agent` header, see
/// <https://opentelemetry.io/docs/specs/otel/protocol/exporter/#user-agent>.
#[cfg(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
))]
const USER_AGENT: &str = concat!("logfire-rust/", env!("CARGO_PKG_VERSION"));

/// Add the Logfire [`USER_AGENT`] to the headers, unless the caller set their own `User-Agent`.
///
/// Note that this replaces the `User-Agent` set by `opentelemetry-otlp` for HTTP exports (there is
/// no way to read it back to append to it); for gRPC exports `tonic` will append its own
/// `tonic/<version>` to this value.
#[cfg(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
))]
fn headers_with_user_agent(headers: Option<HashMap<String, String>>) -> HashMap<String, String> {
    let mut headers = headers.unwrap_or_default();
    if !headers
        .keys()
        .any(|name| name.eq_ignore_ascii_case("user-agent"))
    {
        headers.insert("User-Agent".to_string(), USER_AGENT.to_string());
    }
    headers
}

macro_rules! feature_required {
    ($feature_name:literal, $functionality:expr, $if_enabled:expr) => {{
        #[cfg(feature = $feature_name)]
        {
            let _ = $functionality; // to avoid unused code warning
            $if_enabled
        }

        #[cfg(not(feature = $feature_name))]
        {
            return Err(ConfigureError::LogfireFeatureRequired {
                feature_name: $feature_name,
                functionality: $functionality,
            });
        }
    }};
}

/// Build a [`SpanExporter`] for passing to
/// [`with_additional_span_processor()`][crate::LogfireConfigBuilder::with_additional_span_processor].
///
/// This uses `OTEL_EXPORTER_OTLP_PROTOCOL` and `OTEL_EXPORTER_OTLP_TRACES_PROTOCOL` environment
/// variables to determine the protocol to use (or otherwise defaults to [`Protocol::HttpBinary`]).
///
/// # Errors
///
/// Returns an error if the protocol specified by the env var is not supported or if the required feature is not enabled for
/// the given protocol.
///
/// Returns an error if the endpoint is not a valid URI.
///
/// Returns an error if any headers are not valid HTTP headers.
pub fn span_exporter(
    endpoint: &str,
    headers: Option<HashMap<String, String>>,
) -> Result<impl SpanExporter + use<>, ConfigureError> {
    let protocol = protocol_from_env("OTEL_EXPORTER_OTLP_TRACES_PROTOCOL")?;

    // FIXME: it would be nice to let `opentelemetry-rust` handle this; ideally we could detect if
    // OTEL_EXPORTER_OTLP_PROTOCOL or OTEL_EXPORTER_OTLP_TRACES_PROTOCOL is set and let the SDK
    // make a builder. (If unset, we could supply our preferred exporter.)
    //
    // But at the moment otel-rust ignores these env vars; see
    // https://github.com/open-telemetry/opentelemetry-rust/issues/1983
    let span_exporter = match protocol {
        #[cfg(feature = "export-grpc")]
        Protocol::Grpc => {
            use opentelemetry_otlp::WithTonicConfig;
            opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_channel(
                    tonic::transport::Channel::builder(
                        endpoint
                            .try_into()
                            .map_err(|e: http::uri::InvalidUri| ConfigureError::Other(e.into()))?,
                    )
                    .connect_lazy(),
                )
                .with_metadata(build_metadata_from_headers(headers)?)
                .build()?
        }
        #[cfg(feature = "export-http-protobuf")]
        Protocol::HttpBinary => {
            use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
            opentelemetry_otlp::SpanExporter::builder()
                .with_http()
                .with_protocol(Protocol::HttpBinary)
                .with_headers(headers_with_user_agent(headers))
                .with_endpoint(format!("{endpoint}/v1/traces"))
                .build()?
        }
        #[cfg(feature = "export-http-json")]
        Protocol::HttpJson => {
            use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
            opentelemetry_otlp::SpanExporter::builder()
                .with_http()
                .with_protocol(Protocol::HttpJson)
                .with_headers(headers_with_user_agent(headers))
                .with_endpoint(format!("{endpoint}/v1/traces"))
                .build()?
        }
    };

    #[cfg(not(any(
        feature = "export-grpc",
        feature = "export-http-protobuf",
        feature = "export-http-json"
    )))]
    {
        Ok(UnreachableExporter)
    }

    #[cfg(any(
        feature = "export-grpc",
        feature = "export-http-protobuf",
        feature = "export-http-json"
    ))]
    {
        Ok(
            crate::internal::exporters::remove_pending::RemovePendingSpansExporter::new(
                span_exporter,
            ),
        )
    }
}

/// Build a [`PushMetricExporter`] for passing to
/// [`with_metrics()`][crate::LogfireConfigBuilder::with_metrics].
///
/// This uses `OTEL_EXPORTER_OTLP_PROTOCOL` and `OTEL_EXPORTER_OTLP_TRACES_PROTOCOL` environment
/// variables to determine the protocol to use (or otherwise defaults to [`Protocol::HttpBinary`]).
///
/// # Errors
///
/// Returns an error if the protocol specified by the env var is not supported or if the required feature is not enabled for
/// the given protocol.
///
/// Returns an error if the endpoint is not a valid URI.
///
/// Returns an error if any headers are not valid HTTP headers.
pub fn metric_exporter(
    endpoint: &str,
    headers: Option<HashMap<String, String>>,
) -> Result<impl PushMetricExporter + use<>, ConfigureError> {
    let protocol = protocol_from_env("OTEL_EXPORTER_OTLP_METRICS_PROTOCOL")?;

    // FIXME: it would be nice to let `opentelemetry-rust` handle this; ideally we could detect if
    // OTEL_EXPORTER_OTLP_PROTOCOL or OTEL_EXPORTER_OTLP_METRICS_PROTOCOL is set and let the SDK
    // make a builder. (If unset, we could supply our preferred exporter.)
    //
    // But at the moment otel-rust ignores these env vars; see
    // https://github.com/open-telemetry/opentelemetry-rust/issues/1983
    match protocol {
        #[cfg(feature = "export-grpc")]
        Protocol::Grpc => {
            use opentelemetry_otlp::WithTonicConfig;
            Ok(opentelemetry_otlp::MetricExporter::builder()
                .with_temporality(opentelemetry_sdk::metrics::Temporality::Delta)
                .with_tonic()
                .with_channel(
                    tonic::transport::Channel::builder(
                        endpoint
                            .try_into()
                            .map_err(|e: http::uri::InvalidUri| ConfigureError::Other(e.into()))?,
                    )
                    .connect_lazy(),
                )
                .with_metadata(build_metadata_from_headers(headers)?)
                .build()?)
        }
        #[cfg(feature = "export-http-protobuf")]
        Protocol::HttpBinary => {
            use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
            Ok(opentelemetry_otlp::MetricExporter::builder()
                .with_temporality(opentelemetry_sdk::metrics::Temporality::Delta)
                .with_http()
                .with_protocol(Protocol::HttpBinary)
                .with_headers(headers_with_user_agent(headers))
                .with_endpoint(format!("{endpoint}/v1/metrics"))
                .build()?)
        }
        #[cfg(feature = "export-http-json")]
        Protocol::HttpJson => {
            use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
            Ok(opentelemetry_otlp::MetricExporter::builder()
                .with_temporality(opentelemetry_sdk::metrics::Temporality::Delta)
                .with_http()
                .with_protocol(Protocol::HttpJson)
                .with_headers(headers_with_user_agent(headers))
                .with_endpoint(format!("{endpoint}/v1/metrics"))
                .build()?)
        }
    }

    #[cfg(not(any(
        feature = "export-grpc",
        feature = "export-http-protobuf",
        feature = "export-http-json"
    )))]
    #[expect(unreachable_code)]
    {
        // suppress unused var warnings
        let _ = endpoint;
        let _ = headers;
        let _ = protocol;
        Ok(UnreachableExporter)
    }
}

/// Build a [`LogExporter`] for passing to log processors.
///
/// This uses `OTEL_EXPORTER_OTLP_PROTOCOL` and `OTEL_EXPORTER_OTLP_LOGS_PROTOCOL` environment
/// variables to determine the protocol to use (or otherwise defaults to [`Protocol::HttpBinary`]).
///
/// # Errors
///
/// Returns an error if the protocol specified by the env var is not supported or if the required feature is not enabled for
/// the given protocol.
///
/// Returns an error if the endpoint is not a valid URI.
///
/// Returns an error if any headers are not valid HTTP headers.
pub fn log_exporter(
    endpoint: &str,
    headers: Option<HashMap<String, String>>,
) -> Result<impl LogExporter + use<>, ConfigureError> {
    let protocol = protocol_from_env("OTEL_EXPORTER_OTLP_LOGS_PROTOCOL")?;

    // FIXME: it would be nice to let `opentelemetry-rust` handle this; ideally we could detect if
    // OTEL_EXPORTER_OTLP_PROTOCOL or OTEL_EXPORTER_OTLP_LOGS_PROTOCOL is set and let the SDK
    // make a builder. (If unset, we could supply our preferred exporter.)
    //
    // But at the moment otel-rust ignores these env vars; see
    // https://github.com/open-telemetry/opentelemetry-rust/issues/1983
    match protocol {
        #[cfg(feature = "export-grpc")]
        Protocol::Grpc => {
            use opentelemetry_otlp::WithTonicConfig;
            Ok(opentelemetry_otlp::LogExporter::builder()
                .with_tonic()
                .with_channel(
                    tonic::transport::Channel::builder(
                        endpoint
                            .try_into()
                            .map_err(|e: http::uri::InvalidUri| ConfigureError::Other(e.into()))?,
                    )
                    .connect_lazy(),
                )
                .with_metadata(build_metadata_from_headers(headers)?)
                .build()?)
        }
        #[cfg(feature = "export-http-protobuf")]
        Protocol::HttpBinary => {
            use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
            Ok(opentelemetry_otlp::LogExporter::builder()
                .with_http()
                .with_protocol(Protocol::HttpBinary)
                .with_headers(headers_with_user_agent(headers))
                .with_endpoint(format!("{endpoint}/v1/logs"))
                .build()?)
        }
        #[cfg(feature = "export-http-json")]
        Protocol::HttpJson => {
            use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
            Ok(opentelemetry_otlp::LogExporter::builder()
                .with_http()
                .with_protocol(Protocol::HttpJson)
                .with_headers(headers_with_user_agent(headers))
                .with_endpoint(format!("{endpoint}/v1/logs"))
                .build()?)
        }
    }

    #[cfg(not(any(
        feature = "export-grpc",
        feature = "export-http-protobuf",
        feature = "export-http-json"
    )))]
    #[expect(unreachable_code)]
    {
        // suppress unused var warnings
        let _ = endpoint;
        let _ = headers;
        let _ = protocol;
        Ok(UnreachableExporter)
    }
}

#[cfg(feature = "export-grpc")]
fn build_metadata_from_headers(
    headers: Option<HashMap<String, String>>,
) -> Result<tonic::metadata::MetadataMap, ConfigureError> {
    let mut header_map = http::HeaderMap::new();
    for (key, value) in headers_with_user_agent(headers) {
        header_map.insert(
            http::HeaderName::try_from(key).map_err(|e| ConfigureError::Other(e.into()))?,
            http::HeaderValue::try_from(value).map_err(|e| ConfigureError::Other(e.into()))?,
        );
    }
    Ok(tonic::metadata::MetadataMap::from_headers(header_map))
}

#[cfg(test)]
#[cfg(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
))]
mod tests {
    use super::{USER_AGENT, headers_with_user_agent};
    use std::collections::HashMap;

    #[test]
    fn user_agent_is_added_to_headers() {
        assert_eq!(
            headers_with_user_agent(None),
            HashMap::from([("User-Agent".to_string(), USER_AGENT.to_string())])
        );

        let headers = headers_with_user_agent(Some(HashMap::from([(
            "Authorization".to_string(),
            "Bearer token".to_string(),
        )])));
        assert_eq!(
            headers,
            HashMap::from([
                ("Authorization".to_string(), "Bearer token".to_string()),
                ("User-Agent".to_string(), USER_AGENT.to_string())
            ])
        );
    }

    #[test]
    fn user_agent_set_by_caller_is_preserved() {
        let headers = headers_with_user_agent(Some(HashMap::from([(
            "user-agent".to_string(),
            "my-app/1.2.3".to_string(),
        )])));
        assert_eq!(
            headers,
            HashMap::from([("user-agent".to_string(), "my-app/1.2.3".to_string())])
        );
    }
}

// current default logfire protocol is to export over HTTP in binary format
const DEFAULT_LOGFIRE_PROTOCOL: &str = OTEL_EXPORTER_OTLP_PROTOCOL_HTTP_PROTOBUF;

// standard OTLP protocol values in configuration
const OTEL_EXPORTER_OTLP_PROTOCOL_GRPC: &str = "grpc";
const OTEL_EXPORTER_OTLP_PROTOCOL_HTTP_PROTOBUF: &str = "http/protobuf";
const OTEL_EXPORTER_OTLP_PROTOCOL_HTTP_JSON: &str = "http/json";

/// Get a protocol from the environment (or default value).
fn protocol_from_env(data_env_var: &str) -> Result<Protocol, ConfigureError> {
    // try both data-specific env var and general protocol
    let (source, value) = [data_env_var, "OTEL_EXPORTER_OTLP_PROTOCOL"]
        .into_iter()
        .find_map(|var_name| match get_optional_env(var_name, None) {
            Ok(Some(value)) => Some(Ok((var_name, value))),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        })
        .transpose()?
        .map_or(
            (
                "the default logfire export protocol".to_string(),
                DEFAULT_LOGFIRE_PROTOCOL.to_string(),
            ),
            |(var_name, value)| (format!("`{var_name}={value}`"), value),
        );

    match value.as_str() {
        OTEL_EXPORTER_OTLP_PROTOCOL_GRPC => {
            feature_required!("export-grpc", source, Ok(Protocol::Grpc))
        }
        OTEL_EXPORTER_OTLP_PROTOCOL_HTTP_PROTOBUF => {
            feature_required!("export-http-protobuf", source, Ok(Protocol::HttpBinary))
        }
        OTEL_EXPORTER_OTLP_PROTOCOL_HTTP_JSON => {
            feature_required!("export-http-json", source, Ok(Protocol::HttpJson))
        }
        _ => Err(ConfigureError::Other(
            format!("unsupported protocol: {value}").into(),
        )),
    }
}

/// Internal type used when no export features are available to allow for type inference
/// for impl Trait return of `span_exporter()`, `metric_exporter()`, or `log_exporter()`.
#[cfg(not(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
)))]
#[derive(Debug)]
struct UnreachableExporter;

#[cfg(not(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
)))]
impl SpanExporter for UnreachableExporter {
    fn export(
        &self,
        _batch: Vec<opentelemetry_sdk::trace::SpanData>,
    ) -> impl std::future::Future<Output = opentelemetry_sdk::error::OTelSdkResult> + Send {
        async { unreachable!() }
    }
}

#[cfg(not(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
)))]
impl PushMetricExporter for UnreachableExporter {
    fn export(
        &self,
        _metrics: &opentelemetry_sdk::metrics::data::ResourceMetrics,
    ) -> impl std::future::Future<Output = opentelemetry_sdk::error::OTelSdkResult> + Send {
        async { unreachable!() }
    }

    fn force_flush(&self) -> opentelemetry_sdk::error::OTelSdkResult {
        unreachable!()
    }

    fn shutdown_with_timeout(
        &self,
        _timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        unreachable!()
    }

    fn temporality(&self) -> opentelemetry_sdk::metrics::Temporality {
        unreachable!()
    }
}

#[cfg(not(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
)))]
impl LogExporter for UnreachableExporter {
    fn export(
        &self,
        _batch: opentelemetry_sdk::logs::LogBatch<'_>,
    ) -> impl std::future::Future<Output = opentelemetry_sdk::error::OTelSdkResult> + Send {
        async { unreachable!() }
    }
}
