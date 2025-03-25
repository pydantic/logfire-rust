//! Helper functions to configure mo
use std::collections::HashMap;

use opentelemetry_otlp::{MetricExporter, Protocol};
use opentelemetry_sdk::{metrics::exporter::PushMetricExporter, trace::SpanExporter};

use crate::{
    ConfigureError, get_optional_env,
    internal::exporters::remove_pending::RemovePendingSpansExporter,
};

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

/// Build a [`SpanExporter`][opentelemetry_sdk::trace::SpanExporter] for passing to
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
    let (source, protocol) = protocol_from_env("OTEL_EXPORTER_OTLP_TRACES_PROTOCOL")?;

    let builder = opentelemetry_otlp::SpanExporter::builder();

    // FIXME: it would be nice to let `opentelemetry-rust` handle this; ideally we could detect if
    // OTEL_EXPORTER_OTLP_PROTOCOL or OTEL_EXPORTER_OTLP_TRACES_PROTOCOL is set and let the SDK
    // make a builder. (If unset, we could supply our preferred exporter.)
    //
    // But at the moment otel-rust ignores these env vars; see
    // https://github.com/open-telemetry/opentelemetry-rust/issues/1983
    let span_exporter =
        match protocol {
            Protocol::Grpc => {
                feature_required!("export-grpc", source, {
                    use opentelemetry_otlp::WithTonicConfig;
                    builder
                        .with_tonic()
                        .with_channel(
                            tonic::transport::Channel::builder(endpoint.try_into().map_err(
                                |e: http::uri::InvalidUri| ConfigureError::Other(e.into()),
                            )?)
                            .connect_lazy(),
                        )
                        .with_metadata(build_metadata_from_headers(headers.as_ref())?)
                        .build()?
                })
            }
            Protocol::HttpBinary => {
                feature_required!("export-http-protobuf", source, {
                    use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
                    builder
                        .with_http()
                        .with_protocol(Protocol::HttpBinary)
                        .with_headers(headers.unwrap_or_default())
                        .with_endpoint(format!("{endpoint}/v1/traces"))
                        .build()?
                })
            }
            Protocol::HttpJson => {
                feature_required!("export-http-json", source, {
                    use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
                    builder
                        .with_http()
                        .with_protocol(Protocol::HttpBinary)
                        .with_headers(headers.unwrap_or_default())
                        .with_endpoint(format!("{endpoint}/v1/traces"))
                        .build()?
                })
            }
        };
    Ok(RemovePendingSpansExporter::new(span_exporter))
}

/// Build a [`PushMetricExporter`][opentelemetry_sdk::metrics::exporter::PushMetricExporter] for passing to
/// [`with_metrics_options()`][crate::LogfireConfigBuilder::with_metrics_options].
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
    let (source, protocol) = protocol_from_env("OTEL_EXPORTER_OTLP_METRICS_PROTOCOL")?;

    let builder =
        MetricExporter::builder().with_temporality(opentelemetry_sdk::metrics::Temporality::Delta);

    // FIXME: it would be nice to let `opentelemetry-rust` handle this; ideally we could detect if
    // OTEL_EXPORTER_OTLP_PROTOCOL or OTEL_EXPORTER_OTLP_METRICS_PROTOCOL is set and let the SDK
    // make a builder. (If unset, we could supply our preferred exporter.)
    //
    // But at the moment otel-rust ignores these env vars; see
    // https://github.com/open-telemetry/opentelemetry-rust/issues/1983
    match protocol {
        Protocol::Grpc => {
            feature_required!("export-grpc", source, {
                use opentelemetry_otlp::WithTonicConfig;
                Ok(builder
                    .with_tonic()
                    .with_channel(
                        tonic::transport::Channel::builder(
                            endpoint.try_into().map_err(|e: http::uri::InvalidUri| {
                                ConfigureError::Other(e.into())
                            })?,
                        )
                        .connect_lazy(),
                    )
                    .with_metadata(build_metadata_from_headers(headers.as_ref())?)
                    .build()?)
            })
        }
        Protocol::HttpBinary => {
            feature_required!("export-http-protobuf", source, {
                use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
                Ok(builder
                    .with_http()
                    .with_protocol(Protocol::HttpBinary)
                    .with_headers(headers.unwrap_or_default())
                    .with_endpoint(format!("{endpoint}/v1/metrics"))
                    .build()?)
            })
        }
        Protocol::HttpJson => {
            feature_required!("export-http-json", source, {
                use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
                Ok(builder
                    .with_http()
                    .with_protocol(Protocol::HttpBinary)
                    .with_headers(headers.unwrap_or_default())
                    .with_endpoint(format!("{endpoint}/v1/metrics"))
                    .build()?)
            })
        }
    }
}

#[cfg(feature = "export-grpc")]
fn build_metadata_from_headers(
    headers: Option<&HashMap<String, String>>,
) -> Result<tonic::metadata::MetadataMap, ConfigureError> {
    let Some(headers) = headers else {
        return Ok(tonic::metadata::MetadataMap::new());
    };

    let mut header_map = http::HeaderMap::new();
    for (key, value) in headers {
        header_map.insert(
            http::HeaderName::try_from(key).map_err(|e| ConfigureError::Other(e.into()))?,
            http::HeaderValue::try_from(value).map_err(|e| ConfigureError::Other(e.into()))?,
        );
    }
    Ok(tonic::metadata::MetadataMap::from_headers(header_map))
}

// current default logfire protocol is to export over HTTP in binary format
const DEFAULT_LOGFIRE_PROTOCOL: Protocol = Protocol::HttpBinary;

// standard OTLP protocol values in configuration
const OTEL_EXPORTER_OTLP_PROTOCOL_GRPC: &str = "grpc";
const OTEL_EXPORTER_OTLP_PROTOCOL_HTTP_PROTOBUF: &str = "http/protobuf";
const OTEL_EXPORTER_OTLP_PROTOCOL_HTTP_JSON: &str = "http/json";

/// Temporary workaround for lack of <https://github.com/open-telemetry/opentelemetry-rust/pull/2758>
fn protocol_from_str(value: &str) -> Result<Protocol, ConfigureError> {
    match value {
        OTEL_EXPORTER_OTLP_PROTOCOL_GRPC => Ok(Protocol::Grpc),
        OTEL_EXPORTER_OTLP_PROTOCOL_HTTP_PROTOBUF => Ok(Protocol::HttpBinary),
        OTEL_EXPORTER_OTLP_PROTOCOL_HTTP_JSON => Ok(Protocol::HttpJson),
        _ => Err(ConfigureError::Other(
            format!("unsupported protocol: {value}").into(),
        )),
    }
}

/// Get a protocol from the environment (or default value), returning a string describing the source
/// plus the parsed protocol.
fn protocol_from_env(data_env_var: &str) -> Result<(String, Protocol), ConfigureError> {
    // try both data-specific env var and general protocol
    [data_env_var, "OTEL_EXPORTER_OTLP_PROTOCOL"]
        .into_iter()
        .find_map(|var_name| match get_optional_env(var_name, None) {
            Ok(Some(value)) => Some(Ok((var_name, value))),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        })
        .transpose()?
        .map_or_else(
            || {
                Ok((
                    "the default logfire export protocol".to_string(),
                    DEFAULT_LOGFIRE_PROTOCOL,
                ))
            },
            |(var_name, value)| Ok((format!("`{var_name}={value}`"), protocol_from_str(&value)?)),
        )
}
