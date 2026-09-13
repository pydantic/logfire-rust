//! Without the `tls-aws-lc` feature there is no provider to fall back on, so building a client
//! requires the application to have installed one.
//!
//! Run with:
//! `cargo test -p logfire-client --no-default-features --test test_crypto_provider`
#![cfg(not(feature = "tls-aws-lc"))]

use logfire_client::builder::{BuilderError, LogfireClientBuilder};

#[test]
fn build_client_requires_an_installed_crypto_provider() {
    let mut builder = LogfireClientBuilder::new();
    builder.token("pylf_v1_us_abc123");

    let error = builder
        .build_client()
        .err()
        .expect("expected an error when no crypto provider is installed");

    assert!(
        matches!(error, BuilderError::CryptoProviderRequired),
        "unexpected error: {error}"
    );

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install crypto provider");

    builder
        .build_client()
        .expect("client should build once a provider is installed");
}
