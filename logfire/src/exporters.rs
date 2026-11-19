#![cfg_attr(
    not(any(
        feature = "export-grpc",
        feature = "export-http-protobuf",
        feature = "export-http-json"
    )),
    expect(unreachable_code, unused_variables)
)]
//! Helper functions to configure exporters via OTLP.
//!
//! Exports use OpenTelemetry's bounded retries for transient failures. Retrying after a lost
//! acknowledgement can duplicate telemetry, including delta metric values.
use std::collections::HashMap;

use opentelemetry_otlp::Protocol;
use opentelemetry_sdk::{
    logs::LogExporter, metrics::exporter::PushMetricExporter, trace::SpanExporter,
};

use crate::{ConfigureError, internal::env::get_optional_env};

/// Export transport options, set through [`AdvancedOptions`](crate::config::AdvancedOptions).
#[derive(Default)]
pub(crate) struct ExportOptions {
    #[cfg(feature = "export-grpc")]
    pub(crate) grpc_channel: Option<
        std::sync::Arc<
            dyn Fn(tonic::transport::Endpoint) -> tonic::transport::Channel + Send + Sync,
        >,
    >,
}

/// The `User-Agent` sent with OTLP exports, e.g. `logfire-rust/0.13.0`.
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
    span_exporter_with_options(endpoint, headers, &ExportOptions::default())
}

pub(crate) fn span_exporter_with_options(
    endpoint: &str,
    headers: Option<HashMap<String, String>>,
    options: &ExportOptions,
) -> Result<impl SpanExporter + use<>, ConfigureError> {
    #[cfg(not(feature = "export-grpc"))]
    let _ = options;
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
                .with_channel(grpc_channel(endpoint, &TRACES_TLS_ENV_VARS, options)?)
                .with_metadata(build_metadata_from_headers(headers)?)
                .build()?
        }
        #[cfg(feature = "export-http-protobuf")]
        Protocol::HttpBinary => {
            use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};

            require_crypto_provider()?;
            with_http_tls(
                opentelemetry_otlp::SpanExporter::builder().with_http(),
                endpoint,
                &TRACES_TLS_ENV_VARS,
                None,
            )?
            .with_protocol(Protocol::HttpBinary)
            .with_headers(headers_with_user_agent(headers))
            .with_endpoint(format!("{endpoint}/v1/traces"))
            .build()?
        }
        #[cfg(feature = "export-http-json")]
        Protocol::HttpJson => {
            use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};

            require_crypto_provider()?;
            with_http_tls(
                opentelemetry_otlp::SpanExporter::builder().with_http(),
                endpoint,
                &TRACES_TLS_ENV_VARS,
                None,
            )?
            .with_protocol(Protocol::HttpJson)
            .with_headers(headers_with_user_agent(headers))
            .with_endpoint(format!("{endpoint}/v1/traces"))
            .build()?
        }
        #[allow(unreachable_patterns)]
        _ => {
            return Err(ConfigureError::Other(
                format!("unsupported protocol: {protocol:?}").into(),
            ));
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
    metric_exporter_with_options(endpoint, headers, &ExportOptions::default())
}

pub(crate) fn metric_exporter_with_options(
    endpoint: &str,
    headers: Option<HashMap<String, String>>,
    options: &ExportOptions,
) -> Result<impl PushMetricExporter + use<>, ConfigureError> {
    #[cfg(not(feature = "export-grpc"))]
    let _ = options;
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
                .with_channel(grpc_channel(endpoint, &METRICS_TLS_ENV_VARS, options)?)
                .with_metadata(build_metadata_from_headers(headers)?)
                .build()?)
        }
        #[cfg(feature = "export-http-protobuf")]
        Protocol::HttpBinary => {
            use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};

            require_crypto_provider()?;
            Ok(with_http_tls(
                opentelemetry_otlp::MetricExporter::builder()
                    .with_temporality(opentelemetry_sdk::metrics::Temporality::Delta)
                    .with_http(),
                endpoint,
                &METRICS_TLS_ENV_VARS,
                None,
            )?
            .with_protocol(Protocol::HttpBinary)
            .with_headers(headers_with_user_agent(headers))
            .with_endpoint(format!("{endpoint}/v1/metrics"))
            .build()?)
        }
        #[cfg(feature = "export-http-json")]
        Protocol::HttpJson => {
            use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};

            require_crypto_provider()?;
            Ok(with_http_tls(
                opentelemetry_otlp::MetricExporter::builder()
                    .with_temporality(opentelemetry_sdk::metrics::Temporality::Delta)
                    .with_http(),
                endpoint,
                &METRICS_TLS_ENV_VARS,
                None,
            )?
            .with_protocol(Protocol::HttpJson)
            .with_headers(headers_with_user_agent(headers))
            .with_endpoint(format!("{endpoint}/v1/metrics"))
            .build()?)
        }
        #[allow(unreachable_patterns)]
        #[allow(clippy::needless_return)]
        _ => {
            return Err(ConfigureError::Other(
                format!("unsupported protocol: {protocol:?}").into(),
            ));
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
    log_exporter_with_options(endpoint, headers, &ExportOptions::default())
}

pub(crate) fn log_exporter_with_options(
    endpoint: &str,
    headers: Option<HashMap<String, String>>,
    options: &ExportOptions,
) -> Result<impl LogExporter + use<>, ConfigureError> {
    #[cfg(not(feature = "export-grpc"))]
    let _ = options;
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
                .with_channel(grpc_channel(endpoint, &LOGS_TLS_ENV_VARS, options)?)
                .with_metadata(build_metadata_from_headers(headers)?)
                .build()?)
        }
        #[cfg(feature = "export-http-protobuf")]
        Protocol::HttpBinary => {
            use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};

            require_crypto_provider()?;
            Ok(with_http_tls(
                opentelemetry_otlp::LogExporter::builder().with_http(),
                endpoint,
                &LOGS_TLS_ENV_VARS,
                None,
            )?
            .with_protocol(Protocol::HttpBinary)
            .with_headers(headers_with_user_agent(headers))
            .with_endpoint(format!("{endpoint}/v1/logs"))
            .build()?)
        }
        #[cfg(feature = "export-http-json")]
        Protocol::HttpJson => {
            use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};

            require_crypto_provider()?;
            Ok(with_http_tls(
                opentelemetry_otlp::LogExporter::builder().with_http(),
                endpoint,
                &LOGS_TLS_ENV_VARS,
                None,
            )?
            .with_protocol(Protocol::HttpJson)
            .with_headers(headers_with_user_agent(headers))
            .with_endpoint(format!("{endpoint}/v1/logs"))
            .build()?)
        }
        #[allow(unreachable_patterns)]
        #[allow(clippy::needless_return)]
        _ => {
            return Err(ConfigureError::Other(
                format!("unsupported protocol: {protocol:?}").into(),
            ));
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

/// The environment variables that configure TLS for one signal's exports.
///
/// Each pair holds the signal-specific variable first, which takes precedence, then the general
/// one, as in the OTLP exporter specification:
/// <https://opentelemetry.io/docs/specs/otel/protocol/exporter/#configuration-options>.
#[cfg(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
))]
struct TlsEnvVars {
    /// PEM certificates that verify the server, in place of the system roots.
    certificate: [&'static str; 2],
    /// The PEM certificate chain that the client presents.
    client_certificate: [&'static str; 2],
    /// The PEM private key of `client_certificate`.
    client_key: [&'static str; 2],
    /// The export timeout in milliseconds. `opentelemetry-otlp` reads the same variables.
    #[cfg_attr(
        not(any(feature = "export-http-protobuf", feature = "export-http-json")),
        allow(dead_code)
    )]
    timeout: [&'static str; 2],
}

#[cfg(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
))]
const TRACES_TLS_ENV_VARS: TlsEnvVars = TlsEnvVars {
    certificate: [
        "OTEL_EXPORTER_OTLP_TRACES_CERTIFICATE",
        "OTEL_EXPORTER_OTLP_CERTIFICATE",
    ],
    client_certificate: [
        "OTEL_EXPORTER_OTLP_TRACES_CLIENT_CERTIFICATE",
        "OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE",
    ],
    client_key: [
        "OTEL_EXPORTER_OTLP_TRACES_CLIENT_KEY",
        "OTEL_EXPORTER_OTLP_CLIENT_KEY",
    ],
    timeout: [
        opentelemetry_otlp::OTEL_EXPORTER_OTLP_TRACES_TIMEOUT,
        opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT,
    ],
};

#[cfg(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
))]
const METRICS_TLS_ENV_VARS: TlsEnvVars = TlsEnvVars {
    certificate: [
        "OTEL_EXPORTER_OTLP_METRICS_CERTIFICATE",
        "OTEL_EXPORTER_OTLP_CERTIFICATE",
    ],
    client_certificate: [
        "OTEL_EXPORTER_OTLP_METRICS_CLIENT_CERTIFICATE",
        "OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE",
    ],
    client_key: [
        "OTEL_EXPORTER_OTLP_METRICS_CLIENT_KEY",
        "OTEL_EXPORTER_OTLP_CLIENT_KEY",
    ],
    timeout: [
        opentelemetry_otlp::OTEL_EXPORTER_OTLP_METRICS_TIMEOUT,
        opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT,
    ],
};

#[cfg(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
))]
const LOGS_TLS_ENV_VARS: TlsEnvVars = TlsEnvVars {
    certificate: [
        "OTEL_EXPORTER_OTLP_LOGS_CERTIFICATE",
        "OTEL_EXPORTER_OTLP_CERTIFICATE",
    ],
    client_certificate: [
        "OTEL_EXPORTER_OTLP_LOGS_CLIENT_CERTIFICATE",
        "OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE",
    ],
    client_key: [
        "OTEL_EXPORTER_OTLP_LOGS_CLIENT_KEY",
        "OTEL_EXPORTER_OTLP_CLIENT_KEY",
    ],
    timeout: [
        opentelemetry_otlp::OTEL_EXPORTER_OTLP_LOGS_TIMEOUT,
        opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT,
    ],
};

/// The TLS files for an export, read from the variables in [`TlsEnvVars`].
#[cfg(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
))]
#[derive(Default)]
struct TlsFiles {
    /// The PEM certificates that verify the server. `None` keeps the system roots.
    certificate: Option<Vec<u8>>,
    /// The PEM certificate chain and private key that the client presents.
    client_identity: Option<(Vec<u8>, Vec<u8>)>,
}

#[cfg(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
))]
impl TlsFiles {
    /// Read the files that `vars` name.
    ///
    /// A file that cannot be read, a certificate file that holds no certificate, or a client
    /// certificate without its key (or the reverse) is an error, so that a misconfigured
    /// exporter fails at startup and not on every export.
    fn from_env(
        vars: &TlsEnvVars,
        env: Option<&HashMap<String, String>>,
    ) -> Result<Self, ConfigureError> {
        let certificate = match read_env_file(&vars.certificate, env)? {
            Some((name, pem)) => {
                use rustls::pki_types::pem::PemObject as _;
                let count = rustls::pki_types::CertificateDer::pem_slice_iter(&pem)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| invalid_tls_file(name, error))?
                    .len();
                if count == 0 {
                    return Err(invalid_tls_file(name, "no certificate found"));
                }
                Some(pem)
            }
            None => None,
        };
        let client_identity = match (
            read_env_file(&vars.client_certificate, env)?,
            read_env_file(&vars.client_key, env)?,
        ) {
            (Some((_, certificate)), Some((_, key))) => Some((certificate, key)),
            (None, None) => None,
            (Some((name, _)), None) => {
                return Err(invalid_tls_file(
                    vars.client_key[1],
                    format!("not set, but {name} is set"),
                ));
            }
            (None, Some((name, _))) => {
                return Err(invalid_tls_file(
                    vars.client_certificate[1],
                    format!("not set, but {name} is set"),
                ));
            }
        };
        Ok(Self {
            certificate,
            client_identity,
        })
    }

    #[cfg(any(feature = "export-http-protobuf", feature = "export-http-json"))]
    fn is_empty(&self) -> bool {
        self.certificate.is_none() && self.client_identity.is_none()
    }
}

/// Read the file that the first set variable in `names` points to, with that variable's name.
#[cfg(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
))]
fn read_env_file(
    names: &[&'static str; 2],
    env: Option<&HashMap<String, String>>,
) -> Result<Option<(&'static str, Vec<u8>)>, ConfigureError> {
    for name in names {
        if let Some(path) = get_optional_env(name, env)? {
            let contents = std::fs::read(&path)
                .map_err(|error| invalid_tls_file(name, format!("{path}: {error}")))?;
            return Ok(Some((name, contents)));
        }
    }
    Ok(None)
}

#[cfg(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
))]
fn invalid_tls_file(parameter: &'static str, error: impl std::fmt::Display) -> ConfigureError {
    ConfigureError::InvalidConfigurationValue {
        parameter,
        value: error.to_string(),
    }
}

/// Whether `endpoint` uses TLS. The TLS variables apply only to such an endpoint. gRPC exports
/// check the parsed URI in [`grpc_endpoint`] instead.
#[cfg(any(feature = "export-http-protobuf", feature = "export-http-json"))]
fn is_https(endpoint: &str) -> bool {
    endpoint
        .get(..8)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("https://"))
}

/// Give an HTTP exporter a client with the TLS files that `vars` name, for an `https` endpoint.
///
/// Without TLS files, the builder is unchanged and `opentelemetry-otlp` builds its default client.
/// A custom client replaces that default, so it also takes the export timeout, as the default
/// client does.
#[cfg(any(feature = "export-http-protobuf", feature = "export-http-json"))]
fn with_http_tls<B: opentelemetry_otlp::WithHttpConfig>(
    builder: B,
    endpoint: &str,
    vars: &TlsEnvVars,
    env: Option<&HashMap<String, String>>,
) -> Result<B, ConfigureError> {
    if !is_https(endpoint) {
        return Ok(builder);
    }
    let files = TlsFiles::from_env(vars, env)?;
    if files.is_empty() {
        return Ok(builder);
    }
    Ok(builder.with_http_client(http_tls_client(&files, vars, export_timeout(vars, env)?)?))
}

/// Build a `reqwest` client that verifies the server and identifies itself with `files`.
#[cfg(any(feature = "export-http-protobuf", feature = "export-http-json"))]
fn http_tls_client(
    files: &TlsFiles,
    vars: &TlsEnvVars,
    timeout: std::time::Duration,
) -> Result<reqwest::Client, ConfigureError> {
    let mut client = reqwest::Client::builder().timeout(timeout);
    if let Some(pem) = &files.certificate {
        let certificates = reqwest::Certificate::from_pem_bundle(pem)
            .map_err(|error| invalid_tls_file(vars.certificate[1], error))?;
        client = client.tls_certs_only(certificates);
    }
    if let Some((certificate, key)) = &files.client_identity {
        let identity = reqwest::Identity::from_pem(&[certificate.as_slice(), b"\n", key].concat())
            .map_err(|error| invalid_tls_file(vars.client_certificate[1], error))?;
        client = client.identity(identity);
    }
    client
        .build()
        .map_err(|error| ConfigureError::Other(error.into()))
}

/// The export timeout from `vars`, resolved as `opentelemetry-otlp` resolves it.
#[cfg(any(feature = "export-http-protobuf", feature = "export-http-json"))]
fn export_timeout(
    vars: &TlsEnvVars,
    env: Option<&HashMap<String, String>>,
) -> Result<std::time::Duration, ConfigureError> {
    for name in vars.timeout {
        if let Some(millis) = get_optional_env(name, env)?.and_then(|value| value.parse().ok()) {
            return Ok(std::time::Duration::from_millis(millis));
        }
    }
    Ok(opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT)
}

/// Build the `tonic` endpoint used for gRPC exports.
///
/// TLS is configured here because `tonic` only configures it itself in `Endpoint::new`, which
/// none of its public constructors go through. Without this, an https endpoint fails to connect
/// with "Connecting to HTTPS without TLS enabled".
///
/// For an https endpoint, the TLS files that `vars` name replace the system roots and add a
/// client certificate.
#[cfg(feature = "export-grpc")]
fn grpc_endpoint(
    endpoint: &str,
    vars: &TlsEnvVars,
    env: Option<&HashMap<String, String>>,
) -> Result<tonic::transport::Endpoint, ConfigureError> {
    let uri: http::Uri = endpoint
        .try_into()
        .map_err(|e: http::uri::InvalidUri| ConfigureError::Other(e.into()))?;

    let mut builder = tonic::transport::Channel::builder(uri.clone());

    if uri.scheme() == Some(&http::uri::Scheme::HTTPS) {
        require_crypto_provider()?;
        builder = builder
            .tls_config(grpc_tls_config(TlsFiles::from_env(vars, env)?))
            .map_err(|e| ConfigureError::Other(e.into()))?;
    }

    Ok(builder)
}

/// The `tonic` TLS settings that verify the server and identify the client with `files`.
#[cfg(feature = "export-grpc")]
fn grpc_tls_config(files: TlsFiles) -> tonic::transport::ClientTlsConfig {
    let mut tls = tonic::transport::ClientTlsConfig::new();
    tls = match files.certificate {
        Some(pem) => tls.ca_certificate(tonic::transport::Certificate::from_pem(pem)),
        None => tls.with_enabled_roots(),
    };
    if let Some((certificate, key)) = files.client_identity {
        tls = tls.identity(tonic::transport::Identity::from_pem(certificate, key));
    }
    tls
}

/// Build the `tonic` channel used for gRPC exports.
#[cfg(feature = "export-grpc")]
fn grpc_channel(
    endpoint: &str,
    vars: &TlsEnvVars,
    options: &ExportOptions,
) -> Result<tonic::transport::Channel, ConfigureError> {
    let endpoint = grpc_endpoint(endpoint, vars, None)?;
    Ok(match &options.grpc_channel {
        Some(build) => build(endpoint),
        None => endpoint.connect_lazy(),
    })
}

#[cfg(test)]
#[cfg(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
))]
mod tls_file_tests {
    use std::{collections::HashMap, sync::Arc};

    use rcgen::{BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair};
    use tokio::sync::oneshot;

    use super::{TRACES_TLS_ENV_VARS, TlsFiles};
    use crate::ConfigureError;

    /// A certificate authority, a server certificate for `127.0.0.1` and a client certificate,
    /// all signed by that authority and written as PEM files to a temporary directory.
    struct Pki {
        dir: tempfile::TempDir,
        server_certificate: rustls::pki_types::CertificateDer<'static>,
        server_key: rustls::pki_types::PrivateKeyDer<'static>,
        ca_certificate: rustls::pki_types::CertificateDer<'static>,
    }

    impl Pki {
        fn new() -> Self {
            let dir = tempfile::tempdir().expect("failed to create a temporary directory");

            let ca_key = KeyPair::generate().expect("failed to generate the CA key");
            let mut ca_params = CertificateParams::new(Vec::<String>::new())
                .expect("failed to build the CA parameters");
            ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
            let ca = ca_params
                .self_signed(&ca_key)
                .expect("failed to sign the CA");
            let issuer = Issuer::from_params(&ca_params, &ca_key);

            let server_key = KeyPair::generate().expect("failed to generate the server key");
            let server = CertificateParams::new(vec!["127.0.0.1".to_string()])
                .expect("failed to build the server parameters")
                .signed_by(&server_key, &issuer)
                .expect("failed to sign the server certificate");

            let client_key = KeyPair::generate().expect("failed to generate the client key");
            let client = CertificateParams::new(vec!["client".to_string()])
                .expect("failed to build the client parameters")
                .signed_by(&client_key, &issuer)
                .expect("failed to sign the client certificate");

            for (name, contents) in [
                ("ca.pem", ca.pem()),
                ("client.pem", client.pem()),
                ("client.key", client_key.serialize_pem()),
                ("bad.pem", "not a certificate".to_string()),
            ] {
                std::fs::write(dir.path().join(name), contents).expect("failed to write a file");
            }

            Self {
                dir,
                server_certificate: server.der().clone(),
                server_key: rustls::pki_types::PrivateKeyDer::Pkcs8(
                    server_key.serialize_der().into(),
                ),
                ca_certificate: ca.der().clone(),
            }
        }

        fn path(&self, name: &str) -> String {
            self.dir.path().join(name).display().to_string()
        }

        /// The variables that name the CA and the client certificate.
        fn env(&self) -> HashMap<String, String> {
            HashMap::from([
                (
                    "OTEL_EXPORTER_OTLP_CERTIFICATE".to_string(),
                    self.path("ca.pem"),
                ),
                (
                    "OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE".to_string(),
                    self.path("client.pem"),
                ),
                (
                    "OTEL_EXPORTER_OTLP_CLIENT_KEY".to_string(),
                    self.path("client.key"),
                ),
            ])
        }

        /// Accept one TLS connection that must present a client certificate from the CA. The
        /// receiver gets whether the handshake succeeded with a client certificate.
        async fn serve_once(&self) -> (std::net::SocketAddr, oneshot::Receiver<bool>) {
            let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
            let mut roots = rustls::RootCertStore::empty();
            roots
                .add(self.ca_certificate.clone())
                .expect("failed to add the CA");
            let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
                roots.into(),
                provider.clone(),
            )
            .build()
            .expect("failed to build the client verifier");
            let config = rustls::ServerConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .expect("failed to select protocol versions")
                .with_client_cert_verifier(verifier)
                .with_single_cert(
                    vec![self.server_certificate.clone()],
                    self.server_key.clone_key(),
                )
                .expect("failed to build the server config");
            let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));

            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("failed to bind listener");
            let addr = listener.local_addr().expect("failed to read listener addr");
            let (sender, receiver) = oneshot::channel();
            tokio::spawn(async move {
                let (stream, _) = listener.accept().await.expect("failed to accept");
                let presented = acceptor.accept(stream).await.is_ok_and(|stream| {
                    stream
                        .get_ref()
                        .1
                        .peer_certificates()
                        .is_some_and(|certs| !certs.is_empty())
                });
                let _ = sender.send(presented);
            });
            (addr, receiver)
        }
    }

    fn install_crypto_provider() {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }

    /// The message of the error in `result`, or an empty string for a success.
    fn error_message<T>(result: Result<T, ConfigureError>) -> String {
        result
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default()
    }

    fn assert_invalid(message: &str, parameter: &str) {
        assert_eq!(
            message.split_once(": ").map(|(prefix, _)| prefix),
            Some(format!("Invalid configuration value for {parameter}").as_str())
        );
    }

    #[test]
    fn no_variables_read_no_files() {
        let files = TlsFiles::from_env(&TRACES_TLS_ENV_VARS, Some(&HashMap::new()))
            .expect("no variables is valid");
        assert!(files.certificate.is_none());
        assert!(files.client_identity.is_none());
    }

    #[test]
    fn signal_variable_takes_precedence() {
        let pki = Pki::new();
        let env = HashMap::from([
            (
                "OTEL_EXPORTER_OTLP_CERTIFICATE".to_string(),
                pki.path("missing.pem"),
            ),
            (
                "OTEL_EXPORTER_OTLP_TRACES_CERTIFICATE".to_string(),
                pki.path("ca.pem"),
            ),
        ]);
        let files = TlsFiles::from_env(&TRACES_TLS_ENV_VARS, Some(&env))
            .expect("the signal variable names a valid file");
        assert!(files.certificate.is_some());
    }

    #[test]
    fn client_certificate_without_key_is_an_error() {
        let pki = Pki::new();
        let mut env = pki.env();
        env.remove("OTEL_EXPORTER_OTLP_CLIENT_KEY");
        assert_invalid(
            &error_message(TlsFiles::from_env(&TRACES_TLS_ENV_VARS, Some(&env))),
            "OTEL_EXPORTER_OTLP_CLIENT_KEY",
        );

        let mut env = pki.env();
        env.remove("OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE");
        assert_invalid(
            &error_message(TlsFiles::from_env(&TRACES_TLS_ENV_VARS, Some(&env))),
            "OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE",
        );
    }

    #[test]
    fn unreadable_or_invalid_certificate_file_is_an_error() {
        let pki = Pki::new();
        for file in ["missing.pem", "bad.pem"] {
            let env = HashMap::from([(
                "OTEL_EXPORTER_OTLP_TRACES_CERTIFICATE".to_string(),
                pki.path(file),
            )]);
            assert_invalid(
                &error_message(TlsFiles::from_env(&TRACES_TLS_ENV_VARS, Some(&env))),
                "OTEL_EXPORTER_OTLP_TRACES_CERTIFICATE",
            );
        }
    }

    #[cfg(any(feature = "export-http-protobuf", feature = "export-http-json"))]
    #[test]
    fn export_timeout_follows_the_otlp_variables() {
        let timeout = |env: &[(&str, &str)]| {
            let env = env
                .iter()
                .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
                .collect::<HashMap<_, _>>();
            super::export_timeout(&TRACES_TLS_ENV_VARS, Some(&env)).expect("valid variables")
        };
        assert_eq!(
            timeout(&[]),
            opentelemetry_otlp::OTEL_EXPORTER_OTLP_TIMEOUT_DEFAULT
        );
        assert_eq!(
            timeout(&[("OTEL_EXPORTER_OTLP_TIMEOUT", "2000")]),
            std::time::Duration::from_secs(2)
        );
        assert_eq!(
            timeout(&[
                ("OTEL_EXPORTER_OTLP_TIMEOUT", "2000"),
                ("OTEL_EXPORTER_OTLP_TRACES_TIMEOUT", "3000"),
            ]),
            std::time::Duration::from_secs(3)
        );
    }

    #[cfg(any(feature = "export-http-protobuf", feature = "export-http-json"))]
    #[test]
    fn client_key_that_is_not_a_key_is_an_error() {
        install_crypto_provider();
        let pki = Pki::new();
        let files = TlsFiles {
            certificate: None,
            client_identity: Some((
                std::fs::read(pki.path("client.pem")).expect("failed to read the certificate"),
                b"not a key".to_vec(),
            )),
        };
        assert_invalid(
            &error_message(super::http_tls_client(
                &files,
                &TRACES_TLS_ENV_VARS,
                std::time::Duration::from_secs(5),
            )),
            "OTEL_EXPORTER_OTLP_CLIENT_CERTIFICATE",
        );
    }

    #[cfg(any(feature = "export-http-protobuf", feature = "export-http-json"))]
    #[test]
    fn http_exporter_without_tls_files_or_https_keeps_its_client() {
        let pki = Pki::new();
        for (endpoint, env) in [
            ("http://127.0.0.1:4318", pki.env()),
            ("https://127.0.0.1:4318", HashMap::new()),
        ] {
            super::with_http_tls(
                opentelemetry_otlp::SpanExporter::builder().with_http(),
                endpoint,
                &TRACES_TLS_ENV_VARS,
                Some(&env),
            )
            .expect("no TLS files apply");
        }
    }

    #[cfg(any(feature = "export-http-protobuf", feature = "export-http-json"))]
    #[tokio::test]
    async fn http_exporter_verifies_the_server_and_presents_its_certificate() {
        use opentelemetry_otlp::WithExportConfig as _;
        use opentelemetry_sdk::trace::SpanExporter as _;

        install_crypto_provider();
        let pki = Pki::new();
        let (addr, presented) = pki.serve_once().await;

        let endpoint = format!("https://{addr}");
        let exporter = super::with_http_tls(
            opentelemetry_otlp::SpanExporter::builder().with_http(),
            &endpoint,
            &TRACES_TLS_ENV_VARS,
            Some(&pki.env()),
        )
        .expect("the variables name valid files")
        .with_endpoint(format!("{endpoint}/v1/traces"))
        .build()
        .expect("failed to build the exporter");
        // The server closes the connection after the handshake, so the export itself fails and
        // retries. The test only needs the handshake.
        tokio::spawn(async move { exporter.export(Vec::new()).await });

        assert!(presented.await.expect("the server did not finish"));
    }

    #[cfg(feature = "export-grpc")]
    #[tokio::test]
    async fn grpc_endpoint_verifies_the_server_and_presents_its_certificate() {
        install_crypto_provider();
        let pki = Pki::new();
        let (addr, presented) = pki.serve_once().await;

        let endpoint = super::grpc_endpoint(
            &format!("https://{addr}"),
            &TRACES_TLS_ENV_VARS,
            Some(&pki.env()),
        )
        .expect("the variables name valid files");
        // The server closes the connection after the handshake, so the connection itself fails.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(5), endpoint.connect()).await;

        assert!(presented.await.expect("the server did not finish"));
    }
}

#[cfg(all(test, feature = "export-grpc"))]
mod grpc_tls_tests {
    use super::{TRACES_TLS_ENV_VARS, grpc_endpoint};
    use std::fmt::Write as _;

    /// Connect to a listener which accepts connections but never replies. With TLS configured,
    /// the client sends a handshake and waits, so the connection attempt times out. Without it,
    /// `tonic` fails immediately with "Connecting to HTTPS without TLS enabled".
    #[tokio::test]
    async fn https_endpoints_have_tls_configured() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind listener");
        let addr = listener.local_addr().expect("failed to read listener addr");

        tokio::spawn(async move {
            // hold the connections open, so that the client waits for a handshake it never gets
            let mut accepted = Vec::new();
            while let Ok((stream, _)) = listener.accept().await {
                accepted.push(stream);
            }
        });

        let endpoint = grpc_endpoint(&format!("https://{addr}"), &TRACES_TLS_ENV_VARS, None)
            .expect("failed to build endpoint");

        let result =
            tokio::time::timeout(std::time::Duration::from_secs(2), endpoint.connect()).await;

        match result {
            // the handshake is still in flight, so TLS was configured
            Err(_elapsed) => {}
            Ok(Ok(_)) => panic!("unexpectedly connected to a listener which never replies"),
            Ok(Err(error)) => {
                let mut message = error.to_string();
                let mut source = std::error::Error::source(&error);
                while let Some(inner) = source {
                    write!(message, ": {inner}").expect("writing to a string succeeds");
                    source = inner.source();
                }

                assert!(
                    !message.contains("without TLS enabled"),
                    "TLS was not configured for an https endpoint: {message}"
                );
            }
        }
    }
}

/// Check that a rustls `CryptoProvider` will be available for TLS.
///
/// With the `tls-aws-lc` feature, `tonic` and `reqwest` fall back to aws-lc-rs when the
/// application has not installed a provider, so there is nothing to check. Without it they have
/// no fallback to use, and `reqwest` panics rather than returning an error, so the installed
/// provider is required up front.
#[cfg(any(
    feature = "export-grpc",
    feature = "export-http-protobuf",
    feature = "export-http-json"
))]
#[cfg_attr(feature = "tls-aws-lc", allow(clippy::unnecessary_wraps))]
fn require_crypto_provider() -> Result<(), ConfigureError> {
    #[cfg(not(feature = "tls-aws-lc"))]
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        return Err(ConfigureError::CryptoProviderRequired);
    }

    Ok(())
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
