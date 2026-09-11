//! Without the `tls-aws-lc` feature there is no provider to fall back on, so building an
//! exporter requires the application to have installed one.
//!
//! Run with:
//! `cargo test -p logfire --no-default-features --features export-http-protobuf --test test_crypto_provider`
#![cfg(all(feature = "export-http-protobuf", not(feature = "tls-aws-lc")))]

use logfire::ConfigureError;

#[test]
fn exporters_require_an_installed_crypto_provider() {
    let error = logfire::exporters::span_exporter("http://localhost:4318", None)
        .err()
        .expect("expected an error when no crypto provider is installed");

    assert!(
        matches!(error, ConfigureError::CryptoProviderRequired),
        "unexpected error: {error}"
    );

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install crypto provider");

    logfire::exporters::span_exporter("http://localhost:4318", None)
        .expect("exporter should build once a provider is installed");
}
